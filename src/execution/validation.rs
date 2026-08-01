use std::{path::Path, time::Duration};

use anyhow::Result;
use tokio::sync::mpsc;

use crate::executor::ProcessOutcome;

use super::{policy, steps};

pub(super) async fn run(
    provider: &str,
    worktree: &Path,
    prompt: &str,
    limit: Duration,
    events: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<(ProcessOutcome, Option<ProcessOutcome>)> {
    let (program, mut args) = policy::provider_command(provider)?;
    if program == "codex" || program == "agy" || program == "claude" || program == "copilot" {
        args.push(prompt.to_string());
    }
    let agent = steps::run_provider(
        "agent",
        program,
        &args,
        worktree,
        prompt,
        limit,
        events.clone(),
    )
    .await?;
    let validation = run_validation(&agent, worktree, limit, events).await?;
    Ok((agent, validation))
}

pub(super) async fn run_agent_only(
    provider: &str,
    cwd: &Path,
    prompt: &str,
    limit: Duration,
    events: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<ProcessOutcome> {
    let (program, mut args) = policy::provider_command(provider)?;
    if matches!(program, "codex" | "agy" | "claude" | "copilot") {
        args.push(prompt.to_owned());
    }
    steps::run_provider("agent", program, &args, cwd, prompt, limit, events).await
}

async fn run_validation(
    agent: &ProcessOutcome,
    worktree: &Path,
    limit: Duration,
    events: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<Option<ProcessOutcome>> {
    if agent.exit_code != 0 || agent.timed_out {
        return Ok(None);
    }
    let diff_check = steps::run_shell(
        "validation",
        "git diff --check",
        worktree,
        limit,
        events.clone(),
    )
    .await?;
    if diff_check.exit_code != 0 || diff_check.timed_out {
        return Ok(Some(diff_check));
    }
    Ok(Some(
        steps::run_shell(
            "project-validation",
            policy::verification_command(worktree),
            worktree,
            limit,
            events,
        )
        .await?,
    ))
}
