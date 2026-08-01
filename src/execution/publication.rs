use std::path::Path;

use crate::{
    executor::ProcessOutcome,
    savant::Task,
    worktree::{self, Worktree},
};

use super::{event_log::EventLog, validation};

pub(super) struct Publication {
    pub(super) commit: String,
    pub(super) remote: String,
    pub(super) diff: String,
    pub(super) files: Vec<serde_json::Value>,
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
    let range = format!("{}..{}", worktree.start_commit, commit);
    let diff = match worktree::git(&worktree.path, &["diff", &range]).await {
        Ok(value) => value,
        Err(error) => return record_failure(events, "diff capture", error),
    };
    let changed_files =
        match worktree::git(&worktree.path, &["diff", "--name-status", &range]).await {
            Ok(value) => value,
            Err(error) => return record_failure(events, "changed-file capture", error),
        };
    let files = changed_files
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(status, path)| serde_json::json!({"status":status,"path":path}))
        .collect();
    Some(Publication {
        commit,
        remote,
        diff,
        files,
    })
}

fn verified(agent: &ProcessOutcome, validation_result: Option<&ProcessOutcome>) -> bool {
    validation::agent_completed(agent)
        && validation_result.is_none_or(|result| result.exit_code == 0 && !result.timed_out)
}

fn record_failure(events: &EventLog, stage: &str, error: anyhow::Error) -> Option<Publication> {
    events.record(
        serde_json::json!({"type":"publication-failed","stage":stage,"error":error.to_string()}),
    );
    None
}
