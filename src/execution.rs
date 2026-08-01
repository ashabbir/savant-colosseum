use std::path::PathBuf;

use crate::savant::{SavantClient, Task};
use anyhow::{Context, Result};
use serde::Deserialize;

pub mod decision;
mod event_log;
mod heartbeat;
mod lifecycle;
mod phases;
mod policy;
mod publication;
mod setup;
mod steps;
mod types;
mod validation;
mod worker;

pub use types::{ExecutionOutcome, RunnerConfig};

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkType {
    #[default]
    Development,
    Research,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    Grooming,
    Work,
    Review,
    Merge,
}

impl ExecutionPhase {
    pub fn from_task(task: &Task) -> Result<Self> {
        match task
            .colosseum_claimed_from
            .as_deref()
            .unwrap_or(task.status.as_str())
        {
            "grooming" => Ok(Self::Grooming),
            "ready" => Ok(Self::Work),
            "review" | "code-review" => Ok(Self::Review),
            "approved" => Ok(Self::Merge),
            status => anyhow::bail!("unsupported Colosseum task phase: {status}"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionSpec {
    #[serde(default)]
    pub repository: PathBuf,
    pub provider: String,
    #[serde(default)]
    pub work_type: WorkType,
    #[serde(default)]
    pub autopilot: bool,
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
        if !task.colosseum_ready && task.colosseum_config.is_null() {
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
        lifecycle::execute(&self.savant, &self.config, task).await
    }

    pub async fn run_next(&self, workspace_id: Option<&str>) -> Result<Option<ExecutionOutcome>> {
        worker::run_next(self, workspace_id).await
    }
}
