use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    savant::{SavantClient, Task},
    worktree,
};

mod event_log;
mod policy;
mod publication;
mod setup;
mod steps;
mod types;
mod validation;
mod worker;

pub use types::{ExecutionOutcome, RunnerConfig};

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionSpec {
    pub repository: PathBuf,
    pub provider: String,
    #[serde(default = "default_revision")]
    pub revision: String,
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub push: bool,
}

fn default_revision() -> String {
    "HEAD".to_owned()
}
fn default_timeout() -> u64 {
    3600
}

impl ExecutionSpec {
    pub fn from_task(task: &Task) -> Result<Self> {
        if !task.colosseum_ready {
            anyhow::bail!("task is not ready for Colosseum");
        }
        serde_json::from_value(task.colosseum_config.clone())
            .context("invalid Colosseum task config")
    }
}

pub struct ExecutionRunner {
    savant: SavantClient,
    config: RunnerConfig,
}

impl ExecutionRunner {
    pub fn new(savant: SavantClient, config: RunnerConfig) -> Self {
        Self { savant, config }
    }

    pub(super) async fn claim_next(&self, workspace_id: Option<&str>) -> Result<Option<Task>> {
        let Some(task) = self.savant.next_colosseum_task(workspace_id).await? else {
            return Ok(None);
        };
        self.savant.claim(&task.task_id).await
    }

    pub(super) async fn execute_task(&self, task: Task) -> Result<ExecutionOutcome> {
        let spec = ExecutionSpec::from_task(&task)?;
        let run_id = Uuid::new_v4();
        let worktree = worktree::provision_task(
            &spec.repository,
            &self.config.worktree_root,
            &task.task_id,
            &spec.revision,
        )
        .await?;
        let log_file = self
            .config
            .log_root
            .join(&task.task_id)
            .join(format!("{run_id}.jsonl"));
        let events = event_log::EventLog::start(&log_file).await?;
        let limit = Duration::from_secs(spec.timeout_seconds);
        events.record(serde_json::json!({
            "type":"started",
            "run_id":run_id,
            "task_id":task.task_id,
            "worktree":worktree.path,
        }));
        let ability_prompt =
            setup::resolve_ability_prompt(&self.savant, &spec.repository, &events).await?;
        if let Some(setup_outcome) = setup::failed_setup(
            spec.setup.as_deref(),
            &worktree.path,
            limit,
            events.sender(),
        )
        .await?
        {
            return setup::finish_blocked_setup(
                &self.savant,
                run_id,
                task,
                worktree,
                log_file,
                events,
                setup_outcome,
            )
            .await;
        }
        let prompt = format!(
            "{ability_prompt}\n\n# Colosseum execution contract\nYou have full permission to inspect, edit, and run commands in this worktree. Work on Savant task {}: {}\n\n{}\n\nRun the relevant validation and fix failures you introduce. Leave changes in this worktree; Colosseum will independently verify, commit, push, and post review metadata.",
            task.task_id, task.title, task.description
        );
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
        let status = if publication.is_some() {
            "code-review"
        } else {
            "blocked"
        };
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
        self.savant.update_status(&task.task_id, status).await?;
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

    pub async fn run_next(&self, workspace_id: Option<&str>) -> Result<Option<ExecutionOutcome>> {
        worker::run_next(self, workspace_id).await
    }
}
