use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ulid::Ulid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkerStatus {
    Running,
    Stopped,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRecord {
    pub worker_id: String,
    pub workspace_id: Option<String>,
    pub status: WorkerStatus,
    pub pid: Option<u32>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub log_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct WorkerRegistry {
    root: PathBuf,
}

impl WorkerRegistry {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: data_dir.into().join("workers"),
        }
    }
    fn registry_path(&self) -> PathBuf {
        self.root.join("registry.json")
    }
    fn read(&self) -> Result<Vec<WorkerRecord>> {
        let path = self.registry_path();
        if !path.exists() {
            return Ok(vec![]);
        }
        serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("read {}", path.display()))?,
        )
        .with_context(|| format!("parse {}", path.display()))
    }
    fn write(&self, workers: &[WorkerRecord]) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        let temp = self.root.join("registry.json.tmp");
        fs::write(&temp, serde_json::to_vec_pretty(workers)?)?;
        fs::rename(temp, self.registry_path())?;
        Ok(())
    }
    pub fn create(&self, workspace_id: Option<String>, pid: Option<u32>) -> Result<WorkerRecord> {
        let worker_id = Ulid::new().to_string();
        if let Some(workspace) = workspace_id.as_deref()
            && (workspace.is_empty()
                || workspace == "."
                || workspace == ".."
                || workspace.contains(['/', '\\']))
        {
            bail!("workspace ID is not safe for a worker log path");
        }
        let scope = workspace_id
            .clone()
            .unwrap_or_else(|| "all-workspaces".into());
        let log_path = self.root.join(&scope).join(&worker_id).join("events.jsonl");
        let record = WorkerRecord {
            worker_id,
            workspace_id,
            status: WorkerStatus::Running,
            pid,
            started_at: Utc::now().to_rfc3339(),
            finished_at: None,
            log_path,
        };
        let mut workers = self.read()?;
        workers.push(record.clone());
        self.write(&workers)?;
        self.event(
            &record,
            "worker.created",
            "running",
            "worker created",
            None,
            None,
        )?;
        Ok(record)
    }
    pub fn all(&self) -> Result<Vec<WorkerRecord>> {
        self.read()
    }
    pub fn get(&self, worker_id: &str) -> Result<WorkerRecord> {
        self.read()?
            .into_iter()
            .find(|worker| worker.worker_id == worker_id)
            .ok_or_else(|| anyhow::anyhow!("worker {worker_id} was not found"))
    }
    pub fn active_for_workspace(&self, workspace_id: Option<&str>) -> Result<Option<WorkerRecord>> {
        Ok(self.read()?.into_iter().find(|worker| {
            worker.status == WorkerStatus::Running && worker.workspace_id.as_deref() == workspace_id
        }))
    }
    pub fn update(
        &self,
        worker_id: &str,
        status: WorkerStatus,
        pid: Option<u32>,
    ) -> Result<WorkerRecord> {
        let mut workers = self.read()?;
        let worker = workers
            .iter_mut()
            .find(|worker| worker.worker_id == worker_id)
            .ok_or_else(|| anyhow::anyhow!("worker {worker_id} was not found"))?;
        worker.status = status;
        worker.pid = pid;
        if worker.status != WorkerStatus::Running {
            worker.finished_at = Some(Utc::now().to_rfc3339());
        }
        let result = worker.clone();
        self.write(&workers)?;
        Ok(result)
    }
    pub fn event(
        &self,
        worker: &WorkerRecord,
        event: &str,
        status: &str,
        message: &str,
        data: Option<Value>,
        error: Option<Value>,
    ) -> Result<Value> {
        let value = json!({"timestamp": Utc::now().to_rfc3339(), "event": event, "worker_id": worker.worker_id, "workspace_id": worker.workspace_id, "status": status, "message": message, "data": data, "error": error});
        let path = &worker.log_path;
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid worker log path"))?;
        fs::create_dir_all(parent)?;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        writeln!(file, "{}", serde_json::to_string(&value)?)?;
        Ok(value)
    }
    pub fn stop(&self, worker_id: &str) -> Result<WorkerRecord> {
        let worker = self.get(worker_id)?;
        if worker.status != WorkerStatus::Running {
            bail!("worker {worker_id} is already {:?}", worker.status);
        }
        if let Some(pid) = worker.pid {
            let status = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
            if status.map(|s| !s.success()).unwrap_or(true) {
                return self.update(worker_id, WorkerStatus::Stopped, None);
            }
        }
        Ok(worker)
    }
    pub fn log_exists(&self, worker: &WorkerRecord) -> bool {
        worker.log_path.is_file()
    }
}

pub fn read_log(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("read worker log {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_worker_and_jsonl_event() {
        let temp = tempfile::tempdir().unwrap();
        let registry = WorkerRegistry::new(temp.path());
        let worker = registry
            .create(Some("workspace-1".into()), Some(42))
            .unwrap();
        assert_eq!(worker.worker_id.len(), 26);
        registry
            .event(&worker, "worker.started", "running", "started", None, None)
            .unwrap();
        assert_eq!(registry.all().unwrap().len(), 1);
        let log = read_log(&worker.log_path).unwrap();
        assert!(
            log.lines()
                .all(|line| serde_json::from_str::<Value>(line).is_ok())
        );
    }

    #[test]
    fn rejects_workspace_path_traversal() {
        let temp = tempfile::tempdir().unwrap();
        assert!(
            WorkerRegistry::new(temp.path())
                .create(Some("../outside".into()), None)
                .is_err()
        );
    }
}
