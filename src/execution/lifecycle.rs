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
    let worktree = worktree::provision_task(
        &spec.repository,
        &config.worktree_root,
        &task.task_id,
        &spec.revision,
    )
    .await?;
    let log_file = config
        .log_root
        .join(&task.task_id)
        .join(format!("{run_id}.jsonl"));
    let events = EventLog::start(&log_file).await?;
    events.record(serde_json::json!({"type":"started","run_id":run_id,"task_id":task.task_id,"worktree":worktree.path}));
    let limit = Duration::from_secs(spec.timeout_seconds);
    let ability_prompt = setup::resolve_ability_prompt(savant, &spec.repository, &events).await?;
    if let Some(setup_outcome) = setup::failed_setup(
        spec.setup.as_deref(),
        &worktree.path,
        limit,
        events.sender(),
    )
    .await?
    {
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
    let (agent, validation) = validation::run(
        &spec.provider,
        &worktree.path,
        &prompt,
        limit,
        events.sender(),
    )
    .await?;
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
        "{ability_prompt}\n\n# Colosseum execution contract\nYou have full permission to inspect, edit, and run commands in this worktree. Work on Savant task {}: {}\n\n{}\n\nRun the relevant validation and fix failures you introduce. Leave changes in this worktree; Colosseum will independently verify, commit, push, and post review metadata.",
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
    let status = publication.as_ref().map_or("blocked", |_| "code-review");
    events.record(serde_json::json!({
        "type":"finished",
        "status":status,
        "branch":worktree.branch,
        "commit":publication.as_ref().map(|item| &item.commit),
        "remote":publication.as_ref().map(|item| &item.remote),
        "review":publication.as_ref().map(|item| &item.review),
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
