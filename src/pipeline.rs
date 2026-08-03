use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub persona: String,
    pub tag: String,
    pub provider: String,
    pub model: String,
    pub pickup_location: String,
    #[serde(default)]
    pub working_location: String,
    pub drop_location: String,
}

impl AgentConfig {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        prompt: impl Into<String>,
        persona: impl Into<String>,
        tag: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        pickup_location: impl Into<String>,
        working_location: impl Into<String>,
        drop_location: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            prompt: prompt.into(),
            persona: persona.into(),
            tag: tag.into(),
            provider: provider.into(),
            model: model.into(),
            pickup_location: pickup_location.into(),
            working_location: working_location.into(),
            drop_location: drop_location.into(),
        }
    }

    pub fn get_working_location(&self) -> &str {
        if !self.working_location.trim().is_empty() {
            &self.working_location
        } else if self.pickup_location.to_lowercase() == "backlog" {
            "grooming"
        } else {
            "in-progress"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ColosseumRegistry {
    pub agents: HashMap<String, AgentConfig>,
    pub pipelines: HashMap<String, Pipeline>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PipelineValidationError {
    #[error("Agent '{0}' referenced in pipeline was not found in the agent library")]
    AgentNotFound(String),

    #[error("Duplicate pickup location '{pickup}' in pipeline '{pipeline_name}': agents '{agent_a}' and '{agent_b}' share the same pickup location")]
    DuplicatePickupLocation {
        pipeline_name: String,
        pickup: String,
        agent_a: String,
        agent_b: String,
    },
}

impl ColosseumRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_agent(&mut self, agent: AgentConfig) {
        self.agents.insert(agent.id.clone(), agent);
    }

    pub fn pipeline_count_for_agent(&self, agent_id: &str) -> usize {
        self.pipelines
            .values()
            .filter(|pipeline| pipeline.agent_ids.iter().any(|id| id == agent_id))
            .count()
    }

    pub fn pipelines_using_agent(&self, agent_id: &str) -> Vec<&Pipeline> {
        self.pipelines
            .values()
            .filter(|pipeline| pipeline.agent_ids.iter().any(|id| id == agent_id))
            .collect()
    }

    pub fn update_agent(&mut self, agent: AgentConfig) -> Result<(), String> {
        let count = self.pipeline_count_for_agent(&agent.id);
        if count > 0 {
            return Err(format!(
                "Agent '{}' is attached to {} pipeline(s). Attached agents cannot be edited directly; clone the agent instead.",
                agent.name, count
            ));
        }
        self.agents.insert(agent.id.clone(), agent);
        Ok(())
    }

    pub fn delete_agent(&mut self, agent_id: &str) -> Result<(), String> {
        let count = self.pipeline_count_for_agent(agent_id);
        if count > 0 {
            let name = self.agents.get(agent_id).map(|a| a.name.as_str()).unwrap_or(agent_id);
            return Err(format!(
                "Agent '{}' is attached to {} pipeline(s). Attached agents cannot be deleted.",
                name, count
            ));
        }
        self.agents.remove(agent_id);
        Ok(())
    }

    pub fn clone_agent(
        &mut self,
        source_id: &str,
        new_id: impl Into<String>,
        new_name: impl Into<String>,
    ) -> Result<AgentConfig, String> {
        let source = self
            .agents
            .get(source_id)
            .ok_or_else(|| format!("Source agent '{}' not found", source_id))?;

        let cloned = AgentConfig {
            id: new_id.into(),
            name: new_name.into(),
            prompt: source.prompt.clone(),
            persona: source.persona.clone(),
            tag: source.tag.clone(),
            provider: source.provider.clone(),
            model: source.model.clone(),
            pickup_location: source.pickup_location.clone(),
            working_location: source.working_location.clone(),
            drop_location: source.drop_location.clone(),
        };

        self.agents.insert(cloned.id.clone(), cloned.clone());
        Ok(cloned)
    }

    pub fn validate_pipeline(&self, pipeline: &Pipeline) -> Result<(), PipelineValidationError> {
        let mut seen_pickups: HashMap<String, String> = HashMap::new();

        for agent_id in &pipeline.agent_ids {
            let agent = self
                .agents
                .get(agent_id)
                .ok_or_else(|| PipelineValidationError::AgentNotFound(agent_id.clone()))?;

            let pickup = agent.pickup_location.trim();
            if pickup.is_empty() {
                continue;
            }

            if let Some(existing_agent_name) = seen_pickups.get(pickup) {
                return Err(PipelineValidationError::DuplicatePickupLocation {
                    pipeline_name: pipeline.name.clone(),
                    pickup: pickup.to_string(),
                    agent_a: existing_agent_name.clone(),
                    agent_b: agent.name.clone(),
                });
            }

            seen_pickups.insert(pickup.to_string(), agent.name.clone());
        }

        Ok(())
    }

    pub fn register_pipeline(
        &mut self,
        pipeline: Pipeline,
        is_running: bool,
    ) -> Result<(), String> {
        if is_running {
            return Err(format!(
                "Pipeline '{}' is currently running and cannot be modified.",
                pipeline.name
            ));
        }
        self.validate_pipeline(&pipeline).map_err(|e| e.to_string())?;
        self.pipelines.insert(pipeline.id.clone(), pipeline);
        Ok(())
    }

    pub fn remove_agent(&mut self, agent_id: &str) -> Result<AgentConfig, String> {
        let attached_count = self.pipeline_count_for_agent(agent_id);
        if attached_count > 0 {
            return Err(format!(
                "Cannot delete agent '{}': Attached to {} pipeline(s). Remove it from pipelines first.",
                agent_id, attached_count
            ));
        }

        self.agents
            .remove(agent_id)
            .ok_or_else(|| format!("Agent '{}' not found", agent_id))
    }

    pub fn remove_pipeline(&mut self, pipeline_id: &str, is_running: bool) -> Result<Pipeline, String> {
        if is_running {
            return Err(format!(
                "Cannot delete pipeline '{}': Active running workers present.",
                pipeline_id
            ));
        }

        self.pipelines
            .remove(pipeline_id)
            .ok_or_else(|| format!("Pipeline '{}' not found", pipeline_id))
    }

    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| e.to_string())
    }

    pub fn default_storage_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".savant").join("colosseum").join("pipelines.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_cloning() {
        let mut registry = ColosseumRegistry::new();
        let agent1 = AgentConfig::new(
            "review-x",
            "review X",
            "Check code architecture",
            "reviewer",
            "v1",
            "claude",
            "claude-3-5-sonnet",
            "Status A",
            "Status Work",
            "Status B",
        );
        registry.register_agent(agent1);

        let cloned = registry.clone_agent("review-x", "review-x-copy", "review X Copy").unwrap();

        assert_eq!(cloned.id, "review-x-copy");
        assert_eq!(cloned.name, "review X Copy");
        assert_eq!(cloned.provider, "claude");
        assert_eq!(cloned.model, "claude-3-5-sonnet");
        assert_eq!(cloned.pickup_location, "Status A");
        assert_eq!(cloned.working_location, "Status Work");
        assert_eq!(cloned.drop_location, "Status B");
    }

    #[test]
    fn test_cannot_edit_or_delete_attached_agent() {
        let mut registry = ColosseumRegistry::new();
        let agent = AgentConfig::new(
            "agent-1",
            "Agent 1",
            "Prompt",
            "coder",
            "v1",
            "claude",
            "sonnet",
            "Status A",
            "Status Work",
            "Status B",
        );
        registry.register_agent(agent.clone());

        let pipeline = Pipeline {
            id: "pipe-1".to_string(),
            name: "Pipeline 1".to_string(),
            agent_ids: vec!["agent-1".to_string()],
        };
        registry.register_pipeline(pipeline, false).unwrap();

        assert_eq!(registry.pipeline_count_for_agent("agent-1"), 1);

        // Edit attached agent -> Fails
        let mut updated = agent.clone();
        updated.prompt = "Modified prompt".to_string();
        let edit_err = registry.update_agent(updated).unwrap_err();
        assert!(edit_err.contains("Attached agents cannot be edited directly"));

        // Delete attached agent -> Fails
        let del_err = registry.delete_agent("agent-1").unwrap_err();
        assert!(del_err.contains("Attached agents cannot be deleted"));

        // Clone attached agent -> Succeeds
        let cloned = registry.clone_agent("agent-1", "agent-1-clone", "Agent 1 Clone");
        assert!(cloned.is_ok());
    }

    #[test]
    fn test_cannot_modify_running_pipeline() {
        let mut registry = ColosseumRegistry::new();
        let agent = AgentConfig::new(
            "agent-1",
            "Agent 1",
            "Prompt",
            "coder",
            "v1",
            "claude",
            "sonnet",
            "Status A",
            "Status Work",
            "Status B",
        );
        registry.register_agent(agent);

        let pipeline = Pipeline {
            id: "pipe-1".to_string(),
            name: "Pipeline 1".to_string(),
            agent_ids: vec!["agent-1".to_string()],
        };

        // Modifying running pipeline -> Fails
        let err = registry.register_pipeline(pipeline, true).unwrap_err();
        assert!(err.contains("is currently running and cannot be modified"));
    }

    #[test]
    fn test_pipeline_validation_duplicate_pickup_fails() {
        let mut registry = ColosseumRegistry::new();
        let agent_a = AgentConfig::new("agent-a", "Agent A", "prompt A", "coder", "v1", "claude", "sonnet", "Status A", "Status Work", "Status B");
        let agent_b = AgentConfig::new("agent-b", "Agent B", "prompt B", "auditor", "v1", "codex", "gpt-4o", "Status A", "Status Work", "Status C");

        registry.register_agent(agent_a);
        registry.register_agent(agent_b);

        let pipeline = Pipeline {
            id: "pipe-1".to_string(),
            name: "Invalid Pipeline".to_string(),
            agent_ids: vec!["agent-a".to_string(), "agent-b".to_string()],
        };

        let err = registry.register_pipeline(pipeline, false).unwrap_err();
        assert!(err.contains("share the same pickup location"));
    }
}
