use std::path::Path;

use anyhow::{Result, bail};

use super::Worktree;
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

/// Complete an approved change by fast-forwarding the configured remote base
/// branch to the already-published task commit. A regular push deliberately
/// rejects divergence instead of resolving conflicts without human context.
pub async fn merge_and_push(worktree: &Worktree) -> Result<String> {
    let refspec = format!("HEAD:refs/heads/{}", worktree.base_branch);
    git(&worktree.path, &["push", "origin", &refspec]).await?;
    git(&worktree.path, &["rev-parse", "HEAD"]).await
}
