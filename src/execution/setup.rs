use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use uuid::Uuid;

use crate::{
    executor::ProcessOutcome,
    savant::{SavantClient, Task},
    worktree::Worktree,
};

use super::{ExecutionOutcome, ExecutionPhase, event_log::EventLog, steps};

pub(super) async fn resolve_ability_prompt(
    savant: &SavantClient,
    repository: &Path,
    task: &Task,
    events: &EventLog,
) -> Result<String> {
    let repository = repository_name(repository);
    let persona_str = task
        .colosseum_config
        .get("persona")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let persona = if persona_str.is_empty() {
        "persona.engineer"
    } else {
        persona_str
    };
    let tags_val = task.colosseum_config.get("tags");
    let tags_vec: Vec<String> = tags_val
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(String::from))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let tags_vec = if tags_vec.is_empty() {
        vec![
            "engineering".into(),
            "execution".into(),
            "code-review".into(),
        ]
    } else {
        tags_vec
    };
    let tags_slice: Vec<&str> = tags_vec.iter().map(|s| s.as_str()).collect();

    let abilities = savant
        .resolve_abilities(&repository, persona, &tags_slice)
        .await?;
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

pub(super) async fn resolve_phase_ability_prompt(
    savant: &SavantClient,
    repository: &Path,
    task: &Task,
    phase: ExecutionPhase,
    events: &EventLog,
) -> Result<String> {
    let (persona, default_tags): (&str, Vec<String>) = match phase {
        ExecutionPhase::Grooming => (
            "persona.product",
            vec!["product".into(), "requirements".into(), "grooming".into()],
        ),
        ExecutionPhase::Review => (
            "persona.reviewer",
            vec!["code-review".into(), "verification".into()],
        ),
        ExecutionPhase::Work | ExecutionPhase::Merge => (
            "persona.engineer",
            vec![
                "engineering".into(),
                "execution".into(),
                "code-review".into(),
            ],
        ),
    };
    let configured_persona = task
        .colosseum_config
        .get("persona")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    // Grooming and review must remain independent roles. Only execution work
    // may use the operator-selected persona from ready settings.
    let persona = if phase == ExecutionPhase::Work {
        configured_persona.unwrap_or(persona)
    } else {
        persona
    };
    let configured_tags = task
        .colosseum_config
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tags = if configured_tags.is_empty() {
        default_tags
    } else {
        configured_tags
    };
    let tag_refs = tags.iter().map(String::as_str).collect::<Vec<_>>();
    let abilities = savant
        .resolve_abilities(&repository_name(repository), persona, &tag_refs)
        .await?;
    let prompt = abilities
        .get("prompt")
        .and_then(|value| value.as_str())
        .context("Savant ability prompt missing")?
        .to_owned();
    events.record(serde_json::json!({
        "type":"abilities-resolved",
        "phase":format!("{phase:?}").to_lowercase(),
        "persona":persona,
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
