use std::path::Path;

use crate::{
    executor::ProcessOutcome,
    savant::Task,
    worktree::{self, Worktree},
};

use super::event_log::EventLog;

pub(super) struct Publication {
    pub(super) commit: String,
    pub(super) remote: String,
}

pub(super) async fn publish_if_verified(
    task: &Task,
    worktree: &Worktree,
    log_file: &Path,
    agent: &ProcessOutcome,
    validation: Option<&ProcessOutcome>,
    events: &EventLog,
) -> Option<Publication> {
    if !verified(agent, validation) {
        return None;
    }
    let (commit, remote) = match worktree::commit_and_push(
        &worktree.path,
        &worktree.branch,
        &format!("colosseum: {}", task.title),
    )
    .await
    {
        Ok(publication) => publication,
        Err(error) => return record_failure(events, "commit-push", error),
    };
    events.record(serde_json::json!({
        "type":"published",
        "commit":commit,
        "remote":remote,
        "log":log_file,
    }));
    Some(Publication { commit, remote })
}

fn verified(agent: &ProcessOutcome, validation: Option<&ProcessOutcome>) -> bool {
    agent.exit_code == 0
        && !agent.timed_out
        && validation.is_none_or(|result| result.exit_code == 0 && !result.timed_out)
}

fn record_failure(events: &EventLog, stage: &str, error: anyhow::Error) -> Option<Publication> {
    events.record(
        serde_json::json!({"type":"publication-failed","stage":stage,"error":error.to_string()}),
    );
    None
}
