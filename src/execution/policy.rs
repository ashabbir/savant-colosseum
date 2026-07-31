use std::path::Path;

use anyhow::Result;

pub(super) fn provider_command(provider: &str) -> Result<(&'static str, Vec<String>)> {
    match provider {
        "codex" => Ok((
            "codex",
            vec![
                "exec".into(),
                "--dangerously-bypass-approvals-and-sandbox".into(),
            ],
        )),
        "claude" => Ok((
            "claude",
            vec!["-p".into(), "--dangerously-skip-permissions".into()],
        )),
        "copilot" => Ok(("copilot", vec!["-p".into(), "--allow-all-tools".into()])),
        "hermes" => Ok(("hermes", vec!["--yes".into()])),
        "agy" => Ok((
            "agy",
            vec!["--dangerously-skip-permissions".into(), "--print".into()],
        )),
        other => anyhow::bail!("unsupported Colosseum provider: {other}"),
    }
}

pub(super) fn verification_command(worktree: &Path) -> &'static str {
    if worktree.join("Cargo.toml").exists() {
        "cargo test"
    } else if worktree.join("package.json").exists() {
        "npm test"
    } else if worktree.join("pyproject.toml").exists() || worktree.join("pytest.ini").exists() {
        "pytest"
    } else {
        "git diff --check"
    }
}
