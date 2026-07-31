use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use uuid::Uuid;

use crate::{
    executor::ProcessOutcome,
    savant::{SavantClient, Task},
    worktree::Worktree,
};

use super::{ExecutionOutcome, event_log::EventLog, steps};

pub(super) async fn resolve_ability_prompt(
    savant: &SavantClient,
    repository: &Path,
    events: &EventLog,
) -> Result<String> {
    let repository = repository_name(repository);
    let abilities = savant.resolve_engineer_abilities(&repository).await?;
    let prompt = abilities
        .get("prompt")
        .and_then(|value| value.as_str())
        .context("Savant engineer ability prompt missing")?
        .to_owned();
    events.record(serde_json::json!({
        "type":"abilities-resolved",
        "persona":"persona.engineer",
        "manifest":abilities.get("manifest"),
    }));
    Ok(prompt)
}

pub(super) async fn failed_setup(
    command: Option<&str>,
    worktree: &Path,
    limit: Duration,
    events: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<Option<ProcessOutcome>> {
    let Some(command) = command else {
        return Ok(None);
    };
    let outcome = steps::run_shell("setup", command, worktree, limit, events).await?;
    Ok((outcome.exit_code != 0 || outcome.timed_out).then_some(outcome))
}

fn repository_name(repository: &Path) -> String {
    repository
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn finish_blocked_setup(
    savant: &SavantClient,
    run_id: Uuid,
    task: Task,
    worktree: Worktree,
    log_file: std::path::PathBuf,
    events: EventLog,
    setup_outcome: ProcessOutcome,
) -> Result<ExecutionOutcome> {
    events.record(serde_json::json!({
        "type":"finished",
        "status":"blocked",
        "setup_exit_code":setup_outcome.exit_code,
    }));
    events.finish().await?;
    savant.update_status(&task.task_id, "blocked").await?;
    Ok(ExecutionOutcome {
        run_id,
        task_id: task.task_id,
        status: "blocked".to_owned(),
        worktree: worktree.path,
        log_file,
        agent: setup_outcome,
        validation: None,
    })
}
