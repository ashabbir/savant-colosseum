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
    pub colosseum_claimed_from: Option<String>,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub colosseum_ready: bool,
    #[serde(default)]
    pub colosseum_config: serde_json::Value,
    #[serde(default)]
    pub comments: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Workspace {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
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

    pub async fn set_colosseum_ready(&self, task_id: &str, ready: bool) -> Result<Task> {
        let url = self
            .base_url
            .join(&format!("api/tasks/{task_id}/colosseum-ready-state"))?;
        let response = self
            .client
            .post(url)
            .json(&serde_json::json!({"ready": ready}))
            .send()
            .await?;
        let response = response::ensure_success(response, "Colosseum queue update").await?;
        Ok(response.json().await?)
    }

    pub async fn update_colosseum_metadata(
        &self,
        task_id: &str,
        metadata: &serde_json::Value,
    ) -> Result<Task> {
        let url = self
            .base_url
            .join(&format!("api/tasks/{task_id}/colosseum-metadata"))?;
        let response = self.client.put(url).json(metadata).send().await?;
        let response = response::ensure_success(response, "Colosseum metadata update").await?;
        Ok(response.json().await?)
    }

    pub async fn append_colosseum_run(
        &self,
        task_id: &str,
        run: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = self
            .base_url
            .join(&format!("api/tasks/{task_id}/colosseum-runs"))?;
        let response = self.client.post(url).json(run).send().await?;
        let response = response::ensure_success(response, "Colosseum run evidence").await?;
        Ok(response.json().await?)
    }

    pub async fn create_merge_request(
        &self,
        task: &Task,
        mr_id: &str,
        remote: &str,
        branch: &str,
    ) -> Result<serde_json::Value> {
        let url = self.base_url.join("api/merge-requests")?;
        let response = self
            .client
            .post(url)
            .json(&serde_json::json!({
                "mr_id": mr_id,
                "workspace_id": task.workspace_id,
                "title": format!("{} [{}]", task.title, branch),
                "url": remote,
                "status": "review",
                "author": "Colosseum",
            }))
            .send()
            .await?;
        let response = response::ensure_success(response, "Savant merge request creation").await?;
        Ok(response.json().await?)
    }

    pub async fn update_merge_request_status(&self, mr_id: &str, status: &str) -> Result<()> {
        let url = self.base_url.join(&format!("api/merge-requests/{mr_id}"))?;
        let response = self
            .client
            .put(url)
            .json(&serde_json::json!({"status": status}))
            .send()
            .await?;
        response::ensure_success(response, "Savant merge request update").await?;
        Ok(())
    }

    pub async fn add_comment(&self, task_id: &str, text: &str, author: &str) -> Result<()> {
        let url = self
            .base_url
            .join(&format!("api/tasks/{task_id}/comments"))?;
        let response = self
            .client
            .post(url)
            .json(&serde_json::json!({
                "author": author,
                "text": text,
                "role": "agent"
            }))
            .send()
            .await?;
        response::ensure_success(response, "Savant task comment").await?;
        Ok(())
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        let url = self.base_url.join("api/workspaces")?;
        let response = match self.client.get(url).send().await {
            Ok(res) => res,
            Err(_) => return Ok(vec![]),
        };
        if !response.status().is_success() {
            return Ok(vec![]);
        }
        let val: serde_json::Value = response.json().await?;
        if let Ok(list) = serde_json::from_value::<Vec<Workspace>>(val.clone()) {
            return Ok(list);
        }
        if let Some(arr) = val.get("workspaces")
            && let Ok(list) = serde_json::from_value::<Vec<Workspace>>(arr.clone())
        {
            return Ok(list);
        }
        Ok(vec![])
    }

    pub async fn list_tasks(&self, workspace_id: Option<&str>) -> Result<Vec<Task>> {
        let url = self.base_url.join("api/tasks")?;
        let mut query = vec![];
        if let Some(ws_id) = workspace_id {
            query.push(("workspace_id", ws_id));
        }
        let response = match self.client.get(url).query(&query).send().await {
            Ok(res) => res,
            Err(_) => return Ok(vec![]),
        };
        if !response.status().is_success() {
            return Ok(vec![]);
        }
        let val: serde_json::Value = response.json().await?;
        if let Ok(list) = serde_json::from_value::<Vec<Task>>(val.clone()) {
            return Ok(list);
        }
        if let Some(arr) = val.get("tasks")
            && let Ok(list) = serde_json::from_value::<Vec<Task>>(arr.clone())
        {
            return Ok(list);
        }
        Ok(vec![])
    }
}
