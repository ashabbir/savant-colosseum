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

const ENGINEER_PERSONA: &str = "persona.engineer";

fn phase_ability_tags(phase: ExecutionPhase) -> Vec<String> {
    match phase {
        ExecutionPhase::Grooming => vec![
            "engineering".into(),
            "execution".into(),
            "code-review".into(),
            "requirements".into(),
            "grooming".into(),
        ],
        ExecutionPhase::Review => vec![
            "engineering".into(),
            "execution".into(),
            "code-review".into(),
            "verification".into(),
        ],
        ExecutionPhase::Work | ExecutionPhase::Merge => vec![
            "engineering".into(),
            "execution".into(),
            "code-review".into(),
        ],
    }
}

pub(super) async fn resolve_ability_prompt(
    savant: &SavantClient,
    repository: &Path,
    task: &Task,
    events: &EventLog,
) -> Result<String> {
    let repository = repository_name(repository);
    let persona = ENGINEER_PERSONA;
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
    let mut tags_vec = tags_vec;
    for required in ["engineering", "execution", "code-review"] {
        if !tags_vec.iter().any(|tag| tag == required) {
            tags_vec.push(required.to_owned());
        }
    }
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
    let default_tags = phase_ability_tags(phase);
    // Every provider invocation resolves the mandatory engineering persona.
    // Phase-specific tags select grooming or independent-review rules without
    // weakening the fail-closed engineer ability contract.
    let persona = ENGINEER_PERSONA;
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
    let mut tags = default_tags;
    for configured in configured_tags {
        if !tags.contains(&configured) {
            tags.push(configured);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{ENGINEER_PERSONA, phase_ability_tags};
    use crate::execution::ExecutionPhase;

    #[test]
    fn every_phase_keeps_the_mandatory_engineer_persona() {
        assert_eq!(ENGINEER_PERSONA, "persona.engineer");
        for phase in [
            ExecutionPhase::Grooming,
            ExecutionPhase::Work,
            ExecutionPhase::Review,
            ExecutionPhase::Merge,
        ] {
            assert!(phase_ability_tags(phase).contains(&"engineering".to_owned()));
        }
    }

    #[test]
    fn grooming_and_review_select_independent_phase_rules() {
        assert!(phase_ability_tags(ExecutionPhase::Grooming).contains(&"grooming".to_owned()));
        assert!(phase_ability_tags(ExecutionPhase::Review).contains(&"verification".to_owned()));
    }
}
