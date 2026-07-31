use std::path::Path;

use anyhow::{Result, bail};

use super::git;

pub async fn commit_and_push(
    worktree: &Path,
    branch: &str,
    message: &str,
) -> Result<(String, String)> {
    ensure_changes(worktree).await?;
    git(worktree, &["add", "-A"]).await?;
    git(worktree, &["commit", "-m", message]).await?;
    let commit = git(worktree, &["rev-parse", "HEAD"]).await?;
    let remote = git(worktree, &["remote", "get-url", "origin"]).await?;
    git(worktree, &["push", "-u", "origin", branch]).await?;
    Ok((commit, remote))
}

async fn ensure_changes(worktree: &Path) -> Result<()> {
    if git(worktree, &["status", "--porcelain"])
        .await?
        .trim()
        .is_empty()
    {
        bail!("agent completed without changing the worktree");
    }
    Ok(())
}
