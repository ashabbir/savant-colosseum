use std::path::Path;

use anyhow::{Context, Result};
use tokio::{io::AsyncWriteExt, sync::mpsc, task::JoinHandle};

pub(super) struct EventLog {
    sender: mpsc::UnboundedSender<serde_json::Value>,
    writer: JoinHandle<Result<()>>,
}

impl EventLog {
    pub(super) async fn start(path: &Path) -> Result<Self> {
        tokio::fs::create_dir_all(path.parent().context("log parent")?).await?;
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let path = path.to_owned();
        let writer = tokio::spawn(async move {
            let mut file = tokio::fs::File::create(path).await?;
            while let Some(event) = receiver.recv().await {
                file.write_all(serde_json::to_string(&event)?.as_bytes())
                    .await?;
                file.write_all(b"\n").await?;
            }
            Ok(())
        });
        Ok(Self { sender, writer })
    }

    pub(super) fn record(&self, event: serde_json::Value) {
        self.sender.send(event).ok();
    }

    pub(super) fn sender(&self) -> mpsc::UnboundedSender<serde_json::Value> {
        self.sender.clone()
    }

    pub(super) async fn finish(self) -> Result<()> {
        drop(self.sender);
        self.writer.await??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::EventLog;

    #[tokio::test]
    async fn flushes_records_when_finished() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/run.jsonl");
        let log = EventLog::start(&path).await.unwrap();

        log.record(json!({"type": "started"}));
        log.finish().await.unwrap();

        assert_eq!(
            tokio::fs::read_to_string(path).await.unwrap(),
            "{\"type\":\"started\"}\n"
        );
    }
}
