use std::time::Duration;

use anyhow::Result;
use uuid::Uuid;

use crate::{
    savant::{SavantClient, Task},
    worktree,
};

use super::{
    ExecutionSpec,
    event_log::EventLog,
    publication, setup,
    types::{ExecutionOutcome, RunnerConfig},
    validation,
};

pub(super) async fn execute(
    savant: &SavantClient,
    config: &RunnerConfig,
    task: Task,
) -> Result<ExecutionOutcome> {
    let spec = ExecutionSpec::from_task(&task)?;
    let run_id = Uuid::new_v4();

    tracing::info!(task_id = %task.task_id, status = %task.status, "Colosseum starting task execution");
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!("🚀 **Colosseum Started**: Picked up task in `{}` status. Provisioning Git worktree...", task.status),
            "Colosseum",
        )
        .await;

    let worktree = worktree::provision_task(
        &spec.repository,
        &config.worktree_root,
        &task.task_id,
        &spec.revision,
    )
    .await?;
    tracing::info!(task_id = %task.task_id, worktree = %worktree.path.display(), branch = %worktree.branch, "Provisioned worktree");
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!("📂 **Worktree Ready**: Directory `{}` | Branch `{}`", worktree.path.display(), worktree.branch),
            "Colosseum",
        )
        .await;

    let log_file = config
        .log_root
        .join(&task.task_id)
        .join(format!("{run_id}.jsonl"));
    let events = EventLog::start(&log_file).await?;
    events.record(serde_json::json!({"type":"started","run_id":run_id,"task_id":task.task_id,"worktree":worktree.path}));
    let limit = Duration::from_secs(spec.timeout_seconds);

    tracing::info!(task_id = %task.task_id, "Resolving persona abilities");
    let ability_prompt = setup::resolve_ability_prompt(savant, &spec.repository, &task, &events).await?;
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!("🧠 **Abilities Loaded**: Resolved persona configuration for task execution."),
            "Colosseum",
        )
        .await;

    if let Some(setup_outcome) = setup::failed_setup(
        spec.setup.as_deref(),
        &worktree.path,
        limit,
        events.sender(),
    )
    .await?
    {
        tracing::warn!(task_id = %task.task_id, exit_code = setup_outcome.exit_code, "Setup command failed");
        let _ = savant
            .add_comment(
                &task.task_id,
                &format!("⚠️ **Setup Failed**: Command failed with exit code `{}`. Blocking task.", setup_outcome.exit_code),
                "Colosseum",
            )
            .await;
        return setup::finish_blocked_setup(
            savant,
            run_id,
            task,
            worktree,
            log_file,
            events,
            setup_outcome,
        )
        .await;
    }

    let prompt = execution_prompt(&ability_prompt, &task);
    tracing::info!(task_id = %task.task_id, provider = %spec.provider, "Executing AI agent work");
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!("⚡ **AI Agent Executing**: Provider `{}` is processing the task...", spec.provider),
            "Colosseum",
        )
        .await;

    let (agent, validation) = validation::run(
        &spec.provider,
        &worktree.path,
        &prompt,
        limit,
        events.sender(),
    )
    .await?;

    tracing::info!(task_id = %task.task_id, exit_code = agent.exit_code, "AI agent execution finished");
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!("🔍 **AI Agent Finished**: Exit code `{}`. Verifying and publishing changes...", agent.exit_code),
            "Colosseum",
        )
        .await;

    let publication = publication::publish_if_verified(
        &task,
        &worktree,
        &log_file,
        &agent,
        validation.as_ref(),
        &events,
    )
    .await;

    finish(
        savant,
        run_id,
        task,
        worktree,
        log_file,
        events,
        agent,
        validation,
        publication,
    )
    .await
}

fn execution_prompt(ability_prompt: &str, task: &Task) -> String {
    format!(
        "{ability_prompt}\n\n# Colosseum execution contract\nYou have full permission to inspect, edit, and run commands in this worktree. Work on Savant task {}: {}\n\n{}\n\nRun the relevant validation and fix failures you introduce. Leave changes in this worktree; Colosseum will independently verify, commit, push, and retain the publication evidence.",
        task.task_id, task.title, task.description
    )
}

#[allow(clippy::too_many_arguments)]
async fn finish(
    savant: &SavantClient,
    run_id: Uuid,
    task: Task,
    worktree: worktree::Worktree,
    log_file: std::path::PathBuf,
    events: EventLog,
    agent: crate::executor::ProcessOutcome,
    validation: Option<crate::executor::ProcessOutcome>,
    publication: Option<publication::Publication>,
) -> Result<ExecutionOutcome> {
    let target_status = match task.status.as_str() {
        "grooming" => {
            if agent.exit_code == 0 && !agent.timed_out {
                "ready"
            } else {
                "blocked"
            }
        }
        _ => publication.as_ref().map_or("blocked", |_| "review"),
    };
    let status = target_status;
    let comment = if let Some(pub_info) = &publication {
        format!(
            "Colosseum completed task execution.\n\nWorktree: `{}`\nBranch: `{}`\nCommit: `{}`\nRemote: `{}`\nMoved status to: `{}`",
            worktree.path.display(),
            worktree.branch,
            pub_info.commit,
            pub_info.remote,
            status
        )
    } else {
        format!(
            "Colosseum execution could not complete or verify changes. Worktree: `{}`. Moved status to `blocked`.",
            worktree.path.display()
        )
    };
    let _ = savant.add_comment(&task.task_id, &comment, "Colosseum").await;

    events.record(serde_json::json!({
        "type":"finished",
        "status":status,
        "branch":worktree.branch,
        "commit":publication.as_ref().map(|item| &item.commit),
        "remote":publication.as_ref().map(|item| &item.remote),
        "agent_exit_code":agent.exit_code,
        "validation_exit_code":validation.as_ref().map(|value| value.exit_code),
    }));
    events.finish().await?;
    savant.update_status(&task.task_id, status).await?;
    Ok(ExecutionOutcome {
        run_id,
        task_id: task.task_id,
        status: status.to_owned(),
        worktree: worktree.path,
        log_file,
        agent,
        validation,
    })
}
