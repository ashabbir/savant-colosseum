use std::{collections::HashMap, path::Path, process::Command};

use anyhow::{Context, Result, bail};
use savant_colosseum::{
    Contender, Runner, RunnerConfig, Scenario,
    database::{self, battle_results, list_battles},
    scenario::TokenRates,
};
use tempfile::TempDir;

fn command(cwd: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        bail!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn create_fixture_repo(root: &Path) -> Result<String> {
    std::fs::create_dir_all(root)?;
    command(root, "git", &["init", "-q"])?;
    command(
        root,
        "git",
        &["config", "user.email", "colosseum@example.test"],
    )?;
    command(root, "git", &["config", "user.name", "Colosseum Test"])?;
    std::fs::write(
        root.join("calculator.sh"),
        "#!/bin/sh\n[ \"$1\" != \"0\" ] || exit 1\nexpr 10 / \"$1\"\n",
    )?;
    std::fs::write(
        root.join("validate.sh"),
        "#!/bin/sh\nset -eu\n./calculator.sh 2 | grep -q '^5$'\n./calculator.sh 0 | grep -q '^undefined$'\necho '2 passed, 0 failed'\n",
    )?;
    std::fs::write(
        root.join("agent.sh"),
        "#!/bin/sh\ncat >/dev/null\ncat > calculator.sh <<'EOF'\n#!/bin/sh\nif [ \"$1\" = \"0\" ]; then echo undefined; exit 0; fi\nexpr 10 / \"$1\"\nEOF\nchmod +x calculator.sh\necho '{\"usage\":{\"input_tokens\":120,\"output_tokens\":30,\"cached_input_tokens\":20}}'\n",
    )?;
    command(
        root,
        "chmod",
        &["+x", "calculator.sh", "validate.sh", "agent.sh"],
    )?;
    command(root, "git", &["add", "."])?;
    command(root, "git", &["commit", "-qm", "fixture"])?;
    command(root, "git", &["rev-parse", "HEAD"])
}

#[tokio::test]
async fn runs_contenders_in_isolated_worktrees_and_persists_results() -> Result<()> {
    let temp = TempDir::new()?;
    let repository = temp.path().join("fixture");
    let commit = create_fixture_repo(&repository)?;
    let data = temp.path().join("data");
    let worktrees = temp.path().join("worktrees");
    let pool = database::open(&data.join("colosseum.db")).await?;
    let scenario = Scenario {
        scenario_id: "division-bugfix".to_owned(),
        repository: repository.clone(),
        start_commit: commit.clone(),
        prompt: "Make division by zero return undefined.".to_owned(),
        validation_command: "./validate.sh".to_owned(),
        setup_command: None,
        timeout_seconds: 30,
        retain_worktrees: true,
        contenders: ["alpha", "beta"]
            .into_iter()
            .map(|id| Contender {
                id: id.to_owned(),
                label: Some(format!("Agent {id}")),
                command: "/bin/sh".to_owned(),
                args: vec!["agent.sh".to_owned()],
                env: HashMap::new(),
                rates: TokenRates {
                    input_per_million: 1.0,
                    output_per_million: 2.0,
                },
            })
            .collect(),
    };
    let outcome = Runner::new(RunnerConfig {
        pool: pool.clone(),
        worktree_root: worktrees.clone(),
        log_root: data.join("logs"),
    })
    .run(scenario, None)
    .await?;

    assert_eq!(outcome.status, "completed", "{:?}", outcome.errors);
    assert_eq!(outcome.results.len(), 2);
    assert!(
        outcome
            .results
            .iter()
            .all(|result| result.status == "passed")
    );
    assert!(
        outcome
            .results
            .iter()
            .all(|result| result.tests_passed == Some(2))
    );
    assert!(
        outcome
            .results
            .iter()
            .all(|result| result.input_tokens == 120)
    );
    assert!(
        outcome
            .results
            .iter()
            .all(|result| result.output_tokens == 30)
    );
    assert!(
        outcome
            .results
            .iter()
            .all(|result| result.changed_files == 1)
    );
    for result in &outcome.results {
        assert_eq!(
            command(
                Path::new(&result.worktree_path),
                "git",
                &["rev-parse", "HEAD"]
            )?,
            commit
        );
        assert_eq!(
            result.branch,
            format!("colosseum/division-bugfix/{}", result.contender_id)
        );
        assert!(
            Path::new(&result.worktree_path)
                .join("calculator.sh")
                .exists()
        );
        assert!(
            data.join("logs")
                .join(outcome.battle_id.to_string())
                .join(format!("{}.jsonl", result.contender_id))
                .exists()
        );
    }
    assert_eq!(list_battles(&pool, 10).await?.len(), 1);
    assert_eq!(
        battle_results(&pool, &outcome.battle_id.to_string())
            .await?
            .len(),
        2
    );
    Ok(())
}

#[tokio::test]
async fn records_a_failed_validation_without_losing_the_other_result() -> Result<()> {
    let temp = TempDir::new()?;
    let repository = temp.path().join("fixture");
    let commit = create_fixture_repo(&repository)?;
    let data = temp.path().join("data");
    let pool = database::open(&data.join("colosseum.db")).await?;
    let contenders = vec![
        Contender {
            id: "fixer".to_owned(),
            label: None,
            command: "/bin/sh".to_owned(),
            args: vec!["agent.sh".to_owned()],
            env: HashMap::new(),
            rates: TokenRates::default(),
        },
        Contender {
            id: "observer".to_owned(),
            label: None,
            command: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), "cat >/dev/null; echo no-change".to_owned()],
            env: HashMap::new(),
            rates: TokenRates::default(),
        },
    ];
    let outcome = Runner::new(RunnerConfig {
        pool: pool.clone(),
        worktree_root: temp.path().join("worktrees"),
        log_root: data.join("logs"),
    })
    .run(
        Scenario {
            scenario_id: "mixed-results".to_owned(),
            repository,
            start_commit: commit,
            prompt: "Fix the bug.".to_owned(),
            validation_command: "./validate.sh".to_owned(),
            setup_command: None,
            timeout_seconds: 30,
            retain_worktrees: true,
            contenders,
        },
        None,
    )
    .await
    .context("run mixed battle")?;

    assert_eq!(outcome.status, "failed");
    assert_eq!(outcome.results.len(), 2);
    assert_eq!(
        outcome
            .results
            .iter()
            .filter(|result| result.status == "passed")
            .count(),
        1
    );
    assert_eq!(
        outcome
            .results
            .iter()
            .filter(|result| result.status == "failed")
            .count(),
        1
    );
    assert_eq!(
        battle_results(&pool, &outcome.battle_id.to_string())
            .await?
            .len(),
        2
    );
    Ok(())
}
