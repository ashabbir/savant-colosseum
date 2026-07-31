use anyhow::{Result, bail};

use super::SavantClient;

const ENGINEER_PERSONA: &str = "persona.engineer";
const ENGINEER_TAGS: &[&str] = &["engineering", "execution", "code-review"];

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
