use std::{collections::HashMap, path::Path, time::Duration};

use anyhow::Result;
use tokio::sync::mpsc;

use super::{LogEvent, ProcessOutcome, run_program};

/// Runs terminal-native coding agents behind macOS `script`, which allocates a
/// pseudo-terminal while Colosseum retains a captured stream for JSONL logs.
pub async fn run_pty_program(
    program: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    stdin: Option<&str>,
    limit: Duration,
    events: Option<mpsc::UnboundedSender<LogEvent>>,
) -> Result<ProcessOutcome> {
    #[cfg(target_os = "macos")]
    {
        let mut script_args = vec!["-q".to_owned(), "/dev/null".to_owned(), program.to_owned()];
        script_args.extend(args.iter().cloned());
        return run_program("script", &script_args, cwd, env, stdin, limit, events).await;
    }
    #[cfg(not(target_os = "macos"))]
    run_program(program, args, cwd, env, stdin, limit, events).await
}
