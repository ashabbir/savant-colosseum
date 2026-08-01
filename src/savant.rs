use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

mod abilities;
mod response;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Task {
    #[serde(alias = "id")]
    pub task_id: String,
    pub workspace_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub colosseum_ready: bool,
    #[serde(default)]
    pub colosseum_config: serde_json::Value,
}

#[derive(Clone)]
pub struct SavantClient {
    base_url: Url,
    client: Client,
}

impl SavantClient {
    pub fn new(base_url: &str, api_key: Option<&str>) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        // The Savant server authorizes service clients under this application
        // identity together with the persisted user API key.
        headers.insert("X-App-Name", "savant-server".parse()?);
        if let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) {
            headers.insert(
                "X-API-Key",
                api_key.parse().context("invalid SAVANT_API_KEY header")?,
            );
        }
        Ok(Self {
            base_url: Url::parse(base_url)?.join("/")?,
            client: Client::builder()
                .default_headers(headers)
                .timeout(Duration::from_secs(30))
                .build()?,
        })
    }

    pub async fn next_colosseum_task(&self, workspace_id: Option<&str>) -> Result<Option<Task>> {
        let url = self.base_url.join("api/tasks/colosseum/next")?;
        let mut query = vec![];
        if let Some(ws_id) = workspace_id {
            query.push(("workspace_id", ws_id));
        }
        let response = response::ensure_success(
            self.client.get(url).query(&query).send().await?,
            "Savant task list",
        )
        .await?;
        let value: serde_json::Value = response.json().await?;
        if value.get("task_id").is_none() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_value(value)?))
    }

    pub async fn claim(&self, task_id: &str) -> Result<Option<Task>> {
        let url = self.base_url.join(&format!("api/tasks/{task_id}/claim"))?;
        let response = self.client.post(url).send().await?;
        response::optional_claim(response).await
    }

    pub async fn update_task(
        &self,
        task_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        status: Option<&str>,
    ) -> Result<Task> {
        let url = self.base_url.join(&format!("api/tasks/{task_id}"))?;
        let mut payload = serde_json::Map::new();
        if let Some(t) = title {
            payload.insert("title".into(), serde_json::Value::String(t.into()));
        }
        if let Some(d) = description {
            payload.insert("description".into(), serde_json::Value::String(d.into()));
        }
        if let Some(s) = status {
            payload.insert("status".into(), serde_json::Value::String(s.into()));
        }

        let response = self
            .client
            .put(url)
            .json(&serde_json::Value::Object(payload))
            .send()
            .await?;
        let response = response::ensure_success(response, "Savant task update").await?;
        Ok(response.json().await?)
    }

    pub async fn update_status(&self, task_id: &str, status: &str) -> Result<Task> {
        self.update_task(task_id, None, None, Some(status)).await
    }

    pub async fn add_comment(&self, task_id: &str, text: &str, author: &str) -> Result<()> {
        let url = self.base_url.join(&format!("api/tasks/{task_id}/comments"))?;
        let response = self
            .client
            .post(url)
            .json(&serde_json::json!({
                "author": author,
                "text": text,
                "role": "agent"
            }))
            .send()
            .await;
        // Comment posting is best-effort logging
        if let Ok(resp) = response {
            let _ = response::ensure_success(resp, "Savant task comment").await;
        }
        Ok(())
    }
}
