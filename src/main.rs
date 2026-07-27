use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use savant_colosseum::{
    Runner, RunnerConfig, Scenario,
    database::{self, battle_results, list_battles},
};
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "colosseum", about = "Headless multi-agent benchmark arena")]
struct Cli {
    #[arg(long, env = "COLOSSEUM_DATA_DIR", default_value = "./data")]
    data_dir: PathBuf,
    #[arg(long, env = "COLOSSEUM_WORKTREE_DIR", default_value = "./worktrees")]
    worktree_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        scenario: PathBuf,
        #[arg(long)]
        quiet: bool,
    },
    List {
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    Show {
        battle_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "savant_colosseum=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let pool = database::open(&cli.data_dir.join("colosseum.db")).await?;
    match cli.command {
        Command::Run { scenario, quiet } => {
            let scenario = Scenario::from_path(scenario).await?;
            let runner = Runner::new(RunnerConfig {
                pool,
                worktree_root: cli.worktree_dir,
                log_root: cli.data_dir.join("logs"),
            });
            let (events, mut receiver) = mpsc::unbounded_channel();
            let printer = tokio::spawn(async move {
                while let Some(event) = receiver.recv().await {
                    if !quiet {
                        println!("{}", serde_json::to_string(&event).unwrap());
                    }
                }
            });
            let result = runner.run(scenario, Some(events)).await?;
            printer.await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
            if result.status != "completed" {
                std::process::exit(1);
            }
        }
        Command::List { limit } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&list_battles(&pool, limit).await?)?
            );
        }
        Command::Show { battle_id } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&battle_results(&pool, &battle_id).await?)?
            );
        }
    }
    Ok(())
}
