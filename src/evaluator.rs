use std::{path::Path, time::Duration};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::executor::{LogEvent, run_shell};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    pub success: bool,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub tests_run: Option<u64>,
    pub tests_passed: Option<u64>,
    pub tests_failed: Option<u64>,
    pub lint_violations: u64,
    pub stdout: String,
    pub stderr: String,
}

pub async fn evaluate(
    worktree: &Path,
    command: &str,
    limit: Duration,
    events: Option<mpsc::UnboundedSender<LogEvent>>,
) -> Result<Evaluation> {
    let process = run_shell(command, worktree, limit, events).await?;
    let combined = format!("{}\n{}", process.stdout, process.stderr);
    let (tests_run, tests_passed, tests_failed) = parse_test_counts(&combined);
    let lint_violations = combined
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error") || lower.contains("warning")
        })
        .count() as u64;
    Ok(Evaluation {
        success: process.exit_code == 0,
        exit_code: process.exit_code,
        duration_ms: process.duration_ms,
        timed_out: process.timed_out,
        tests_run,
        tests_passed,
        tests_failed,
        lint_violations,
        stdout: process.stdout,
        stderr: process.stderr,
    })
}

fn parse_test_counts(output: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    for line in output.lines() {
        let words: Vec<_> = line.split_whitespace().collect();
        for (index, word) in words.iter().enumerate() {
            if word
                .trim_matches(|character: char| !character.is_ascii_alphabetic())
                .eq_ignore_ascii_case("passed")
                && index > 0
                && let Ok(passed) = words[index - 1].trim_matches(',').parse::<u64>()
            {
                let failed = words
                    .iter()
                    .position(|word| {
                        word.trim_matches(|character: char| !character.is_ascii_alphabetic())
                            .eq_ignore_ascii_case("failed")
                    })
                    .and_then(|failed_index| failed_index.checked_sub(1))
                    .and_then(|failed_index| words[failed_index].trim_matches(',').parse().ok())
                    .unwrap_or(0);
                return (Some(passed + failed), Some(passed), Some(failed));
            }
        }
    }
    (None, None, None)
}
