use std::{path::PathBuf, time::Duration};

use anyhow::Result;
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
    #[arg(long, env = "SAVANT_WORKSPACE_ID", hide_env_values = true)]
    workspace_id: String,
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
    let runner = ExecutionRunner::new(
        SavantClient::new(&cli.server_url, cli.api_key.as_deref())?,
        RunnerConfig {
            worktree_root: cli.data_dir.join("worktrees"),
            log_root: cli.data_dir.join("logs"),
        },
    );
    match cli.command {
        Command::Once => {
            print_once(&runner, &cli.workspace_id).await?;
        }
        Command::Worker { poll_seconds } => loop {
            if print_once(&runner, &cli.workspace_id).await? {
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

async fn print_once(runner: &ExecutionRunner, workspace_id: &str) -> Result<bool> {
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
