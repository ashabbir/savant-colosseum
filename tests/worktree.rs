use std::path::Path;

use anyhow::Result;
use savant_executioner::worktree::{
    commit_and_push, merge_and_push, provision_task, refresh_review_boundaries,
};
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
    fs::write(created.path.join("WIP.md"), "unfinished prior attempt\n").await?;
    let reused = provision_task(&repository, &root, "task-1", "HEAD").await?;

    assert!(created.path.join("README.md").is_file());
    assert_eq!(created.path, reused.path);
    assert_eq!(created.start_commit, reused.start_commit);
    assert_eq!(created.branch, "savant-execution/task-1");
    assert!(!created.resumed);
    assert!(reused.resumed);
    assert_eq!(created.review_base_commit, reused.review_base_commit);
    assert_eq!(
        fs::read_to_string(reused.path.join("WIP.md")).await?,
        "unfinished prior attempt\n"
    );
    assert!(!created.base_branch.is_empty());
    Ok(())
}

#[tokio::test]
async fn preserves_the_whole_mr_base_when_main_advances_between_attempts() -> Result<()> {
    let (directory, repository) = initialized_repository().await?;
    let root = directory.path().join("worktrees");
    let created = provision_task(&repository, &root, "task-1", "HEAD").await?;
    let original_base = created.review_base_commit.clone();
    fs::write(created.path.join("feature.txt"), "feature\n").await?;
    run_git(&created.path, &["add", "feature.txt"]).await?;
    run_git(&created.path, &["commit", "-m", "feature attempt"]).await?;

    fs::write(repository.join("main.txt"), "main advanced\n").await?;
    run_git(&repository, &["add", "main.txt"]).await?;
    run_git(&repository, &["commit", "-m", "advance main"]).await?;

    let resumed = provision_task(&repository, &root, "task-1", "HEAD").await?;

    assert!(resumed.resumed);
    assert_eq!(resumed.review_base_commit, original_base);
    assert_ne!(resumed.start_commit, resumed.review_base_commit);
    assert!(!resumed.base_is_ancestor);
    Ok(())
}

#[tokio::test]
async fn refreshes_the_full_mr_base_after_the_coder_incorporates_main() -> Result<()> {
    let (directory, repository) = initialized_repository().await?;
    let root = directory.path().join("worktrees");
    let mut worktree = provision_task(&repository, &root, "task-1", "HEAD").await?;
    fs::write(worktree.path.join("feature.txt"), "feature\n").await?;
    run_git(&worktree.path, &["add", "feature.txt"]).await?;
    run_git(&worktree.path, &["commit", "-m", "feature attempt"]).await?;
    fs::write(repository.join("main.txt"), "main advanced\n").await?;
    run_git(&repository, &["add", "main.txt"]).await?;
    run_git(&repository, &["commit", "-m", "advance main"]).await?;
    let main_head = git_output(&repository, &["rev-parse", "HEAD"]).await?;

    run_git(&worktree.path, &["merge", "main", "--no-edit"]).await?;
    refresh_review_boundaries(&mut worktree).await?;

    assert!(worktree.base_is_ancestor);
    assert_eq!(worktree.review_base_commit, main_head);
    assert_eq!(worktree.base_branch_commit, main_head);
    Ok(())
}

#[tokio::test]
async fn fast_forwards_the_remote_base_branch_after_approval() -> Result<()> {
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
        "task-merge",
        "HEAD",
    )
    .await?;
    fs::write(worktree.path.join("README.md"), "approved\n").await?;
    let (commit, _) = commit_and_push(&worktree.path, &worktree.branch, "approved work").await?;

    let base_branch = worktree.base_branch.clone();
    merge_and_push(&worktree).await?;

    let remote_base = git_output(
        &remote,
        &["rev-parse", &format!("refs/heads/{base_branch}")],
    )
    .await?;
    assert_eq!(remote_base, commit);
    Ok(())
}

#[tokio::test]
async fn refuses_to_bypass_an_existing_unregistered_task_directory() -> Result<()> {
    let (directory, repository) = initialized_repository().await?;
    let root = directory.path().join("worktrees");
    fs::create_dir_all(root.join("task-1")).await?;

    let error = provision_task(&repository, &root, "task-1", "HEAD")
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("refusing to replace or bypass prior work")
    );
    Ok(())
}

#[tokio::test]
async fn resumes_an_existing_task_branch_when_its_worktree_was_removed() -> Result<()> {
    let (directory, repository) = initialized_repository().await?;
    let root = directory.path().join("worktrees");
    let created = provision_task(&repository, &root, "task-1", "HEAD").await?;
    fs::write(created.path.join("README.md"), "continued\n").await?;
    run_git(&created.path, &["add", "README.md"]).await?;
    run_git(&created.path, &["commit", "-m", "partial attempt"]).await?;
    let partial_head = git_output(&created.path, &["rev-parse", "HEAD"]).await?;
    run_git(
        &repository,
        &["worktree", "remove", &created.path.to_string_lossy()],
    )
    .await?;

    let resumed = provision_task(&repository, &root, "task-1", "HEAD").await?;

    assert!(resumed.resumed);
    assert_eq!(resumed.start_commit, partial_head);
    assert_eq!(
        fs::read_to_string(resumed.path.join("README.md")).await?,
        "continued\n"
    );
    Ok(())
}

#[tokio::test]
async fn resumes_the_latest_registered_legacy_suffixed_worktree() -> Result<()> {
    let (directory, repository) = initialized_repository().await?;
    let root = directory.path().join("worktrees");
    fs::create_dir_all(&root).await?;
    let legacy = root.join("task-1-2");
    run_git(
        &repository,
        &[
            "worktree",
            "add",
            "-b",
            "savant-execution/task-1-2",
            &legacy.to_string_lossy(),
            "HEAD",
        ],
    )
    .await?;
    fs::write(legacy.join("WIP.md"), "legacy continuation\n").await?;

    let resumed = provision_task(&repository, &root, "task-1", "HEAD").await?;

    assert!(resumed.resumed);
    assert_eq!(resumed.path, fs::canonicalize(&legacy).await?);
    assert_eq!(resumed.branch, "savant-execution/task-1-2");
    assert_eq!(
        fs::read_to_string(resumed.path.join("WIP.md")).await?,
        "legacy continuation\n"
    );
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
