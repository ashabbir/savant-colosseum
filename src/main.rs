use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use savant_executioner::{ExecutionRunner, RunnerConfig, savant::SavantClient};

#[derive(Parser)]
#[command(
    name = "savant-executioner",
    about = "Runs opted-in Savant development tasks in isolated Git worktrees"
)]
struct Cli {
    #[arg(
        long,
        env = "SAVANT_SERVER_URL",
        default_value = "http://127.0.0.1:8090",
        hide_env_values = true
    )]
    server_url: String,
    #[arg(long, env = "SAVANT_API_KEY", hide_env_values = true)]
    api_key: Option<String>,
    #[arg(long, env = "SAVANT_API_KEY_FILE", hide_env_values = true)]
    api_key_file: Option<PathBuf>,
    #[arg(long, env = "SAVANT_WORKSPACE_ID", hide_env_values = true)]
    workspace_id: Option<String>,
    #[arg(
        long,
        env = "SAVANT_EXECUTIONER_HOME",
        default_value = ".savant-executioner",
        hide_env_values = true
    )]
    data_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Claim and execute at most one ready development task, then exit.
    Once,
    /// Keep processing ready development tasks. Ctrl-C stops after the current task.
    Worker {
        #[arg(long, default_value_t = 15)]
        poll_seconds: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "savant_executioner=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let api_key = resolve_api_key(cli.api_key, cli.api_key_file.as_deref())?;
    let runner = ExecutionRunner::new(
        SavantClient::new(&cli.server_url, api_key.as_deref())?,
        RunnerConfig {
            worktree_root: cli.data_dir.join("worktrees"),
            log_root: cli.data_dir.join("logs"),
        },
    );
    match cli.command {
        Command::Once => {
            print_once(&runner, cli.workspace_id.as_deref()).await?;
        }
        Command::Worker { poll_seconds } => loop {
            if print_once(&runner, cli.workspace_id.as_deref()).await? {
                continue;
            }
            tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                _ = tokio::time::sleep(Duration::from_secs(poll_seconds)) => {}
            }
        },
    }
    Ok(())
}

fn resolve_api_key(
    inline: Option<String>,
    file: Option<&std::path::Path>,
) -> Result<Option<String>> {
    if let Some(key) = inline.filter(|key| !key.trim().is_empty()) {
        return Ok(Some(key));
    }
    let Some(file) = file else {
        return Ok(None);
    };
    let key = std::fs::read_to_string(file)
        .with_context(|| format!("read SAVANT_API_KEY_FILE {}", file.display()))?;
    let key = key.trim().to_owned();
    if key.is_empty() {
        bail!("SAVANT_API_KEY_FILE {} is empty", file.display());
    }
    Ok(Some(key))
}

async fn print_once(runner: &ExecutionRunner, workspace_id: Option<&str>) -> Result<bool> {
    match runner.run_next(workspace_id).await? {
        Some(outcome) => {
            println!("{}", serde_json::to_string_pretty(&outcome)?);
            Ok(true)
        }
        None => {
            println!("{{\"status\":\"idle\",\"message\":\"no ready Colosseum task\"}}");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::resolve_api_key;

    #[test]
    fn reads_a_trimmed_api_key_from_a_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "  key-from-file  ").unwrap();
        assert_eq!(
            resolve_api_key(None, Some(file.path())).unwrap().as_deref(),
            Some("key-from-file")
        );
    }

    #[test]
    fn inline_api_key_takes_precedence_over_a_file() {
        assert_eq!(
            resolve_api_key(
                Some("inline-key".into()),
                Some(std::path::Path::new("/missing"))
            )
            .unwrap(),
            Some("inline-key".into())
        );
    }
}
