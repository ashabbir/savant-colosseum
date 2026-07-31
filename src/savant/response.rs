use anyhow::{Result, bail};
use reqwest::{Response, StatusCode};

use super::Task;

pub(super) async fn ensure_success(response: Response, action: &str) -> Result<Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    bail!("{action} failed ({status}): {}", response.text().await?)
}

pub(super) async fn optional_claim(response: Response) -> Result<Option<Task>> {
    if response.status() == StatusCode::CONFLICT {
        return Ok(None);
    }
    Ok(Some(
        ensure_success(response, "Savant task claim")
            .await?
            .json()
            .await?,
    ))
}
