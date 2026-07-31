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
    pub(super) review: String,
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
    let note = review_note(worktree, log_file, &commit, &remote);
    match worktree::create_or_comment_github_review(
        &worktree.path,
        &worktree.branch,
        &task.title,
        &note,
    )
    .await
    {
        Ok(review) => Some(Publication {
            commit,
            remote,
            review,
        }),
        Err(error) => record_failure(events, "github-review", error),
    }
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

fn review_note(worktree: &Worktree, log_file: &Path, commit: &str, remote: &str) -> String {
    format!(
        "Colosseum execution verified.\n\n- Worktree: `{}`\n- Branch: `{}`\n- Commit: `{commit}`\n- Remote: `{remote}`\n- Log: `{}`\n- Validation: passed",
        worktree.path.display(),
        worktree.branch,
        log_file.display()
    )
}
