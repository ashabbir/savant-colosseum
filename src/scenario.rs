use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub scenario_id: String,
    pub repository: PathBuf,
    pub start_commit: String,
    pub prompt: String,
    pub validation_command: String,
    #[serde(default)]
    pub setup_command: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_retain")]
    pub retain_worktrees: bool,
    pub contenders: Vec<Contender>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contender {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub rates: TokenRates,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenRates {
    #[serde(default)]
    pub input_per_million: f64,
    #[serde(default)]
    pub output_per_million: f64,
}

fn default_timeout() -> u64 {
    1_800
}

fn default_retain() -> bool {
    true
}

fn validate_id(value: &str, field: &str) -> Result<()> {
    if value.len() < 2
        || value.len() > 63
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !value.as_bytes()[0].is_ascii_alphanumeric()
    {
        bail!("{field} must use 2-63 lowercase letters, numbers, or hyphens");
    }
    Ok(())
}

impl Scenario {
    pub async fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("read scenario {}", path.display()))?;
        let mut scenario: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse scenario {}", path.display()))?;
        scenario.repository = tokio::fs::canonicalize(&scenario.repository)
            .await
            .with_context(|| format!("resolve repository {}", scenario.repository.display()))?;
        scenario.validate()?;
        Ok(scenario)
    }

    pub fn validate(&self) -> Result<()> {
        validate_id(&self.scenario_id, "scenario_id")?;
        if self.start_commit.trim().is_empty()
            || self.prompt.trim().is_empty()
            || self.validation_command.trim().is_empty()
        {
            bail!("start_commit, prompt, and validation_command are required");
        }
        if !(1..=7_200).contains(&self.timeout_seconds) {
            bail!("timeout_seconds must be between 1 and 7200");
        }
        if self.contenders.len() < 2 {
            bail!("at least two contenders are required");
        }
        let mut ids = HashSet::new();
        for contender in &self.contenders {
            validate_id(&contender.id, "contender id")?;
            if !ids.insert(&contender.id) {
                bail!("duplicate contender id: {}", contender.id);
            }
            if contender.command.trim().is_empty() {
                bail!("contender {} command is required", contender.id);
            }
        }
        if !self.repository.join(".git").exists() {
            bail!(
                "repository is not a Git checkout: {}",
                self.repository.display()
            );
        }
        Ok(())
    }
}
