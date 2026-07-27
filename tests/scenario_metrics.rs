use std::{collections::HashMap, path::PathBuf};

use savant_colosseum::{
    Contender, Scenario,
    metrics::{estimate_cost, parse_token_usage},
    scenario::TokenRates,
};

#[test]
fn extracts_latest_usage_shape_and_estimates_cost() {
    let output = r#"noise
{"usage":{"input_tokens":100,"output_tokens":20,"cached_input_tokens":10}}
{"result":{"usage":{"inputTokens":150,"outputTokens":40,"cachedInputTokens":20}}}"#;
    let usage = parse_token_usage(output);
    assert_eq!(usage.input_tokens, 150);
    assert_eq!(usage.output_tokens, 40);
    assert_eq!(usage.cached_tokens, 20);
    assert_eq!(
        estimate_cost(
            &usage,
            &TokenRates {
                input_per_million: 2.0,
                output_per_million: 8.0
            }
        ),
        0.00058
    );
}

#[test]
fn rejects_duplicate_contenders() {
    let contender = Contender {
        id: "codex".to_owned(),
        label: None,
        command: "codex".to_owned(),
        args: vec![],
        env: HashMap::new(),
        rates: TokenRates::default(),
    };
    let scenario = Scenario {
        scenario_id: "duplicate-agents".to_owned(),
        repository: PathBuf::from("."),
        start_commit: "HEAD".to_owned(),
        prompt: "Test".to_owned(),
        validation_command: "true".to_owned(),
        setup_command: None,
        timeout_seconds: 30,
        retain_worktrees: true,
        contenders: vec![contender.clone(), contender],
    };
    assert!(
        scenario
            .validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
}
