use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tokio::process::Command;

mod locks;
mod publication;

pub use publication::{commit_and_push, merge_and_push};

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    /// HEAD at the start of this execution attempt.
    pub start_commit: String,
    /// Stable merge-base used for every whole-MR review and diff snapshot.
    pub review_base_commit: String,
    pub base_branch: String,
    pub base_branch_commit: String,
    pub base_is_ancestor: bool,
    pub resumed: bool,
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
    let base_branch = match git(repository, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await {
        Ok(branch) if !branch.trim().is_empty() => branch,
        _ if revision != "HEAD" => revision.to_owned(),
        _ => bail!("repository HEAD is detached; configure an explicit revision branch"),
    };
    // Git prints macOS /tmp worktrees under /private/tmp. Canonicalize the
    // root before comparing with `git worktree list --porcelain` so a resumed
    // task recognizes its own existing worktree rather than refusing it.
    tokio::fs::create_dir_all(root).await?;
    let root = tokio::fs::canonicalize(root)
        .await
        .with_context(|| format!("canonicalize worktree root {}", root.display()))?;
    let expected_path = root.join(task_id);
    let branch = format!("savant-execution/{task_id}");

    git(repository, &["worktree", "prune"]).await?;
    let path = if expected_path.exists() {
        expected_path
    } else {
        latest_registered_task_worktree(repository, &root, task_id)
            .await?
            .unwrap_or(expected_path)
    };
    let _guard = locks::for_path(&path).await;
    if path.exists() {
        return resume_worktree(repository, path, task_id, base_branch).await;
    }

    tokio::fs::create_dir_all(path.parent().context("worktree parent")?).await?;
    git(repository, &["worktree", "prune"]).await?;
    let branch_ref = format!("refs/heads/{branch}");
    let resumed = git(repository, &["show-ref", "--verify", &branch_ref])
        .await
        .is_ok();
    if resumed {
        git(
            repository,
            &["worktree", "add", &path.to_string_lossy(), &branch],
        )
        .await?;
    } else {
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
    }
    let actual = git(&path, &["rev-parse", "HEAD"]).await?;
    let (review_base_commit, base_branch_commit, base_is_ancestor) =
        review_boundaries(&path, &base_branch).await?;
    Ok(Worktree {
        path,
        branch,
        start_commit: actual,
        review_base_commit,
        base_branch,
        base_branch_commit,
        base_is_ancestor,
        resumed,
    })
}

async fn resume_worktree(
    repository: &Path,
    path: PathBuf,
    task_id: &str,
    base_branch: String,
) -> Result<Worktree> {
    let registered = git(repository, &["worktree", "list", "--porcelain"])
        .await?
        .lines()
        .any(|line| line == format!("worktree {}", path.display()));
    if !registered {
        bail!(
            "task worktree path {} already exists but is not registered to this repository; refusing to replace or bypass prior work",
            path.display()
        );
    }
    let branch = git(&path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .await
        .context("existing task worktree has a detached HEAD")?;
    let directory_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("task worktree path has no UTF-8 directory name")?;
    let expected = format!("savant-execution/{directory_name}");
    let valid_name = directory_name == task_id
        || directory_name
            .strip_prefix(&format!("{task_id}-"))
            .is_some_and(|suffix| suffix.parse::<u64>().is_ok());
    if !valid_name {
        bail!(
            "registered worktree {} does not belong to task {task_id}",
            path.display()
        );
    }
    if branch != expected {
        bail!(
            "task worktree {} uses branch {branch}, expected {expected}; refusing unsafe continuation",
            path.display()
        );
    }
    let start_commit = git(&path, &["rev-parse", "HEAD"]).await?;
    let (review_base_commit, base_branch_commit, base_is_ancestor) =
        review_boundaries(&path, &base_branch).await?;
    Ok(Worktree {
        path,
        branch,
        start_commit,
        review_base_commit,
        base_branch,
        base_branch_commit,
        base_is_ancestor,
        resumed: true,
    })
}

async fn latest_registered_task_worktree(
    repository: &Path,
    root: &Path,
    task_id: &str,
) -> Result<Option<PathBuf>> {
    let listing = git(repository, &["worktree", "list", "--porcelain"]).await?;
    let mut candidates = listing
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(PathBuf::from)
        .filter(|path| path.parent() == Some(root))
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            let suffix = if name == task_id {
                0
            } else {
                name.strip_prefix(&format!("{task_id}-"))?.parse().ok()?
            };
            Some((suffix, path))
        })
        .collect::<Vec<(u64, PathBuf)>>();
    candidates.sort_by_key(|(suffix, _)| *suffix);
    Ok(candidates.pop().map(|(_, path)| path))
}

async fn review_boundaries(worktree: &Path, base_branch: &str) -> Result<(String, String, bool)> {
    let review_base = git(worktree, &["merge-base", base_branch, "HEAD"])
        .await
        .with_context(|| format!("resolve full-MR base against {base_branch}"))?;
    let base_commit = git(worktree, &["rev-parse", base_branch]).await?;
    let base_is_ancestor = git(
        worktree,
        &["merge-base", "--is-ancestor", base_branch, "HEAD"],
    )
    .await
    .is_ok();
    Ok((review_base, base_commit, base_is_ancestor))
}

pub async fn refresh_review_boundaries(worktree: &mut Worktree) -> Result<()> {
    let (review_base_commit, base_branch_commit, base_is_ancestor) =
        review_boundaries(&worktree.path, &worktree.base_branch).await?;
    worktree.review_base_commit = review_base_commit;
    worktree.base_branch_commit = base_branch_commit;
    worktree.base_is_ancestor = base_is_ancestor;
    Ok(())
}
