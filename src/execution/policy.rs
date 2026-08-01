use std::path::Path;

use anyhow::Result;

pub(super) fn provider_command(
    provider: &str,
    model: Option<&str>,
) -> Result<(&'static str, Vec<String>)> {
    let (program, mut args) = match provider {
        "" | "codex" => (
            "codex",
            vec![
                "exec".into(),
                "--dangerously-bypass-approvals-and-sandbox".into(),
            ],
        ),
        "claude" => (
            "claude",
            vec!["-p".into(), "--dangerously-skip-permissions".into()],
        ),
        "copilot" => ("copilot", vec!["-p".into(), "--allow-all-tools".into()]),
        "hermes" => ("hermes", vec!["--yolo".into()]),
        "agy" => (
            "agy",
            vec!["--dangerously-skip-permissions".into(), "--print".into()],
        ),
        other => anyhow::bail!("unsupported Colosseum provider: {other}"),
    };
    if let Some(model) = model.filter(|value| !value.trim().is_empty()) {
        match provider {
            "" | "codex" | "claude" | "copilot" | "agy" => {
                args.push("--model".into());
                args.push(model.to_owned());
            }
            // Hermes installations do not expose a stable model-selection flag.
            "hermes" => {}
            _ => unreachable!("unsupported provider returned above"),
        }
    }
    Ok((program, args))
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
        let (program, args) = provider_command("agy", None).unwrap();
        assert_eq!(program, "agy");
        assert_eq!(args, ["--dangerously-skip-permissions", "--print"]);
    }

    #[test]
    fn unsupported_provider_fails_closed() {
        assert!(provider_command("unknown-provider", None).is_err());
    }

    #[test]
    fn configured_model_is_forwarded_to_supported_provider() {
        let (_, args) = provider_command("codex", Some("gpt-5.6-terra")).unwrap();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--model", "gpt-5.6-terra"])
        );
    }
}
