use std::time::Duration;

use anyhow::Result;
use uuid::Uuid;

use crate::{
    savant::{SavantClient, Task},
    worktree,
};

use super::{
    ExecutionPhase, ExecutionSpec, WorkType,
    event_log::EventLog,
    handoff,
    heartbeat::Heartbeat,
    phases, publication, setup,
    types::{ExecutionOutcome, RunnerConfig},
    validation,
};

pub(super) async fn execute(
    savant: &SavantClient,
    config: &RunnerConfig,
    task: Task,
) -> Result<ExecutionOutcome> {
    let spec = ExecutionSpec::from_task(&task)?;
    let phase = ExecutionPhase::from_task(&task)?;
    if phase != ExecutionPhase::Work || spec.work_type == WorkType::Research {
        return phases::execute(savant, config, task, spec, phase).await;
    }
    if spec.repository.as_os_str().is_empty() {
        anyhow::bail!("development work requires an assigned repository");
    }
    let run_id = Uuid::new_v4();
    let phase_config = setup::phase_execution_config(&task, &spec.provider, phase);

    if let Some(ref working_loc) = phase_config.working_location {
        if working_loc.to_lowercase() != task.status.to_lowercase() {
            let _ = savant.update_status(&task.task_id, working_loc).await;
        }
    }

    tracing::info!(task_id = %task.task_id, status = %task.status, "Colosseum starting task execution");
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!("🚀 **Colosseum Started**: Picked up task in `{}` status and transitioned to working lock status `{}`. Provisioning Git worktree...", task.status, phase_config.working_location.as_deref().unwrap_or("in-progress")),
            "Colosseum",
        )
        .await;

    let mut worktree = worktree::provision_task(
        &spec.repository,
        &config.worktree_root,
        &task.task_id,
        &spec.revision,
    )
    .await?;
    tracing::info!(task_id = %task.task_id, worktree = %worktree.path.display(), branch = %worktree.branch, resumed = worktree.resumed, "Provisioned worktree");
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!(
                "📂 **Worktree {}**: Directory `{}` | Branch `{}` | HEAD `{}` | Full MR base `{}` | Base contained: `{}`. {}",
                if worktree.resumed { "Resumed" } else { "Ready" },
                worktree.path.display(),
                worktree.branch,
                worktree.start_commit,
                worktree.review_base_commit,
                worktree.base_is_ancestor,
                if worktree.resumed {
                    "Continuing the previous attempt with all existing files and history preserved."
                } else {
                    "Starting the first isolated attempt."
                }
            ),
            "Colosseum",
        )
        .await;

    let log_file = config
        .log_root
        .join(&task.task_id)
        .join(format!("{run_id}.jsonl"));
    let events = EventLog::start(&log_file).await?;
    events.record(serde_json::json!({
        "type":"started",
        "run_id":run_id,
        "task_id":task.task_id,
        "worktree":worktree.path,
        "branch":worktree.branch,
        "attempt_start_commit":worktree.start_commit,
        "review_base_commit":worktree.review_base_commit,
        "base_branch_commit":worktree.base_branch_commit,
        "base_is_ancestor":worktree.base_is_ancestor,
        "resumed":worktree.resumed,
    }));
    let heartbeat = Heartbeat::start(
        savant,
        &task,
        &spec,
        phase,
        run_id,
        &log_file,
        &worktree.path,
        events.sender(),
    )
    .await;
    let limit = Duration::from_secs(spec.timeout_seconds);

    tracing::info!(task_id = %task.task_id, "Resolving persona abilities");
    let ability_prompt =
        setup::resolve_ability_prompt(savant, &spec.repository, &task, &events).await?;
    let _ = savant
        .add_comment(
            &task.task_id,
            "🧠 **Abilities Loaded**: Resolved persona configuration for task execution.",
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
                &format!(
                    "⚠️ **Setup Failed**: Command failed with exit code `{}`. Blocking task.",
                    setup_outcome.exit_code
                ),
                "Colosseum",
            )
            .await;
        heartbeat
            .finish("failed", "Setup failed; Colosseum blocked the task.")
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

    let prompt = execution_prompt(&ability_prompt, &task, &worktree);
    heartbeat.update(
        "running",
        "Coder is actively working in the isolated worktree.",
    );
    tracing::info!(task_id = %task.task_id, provider = %phase_config.provider, "Executing AI agent work");
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!(
                "⚡ **AI Agent Executing**: Provider `{}` is processing the task...",
                phase_config.provider
            ),
            "Colosseum",
        )
        .await;

    let (agent, validation) = validation::run(
        &phase_config.provider,
        phase_config.model.as_deref(),
        &worktree.path,
        &prompt,
        limit,
        events.sender(),
        &heartbeat,
    )
    .await?;

    // The coder may have incorporated an advanced base branch. Recompute the
    // whole-MR boundary before capturing publication evidence.
    worktree::refresh_review_boundaries(&mut worktree).await?;

    tracing::info!(task_id = %task.task_id, exit_code = agent.exit_code, "AI agent execution finished");
    let _ = savant
        .add_comment(
            &task.task_id,
            &format!(
                "🔍 **AI Agent Finished**: Exit code `{}`. Verifying and publishing changes...",
                agent.exit_code
            ),
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

    heartbeat.update(
        "publishing",
        "Validation finished; recording publication evidence.",
    );
    heartbeat
        .finish(
            if publication.is_some() {
                "completed"
            } else {
                "failed"
            },
            if publication.is_some() {
                "Work validated and published for review."
            } else {
                "Execution did not produce verified publication evidence."
            },
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
        phase_config,
    )
    .await
}

fn execution_prompt(ability_prompt: &str, task: &Task, worktree: &worktree::Worktree) -> String {
    let dossier = handoff::dossier_prompt(task, Some(worktree));
    let repair_context = handoff::repair_instructions(task);
    format!(
        concat!(
            "{}\n\n{}\n\n# Colosseum execution contract\n",
            "You have full permission to inspect, edit, and run commands in this worktree. ",
            "Work on Savant task {}: {}\n\n{}{}\n\n",
            "Continue from the current worktree and HEAD. If the worktree was resumed, preserve and verify ",
            "all prior committed and uncommitted work. Inspect the entire full-MR range from the dossier base ",
            "to HEAD before declaring completion. Verify the base branch is an ancestor of HEAD; if it has ",
            "advanced, incorporate it without rewriting already-published history, resolve conflicts in context, ",
            "and rerun the full validation suite. ",
            "Run the relevant validation and fix failures you introduce. Leave changes in this worktree; ",
            "Colosseum will independently verify, commit, push, and retain the publication evidence. ",
            "End with exactly one single-line marker: ",
            "COLOSSEUM_RESULT: {{\"decision\":\"complete\",\"summary\":\"what changed\",",
            "\"rationale\":\"why the implementation is correct\",\"questions\":[]}}"
        ),
        ability_prompt, dossier, task.task_id, task.title, task.description, repair_context
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
    phase_config: setup::PhaseExecutionConfig,
) -> Result<ExecutionOutcome> {
    let status = publication.as_ref().map_or("blocked", |_| "review");
    let mut mr_id = None;
    if let Some(pub_info) = &publication {
        let id = format!("mr-colosseum-{}", task.task_id);
        savant
            .create_merge_request(&task, &id, &pub_info.remote, &worktree.branch)
            .await?;
        savant
            .update_colosseum_metadata(
                &task.task_id,
                &serde_json::json!({
                    "worktree_path":worktree.path,
                    "branch":worktree.branch,
                    "base_branch":worktree.base_branch,
                    "base_commit":worktree.review_base_commit,
                    "attempt_start_commit":worktree.start_commit,
                    "resumed":worktree.resumed,
                    "commit":pub_info.commit,
                    "remote":pub_info.remote,
                    "mr_id":id,
                    "log_path":log_file,
                    "diff":pub_info.diff,
                    "files":pub_info.files,
                }),
            )
            .await?;
        mr_id = Some(id);
    }
    let comment = if let Some(pub_info) = &publication {
        format!(
            concat!(
                "## Colosseum work published\n\nWorktree: `{}`\nBranch: `{}`\n",
                "Base commit: `{}`\nCommit: `{}`\nRemote: `{}`\nMerge request: `{}`\n",
                "Validation: passed\nMoved status to: `{}`"
            ),
            worktree.path.display(),
            worktree.branch,
            worktree.review_base_commit,
            pub_info.commit,
            pub_info.remote,
            mr_id.as_deref().unwrap_or("not-created"),
            status
        )
    } else {
        format!(
            "Colosseum execution could not complete or verify changes. Worktree: `{}`. Moved status to `blocked`.",
            worktree.path.display()
        )
    };
    savant
        .add_comment(&task.task_id, &comment, "Colosseum")
        .await?;

    let (run_summary, run_rationale) = if publication.is_some() {
        (
            "Development work validated and published for review",
            "Independent project validation passed and the branch was pushed",
        )
    } else {
        (
            "Development work failed validation or publication",
            "Review is blocked until execution, validation, and publication all pass",
        )
    };
    savant
        .append_colosseum_run(
            &task.task_id,
            &serde_json::json!({
                "run_id":run_id,
                "phase":"work",
                "status":if publication.is_some() { "passed" } else { "failed" },
                "summary":run_summary,
                "rationale":run_rationale,
                "worktree_path":worktree.path,
                "branch":worktree.branch,
                "base_commit":worktree.review_base_commit,
                "attempt_start_commit":worktree.start_commit,
                "resumed":worktree.resumed,
                "commit":publication.as_ref().map(|item| &item.commit),
                "remote":publication.as_ref().map(|item| &item.remote),
                "mr_id":mr_id,
                "log_path":log_file,
                "agent_exit_code":agent.exit_code,
                "validation_exit_code":validation.as_ref().map(|value| value.exit_code),
                "handoff_version":handoff::HANDOFF_VERSION,
                "persona":phase_config.persona,
                "provider":phase_config.provider,
                "model":phase_config.model,
            }),
        )
        .await?;

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
    let target_status = if publication.is_some() {
        phase_config.drop_location.as_deref().unwrap_or(status)
    } else {
        "blocked"
    };
    savant.update_status(&task.task_id, target_status).await?;
    savant
        .set_colosseum_ready(&task.task_id, target_status == "review" || target_status == "ready")
        .await?;
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
