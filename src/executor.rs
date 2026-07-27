use std::{collections::HashMap, path::Path, process::Stdio, time::Duration};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc,
    time::{Instant, timeout},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub stream: String,
    pub text: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutcome {
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

pub async fn run_program(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    stdin: Option<&str>,
    limit: Duration,
    events: Option<mpsc::UnboundedSender<LogEvent>>,
) -> Result<ProcessOutcome> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .envs(env)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("spawn executable {program}"))?;

    if let Some(input) = stdin
        && let Some(mut child_stdin) = child.stdin.take()
    {
        child_stdin.write_all(input.as_bytes()).await?;
    }
    drop(child.stdin.take());

    let stdout = child.stdout.take().context("capture stdout")?;
    let stderr = child.stderr.take().context("capture stderr")?;
    let stdout_events = events.clone();
    let stdout_task = tokio::spawn(read_stream("stdout", stdout, stdout_events));
    let stderr_task = tokio::spawn(read_stream("stderr", stderr, events));
    let started = Instant::now();
    let status = timeout(limit, child.wait()).await;
    let (exit_code, timed_out) = match status {
        Ok(status) => (status?.code().unwrap_or(1), false),
        Err(_) => {
            child.kill().await.ok();
            child.wait().await.ok();
            (124, true)
        }
    };
    let stdout = stdout_task.await??;
    let stderr = stderr_task.await??;
    Ok(ProcessOutcome {
        exit_code,
        duration_ms: started.elapsed().as_millis() as u64,
        timed_out,
        stdout,
        stderr,
    })
}

async fn read_stream<R>(
    name: &str,
    stream: R,
    events: Option<mpsc::UnboundedSender<LogEvent>>,
) -> Result<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stream).lines();
    let mut output = String::new();
    while let Some(line) = lines.next_line().await? {
        output.push_str(&line);
        output.push('\n');
        if let Some(sender) = &events {
            sender
                .send(LogEvent {
                    stream: name.to_owned(),
                    text: line,
                    at: chrono::Utc::now().to_rfc3339(),
                })
                .ok();
        }
    }
    Ok(output)
}

pub async fn run_shell(
    script: &str,
    cwd: &Path,
    limit: Duration,
    events: Option<mpsc::UnboundedSender<LogEvent>>,
) -> Result<ProcessOutcome> {
    if script.trim().is_empty() {
        bail!("shell command must not be empty");
    }
    run_program(
        "/bin/sh",
        &["-lc".to_owned(), script.to_owned()],
        cwd,
        &HashMap::new(),
        None,
        limit,
        events,
    )
    .await
}
