use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::SavantClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerAbilityAsset {
    pub id: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub name: Option<String>,
    pub path: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub includes: Option<Vec<String>>,
}

const ENGINEER_PERSONA: &str = "persona.engineer";
const ENGINEER_TAGS: &[&str] = &["engineering", "execution", "code-review"];

const PRODUCT_PERSONA: &str = "persona.product";
const PRODUCT_TAGS: &[&str] = &["product", "requirements", "grooming"];

impl SavantClient {
    pub async fn resolve_abilities(
        &self,
        repo_id: &str,
        persona: &str,
        tags: &[&str],
    ) -> Result<serde_json::Value> {
        let url = self.base_url.join("api/abilities/resolve")?;
        let response = self
            .client
            .post(url)
            .json(&serde_json::json!({
                "persona": persona,
                "tags": tags,
                "repo_id": repo_id,
                "trace": true,
            }))
            .send()
            .await?;
        if !response.status().is_success() {
            bail!(
                "Savant ability resolution failed ({}): {}",
                response.status(),
                response.text().await?
            );
        }
        Ok(response.json().await?)
    }

    pub async fn resolve_engineer_abilities(&self, repo_id: &str) -> Result<serde_json::Value> {
        self.resolve_abilities(repo_id, ENGINEER_PERSONA, ENGINEER_TAGS)
            .await
    }

    pub async fn resolve_product_abilities(&self, repo_id: &str) -> Result<serde_json::Value> {
        self.resolve_abilities(repo_id, PRODUCT_PERSONA, PRODUCT_TAGS)
            .await
    }

    pub async fn list_abilities(&self) -> Result<Vec<ServerAbilityAsset>> {
        let url = self.base_url.join("api/abilities/assets")?;
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            bail!("Failed to fetch abilities assets: {}", response.status());
        }
        let val: serde_json::Value = response.json().await?;
        let mut list = Vec::new();
        if let Some(obj) = val.as_object() {
            for (_key, val_arr) in obj {
                if let Ok(assets) = serde_json::from_value::<Vec<ServerAbilityAsset>>(val_arr.clone()) {
                    list.extend(assets);
                }
            }
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::{ENGINEER_PERSONA, ENGINEER_TAGS};

    #[test]
    fn engineer_ability_contract_uses_the_expected_persona_and_tags() {
        assert_eq!(ENGINEER_PERSONA, "persona.engineer");
        assert_eq!(ENGINEER_TAGS, ["engineering", "execution", "code-review"]);
    }
}
