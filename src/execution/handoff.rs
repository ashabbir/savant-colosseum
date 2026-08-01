use serde_json::{Value, json};

use crate::{savant::Task, worktree::Worktree};

pub(super) const HANDOFF_VERSION: u64 = 2;
const FAILED_REVIEW_HEADING: &str = "## Colosseum review: failed";

pub(super) fn latest_failed_review(comments: &Value) -> Option<&str> {
    comments
        .as_array()?
        .iter()
        .rev()
        .filter_map(comment_text)
        .find(|text| text.contains(FAILED_REVIEW_HEADING))
}

pub(super) fn prior_bounded_review_failures(task: &Task) -> usize {
    task.colosseum_config
        .get("runs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|run| {
            run.get("phase").and_then(Value::as_str) == Some("review")
                && run.get("status").and_then(Value::as_str) == Some("failed")
                && run.get("handoff_version").and_then(Value::as_u64) == Some(HANDOFF_VERSION)
        })
        .count()
}

pub(super) fn task_dossier(task: &Task, worktree: Option<&Worktree>) -> Value {
    let config = &task.colosseum_config;
    let continuation = worktree
        .map(|item| {
            json!({
                "worktree_path": item.path,
                "branch": item.branch,
                "resumed_previous_attempt": item.resumed,
                "attempt_start_commit": item.start_commit,
                "full_mr_base_commit": item.review_base_commit,
                "full_mr_range": format!("{}..HEAD", item.review_base_commit),
                "base_branch": item.base_branch,
                "base_branch_commit": item.base_branch_commit,
                "base_branch_is_ancestor_of_head": item.base_is_ancestor,
            })
        })
        .or_else(|| {
            config.get("worktree_path").map(|path| {
                json!({
                    "worktree_path": path,
                    "branch": config.get("branch"),
                    "resumed_previous_attempt": true,
                    "attempt_start_commit": config.get("attempt_start_commit"),
                    "full_mr_base_commit": config.get("base_commit"),
                    "published_commit": config.get("commit"),
                    "base_branch": config.get("base_branch"),
                    "source": "persisted task metadata; verify against Git before acting",
                })
            })
        });
    json!({
        "contract_version": HANDOFF_VERSION,
        "ticket": {
            "task_id": task.task_id,
            "workspace_id": task.workspace_id,
            "title": task.title,
            "description": task.description,
            "priority": task.priority,
            "depends_on": task.depends_on,
            "claimed_from": task.colosseum_claimed_from,
        },
        "execution": {
            "work_type": config.get("work_type"),
            "repository": config.get("repository"),
            "revision": config.get("revision"),
            "autopilot": config.get("autopilot"),
            "provider": config.get("provider"),
            "model": config.get("model"),
            "phase_configs": config.get("phase_configs"),
        },
        "merge_request": {
            "mr_id": config.get("mr_id"),
            "remote": config.get("remote"),
            "branch": config.get("branch"),
            "base_branch": config.get("base_branch"),
            "published_commit": config.get("commit"),
            "files": config.get("files"),
        },
        "continuation": continuation,
        "run_history": config.get("runs").cloned().unwrap_or_else(|| json!([])),
        "substantive_activity": substantive_activity(&task.comments),
        "latest_failed_review": latest_failed_review(&task.comments),
        "bounded_review_failures": prior_bounded_review_failures(task),
    })
}

pub(super) fn dossier_prompt(task: &Task, worktree: Option<&Worktree>) -> String {
    format!(
        concat!(
            "# Shared Colosseum task dossier\n",
            "This exact dossier is the durable handoff shared across architect, coder, and reviewer. ",
            "Use repository state as the source of truth and the dossier as the complete decision/audit context. ",
            "Never discard valid prior work or restart merely because this is a new agent session.\n\n{}"
        ),
        serde_json::to_string_pretty(&task_dossier(task, worktree))
            .expect("task dossier is JSON serializable")
    )
}

pub(super) fn repair_instructions(task: &Task) -> String {
    let Some(review) = latest_failed_review(&task.comments) else {
        return String::new();
    };
    format!(
        concat!(
            "\n\n# Complete review incorporation contract\n",
            "This is the single bounded repair handback. Treat every finding from the latest whole-MR review ",
            "and the canonical `code-reviews/**/review.md` as one required checklist. Verify each item against ",
            "the current full MR, preserve resolved work, fix every valid unresolved item, and add regression ",
            "coverage for each substantive repair. Before returning complete, self-review the entire full-MR ",
            "range—not only your latest edits—and map every reviewer finding to concrete code and tests.\n\n{}",
        ),
        review.trim()
    )
}

fn substantive_activity(comments: &Value) -> Vec<Value> {
    comments
        .as_array()
        .into_iter()
        .flatten()
        .filter(|comment| {
            let text = comment_text(comment).unwrap_or_default();
            !["🚀", "📂", "🧠", "⚡", "🔍", "💓"]
                .iter()
                .any(|prefix| text.starts_with(prefix))
        })
        .cloned()
        .collect()
}

fn comment_text(comment: &Value) -> Option<&str> {
    comment.get("text").and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::{HANDOFF_VERSION, prior_bounded_review_failures, task_dossier};
    use crate::{savant::Task, worktree::Worktree};
    use serde_json::json;
    use std::path::PathBuf;

    fn task() -> Task {
        Task {
            task_id: "task-1".into(),
            workspace_id: "ws-1".into(),
            title: "Continue work".into(),
            description: "Finish the MR".into(),
            status: "in-progress".into(),
            colosseum_claimed_from: Some("ready".into()),
            priority: "high".into(),
            depends_on: vec![],
            colosseum_ready: false,
            colosseum_config: json!({
                "runs":[
                    {"phase":"review","status":"failed"},
                    {"phase":"review","status":"failed","handoff_version":HANDOFF_VERSION}
                ]
            }),
            comments: json!([
                {"role":"agent","text":"💓 Colosseum Active"},
                {"role":"agent","text":"## Colosseum review: failed\n\n- unsafe stop"},
                {"role":"user","text":"Keep compatibility"}
            ]),
        }
    }

    #[test]
    fn dossier_shares_full_history_and_continuation_boundaries() {
        let task = task();
        let worktree = Worktree {
            path: PathBuf::from("/tmp/task-1"),
            branch: "savant-execution/task-1".into(),
            start_commit: "head-2".into(),
            review_base_commit: "base-1".into(),
            base_branch: "main".into(),
            base_branch_commit: "main-2".into(),
            base_is_ancestor: false,
            resumed: true,
        };

        let dossier = task_dossier(&task, Some(&worktree));

        assert_eq!(dossier["continuation"]["resumed_previous_attempt"], true);
        assert_eq!(dossier["continuation"]["full_mr_range"], "base-1..HEAD");
        assert_eq!(
            dossier["continuation"]["base_branch_is_ancestor_of_head"],
            false
        );
        assert_eq!(dossier["run_history"].as_array().unwrap().len(), 2);
        assert_eq!(dossier["substantive_activity"].as_array().unwrap().len(), 2);
        assert_eq!(dossier["bounded_review_failures"], 1);
    }

    #[test]
    fn only_new_contract_failures_consume_the_bounded_repair() {
        assert_eq!(prior_bounded_review_failures(&task()), 1);
    }
}
