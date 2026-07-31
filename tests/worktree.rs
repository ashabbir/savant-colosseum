use std::path::Path;

use anyhow::Result;
use savant_executioner::worktree::{commit_and_push, provision_task};
use tempfile::TempDir;
use tokio::{fs, process::Command};

async fn run_git(repository: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .status()
        .await?;
    assert!(status.success(), "git {} failed", args.join(" "));
    Ok(())
}

async fn initialized_repository() -> Result<(TempDir, std::path::PathBuf)> {
    let directory = tempfile::tempdir()?;
    let repository = directory.path().join("repository");
    fs::create_dir(&repository).await?;
    run_git(&repository, &["init"]).await?;
    run_git(&repository, &["config", "user.name", "Colosseum Test"]).await?;
    run_git(
        &repository,
        &["config", "user.email", "test@example.invalid"],
    )
    .await?;
    fs::write(repository.join("README.md"), "initial\n").await?;
    run_git(&repository, &["add", "README.md"]).await?;
    run_git(&repository, &["commit", "-m", "initial"]).await?;
    Ok((directory, repository))
}

async fn git_output(repository: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .await?;
    assert!(output.status.success(), "git {} failed", args.join(" "));
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[tokio::test]
async fn provisions_and_reuses_a_task_worktree() -> Result<()> {
    let (directory, repository) = initialized_repository().await?;
    let root = directory.path().join("worktrees");

    let created = provision_task(&repository, &root, "task-1", "HEAD").await?;
    let reused = provision_task(&repository, &root, "task-1", "HEAD").await?;

    assert!(created.path.join("README.md").is_file());
    assert_eq!(created.path, reused.path);
    assert_eq!(created.start_commit, reused.start_commit);
    assert_eq!(created.branch, "savant-execution/task-1");
    Ok(())
}

#[tokio::test]
async fn refuses_to_overwrite_an_unregistered_task_directory() -> Result<()> {
    let (directory, repository) = initialized_repository().await?;
    let root = directory.path().join("worktrees");
    fs::create_dir_all(root.join("task-1")).await?;

    let error = provision_task(&repository, &root, "task-1", "HEAD")
        .await
        .expect_err("unregistered paths must not be overwritten");

    assert!(error.to_string().contains("refusing to overwrite"));
    Ok(())
}

#[tokio::test]
async fn commits_and_pushes_a_changed_task_worktree() -> Result<()> {
    let (directory, repository) = initialized_repository().await?;
    let remote = directory.path().join("remote.git");
    let status = Command::new("git")
        .args(["init", "--bare", &remote.to_string_lossy()])
        .status()
        .await?;
    assert!(status.success());
    run_git(
        &repository,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    )
    .await?;
    run_git(&repository, &["push", "-u", "origin", "HEAD"]).await?;

    let worktree = provision_task(
        &repository,
        &directory.path().join("worktrees"),
        "task-1",
        "HEAD",
    )
    .await?;
    fs::write(worktree.path.join("README.md"), "changed\n").await?;

    let (commit, pushed_remote) =
        commit_and_push(&worktree.path, &worktree.branch, "task update").await?;
    let remote_commit = git_output(
        &remote,
        &["rev-parse", &format!("refs/heads/{}", worktree.branch)],
    )
    .await?;

    assert_eq!(commit, remote_commit);
    assert_eq!(pushed_remote, remote.to_string_lossy());
    Ok(())
}
