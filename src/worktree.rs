use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use anyhow::{Context, Result, bail};
use tokio::{process::Command, sync::Mutex};

static CREATION_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub start_commit: String,
}

async fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .await
        .with_context(|| format!("run git in {}", repo.display()))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub async fn resolve_commit(repository: &Path, revision: &str) -> Result<String> {
    git(
        repository,
        &["rev-parse", "--verify", &format!("{revision}^{{commit}}")],
    )
    .await
}

pub async fn provision(
    repository: &Path,
    root: &Path,
    scenario_id: &str,
    contender_id: &str,
    commit: &str,
) -> Result<Worktree> {
    let path = root.join(scenario_id).join(contender_id);
    let branch = format!("colosseum/{scenario_id}/{contender_id}");
    let lock = {
        let locks = CREATION_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut locks = locks.lock().await;
        locks
            .entry(path.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock.lock().await;
    tokio::fs::create_dir_all(path.parent().context("worktree parent")?).await?;

    let registered = git(repository, &["worktree", "list", "--porcelain"])
        .await?
        .lines()
        .any(|line| line == format!("worktree {}", path.display()));
    if registered {
        git(
            repository,
            &["worktree", "remove", "--force", &path.to_string_lossy()],
        )
        .await?;
    } else if path.exists() {
        tokio::fs::remove_dir_all(&path).await?;
    }
    git(repository, &["worktree", "prune"]).await?;
    let _ = git(repository, &["branch", "-D", &branch]).await;
    git(
        repository,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            &path.to_string_lossy(),
            commit,
        ],
    )
    .await?;
    let actual = git(&path, &["rev-parse", "HEAD"]).await?;
    if actual != commit {
        bail!("worktree started at {actual}; expected {commit}");
    }
    Ok(Worktree {
        path,
        branch,
        start_commit: actual,
    })
}

/// Create (or safely reuse) the one worktree that belongs to a Savant task.
/// A task never shares a branch or working directory with another task.
pub async fn provision_task(
    repository: &Path,
    root: &Path,
    task_id: &str,
    revision: &str,
) -> Result<Worktree> {
    let commit = resolve_commit(repository, revision).await?;
    // Git prints macOS /tmp worktrees under /private/tmp. Canonicalize the
    // root before comparing with `git worktree list --porcelain` so a resumed
    // task recognizes its own existing worktree rather than refusing it.
    tokio::fs::create_dir_all(root).await?;
    let root = tokio::fs::canonicalize(root)
        .await
        .with_context(|| format!("canonicalize worktree root {}", root.display()))?;
    let path = root.join(task_id);
    let branch = format!("savant-execution/{task_id}");
    let lock = {
        let locks = CREATION_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut locks = locks.lock().await;
        locks
            .entry(path.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _guard = lock.lock().await;

    let registered = git(repository, &["worktree", "list", "--porcelain"])
        .await?
        .lines()
        .any(|line| line == format!("worktree {}", path.display()));
    if registered {
        let actual = git(&path, &["rev-parse", "HEAD"]).await?;
        return Ok(Worktree {
            path,
            branch,
            start_commit: actual,
        });
    }
    if path.exists() {
        bail!(
            "refusing to overwrite unregistered worktree {}",
            path.display()
        );
    }
    tokio::fs::create_dir_all(path.parent().context("worktree parent")?).await?;
    git(repository, &["worktree", "prune"]).await?;
    git(
        repository,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            &path.to_string_lossy(),
            &commit,
        ],
    )
    .await?;
    Ok(Worktree {
        path,
        branch,
        start_commit: commit,
    })
}

pub async fn cleanup(repository: &Path, worktree: &Worktree) -> Result<()> {
    let _ = git(
        repository,
        &[
            "worktree",
            "remove",
            "--force",
            &worktree.path.to_string_lossy(),
        ],
    )
    .await;
    let _ = git(repository, &["branch", "-D", &worktree.branch]).await;
    let _ = git(repository, &["worktree", "prune"]).await;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct GitOutcome {
    pub changed_files: u64,
    pub status: String,
    pub diff_stat: String,
}

pub async fn outcome(worktree: &Path) -> Result<GitOutcome> {
    let status = git(worktree, &["status", "--short"]).await?;
    let diff_stat = git(worktree, &["diff", "--stat"]).await?;
    Ok(GitOutcome {
        changed_files: status.lines().filter(|line| !line.is_empty()).count() as u64,
        status,
        diff_stat,
    })
}
