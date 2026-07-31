use std::{collections::HashMap, path::Path, time::Duration};

use anyhow::Result;
use tokio::sync::mpsc;

use crate::executor::{self, LogEvent, ProcessOutcome};

pub(super) async fn run_provider(
    phase: &str,
    program: &str,
    args: &[String],
    cwd: &Path,
    prompt: &str,
    limit: Duration,
    sink: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<ProcessOutcome> {
    let (events, forward) = start_forwarding(phase, sink);
    let outcome = executor::run_pty_program(
        program,
        args,
        cwd,
        &HashMap::new(),
        Some(prompt),
        limit,
        Some(events),
    )
    .await?;
    forward.await?;
    Ok(outcome)
}

pub(super) async fn run_shell(
    phase: &str,
    command: &str,
    cwd: &Path,
    limit: Duration,
    sink: mpsc::UnboundedSender<serde_json::Value>,
) -> Result<ProcessOutcome> {
    let (events, forward) = start_forwarding(phase, sink);
    let outcome = executor::run_shell(command, cwd, limit, Some(events)).await?;
    forward.await?;
    Ok(outcome)
}

fn start_forwarding(
    phase: &str,
    sink: mpsc::UnboundedSender<serde_json::Value>,
) -> (mpsc::UnboundedSender<LogEvent>, tokio::task::JoinHandle<()>) {
    let (events, mut receiver) = mpsc::unbounded_channel();
    let phase = phase.to_owned();
    let forward = tokio::spawn(async move {
        while let Some(event) = receiver.recv().await {
            sink.send(serde_json::json!({"type":"log","phase":phase,"event":event}))
                .ok();
        }
    });
    (events, forward)
}
