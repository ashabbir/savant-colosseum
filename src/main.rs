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
                &anyhow::anyhow!(error.to_string()),
            ));
            return ExitCode::from(EXIT_ARGUMENT);
        }
    };
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let code = error_code(&error);
            emit(error_event("command.failed", code, &error));
            ExitCode::from(code)
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    let data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);
    let registry = WorkerRegistry::new(&data_dir);
    match cli.command.clone() {
        Command::Help => {
            emit(help_event(&data_dir));
        }
        Command::Ps => {
            emit(
                json!({"timestamp":now(),"event":"workers.listed","worker_id":null,"workspace_id":null,"status":"success","message":"worker registry", "data":{"workers":registry.all()?},"error":null}),
            );
        }
        Command::Logs { id } => {
            let worker = registry
                .get(&id)
                .map_err(|error| lifecycle_error_for(Some(id.clone()), None, error))?;
            if !registry.log_exists(&worker) {
                return Err(lifecycle_error_for(
                    Some(id),
                    worker.workspace_id.clone(),
                    anyhow::anyhow!("worker log is unavailable"),
                ));
            }
            print!(
                "{}",
                read_log(&worker.log_path).map_err(|error| lifecycle_error_for(
                    Some(id),
                    worker.workspace_id.clone(),
                    error,
                ))?
            );
        }
        Command::Stop { id } => {
            let (_, event) = registry
                .stop(&id)
                .map_err(|error| lifecycle_error_for(Some(id.clone()), None, error))?;
            emit(event);
        }
        Command::Start {
            workspace,
            daemon,
            poll_seconds,
        } => {
            let workspace = workspace.or(cli.workspace_id.clone());
            validate_workspace_id(workspace.as_deref())?;
            if daemon {
                start_daemon(&cli, &data_dir, &registry, workspace, poll_seconds)?;
            } else {
                let worker = registry
                    .create_if_inactive(workspace.clone(), Some(std::process::id()))
                    .map_err(lifecycle_error)?;
                // Creation is a worker event too.  Attached mode forwards it
                // before emitting later lifecycle events so stdout exactly
                // reflects the JSONL stream from the first record onward.
                emit_existing_events(&worker)?;
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
            validate_workspace_id(cli.workspace_id.as_deref())?;
            let worker = registry
                .create_if_inactive(cli.workspace_id.clone(), Some(std::process::id()))
                .map_err(lifecycle_error)?;
            run_managed(cli, data_dir, registry, worker, poll_seconds).await?;
        }
        Command::__RunManaged {
            worker_id,
            workspace: _,
            poll_seconds,
        } => {
            let worker = registry
                .wait_until_running(&worker_id)
                .map_err(lifecycle_error)?;
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
    let worker = registry
        .create_starting_if_inactive(workspace.clone())
        .map_err(lifecycle_error)?;
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
        command.env("SAVANT_API_KEY", key);
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
    let child = match command.spawn().context("start daemon worker") {
        Ok(child) => child,
        Err(error) => {
            if let Some(failed) =
                registry.finish_if_active(&worker.worker_id, WorkerStatus::Failed)?
            {
                registry.event(
                    &failed,
                    "worker.failed",
                    "failed",
                    "daemon could not start",
                    None,
                    Some(json!({"code":"worker.spawn_failed","message":error.to_string()})),
                )?;
            }
            return Err(anyhow::anyhow!("EXECUTION: {error}"));
        }
    };
    let Some(worker) = registry.mark_running_if_starting(&worker.worker_id, child.id())? else {
        // `stop` won the race while the daemon process was being spawned.
        // This PID belongs to us, but was never published, so it is safe to
        // terminate it directly rather than risk reviving a stopped record.
        let mut child = child;
        let _ = child.kill();
        return Ok(());
    };
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
            if let Some(failed) =
                registry.finish_if_active(&worker.worker_id, WorkerStatus::Failed)?
            {
                emit_failure_event(
                    &registry,
                    &failed,
                    "worker.configuration_failed",
                    "failed",
                    "configuration could not be loaded",
                    "configuration.invalid",
                    &error.to_string(),
                )?;
            }
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
        let result = tokio::select! {
            _ = tokio::signal::ctrl_c() => return stop_worker(&registry, &worker),
            _ = terminate.recv() => return stop_worker(&registry, &worker),
            result = runner.run_next(worker.workspace_id.as_deref()) => result,
        };
        match result {
            Ok(Some(outcome)) => {
                forward_task_log(&registry, &worker, &outcome.log_file)?;
                emit_event(
                    &registry,
                    &worker,
                    "task.completed",
                    "running",
                    "task execution completed",
                    Some(serde_json::to_value(outcome)?),
                )?;
                if let Some(succeeded) =
                    registry.finish_if_active(&worker.worker_id, WorkerStatus::Succeeded)?
                {
                    emit_event(
                        &registry,
                        &succeeded,
                        "worker.succeeded",
                        "succeeded",
                        "worker completed successfully",
                        None,
                    )?;
                }
                return Ok(());
            }
            Ok(None) => emit_event(
                &registry,
                &worker,
                "worker.idle",
                "idle",
                "no ready Colosseum task",
                None,
            )?,
            Err(error) => {
                if let Some(failed) =
                    registry.finish_if_active(&worker.worker_id, WorkerStatus::Failed)?
                {
                    emit_failure_event(
                        &registry,
                        &failed,
                        "worker.failed",
                        "failed",
                        "worker execution failed",
                        "worker.execution_failed",
                        &error.to_string(),
                    )?;
                }
                bail!("EXECUTION: {error}");
            }
        }
        tokio::select! {
            _ = tokio::signal::ctrl_c() => return stop_worker(&registry, &worker),
            _ = terminate.recv() => return stop_worker(&registry, &worker),
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
fn emit_failure_event(
    registry: &WorkerRegistry,
    worker: &WorkerRecord,
    event: &str,
    status: &str,
    message: &str,
    code: &str,
    detail: &str,
) -> Result<()> {
    emit(registry.event(
        worker,
        event,
        status,
        message,
        None,
        Some(json!({"code":code,"message":detail})),
    )?);
    Ok(())
}
fn forward_task_log(
    registry: &WorkerRegistry,
    worker: &WorkerRecord,
    path: &std::path::Path,
) -> Result<()> {
    for line in read_log(path)?.lines() {
        let task_event: Value = serde_json::from_str(line)
            .with_context(|| format!("parse task event in {}", path.display()))?;
        let event = registry.event(
            worker,
            "task.event",
            "running",
            "task lifecycle event",
            Some(task_event),
            None,
        )?;
        emit(event);
    }
    Ok(())
}
fn stop_worker(registry: &WorkerRegistry, worker: &WorkerRecord) -> Result<()> {
    if let Some(stopped) = registry.finish_if_active(&worker.worker_id, WorkerStatus::Stopped)? {
        emit_event(
            registry,
            &stopped,
            "worker.stopped",
            "stopped",
            "worker stopped",
            None,
        )?;
    }
    Ok(())
}

fn emit_existing_events(worker: &WorkerRecord) -> Result<()> {
    for event in worker_events(&worker.log_path)? {
        emit(event);
    }
    Ok(())
}

fn worker_events(path: &std::path::Path) -> Result<Vec<Value>> {
    read_log(path)?
        .lines()
        .map(|line| {
            serde_json::from_str(line)
                .with_context(|| format!("parse worker event in {}", path.display()))
        })
        .collect()
}
fn emit(value: Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(&value).expect("JSON event serializes")
    );
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
fn help_event(data_dir: &std::path::Path) -> Value {
    json!({
        "timestamp": now(),
        "event":"help",
        "worker_id":null,
        "workspace_id":null,
        "status":"success",
        "message":"managed Colosseum CLI",
        "data":{
            "commands":[
                {"name":"start","flags":["-w, --workspace <workspace-id>","-d, --daemon","--poll-seconds <seconds>"],"description":"start one managed worker"},
                {"name":"ps","flags":[],"description":"list retained workers"},
                {"name":"logs <id>","flags":[],"description":"print a worker JSONL log"},
                {"name":"stop <id>","flags":[],"description":"request graceful worker shutdown"},
                {"name":"once","flags":[],"description":"compatibility alias: execute at most one task"},
                {"name":"worker","flags":["--poll-seconds <seconds>"],"description":"compatibility alias: attached polling worker"}
            ],
            "global_flags":["--server-url <url>","--api-key-file <path>","--workspace-id <workspace-id>","--data-dir <path>"],
            "examples":[
                "savant-colosseum start --workspace <workspace-id>",
                "savant-colosseum start --workspace <workspace-id> --daemon",
                "savant-colosseum logs <worker-id>",
                "savant-colosseum stop <worker-id>"
            ],
            "exit_codes":{"0":"success","1":"execution failure","2":"invalid argument","3":"configuration","4":"API or service dependency","5":"worker unavailable"},
            "log_root":data_dir.join("workers"),
            "retention":"Worker registry records and JSONL logs are retained until manually removed."
        },
        "error":null
    })
}
fn error_event(event: &str, exit_code: u8, error: &anyhow::Error) -> Value {
    let lifecycle = error.downcast_ref::<LifecycleFailure>();
    let (worker_id, workspace_id, code) = match lifecycle {
        Some(failure) => (
            failure.worker_id.as_deref(),
            failure.workspace_id.as_deref(),
            "worker.unavailable",
        ),
        None => (
            None,
            None,
            match exit_code {
                EXIT_ARGUMENT => "argument.invalid",
                EXIT_CONFIGURATION => "configuration.invalid",
                EXIT_DEPENDENCY => "dependency.unavailable",
                _ => "execution.failed",
            },
        ),
    };
    json!({"timestamp":now(),"event":event,"worker_id":worker_id,"workspace_id":workspace_id,"status":"failed","message":"command failed","data":{"exit_code":exit_code},"error":{"code":code,"message":error.to_string()}})
}

#[derive(Debug, thiserror::Error)]
#[error("LIFECYCLE: {message}")]
struct LifecycleFailure {
    worker_id: Option<String>,
    workspace_id: Option<String>,
    message: String,
}

fn lifecycle_error(error: anyhow::Error) -> anyhow::Error {
    lifecycle_error_for(None, None, error)
}

fn lifecycle_error_for(
    worker_id: Option<String>,
    workspace_id: Option<String>,
    error: anyhow::Error,
) -> anyhow::Error {
    anyhow::Error::new(LifecycleFailure {
        worker_id,
        workspace_id,
        message: error.to_string(),
    })
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

fn validate_workspace_id(workspace_id: Option<&str>) -> Result<()> {
    if let Some(workspace_id) = workspace_id
        && (workspace_id.is_empty()
            || workspace_id == "."
            || workspace_id == ".."
            || workspace_id.contains(['/', '\\']))
    {
        bail!("CONFIGURATION: workspace ID is not safe for a worker log path");
    }
    Ok(())
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

    #[test]
    fn lifecycle_failure_identifies_the_requested_worker() {
        let error = lifecycle_error_for(Some("01WORKER".into()), None, anyhow::anyhow!("missing"));
        let payload = error_event("command.failed", error_code(&error), &error);

        assert_eq!(payload["worker_id"], "01WORKER");
        assert_eq!(payload["error"]["code"], "worker.unavailable");
    }

    #[test]
    fn help_payload_documents_flags_examples_and_log_location() {
        let payload = help_event(std::path::Path::new("/tmp/colosseum"));
        let data = &payload["data"];

        assert!(data["commands"].as_array().unwrap().iter().any(|command| {
            command["name"] == "start"
                && command["flags"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|flag| flag == "-d, --daemon")
        }));
        assert!(data["examples"].as_array().unwrap().len() >= 4);
        assert_eq!(data["log_root"], "/tmp/colosseum/workers");
    }

    #[test]
    fn attached_stream_replays_the_creation_event_first() {
        let temp = tempfile::tempdir().unwrap();
        let worker = WorkerRegistry::new(temp.path())
            .create(Some("workspace-1".into()), None)
            .unwrap();

        let events = worker_events(&worker.log_path).unwrap();
        assert_eq!(events.first().unwrap()["event"], "worker.created");
    }

    #[test]
    fn unsafe_workspace_is_a_configuration_error() {
        let error = validate_workspace_id(Some("../not-a-workspace")).unwrap_err();
        assert_eq!(error_code(&error), EXIT_CONFIGURATION);
    }
}
