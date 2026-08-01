use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tokio::process::Command;

mod locks;
mod publication;

pub use publication::commit_and_push;

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub start_commit: String,
}

pub(super) async fn git(repo: &Path, args: &[&str]) -> Result<String> {
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
    let mut path = root.join(task_id);
    let mut branch = format!("savant-execution/{task_id}");
    
    // Auto-increment suffix if directory already exists but is not registered
    let mut suffix = 1;
    while path.exists() {
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
        path = root.join(format!("{task_id}-{suffix}"));
        branch = format!("savant-execution/{task_id}-{suffix}");
        suffix += 1;
    }

    let _guard = locks::for_path(&path).await;
    tokio::fs::create_dir_all(path.parent().context("worktree parent")?).await?;
    git(repository, &["worktree", "prune"]).await?;
    let _ = git(repository, &["branch", "-D", &branch]).await;
    git(
        repository,
        &[
            "worktree",
            "add",
            "-B",
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
