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
    task: &Task,
    events: &EventLog,
) -> Result<String> {
    let repository = repository_name(repository);
    let persona = task.colosseum_config.get("persona").and_then(|v| v.as_str()).unwrap_or("persona.engineer");
    let tags_val = task.colosseum_config.get("tags");
    let tags_vec: Vec<String> = tags_val
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|t| t.as_str().map(String::from)).collect())
        .unwrap_or_else(|| vec!["engineering".into(), "execution".into(), "code-review".into()]);
    let tags_slice: Vec<&str> = tags_vec.iter().map(|s| s.as_str()).collect();

    let abilities = savant.resolve_abilities(&repository, persona, &tags_slice).await?;
    let prompt = abilities
        .get("prompt")
        .and_then(|value| value.as_str())
        .context("Savant ability prompt missing")?
        .to_owned();
    events.record(serde_json::json!({
        "type":"abilities-resolved",
        "persona": persona,
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
