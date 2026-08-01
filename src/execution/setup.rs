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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PhaseExecutionConfig {
    pub(super) persona: String,
    pub(super) tags: Vec<String>,
    pub(super) provider: String,
    pub(super) model: Option<String>,
}

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

pub(super) fn phase_execution_config(
    task: &Task,
    spec_provider: &str,
    phase: ExecutionPhase,
) -> PhaseExecutionConfig {
    let phase_key = match phase {
        ExecutionPhase::Grooming => "grooming",
        ExecutionPhase::Work => "ready",
        ExecutionPhase::Review | ExecutionPhase::Merge => "review",
    };
    let configured = task
        .colosseum_config
        .get("phase_configs")
        .and_then(|value| value.get(phase_key));
    let default_persona = match phase {
        ExecutionPhase::Grooming => "persona.architect",
        ExecutionPhase::Work => "persona.coder",
        ExecutionPhase::Review | ExecutionPhase::Merge => "persona.reviewer",
    };
    let configured_persona = configured
        .and_then(|value| value.get("persona"))
        .and_then(|value| value.as_str());
    // Tasks created before persona.coder existed persisted persona.engineer as
    // the Ready default. Migrate that legacy default at execution time so a
    // resumed attempt receives the dedicated coder contract.
    let persona = configured_persona
        .filter(|persona| !(phase == ExecutionPhase::Work && *persona == "persona.engineer"))
        .unwrap_or(default_persona)
        .to_owned();
    let provider = configured
        .and_then(|value| value.get("provider"))
        .and_then(|value| value.as_str())
        .unwrap_or(spec_provider)
        .to_owned();
    let model = configured
        .and_then(|value| value.get("model"))
        .and_then(|value| value.as_str())
        .or_else(|| {
            task.colosseum_config
                .get("model")
                .and_then(|value| value.as_str())
        })
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    let configured_tags = configured
        .and_then(|value| value.get("tags"))
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut tags = phase_ability_tags(phase);
    for tag in configured_tags {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    PhaseExecutionConfig {
        persona,
        tags,
        provider,
        model,
    }
}

pub(super) async fn resolve_ability_prompt(
    savant: &SavantClient,
    repository: &Path,
    task: &Task,
    events: &EventLog,
) -> Result<String> {
    let repository = repository_name(repository);
    let config = phase_execution_config(
        task,
        task.colosseum_config["provider"]
            .as_str()
            .unwrap_or("codex"),
        ExecutionPhase::Work,
    );
    let tags_slice: Vec<&str> = config.tags.iter().map(String::as_str).collect();

    let abilities = savant
        .resolve_abilities(&repository, &config.persona, &tags_slice)
        .await?;
    let prompt = abilities
        .get("prompt")
        .and_then(|value| value.as_str())
        .context("Savant ability prompt missing")?
        .to_owned();
    events.record(serde_json::json!({
        "type":"abilities-resolved",
        "persona": config.persona,
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
    let config = phase_execution_config(
        task,
        task.colosseum_config["provider"]
            .as_str()
            .unwrap_or("codex"),
        phase,
    );
    let tag_refs = config.tags.iter().map(String::as_str).collect::<Vec<_>>();
    let abilities = savant
        .resolve_abilities(&repository_name(repository), &config.persona, &tag_refs)
        .await?;
    let prompt = abilities
        .get("prompt")
        .and_then(|value| value.as_str())
        .context("Savant ability prompt missing")?
        .to_owned();
    events.record(serde_json::json!({
        "type":"abilities-resolved",
        "phase":format!("{phase:?}").to_lowercase(),
        "persona":config.persona,
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
    use super::{phase_ability_tags, phase_execution_config};
    use crate::execution::ExecutionPhase;
    use crate::savant::Task;
    use serde_json::json;

    fn task(config: serde_json::Value) -> Task {
        Task {
            task_id: "task-1".into(),
            workspace_id: "ws-1".into(),
            title: "Test".into(),
            description: String::new(),
            status: "in-progress".into(),
            colosseum_claimed_from: Some("ready".into()),
            priority: String::new(),
            depends_on: vec![],
            colosseum_ready: true,
            colosseum_config: config,
            comments: json!([]),
        }
    }

    #[test]
    fn every_phase_uses_its_specialist_persona() {
        let task =
            task(json!({"provider":"codex","persona":"persona.engineer","tags":["generic"]}));
        assert_eq!(
            phase_execution_config(&task, "codex", ExecutionPhase::Grooming).persona,
            "persona.architect"
        );
        assert_eq!(
            phase_execution_config(&task, "codex", ExecutionPhase::Work).persona,
            "persona.coder"
        );
        assert_eq!(
            phase_execution_config(&task, "codex", ExecutionPhase::Review).persona,
            "persona.reviewer"
        );
    }

    #[test]
    fn grooming_and_review_select_independent_phase_rules() {
        assert!(phase_ability_tags(ExecutionPhase::Grooming).contains(&"grooming".to_owned()));
        assert!(phase_ability_tags(ExecutionPhase::Review).contains(&"verification".to_owned()));
    }

    #[test]
    fn legacy_ready_engineer_config_migrates_to_coder() {
        let task = task(json!({"provider":"codex","phase_configs":{"ready":{
            "persona":"persona.engineer"
        }}}));

        assert_eq!(
            phase_execution_config(&task, "codex", ExecutionPhase::Work).persona,
            "persona.coder"
        );
    }

    #[test]
    fn phase_config_overrides_provider_model_persona_and_tags() {
        let task = task(json!({"provider":"codex","phase_configs":{"review":{
            "provider":"claude","model":"opus","persona":"persona.reviewer","tags":["security"]
        }}}));
        let config = phase_execution_config(&task, "codex", ExecutionPhase::Review);
        assert_eq!(config.provider, "claude");
        assert_eq!(config.model.as_deref(), Some("opus"));
        assert_eq!(config.persona, "persona.reviewer");
        assert!(config.tags.contains(&"security".into()));
        assert!(config.tags.contains(&"verification".into()));
    }
}
