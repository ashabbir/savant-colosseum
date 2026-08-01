use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail};
use uuid::Uuid;

use crate::{
    executor::ProcessOutcome,
    savant::{SavantClient, Task},
    worktree::{self, Worktree},
};

use super::{
    ExecutionOutcome, ExecutionPhase, ExecutionSpec, WorkType,
    decision::{AgentDecision, Decision},
    event_log::EventLog,
    setup,
    types::RunnerConfig,
    validation,
};

pub(super) async fn execute(
    savant: &SavantClient,
    config: &RunnerConfig,
    task: Task,
    spec: ExecutionSpec,
    phase: ExecutionPhase,
) -> Result<ExecutionOutcome> {
    if phase == ExecutionPhase::Merge {
        return merge_approved(savant, config, task, spec).await;
    }

    let run_id = Uuid::new_v4();
    let (cwd, worktree) = phase_workspace(config, &task, &spec, phase).await?;
    let log_file = config
        .log_root
        .join(&task.task_id)
        .join(format!("{run_id}.jsonl"));
    let events = EventLog::start(&log_file).await?;
    events.record(serde_json::json!({
        "type":"started",
        "run_id":run_id,
        "task_id":task.task_id,
        "phase":phase_name(phase),
        "cwd":cwd,
    }));

    let ability_prompt =
        setup::resolve_phase_ability_prompt(savant, &spec.repository, &task, phase, &events)
            .await?;
    let prompt = decision_prompt(&ability_prompt, &task, &spec, phase, worktree.as_ref());
    let agent = validation::run_agent_only(
        &spec.provider,
        &cwd,
        &prompt,
        Duration::from_secs(spec.timeout_seconds),
        events.sender(),
    )
    .await?;
    let decision = parse_agent_decision(phase, &agent)?;
    let (status, ready, merged_commit) =
        apply_decision(savant, &task, &spec, phase, &decision, worktree.as_ref()).await?;

    let comment = format!(
        "## Colosseum {}: {}\n\n{}\n\n**Next status:** `{status}`",
        phase_name(phase),
        decision_label(decision.decision),
        decision.comment_body(),
    );
    savant
        .add_comment(&task.task_id, &comment, "Colosseum")
        .await?;
    savant
        .append_colosseum_run(
            &task.task_id,
            &serde_json::json!({
                "run_id":run_id,
                "phase":phase_name(phase),
                "status":decision_label(decision.decision),
                "summary":decision.summary,
                "rationale":decision.rationale,
                "questions":decision.questions,
                "provider":spec.provider,
                "log_path":log_file,
                "worktree_path":worktree.as_ref().map(|item| &item.path),
                "branch":worktree.as_ref().map(|item| &item.branch),
                "base_commit":evidence_base_commit(&task, phase, worktree.as_ref()),
                "merge_commit":merged_commit,
                "agent_exit_code":agent.exit_code,
                "duration_ms":agent.duration_ms,
            }),
        )
        .await?;
    events.record(serde_json::json!({
        "type":"finished",
        "status":status,
        "ready":ready,
        "decision":decision,
    }));
    events.finish().await?;
    savant.update_status(&task.task_id, status).await?;
    savant.set_colosseum_ready(&task.task_id, ready).await?;

    Ok(ExecutionOutcome {
        run_id,
        task_id: task.task_id,
        status: status.to_owned(),
        worktree: cwd,
        log_file,
        agent,
        validation: None,
    })
}

fn parse_agent_decision(phase: ExecutionPhase, agent: &ProcessOutcome) -> Result<AgentDecision> {
    if agent.timed_out {
        bail!("{} agent timed out", phase_name(phase));
    }
    let combined = format!("{}\n{}", agent.stdout, agent.stderr);
    // Some interactive provider wrappers return a non-zero PTY teardown code
    // after emitting a complete result. The structured marker remains the
    // fail-closed contract: without it, a non-zero exit never advances state.
    AgentDecision::parse(&combined).with_context(|| {
        format!(
            "{} agent failed with exit code {} and no valid result",
            phase_name(phase),
            agent.exit_code
        )
    })
}

async fn phase_workspace(
    config: &RunnerConfig,
    task: &Task,
    spec: &ExecutionSpec,
    phase: ExecutionPhase,
) -> Result<(PathBuf, Option<Worktree>)> {
    if phase == ExecutionPhase::Review && spec.work_type == WorkType::Development {
        require_repository(spec)?;
        let worktree = worktree::provision_task(
            &spec.repository,
            &config.worktree_root,
            &task.task_id,
            &spec.revision,
        )
        .await?;
        return Ok((worktree.path.clone(), Some(worktree)));
    }
    let path = config
        .log_root
        .join(&task.task_id)
        .join(format!("{}-workspace", phase_name(phase)));
    tokio::fs::create_dir_all(&path).await?;
    Ok((path, None))
}

fn decision_prompt(
    ability_prompt: &str,
    task: &Task,
    spec: &ExecutionSpec,
    phase: ExecutionPhase,
    worktree: Option<&Worktree>,
) -> String {
    let instructions = match phase {
        ExecutionPhase::Grooming => {
            concat!(
                "Groom the ticket without editing files. Identify assumptions, acceptance criteria, risks, ",
                "and every ambiguity. Choose ready only when no human clarification is required; ",
                "otherwise choose needs-input."
            )
        }
        ExecutionPhase::Work => {
            concat!(
                "Complete this research/information task. Put the useful result in the summary and explain ",
                "the sources or reasoning in rationale. Choose complete only when the requested information ",
                "is ready for independent review; otherwise choose fail."
            )
        }
        ExecutionPhase::Review if spec.work_type == WorkType::Development => {
            concat!(
                "Review the already-published code in this worktree. Inspect the full base-to-HEAD diff and ",
                "run focused checks when useful, but do not edit files. Choose pass only when the implementation ",
                "and validation evidence satisfy the ticket; otherwise choose fail and enumerate the defects."
            )
        }
        ExecutionPhase::Review => {
            concat!(
                "Independently review the ticket result and prior activity. Choose pass only when the result ",
                "answers the task accurately and completely; otherwise choose fail and enumerate the gaps."
            )
        }
        ExecutionPhase::Merge => unreachable!("merge does not invoke a provider"),
    };
    let allowed = match phase {
        ExecutionPhase::Grooming => "ready | needs-input",
        ExecutionPhase::Work => "complete | fail",
        ExecutionPhase::Review => "pass | fail",
        ExecutionPhase::Merge => "pass",
    };
    let worktree_context = worktree.map_or_else(String::new, |item| {
        let base_commit =
            evidence_base_commit(task, phase, Some(item)).unwrap_or(item.start_commit.as_str());
        format!(
            "\nWorktree: {}\nBase commit: {}\nBranch: {}\n",
            item.path.display(),
            base_commit,
            item.branch
        )
    });
    format!(
        concat!(
            "{}\n\n# Colosseum {} contract\n{}\n\nTask {}: {}\n\n{}\n",
            "Work type: {:?}\nPrior ticket activity: {}{}\n\n",
            "Your final output MUST contain exactly one single-line marker:\n",
            "COLOSSEUM_RESULT: {{\"decision\":\"<{}>\",\"summary\":\"what you found or did\",",
            "\"rationale\":\"why this decision is justified\",\"questions\":[]}}"
        ),
        ability_prompt,
        phase_name(phase),
        instructions,
        task.task_id,
        task.title,
        task.description,
        spec.work_type,
        task.comments,
        worktree_context,
        allowed,
    )
}

fn evidence_base_commit<'a>(
    task: &'a Task,
    phase: ExecutionPhase,
    worktree: Option<&'a Worktree>,
) -> Option<&'a str> {
    if phase == ExecutionPhase::Review {
        task.colosseum_config
            .get("base_commit")
            .and_then(|value| value.as_str())
            .or_else(|| worktree.map(|item| item.start_commit.as_str()))
    } else {
        worktree.map(|item| item.start_commit.as_str())
    }
}

async fn apply_decision(
    savant: &SavantClient,
    task: &Task,
    spec: &ExecutionSpec,
    phase: ExecutionPhase,
    decision: &AgentDecision,
    worktree: Option<&Worktree>,
) -> Result<(&'static str, bool, Option<String>)> {
    match (phase, decision.decision) {
        (ExecutionPhase::Grooming, Decision::Ready) => Ok(("ready", true, None)),
        (ExecutionPhase::Grooming, Decision::NeedsInput) => Ok(("grooming", false, None)),
        (ExecutionPhase::Work, Decision::Complete) => Ok(("review", true, None)),
        (ExecutionPhase::Work, Decision::Fail) | (ExecutionPhase::Review, Decision::Fail) => {
            Ok(("ready", true, None))
        }
        (ExecutionPhase::Review, Decision::Pass) if !spec.autopilot => {
            Ok(("human-review", false, None))
        }
        (ExecutionPhase::Review, Decision::Pass) if spec.work_type == WorkType::Research => {
            Ok(("done", false, None))
        }
        (ExecutionPhase::Review, Decision::Pass) => {
            let worktree = worktree.context("development review is missing its worktree")?;
            let commit = worktree::merge_and_push(worktree).await?;
            let mr_id = merge_request_id(task);
            savant.update_merge_request_status(&mr_id, "merged").await?;
            Ok(("done", false, Some(commit)))
        }
        _ => bail!(
            "decision {:?} is invalid for {} phase",
            decision.decision,
            phase_name(phase)
        ),
    }
}

async fn merge_approved(
    savant: &SavantClient,
    config: &RunnerConfig,
    task: Task,
    spec: ExecutionSpec,
) -> Result<ExecutionOutcome> {
    require_repository(&spec)?;
    let run_id = Uuid::new_v4();
    let worktree = worktree::provision_task(
        &spec.repository,
        &config.worktree_root,
        &task.task_id,
        &spec.revision,
    )
    .await?;
    let commit = worktree::merge_and_push(&worktree).await?;
    let mr_id = merge_request_id(&task);
    savant.update_merge_request_status(&mr_id, "merged").await?;
    let log_file = config
        .log_root
        .join(&task.task_id)
        .join(format!("{run_id}.jsonl"));
    let events = EventLog::start(&log_file).await?;
    events.record(serde_json::json!({
        "type":"finished",
        "phase":"merge",
        "status":"done",
        "commit":commit,
        "branch":worktree.branch,
        "base_branch":worktree.base_branch,
    }));
    events.finish().await?;
    savant
        .add_comment(
            &task.task_id,
            &format!(
                concat!(
                    "## Colosseum merge complete\n\nApproved branch `{}` was fast-forwarded into `{}` ",
                    "at commit `{}`. Merge request `{}` is closed as merged."
                ),
                worktree.branch, worktree.base_branch, commit, mr_id
            ),
            "Colosseum",
        )
        .await?;
    savant
        .append_colosseum_run(
            &task.task_id,
            &serde_json::json!({
                "run_id":run_id,
                "phase":"merge",
                "status":"passed",
                "summary":"Approved work merged",
                "rationale":"Human approval authorized the fast-forward merge",
                "commit":commit,
                "branch":worktree.branch,
                "base_branch":worktree.base_branch,
                "mr_id":mr_id,
                "log_path":log_file,
            }),
        )
        .await?;
    savant.update_status(&task.task_id, "done").await?;
    savant.set_colosseum_ready(&task.task_id, false).await?;
    let agent = successful_noop();
    Ok(ExecutionOutcome {
        run_id,
        task_id: task.task_id,
        status: "done".into(),
        worktree: worktree.path,
        log_file,
        agent,
        validation: None,
    })
}

fn require_repository(spec: &ExecutionSpec) -> Result<()> {
    if spec.repository.as_os_str().is_empty() {
        bail!("development work requires an assigned repository");
    }
    Ok(())
}

fn merge_request_id(task: &Task) -> String {
    task.colosseum_config
        .get("mr_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("mr-colosseum-{}", task.task_id))
}

fn phase_name(phase: ExecutionPhase) -> &'static str {
    match phase {
        ExecutionPhase::Grooming => "grooming",
        ExecutionPhase::Work => "work",
        ExecutionPhase::Review => "review",
        ExecutionPhase::Merge => "merge",
    }
}

fn decision_label(decision: Decision) -> &'static str {
    match decision {
        Decision::Ready => "ready",
        Decision::NeedsInput => "needs-input",
        Decision::Pass => "passed",
        Decision::Fail => "failed",
        Decision::Complete => "completed",
    }
}

fn successful_noop() -> ProcessOutcome {
    ProcessOutcome {
        exit_code: 0,
        duration_ms: 0,
        timed_out: false,
        stdout: String::new(),
        stderr: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{ExecutionPhase, evidence_base_commit, parse_agent_decision};
    use crate::{executor::ProcessOutcome, savant::Task, worktree::Worktree};

    fn outcome(exit_code: i32, timed_out: bool, stdout: &str) -> ProcessOutcome {
        ProcessOutcome {
            exit_code,
            duration_ms: 1,
            timed_out,
            stdout: stdout.to_owned(),
            stderr: "script: write master: Input/output error".to_owned(),
        }
    }

    #[test]
    fn review_evidence_uses_the_original_publication_base() {
        let task = Task {
            task_id: "task-1".into(),
            workspace_id: "workspace-1".into(),
            title: "Review evidence".into(),
            description: String::new(),
            status: "in-progress".into(),
            colosseum_claimed_from: Some("review".into()),
            priority: "medium".into(),
            depends_on: vec![],
            colosseum_ready: false,
            colosseum_config: serde_json::json!({"base_commit":"published-base"}),
            comments: serde_json::json!([]),
        };
        let worktree = Worktree {
            path: PathBuf::from("/tmp/worktree"),
            branch: "task-branch".into(),
            start_commit: "published-head".into(),
            base_branch: "main".into(),
        };

        assert_eq!(
            evidence_base_commit(&task, ExecutionPhase::Review, Some(&worktree)),
            Some("published-base")
        );
        assert_eq!(
            evidence_base_commit(&task, ExecutionPhase::Work, Some(&worktree)),
            Some("published-head")
        );
    }

    #[test]
    fn accepts_a_valid_decision_after_a_provider_teardown_error() {
        let output = concat!(
            "COLOSSEUM_RESULT: {\"decision\":\"ready\",\"summary\":\"Clear\",",
            "\"rationale\":\"No ambiguity\",\"questions\":[]}"
        );
        assert!(parse_agent_decision(ExecutionPhase::Grooming, &outcome(1, false, output)).is_ok());
    }

    #[test]
    fn nonzero_exit_without_a_valid_decision_fails_closed() {
        assert!(
            parse_agent_decision(ExecutionPhase::Grooming, &outcome(1, false, "done")).is_err()
        );
    }

    #[test]
    fn timeout_rejects_even_a_valid_decision() {
        let output = concat!(
            "COLOSSEUM_RESULT: {\"decision\":\"ready\",\"summary\":\"Clear\",",
            "\"rationale\":\"No ambiguity\",\"questions\":[]}"
        );
        assert!(
            parse_agent_decision(ExecutionPhase::Grooming, &outcome(124, true, output)).is_err()
        );
    }
}
