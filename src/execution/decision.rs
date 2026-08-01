use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const RESULT_MARKER: &str = "COLOSSEUM_RESULT:";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    Ready,
    NeedsInput,
    Pass,
    Fail,
    Complete,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentDecision {
    pub decision: Decision,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub questions: Vec<String>,
}

impl AgentDecision {
    pub fn parse(output: &str) -> Result<Self> {
        let marker = output
            .rfind(RESULT_MARKER)
            .context("provider omitted COLOSSEUM_RESULT")?;
        let json = output[marker + RESULT_MARKER.len()..]
            .lines()
            .next()
            .unwrap_or_default()
            .trim();
        serde_json::from_str(json).context("provider returned invalid COLOSSEUM_RESULT JSON")
    }

    pub fn comment_body(&self) -> String {
        let mut sections = vec![format!("**Summary**\n{}", self.summary.trim())];
        if !self.rationale.trim().is_empty() {
            sections.push(format!("**Rationale**\n{}", self.rationale.trim()));
        }
        if !self.questions.is_empty() {
            sections.push(format!(
                "**Questions**\n{}",
                self.questions
                    .iter()
                    .map(|question| format!("- {question}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        sections.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentDecision, Decision};

    #[test]
    fn parses_the_last_structured_result_from_provider_output() {
        let output = concat!(
            "analysis\nCOLOSSEUM_RESULT: {\"decision\":\"needs-input\",",
            "\"summary\":\"Scope is unclear\",",
            "\"rationale\":\"Two APIs could own the transition\",",
            "\"questions\":[\"Which API is canonical?\"]}\n"
        );

        let result = AgentDecision::parse(output).unwrap();

        assert_eq!(result.decision, Decision::NeedsInput);
        assert_eq!(result.summary, "Scope is unclear");
        assert_eq!(result.questions, ["Which API is canonical?"]);
    }

    #[test]
    fn fails_closed_when_the_provider_omits_a_structured_result() {
        assert!(AgentDecision::parse("looks good").is_err());
    }
}
