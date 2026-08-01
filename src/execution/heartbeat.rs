use std::{path::Path, time::Duration};

use chrono::{DateTime, Utc};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
    time::{Instant, MissedTickBehavior},
};
use uuid::Uuid;

use crate::savant::{SavantClient, Task};

use super::{ExecutionPhase, ExecutionSpec, setup::phase_execution_config};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const COMMENT_INTERVAL: Duration = Duration::from_secs(60);
const INITIAL_HEARTBEAT_WAIT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct HeartbeatState {
    status: &'static str,
    message: String,
    stop: bool,
}

pub(super) struct Heartbeat {
    task_id: String,
    savant: SavantClient,
    state: watch::Sender<HeartbeatState>,
    worker: Option<JoinHandle<()>>,
}

impl Heartbeat {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn start(
        savant: &SavantClient,
        task: &Task,
        spec: &ExecutionSpec,
        phase: ExecutionPhase,
        run_id: Uuid,
        log_path: &Path,
        worktree_path: &Path,
        events: mpsc::UnboundedSender<serde_json::Value>,
    ) -> Self {
        let task_id = task.task_id.clone();
        let workspace_id = task.workspace_id.clone();
        let phase_name = phase_name(phase).to_owned();
        let execution = phase_execution_config(task, &spec.provider, phase);
        let persona = execution.persona;
        let provider = execution.provider;
        let model = execution.model;
        let run_id = run_id.to_string();
        let log_path = log_path.display().to_string();
        let worktree_path = worktree_path.display().to_string();
        let started_at = Utc::now();
        let initial = HeartbeatState {
            status: "starting",
            message: format!("Preparing {phase_name} execution."),
            stop: false,
        };
        let (state, mut receiver) = watch::channel(initial);
        let (initial_sent, initial_received) = oneshot::channel();
        let heartbeat_savant = savant.clone();
        let heartbeat_task_id = task_id.clone();
        let worker = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(HEARTBEAT_INTERVAL);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            let mut sequence = 0_u64;
            let mut last_comment = Instant::now();
            let mut initial_sent = Some(initial_sent);

            loop {
                let current = tokio::select! {
                    _ = ticker.tick() => receiver.borrow().clone(),
                    changed = receiver.changed() => {
                        if changed.is_err() { break; }
                        receiver.borrow().clone()
                    }
                };
                sequence += 1;
                let heartbeat_at = Utc::now();
                let active_run = heartbeat_payload(
                    &run_id,
                    &heartbeat_task_id,
                    &workspace_id,
                    &phase_name,
                    &persona,
                    &provider,
                    model.as_deref(),
                    started_at,
                    heartbeat_at,
                    sequence,
                    &current,
                    &log_path,
                    &worktree_path,
                );
                events
                    .send(serde_json::json!({
                        "type": "heartbeat",
                        "active_run": active_run,
                    }))
                    .ok();
                let _ = heartbeat_savant
                    .update_colosseum_metadata(
                        &heartbeat_task_id,
                        &serde_json::json!({"active_run": active_run}),
                    )
                    .await;
                if let Some(sent) = initial_sent.take() {
                    let _ = sent.send(());
                }

                if !current.stop && last_comment.elapsed() >= COMMENT_INTERVAL {
                    let elapsed = heartbeat_at.signed_duration_since(started_at).num_seconds();
                    let comment = format!(
                        "💓 **Colosseum Active**: {} is still {}. {} · elapsed {}",
                        persona.trim_start_matches("persona."),
                        phase_name,
                        current.message,
                        format_elapsed(elapsed),
                    );
                    let _ = heartbeat_savant
                        .add_comment(&heartbeat_task_id, &comment, "Colosseum")
                        .await;
                    last_comment = Instant::now();
                }

                if current.stop {
                    break;
                }
            }
        });

        let heartbeat = Self {
            task_id,
            savant: savant.clone(),
            state,
            worker: Some(worker),
        };
        // Give the first heartbeat a bounded chance to reach Sanctum before
        // setup or ability resolution can fail. Observability never blocks
        // execution for longer than this small startup budget.
        let _ = tokio::time::timeout(INITIAL_HEARTBEAT_WAIT, initial_received).await;
        heartbeat
    }

    pub(super) fn update(&self, status: &'static str, message: impl Into<String>) {
        self.state.send_modify(|current| {
            current.status = status;
            current.message = message.into();
        });
    }

    pub(super) async fn finish(mut self, status: &'static str, message: impl Into<String>) {
        let message = message.into();
        self.state.send_modify(|current| {
            current.status = status;
            current.message = message.clone();
            current.stop = true;
        });
        if let Some(worker) = self.worker.take() {
            let _ = worker.await;
        }
        // Await the writer before clearing so no late heartbeat can overwrite
        // final run metadata written by the lifecycle.
        let _ = self
            .savant
            .update_colosseum_metadata(&self.task_id, &serde_json::json!({"active_run": null}))
            .await;
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.abort();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn heartbeat_payload(
    run_id: &str,
    task_id: &str,
    workspace_id: &str,
    phase: &str,
    persona: &str,
    provider: &str,
    model: Option<&str>,
    started_at: DateTime<Utc>,
    heartbeat_at: DateTime<Utc>,
    sequence: u64,
    state: &HeartbeatState,
    log_path: &str,
    worktree_path: &str,
) -> serde_json::Value {
    serde_json::json!({
        "run_id": run_id,
        "task_id": task_id,
        "workspace_id": workspace_id,
        "phase": phase,
        "status": state.status,
        "persona": persona,
        "provider": provider,
        "model": model,
        "started_at": started_at.to_rfc3339(),
        "heartbeat_at": heartbeat_at.to_rfc3339(),
        "message": state.message,
        "sequence": sequence,
        "log_path": log_path,
        "worktree_path": worktree_path,
    })
}

fn phase_name(phase: ExecutionPhase) -> &'static str {
    match phase {
        ExecutionPhase::Grooming => "grooming",
        ExecutionPhase::Work => "work",
        ExecutionPhase::Review => "review",
        ExecutionPhase::Merge => "merge",
    }
}

fn format_elapsed(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!("{}m {:02}s", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::{HeartbeatState, heartbeat_payload};
    use chrono::{TimeZone, Utc};

    #[test]
    fn payload_has_the_stable_liveness_contract() {
        let time = Utc.with_ymd_and_hms(2026, 8, 1, 20, 0, 0).unwrap();
        let payload = heartbeat_payload(
            "run-1",
            "task-1",
            "ws-1",
            "work",
            "persona.coder",
            "codex",
            Some("gpt-5.6"),
            time,
            time,
            3,
            &HeartbeatState {
                status: "running",
                message: "Coding".into(),
                stop: false,
            },
            "run.jsonl",
            "/tmp/worktree",
        );
        assert_eq!(payload["sequence"], 3);
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["persona"], "persona.coder");
        assert_eq!(payload["message"], "Coding");
    }
}
