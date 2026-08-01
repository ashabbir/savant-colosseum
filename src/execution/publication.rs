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
    if !worktree.base_is_ancestor {
        return record_failure(
            events,
            "base-branch-containment",
            anyhow::anyhow!(
                "base branch {} at {} is not an ancestor of task HEAD; incorporate the current base before publication",
                worktree.base_branch,
                worktree.base_branch_commit
            ),
        );
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
    // Persist the complete MR, not only the latest repair attempt. The review
    // base is the stable merge-base with the configured base branch.
    let range = format!("{}..{}", worktree.review_base_commit, commit);
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
