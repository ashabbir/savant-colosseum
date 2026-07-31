use std::path::PathBuf;

use serde::Serialize;
use uuid::Uuid;

use crate::executor::ProcessOutcome;

#[derive(Clone)]
pub struct RunnerConfig {
    pub worktree_root: PathBuf,
    pub log_root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionOutcome {
    pub run_id: Uuid,
    pub task_id: String,
    pub status: String,
    pub worktree: PathBuf,
    pub log_file: PathBuf,
    pub agent: ProcessOutcome,
    pub validation: Option<ProcessOutcome>,
}
