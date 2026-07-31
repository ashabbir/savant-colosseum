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

#[cfg(test)]
mod tests {
    use super::provider_command;

    #[test]
    fn agy_profile_keeps_permissions_before_print_mode() {
        let (program, args) = provider_command("agy").unwrap();
        assert_eq!(program, "agy");
        assert_eq!(args, ["--dangerously-skip-permissions", "--print"]);
    }

    #[test]
    fn unsupported_provider_fails_closed() {
        assert!(provider_command("unknown-provider").is_err());
    }
}
