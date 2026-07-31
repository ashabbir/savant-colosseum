use std::{collections::HashMap, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::mpsc};
use uuid::Uuid;

use crate::{
    executor::{self, LogEvent, ProcessOutcome},
    savant::{SavantClient, Task},
    worktree,
};

mod policy;

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionSpec {
    pub repository: PathBuf,
    pub provider: String,
    #[serde(default = "default_revision")]
    pub revision: String,
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub push: bool,
}

fn default_revision() -> String {
    "HEAD".to_owned()
}
fn default_timeout() -> u64 {
    3600
}

impl ExecutionSpec {
    pub fn from_task(task: &Task) -> Result<Self> {
        if !task.colosseum_ready {
            anyhow::bail!("task is not ready for Colosseum");
        }
        serde_json::from_value(task.colosseum_config.clone())
            .context("invalid Colosseum task config")
    }
}

#[derive(Clone)]
pub struct RunnerConfig {
    pub worktree_root: PathBuf,
    pub log_root: PathBuf,
}
pub struct ExecutionRunner {
    savant: SavantClient,
    config: RunnerConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionOutcome {
    pub run_id: Uuid,
    pub task_id: String,
    pub status: String,
    pub worktree: PathBuf,
    pub log_file: PathBuf,
    pub agent: ProcessOutcome,
    pub validation: Option<ProcessOutcome>,
}

impl ExecutionRunner {
    pub fn new(savant: SavantClient, config: RunnerConfig) -> Self {
        Self { savant, config }
    }

    pub async fn claim_next(&self, workspace_id: Option<&str>) -> Result<Option<Task>> {
        let Some(task) = self.savant.next_colosseum_task(workspace_id).await? else {
            return Ok(None);
        };
        self.savant.claim(&task.task_id).await
    }

    pub async fn execute_task(&self, task: Task) -> Result<ExecutionOutcome> {
        let spec = ExecutionSpec::from_task(&task)?;
        let run_id = Uuid::new_v4();
        let worktree = worktree::provision_task(
            &spec.repository,
            &self.config.worktree_root,
            &task.task_id,
            &spec.revision,
        )
        .await?;
        let log_file = self
            .config
            .log_root
            .join(&task.task_id)
            .join(format!("{run_id}.jsonl"));
        tokio::fs::create_dir_all(log_file.parent().context("log parent")?).await?;
        let (events, mut receiver) = mpsc::unbounded_channel::<serde_json::Value>();
        let writer_path = log_file.clone();
        let writer = tokio::spawn(async move {
            let mut file = tokio::fs::File::create(writer_path).await?;
            while let Some(event) = receiver.recv().await {
                file.write_all(serde_json::to_string(&event)?.as_bytes())
                    .await?;
                file.write_all(b"\n").await?;
            }
            Ok::<_, anyhow::Error>(())
        });
        let limit = Duration::from_secs(spec.timeout_seconds);
        events.send(serde_json::json!({"type":"started","run_id":run_id,"task_id":task.task_id,"worktree":worktree.path})).ok();
        let repo_id = spec
            .repository
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let abilities = self.savant.resolve_engineer_abilities(repo_id).await?;
        let ability_prompt = abilities
            .get("prompt")
            .and_then(|value| value.as_str())
            .context("Savant engineer ability prompt missing")?;
        events.send(serde_json::json!({"type":"abilities-resolved","persona":"persona.engineer","manifest":abilities.get("manifest")})).ok();
        if let Some(setup) = spec.setup.as_deref() {
            let setup_outcome = self
                .run_shell_step("setup", setup, &worktree.path, limit, events.clone())
                .await?;
            if setup_outcome.exit_code != 0 || setup_outcome.timed_out {
                events.send(serde_json::json!({"type":"finished","status":"blocked","setup_exit_code":setup_outcome.exit_code})).ok();
                drop(events);
                writer.await??;
                self.savant.update_status(&task.task_id, "blocked").await?;
                return Ok(ExecutionOutcome {
                    run_id,
                    task_id: task.task_id,
                    status: "blocked".to_owned(),
                    worktree: worktree.path,
                    log_file,
                    agent: setup_outcome,
                    validation: None,
                });
            }
        }
        let prompt = format!(
            "{ability_prompt}\n\n# Colosseum execution contract\nYou have full permission to inspect, edit, and run commands in this worktree. Work on Savant task {}: {}\n\n{}\n\nRun the relevant validation and fix failures you introduce. Leave changes in this worktree; Colosseum will independently verify, commit, push, and post review metadata.",
            task.task_id, task.title, task.description
        );
        let (program, args) = policy::provider_command(&spec.provider)?;
        let agent = self
            .run_step(
                "agent",
                program,
                &args,
                &worktree.path,
                Some(&prompt),
                limit,
                events.clone(),
            )
            .await?;
        let validation = if agent.exit_code == 0 && !agent.timed_out {
            let diff_check = self
                .run_shell_step(
                    "validation",
                    "git diff --check",
                    &worktree.path,
                    limit,
                    events.clone(),
                )
                .await?;
            if diff_check.exit_code != 0 || diff_check.timed_out {
                Some(diff_check)
            } else {
                Some(
                    self.run_shell_step(
                        "project-validation",
                        policy::verification_command(&worktree.path),
                        &worktree.path,
                        limit,
                        events.clone(),
                    )
                    .await?,
                )
            }
        } else {
            None
        };
        let verified = agent.exit_code == 0
            && !agent.timed_out
            && validation
                .as_ref()
                .is_none_or(|result| result.exit_code == 0 && !result.timed_out);
        let publication = if verified {
            match worktree::commit_and_push(
                &worktree.path,
                &worktree.branch,
                &format!("colosseum: {}", task.title),
            )
            .await
            {
                Ok((commit, remote)) => {
                    let review_note = format!(
                        "Colosseum execution verified.\n\n- Worktree: `{}`\n- Branch: `{}`\n- Commit: `{}`\n- Remote: `{}`\n- Log: `{}`\n- Validation: passed",
                        worktree.path.display(),
                        worktree.branch,
                        commit,
                        remote,
                        log_file.display()
                    );
                    worktree::create_or_comment_github_review(
                        &worktree.path,
                        &worktree.branch,
                        &task.title,
                        &review_note,
                    )
                    .await
                    .ok()
                    .map(|review| (commit, remote, review))
                }
                Err(_) => None,
            }
        } else {
            None
        };
        let status = if publication.is_some() {
            "code-review"
        } else {
            "blocked"
        };
        events.send(serde_json::json!({"type":"finished","status":status,"branch":worktree.branch,"commit":publication.as_ref().map(|item| &item.0),"remote":publication.as_ref().map(|item| &item.1),"review":publication.as_ref().map(|item| &item.2),"agent_exit_code":agent.exit_code,"validation_exit_code":validation.as_ref().map(|value| value.exit_code)})).ok();
        drop(events);
        writer.await??;
        self.savant.update_status(&task.task_id, status).await?;
        Ok(ExecutionOutcome {
            run_id,
            task_id: task.task_id,
            status: status.to_owned(),
            worktree: worktree.path,
            log_file,
            agent,
            validation,
        })
    }

    pub async fn run_next(&self, workspace_id: Option<&str>) -> Result<Option<ExecutionOutcome>> {
        match self.claim_next(workspace_id).await? {
            Some(task) => match self.execute_task(task.clone()).await {
                Ok(outcome) => Ok(Some(outcome)),
                Err(error) => {
                    // A claimed task must never be stranded in progress if
                    // provisioning, agent launch, or log setup fails early.
                    self.savant.update_status(&task.task_id, "blocked").await?;
                    Err(error.context(format!("execution for task {} was blocked", task.task_id)))
                }
            },
            None => Ok(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_step(
        &self,
        phase: &str,
        program: &str,
        args: &[String],
        cwd: &std::path::Path,
        stdin: Option<&str>,
        limit: Duration,
        sink: mpsc::UnboundedSender<serde_json::Value>,
    ) -> Result<ProcessOutcome> {
        let (tx, mut rx) = mpsc::unbounded_channel::<LogEvent>();
        let phase = phase.to_owned();
        let forward = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                sink.send(serde_json::json!({"type":"log","phase":phase,"event":event}))
                    .ok();
            }
        });
        let outcome =
            executor::run_pty_program(program, args, cwd, &HashMap::new(), stdin, limit, Some(tx))
                .await?;
        forward.await?;
        Ok(outcome)
    }

    async fn run_shell_step(
        &self,
        phase: &str,
        command: &str,
        cwd: &std::path::Path,
        limit: Duration,
        sink: mpsc::UnboundedSender<serde_json::Value>,
    ) -> Result<ProcessOutcome> {
        let (tx, mut rx) = mpsc::unbounded_channel::<LogEvent>();
        let phase = phase.to_owned();
        let forward = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                sink.send(serde_json::json!({"type":"log","phase":phase,"event":event}))
                    .ok();
            }
        });
        let outcome = executor::run_shell(command, cwd, limit, Some(tx)).await?;
        forward.await?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::ExecutionSpec;
    use crate::savant::Task;
    #[test]
    fn parses_ready_workspace_config() {
        let task = Task {
            task_id: "task".into(),
            workspace_id: "ws".into(),
            title: "task".into(),
            description: "".into(),
            status: "todo".into(),
            priority: "medium".into(),
            depends_on: vec![],
            colosseum_ready: true,
            colosseum_config: serde_json::json!({"repository":"/tmp/repo","provider":"codex"}),
        };
        let parsed = ExecutionSpec::from_task(&task).unwrap();
        assert_eq!(parsed.provider, "codex");
    }
}
