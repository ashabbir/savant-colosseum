use std::path::Path;

use anyhow::{Result, bail};
use tokio::process::Command;

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

pub async fn create_or_comment_github_review(
    worktree: &Path,
    branch: &str,
    title: &str,
    body: &str,
) -> Result<String> {
    let review = match review_url(worktree, branch).await {
        Ok(url) => url,
        Err(_) => {
            run_gh(
                worktree,
                &[
                    "pr", "create", "--head", branch, "--title", title, "--body", body,
                ],
            )
            .await?;
            review_url(worktree, branch).await?
        }
    };
    run_gh(worktree, &["pr", "comment", branch, "--body", body]).await?;
    Ok(review)
}

async fn review_url(worktree: &Path, branch: &str) -> Result<String> {
    run_gh(
        worktree,
        &["pr", "view", branch, "--json", "url", "--jq", ".url"],
    )
    .await
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

async fn run_gh(worktree: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("gh")
        .current_dir(worktree)
        .args(args)
        .output()
        .await?;
    if !output.status.success() {
        bail!(
            "gh {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
