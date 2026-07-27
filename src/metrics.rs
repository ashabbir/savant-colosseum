use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::scenario::TokenRates;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
}

fn number(value: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| value.get(key).and_then(Value::as_u64))
        .unwrap_or_default()
}

pub fn parse_token_usage(output: &str) -> TokenUsage {
    let mut result = TokenUsage::default();
    for line in output
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
    {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let usage = event
            .get("usage")
            .or_else(|| event.get("token_usage"))
            .or_else(|| event.pointer("/result/usage"));
        let Some(usage) = usage else { continue };
        result.input_tokens = result.input_tokens.max(number(
            usage,
            &["input_tokens", "inputTokens", "prompt_tokens"],
        ));
        result.output_tokens = result.output_tokens.max(number(
            usage,
            &["output_tokens", "outputTokens", "completion_tokens"],
        ));
        result.cached_tokens = result.cached_tokens.max(number(
            usage,
            &[
                "cached_input_tokens",
                "cachedInputTokens",
                "cache_read_input_tokens",
            ],
        ));
    }
    result
}

pub fn estimate_cost(usage: &TokenUsage, rates: &TokenRates) -> f64 {
    let uncached = usage.input_tokens.saturating_sub(usage.cached_tokens) as f64;
    ((uncached * rates.input_per_million + usage.output_tokens as f64 * rates.output_per_million)
        / 1_000_000.0
        * 1_000_000.0)
        .round()
        / 1_000_000.0
}
