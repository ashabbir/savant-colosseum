use std::{path::PathBuf, process::ExitCode, time::Duration};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use savant_executioner::{
    ExecutionRunner, RunnerConfig,
    managed::{WorkerRecord, WorkerRegistry, WorkerStatus, read_log},
    savant::SavantClient,
};
use serde_json::{Value, json};

const EXIT_EXECUTION: u8 = 1;
const EXIT_ARGUMENT: u8 = 2;
const EXIT_CONFIGURATION: u8 = 3;
const EXIT_DEPENDENCY: u8 = 4;
const EXIT_LIFECYCLE: u8 = 5;

#[derive(Parser, Clone)]
#[command(
    name = "savant-colosseum",
    version,
    disable_help_subcommand = true,
    about = "Managed Savant Colosseum workers"
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
    #[arg(long, env = "SAVANT_EXECUTIONER_HOME", hide_env_values = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Clone)]
enum Command {
    /// Show the JSON CLI contract, examples, exit codes, and log locations.
    Help,
    /// Start one managed worker. Omit --daemon to remain attached.
    Start {
        #[arg(short = 'w', long)]
        workspace: Option<String>,
        #[arg(short = 'd', long)]
        daemon: bool,
        #[arg(long, default_value_t = 15)]
        poll_seconds: u64,
    },
    /// List retained worker records.
    Ps,
    /// Print a worker's complete JSONL event log.
    Logs { id: String },
    /// Request graceful shutdown of a running worker.
    Stop { id: String },
    /// Compatibility alias: claim and execute at most one ready task.
    Once,
    /// Compatibility alias: run an attached legacy polling worker.
    Worker {
        #[arg(long, default_value_t = 15)]
        poll_seconds: u64,
    },
    #[command(name = "__run-managed", hide = true)]
    __RunManaged {
        worker_id: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long, default_value_t = 15)]
        poll_seconds: u64,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if error.kind() == clap::error::ErrorKind::DisplayVersion {
                emit(
                    json!({"timestamp":now(),"event":"version","worker_id":null,"workspace_id":null,"status":"success","message":"savant-colosseum version","data":{"version":env!("CARGO_PKG_VERSION")},"error":null}),
                );
                return ExitCode::SUCCESS;
            }
            emit(error_event(
                "argument.invalid",
                EXIT_ARGUMENT,
                &error.to_string(),
            ));
            return ExitCode::from(EXIT_ARGUMENT);
        }
    };
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let code = error_code(&error);
            emit(error_event("command.failed", code, &error.to_string()));
            ExitCode::from(code)
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    let data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);
    let registry = WorkerRegistry::new(&data_dir);
    match cli.command.clone() {
        Command::Help => {
            emit(
                json!({"timestamp": now(), "event":"help", "worker_id":null, "workspace_id":null, "status":"success", "message":"managed Colosseum CLI", "data":{"commands":["start [-w WORKSPACE] [-d]","ps","logs <id>","stop <id>","once (compatibility)","worker (compatibility)"],"exit_codes":{"0":"success","1":"execution failure","2":"invalid argument","3":"configuration","4":"API or service dependency","5":"worker unavailable"},"log_root":data_dir.join("workers")}, "error":null}),
            );
        }
        Command::Ps => {
            emit(
                json!({"timestamp":now(),"event":"workers.listed","worker_id":null,"workspace_id":null,"status":"success","message":"worker registry", "data":{"workers":registry.all()?},"error":null}),
            );
        }
        Command::Logs { id } => {
            let worker = registry.get(&id).map_err(lifecycle_error)?;
            if !registry.log_exists(&worker) {
                bail!("LIFECYCLE: worker log is unavailable");
            }
            print!("{}", read_log(&worker.log_path)?);
        }
        Command::Stop { id } => {
            let (_, event) = registry.stop(&id).map_err(lifecycle_error)?;
            emit(event);
        }
        Command::Start {
            workspace,
            daemon,
            poll_seconds,
        } => {
            let workspace = workspace.or(cli.workspace_id.clone());
            if let Some(existing) = registry.active_for_workspace(workspace.as_deref())? {
                bail!(
                    "LIFECYCLE: workspace already has running worker {}",
                    existing.worker_id
                );
            }
            if daemon {
                start_daemon(&cli, &data_dir, &registry, workspace, poll_seconds)?;
            } else {
                let worker = registry.create(workspace.clone(), Some(std::process::id()))?;
                emit_event(
                    &registry,
                    &worker,
                    "worker.starting",
                    "running",
                    "attached worker starting",
                    None,
                )?;
                run_managed(cli, data_dir, registry, worker, poll_seconds).await?;
            }
        }
        Command::Once => {
            let runner = runner(&cli, &data_dir)?;
            run_once(&runner, cli.workspace_id.as_deref()).await?;
        }
        Command::Worker { poll_seconds } => {
            let worker = registry.create(cli.workspace_id.clone(), Some(std::process::id()))?;
            run_managed(cli, data_dir, registry, worker, poll_seconds).await?;
        }
        Command::__RunManaged {
            worker_id,
            workspace: _,
            poll_seconds,
        } => {
            let worker = registry.get(&worker_id).map_err(lifecycle_error)?;
            run_managed(cli, data_dir, registry, worker, poll_seconds).await?;
        }
    }
    Ok(())
}

fn start_daemon(
    cli: &Cli,
    data_dir: &std::path::Path,
    registry: &WorkerRegistry,
    workspace: Option<String>,
    poll_seconds: u64,
) -> Result<()> {
    let worker = registry.create(workspace.clone(), None)?;
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.args([
        "--server-url",
        &cli.server_url,
        "--data-dir",
        &data_dir.display().to_string(),
    ]);
    if let Some(file) = &cli.api_key_file {
        command.args(["--api-key-file", &file.display().to_string()]);
    }
    if let Some(key) = &cli.api_key {
        command.args(["--api-key", key]);
    }
    command.args([
        "__run-managed",
        &worker.worker_id,
        "--poll-seconds",
        &poll_seconds.to_string(),
    ]);
    if let Some(workspace) = workspace {
        command.args(["--workspace", &workspace]);
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let child = command.spawn().context("start daemon worker")?;
    let worker = registry.update(&worker.worker_id, WorkerStatus::Running, Some(child.id()))?;
    registry.event(
        &worker,
        "worker.started",
        "running",
        "daemon worker started",
        Some(json!({"daemon":true})),
        None,
    )?;
    emit(
        json!({"timestamp":now(),"event":"worker.started","worker_id":worker.worker_id,"workspace_id":worker.workspace_id,"status":"running","message":"daemon worker started","data":{"pid":worker.pid,"log_path":worker.log_path},"error":null}),
    );
    Ok(())
}

async fn run_managed(
    cli: Cli,
    data_dir: PathBuf,
    registry: WorkerRegistry,
    worker: WorkerRecord,
    poll_seconds: u64,
) -> Result<()> {
    let runner = match runner(&cli, &data_dir) {
        Ok(runner) => runner,
        Err(error) => {
            let failed = registry.update(&worker.worker_id, WorkerStatus::Failed, None)?;
            emit_event(
                &registry,
                &failed,
                "worker.configuration_failed",
                "failed",
                "configuration could not be loaded",
                Some(json!({"cause": error.to_string()})),
            )?;
            return Err(anyhow::anyhow!("CONFIGURATION: {error}"));
        }
    };
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("register SIGTERM handler")?;
    emit_event(
        &registry,
        &worker,
        "worker.configuration_loaded",
        "running",
        "configuration loaded",
        None,
    )?;
    loop {
        match runner.run_next(worker.workspace_id.as_deref()).await {
            Ok(Some(outcome)) => emit_event(
                &registry,
                &worker,
                "task.completed",
                "running",
                "task execution completed",
                Some(serde_json::to_value(outcome)?),
            )?,
            Ok(None) => emit_event(
                &registry,
                &worker,
                "worker.idle",
                "idle",
                "no ready Colosseum task",
                None,
            )?,
            Err(error) => {
                let failed = registry.update(&worker.worker_id, WorkerStatus::Failed, None)?;
                emit_event(
                    &registry,
                    &failed,
                    "worker.failed",
                    "failed",
                    "worker execution failed",
                    Some(json!({"cause":error.to_string()})),
                )?;
                bail!("EXECUTION: {error}");
            }
        }
        tokio::select! {
            _ = tokio::signal::ctrl_c() => { let stopped = registry.update(&worker.worker_id, WorkerStatus::Stopped, None)?; emit_event(&registry, &stopped, "worker.stopped", "stopped", "worker stopped", None)?; return Ok(()); }
            _ = terminate.recv() => { let stopped = registry.update(&worker.worker_id, WorkerStatus::Stopped, None)?; emit_event(&registry, &stopped, "worker.stopped", "stopped", "worker stopped", None)?; return Ok(()); }
            _ = tokio::time::sleep(Duration::from_secs(poll_seconds)) => {}
        }
    }
}

fn runner(cli: &Cli, data_dir: &std::path::Path) -> Result<ExecutionRunner> {
    let key = resolve_api_key(cli.api_key.clone(), cli.api_key_file.as_deref())?;
    Ok(ExecutionRunner::new(
        SavantClient::new(&cli.server_url, key.as_deref())?,
        RunnerConfig {
            worktree_root: data_dir.join("worktrees"),
            log_root: data_dir.join("logs"),
        },
    ))
}
async fn run_once(runner: &ExecutionRunner, workspace: Option<&str>) -> Result<()> {
    match runner.run_next(workspace).await? {
        Some(outcome) => emit(
            json!({"timestamp":now(),"event":"task.completed","worker_id":null,"workspace_id":workspace,"status":"success","message":"task executed","data":outcome,"error":null}),
        ),
        None => emit(
            json!({"timestamp":now(),"event":"worker.idle","worker_id":null,"workspace_id":workspace,"status":"idle","message":"no ready Colosseum task","data":null,"error":null}),
        ),
    };
    Ok(())
}
fn emit_event(
    registry: &WorkerRegistry,
    worker: &WorkerRecord,
    event: &str,
    status: &str,
    message: &str,
    data: Option<Value>,
) -> Result<()> {
    emit(registry.event(worker, event, status, message, data, None)?);
    Ok(())
}
fn emit(value: Value) {
    println!(
        "{}",
        serde_json::to_string(&value).expect("JSON event serializes")
    );
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn error_event(event: &str, code: u8, message: &str) -> Value {
    json!({"timestamp":now(),"event":event,"worker_id":null,"workspace_id":null,"status":"failed","message":"command failed","data":null,"error":{"code":code,"message":message}})
}
fn lifecycle_error(error: anyhow::Error) -> anyhow::Error {
    anyhow::anyhow!("LIFECYCLE: {error}")
}
fn error_code(error: &anyhow::Error) -> u8 {
    let message = error.to_string();
    if message.starts_with("LIFECYCLE:") {
        EXIT_LIFECYCLE
    } else if message.starts_with("CONFIGURATION:")
        || message.contains("SAVANT_API_KEY")
        || message.contains("invalid base URL")
    {
        EXIT_CONFIGURATION
    } else if message.contains("HTTP") || message.contains("request") {
        EXIT_DEPENDENCY
    } else {
        EXIT_EXECUTION
    }
}
fn default_data_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".savant")
        .join("colosseum")
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn exposes_a_version_for_the_installer() {
        assert_eq!(
            Cli::command().get_version(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        let error = match Cli::try_parse_from(["savant-colosseum", "--version"]) {
            Err(error) => error,
            Ok(_) => panic!("--version must short-circuit parsing"),
        };
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn api_key_file_is_trimmed() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        writeln!(file, " key ").unwrap();
        assert_eq!(
            resolve_api_key(None, Some(file.path())).unwrap().as_deref(),
            Some("key")
        );
    }
    #[test]
    fn lifecycle_errors_get_exit_five() {
        assert_eq!(
            error_code(&anyhow::anyhow!("LIFECYCLE: missing")),
            EXIT_LIFECYCLE
        );
    }
}
