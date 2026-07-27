use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::{sync::mpsc, time::Instant};
use uuid::Uuid;

use crate::{
    database::{self, ContenderRecord, NewBattle},
    evaluator,
    executor::{LogEvent, run_program, run_shell},
    metrics::{estimate_cost, parse_token_usage},
    scenario::{Contender, Scenario},
    worktree,
};

#[derive(Clone)]
pub struct RunnerConfig {
    pub pool: SqlitePool,
    pub worktree_root: PathBuf,
    pub log_root: PathBuf,
}

pub struct Runner {
    config: RunnerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BattleEvent {
    BattleStarted {
        battle_id: Uuid,
        scenario_id: String,
    },
    Phase {
        battle_id: Uuid,
        contender_id: String,
        phase: String,
    },
    Log {
        battle_id: Uuid,
        contender_id: String,
        phase: String,
        event: LogEvent,
    },
    ContenderFinished {
        battle_id: Uuid,
        contender_id: String,
        status: String,
    },
    BattleFinished {
        battle_id: Uuid,
        status: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleOutcome {
    pub battle_id: Uuid,
    pub scenario_id: String,
    pub status: String,
    pub results: Vec<ContenderRecord>,
    pub errors: Vec<String>,
}

impl Runner {
    pub fn new(config: RunnerConfig) -> Self {
        Self { config }
    }

    pub async fn run(
        &self,
        scenario: Scenario,
        events: Option<mpsc::UnboundedSender<BattleEvent>>,
    ) -> Result<BattleOutcome> {
        scenario.validate()?;
        let battle_id = Uuid::new_v4();
        let started = Instant::now();
        let started_at = chrono::Utc::now().to_rfc3339();
        let commit = worktree::resolve_commit(&scenario.repository, &scenario.start_commit).await?;
        database::create_battle(
            &self.config.pool,
            NewBattle {
                id: battle_id,
                scenario_id: &scenario.scenario_id,
                repository: &scenario.repository.to_string_lossy(),
                start_commit: &commit,
                prompt: &scenario.prompt,
                validation_command: &scenario.validation_command,
                started_at: &started_at,
            },
        )
        .await?;
        send(
            &events,
            BattleEvent::BattleStarted {
                battle_id,
                scenario_id: scenario.scenario_id.clone(),
            },
        );

        let scenario = Arc::new(scenario);
        let futures =
            scenario.contenders.iter().map(|contender| {
                let contender = contender.clone();
                let scenario = scenario.clone();
                let config = self.config.clone();
                let events = events.clone();
                let commit = commit.clone();
                async move {
                    run_contender(config, battle_id, scenario, contender, commit, events).await
                }
            });
        let settled = join_all(futures).await;
        let mut results = Vec::new();
        let mut errors = Vec::new();
        for result in settled {
            match result {
                Ok(result) => results.push(result),
                Err(error) => errors.push(format!("{error:#}")),
            }
        }
        let status = if errors.is_empty() && results.iter().all(|result| result.status == "passed")
        {
            "completed"
        } else {
            "failed"
        };
        database::finish_battle(
            &self.config.pool,
            battle_id,
            status,
            &chrono::Utc::now().to_rfc3339(),
            started.elapsed().as_millis() as u64,
        )
        .await?;
        send(
            &events,
            BattleEvent::BattleFinished {
                battle_id,
                status: status.to_owned(),
            },
        );
        Ok(BattleOutcome {
            battle_id,
            scenario_id: scenario.scenario_id.clone(),
            status: status.to_owned(),
            results,
            errors,
        })
    }
}

async fn run_contender(
    config: RunnerConfig,
    battle_id: Uuid,
    scenario: Arc<Scenario>,
    contender: Contender,
    commit: String,
    battle_events: Option<mpsc::UnboundedSender<BattleEvent>>,
) -> Result<ContenderRecord> {
    phase(&battle_events, battle_id, &contender.id, "provisioning");
    let worktree = worktree::provision(
        &scenario.repository,
        &config.worktree_root,
        &scenario.scenario_id,
        &contender.id,
        &commit,
    )
    .await?;
    let limit = Duration::from_secs(scenario.timeout_seconds);
    let (log_tx, mut log_rx) = mpsc::unbounded_channel();
    let log_file = config
        .log_root
        .join(battle_id.to_string())
        .join(format!("{}.jsonl", contender.id));
    let contender_id = contender.id.clone();
    let event_forwarder = battle_events.clone();
    tokio::fs::create_dir_all(log_file.parent().context("log parent")?).await?;
    let writer = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(log_file).await?;
        while let Some((phase, event)) = log_rx.recv().await {
            file.write_all(serde_json::to_string(&event)?.as_bytes())
                .await?;
            file.write_all(b"\n").await?;
            send(
                &event_forwarder,
                BattleEvent::Log {
                    battle_id,
                    contender_id: contender_id.clone(),
                    phase,
                    event,
                },
            );
        }
        Ok::<_, anyhow::Error>(())
    });

    if let Some(setup) = &scenario.setup_command {
        phase(&battle_events, battle_id, &contender.id, "setup");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let forward = log_tx.clone();
        let task = tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                forward.send(("setup".to_owned(), event)).ok();
            }
        });
        let result = run_shell(setup, &worktree.path, limit, Some(sender)).await?;
        task.await?;
        if result.exit_code != 0 {
            anyhow::bail!("setup failed for {}: {}", contender.id, result.stderr);
        }
    }

    phase(&battle_events, battle_id, &contender.id, "running");
    let (agent_tx, mut agent_rx) = mpsc::unbounded_channel();
    let agent_forward = log_tx.clone();
    let agent_log = tokio::spawn(async move {
        while let Some(event) = agent_rx.recv().await {
            agent_forward.send(("agent".to_owned(), event)).ok();
        }
    });
    let mut env = contender.env.clone();
    env.extend(HashMap::from([
        (
            "SAVANT_COLOSSEUM_BATTLE_ID".to_owned(),
            battle_id.to_string(),
        ),
        (
            "SAVANT_COLOSSEUM_SCENARIO_ID".to_owned(),
            scenario.scenario_id.clone(),
        ),
        (
            "SAVANT_COLOSSEUM_CONTENDER_ID".to_owned(),
            contender.id.clone(),
        ),
    ]));
    let agent = run_program(
        &contender.command,
        &contender.args,
        &worktree.path,
        &env,
        Some(&format!("{}\n", scenario.prompt)),
        limit,
        Some(agent_tx),
    )
    .await?;
    agent_log.await?;

    phase(&battle_events, battle_id, &contender.id, "validating");
    let (validation_tx, mut validation_rx) = mpsc::unbounded_channel();
    let validation_forward = log_tx.clone();
    let validation_log = tokio::spawn(async move {
        while let Some(event) = validation_rx.recv().await {
            validation_forward
                .send(("validation".to_owned(), event))
                .ok();
        }
    });
    let evaluation = evaluator::evaluate(
        &worktree.path,
        &scenario.validation_command,
        limit,
        Some(validation_tx),
    )
    .await?;
    validation_log.await?;
    let git = worktree::outcome(&worktree.path).await?;
    let usage = parse_token_usage(&format!("{}\n{}", agent.stdout, agent.stderr));
    let status = if agent.exit_code == 0 && evaluation.success {
        "passed"
    } else {
        "failed"
    };
    let result = ContenderRecord {
        id: Uuid::new_v4().to_string(),
        battle_id: battle_id.to_string(),
        contender_id: contender.id.clone(),
        contender_label: contender
            .label
            .clone()
            .unwrap_or_else(|| contender.id.clone()),
        branch: worktree.branch.clone(),
        worktree_path: worktree.path.to_string_lossy().to_string(),
        status: status.to_owned(),
        agent_exit_code: agent.exit_code as i64,
        validation_exit_code: evaluation.exit_code as i64,
        duration_ms: agent.duration_ms as i64,
        validation_duration_ms: evaluation.duration_ms as i64,
        input_tokens: usage.input_tokens as i64,
        output_tokens: usage.output_tokens as i64,
        cached_tokens: usage.cached_tokens as i64,
        estimated_cost: estimate_cost(&usage, &contender.rates),
        tests_run: evaluation.tests_run.map(|value| value as i64),
        tests_passed: evaluation.tests_passed.map(|value| value as i64),
        tests_failed: evaluation.tests_failed.map(|value| value as i64),
        lint_violations: evaluation.lint_violations as i64,
        changed_files: git.changed_files as i64,
        stdout: agent.stdout,
        stderr: agent.stderr,
        validation_stdout: evaluation.stdout,
        validation_stderr: evaluation.stderr,
        git_status: git.status,
        diff_stat: git.diff_stat,
    };
    database::insert_result(&config.pool, &result).await?;
    drop(log_tx);
    writer.await??;
    send(
        &battle_events,
        BattleEvent::ContenderFinished {
            battle_id,
            contender_id: contender.id,
            status: status.to_owned(),
        },
    );
    if !scenario.retain_worktrees {
        worktree::cleanup(&scenario.repository, &worktree).await?;
    }
    Ok(result)
}

fn phase(
    events: &Option<mpsc::UnboundedSender<BattleEvent>>,
    battle_id: Uuid,
    contender_id: &str,
    phase: &str,
) {
    send(
        events,
        BattleEvent::Phase {
            battle_id,
            contender_id: contender_id.to_owned(),
            phase: phase.to_owned(),
        },
    );
}

fn send(events: &Option<mpsc::UnboundedSender<BattleEvent>>, event: BattleEvent) {
    if let Some(events) = events {
        events.send(event).ok();
    }
}
