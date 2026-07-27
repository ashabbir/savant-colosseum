use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, sqlite::SqliteConnectOptions};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BattleRecord {
    pub id: String,
    pub scenario_id: String,
    pub repository: String,
    pub start_commit: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<i64>,
}

pub struct NewBattle<'a> {
    pub id: Uuid,
    pub scenario_id: &'a str,
    pub repository: &'a str,
    pub start_commit: &'a str,
    pub prompt: &'a str,
    pub validation_command: &'a str,
    pub started_at: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContenderRecord {
    pub id: String,
    pub battle_id: String,
    pub contender_id: String,
    pub contender_label: String,
    pub branch: String,
    pub worktree_path: String,
    pub status: String,
    pub agent_exit_code: i64,
    pub validation_exit_code: i64,
    pub duration_ms: i64,
    pub validation_duration_ms: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_tokens: i64,
    pub estimated_cost: f64,
    pub tests_run: Option<i64>,
    pub tests_passed: Option<i64>,
    pub tests_failed: Option<i64>,
    pub lint_violations: i64,
    pub changed_files: i64,
    pub stdout: String,
    pub stderr: String,
    pub validation_stdout: String,
    pub validation_stderr: String,
    pub git_status: String,
    pub diff_stat: String,
}

pub async fn open(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePool::connect_with(options)
        .await
        .with_context(|| format!("open database {}", path.display()))?;
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS battle_runs (
          id TEXT PRIMARY KEY,
          scenario_id TEXT NOT NULL,
          repository TEXT NOT NULL,
          start_commit TEXT NOT NULL,
          prompt TEXT NOT NULL,
          validation_command TEXT NOT NULL,
          status TEXT NOT NULL,
          started_at TEXT NOT NULL,
          completed_at TEXT,
          duration_ms INTEGER
        );
        CREATE TABLE IF NOT EXISTS contender_results (
          id TEXT PRIMARY KEY,
          battle_id TEXT NOT NULL REFERENCES battle_runs(id) ON DELETE CASCADE,
          contender_id TEXT NOT NULL,
          contender_label TEXT NOT NULL,
          branch TEXT NOT NULL,
          worktree_path TEXT NOT NULL,
          status TEXT NOT NULL,
          agent_exit_code INTEGER NOT NULL,
          validation_exit_code INTEGER NOT NULL,
          duration_ms INTEGER NOT NULL,
          validation_duration_ms INTEGER NOT NULL,
          input_tokens INTEGER NOT NULL DEFAULT 0,
          output_tokens INTEGER NOT NULL DEFAULT 0,
          cached_tokens INTEGER NOT NULL DEFAULT 0,
          estimated_cost REAL NOT NULL DEFAULT 0,
          tests_run INTEGER,
          tests_passed INTEGER,
          tests_failed INTEGER,
          lint_violations INTEGER NOT NULL DEFAULT 0,
          changed_files INTEGER NOT NULL DEFAULT 0,
          stdout TEXT NOT NULL DEFAULT '',
          stderr TEXT NOT NULL DEFAULT '',
          validation_stdout TEXT NOT NULL DEFAULT '',
          validation_stderr TEXT NOT NULL DEFAULT '',
          git_status TEXT NOT NULL DEFAULT '',
          diff_stat TEXT NOT NULL DEFAULT '',
          UNIQUE (battle_id, contender_id)
        );
        CREATE INDEX IF NOT EXISTS idx_battles_started ON battle_runs(started_at DESC);
        CREATE INDEX IF NOT EXISTS idx_results_battle ON contender_results(battle_id);
        "#,
    )
    .execute(&pool)
    .await?;
    Ok(pool)
}

pub async fn create_battle(pool: &SqlitePool, battle: NewBattle<'_>) -> Result<()> {
    sqlx::query(
        "INSERT INTO battle_runs
        (id, scenario_id, repository, start_commit, prompt, validation_command, status, started_at)
        VALUES (?, ?, ?, ?, ?, ?, 'running', ?)",
    )
    .bind(battle.id.to_string())
    .bind(battle.scenario_id)
    .bind(battle.repository)
    .bind(battle.start_commit)
    .bind(battle.prompt)
    .bind(battle.validation_command)
    .bind(battle.started_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn finish_battle(
    pool: &SqlitePool,
    id: Uuid,
    status: &str,
    completed_at: &str,
    duration_ms: u64,
) -> Result<()> {
    sqlx::query(
        "UPDATE battle_runs SET status = ?, completed_at = ?, duration_ms = ? WHERE id = ?",
    )
    .bind(status)
    .bind(completed_at)
    .bind(duration_ms as i64)
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_result(pool: &SqlitePool, result: &ContenderRecord) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO contender_results (
          id, battle_id, contender_id, contender_label, branch, worktree_path, status,
          agent_exit_code, validation_exit_code, duration_ms, validation_duration_ms,
          input_tokens, output_tokens, cached_tokens, estimated_cost, tests_run,
          tests_passed, tests_failed, lint_violations, changed_files, stdout, stderr,
          validation_stdout, validation_stderr, git_status, diff_stat
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&result.id)
    .bind(&result.battle_id)
    .bind(&result.contender_id)
    .bind(&result.contender_label)
    .bind(&result.branch)
    .bind(&result.worktree_path)
    .bind(&result.status)
    .bind(result.agent_exit_code)
    .bind(result.validation_exit_code)
    .bind(result.duration_ms)
    .bind(result.validation_duration_ms)
    .bind(result.input_tokens)
    .bind(result.output_tokens)
    .bind(result.cached_tokens)
    .bind(result.estimated_cost)
    .bind(result.tests_run)
    .bind(result.tests_passed)
    .bind(result.tests_failed)
    .bind(result.lint_violations)
    .bind(result.changed_files)
    .bind(&result.stdout)
    .bind(&result.stderr)
    .bind(&result.validation_stdout)
    .bind(&result.validation_stderr)
    .bind(&result.git_status)
    .bind(&result.diff_stat)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_battles(pool: &SqlitePool, limit: u32) -> Result<Vec<BattleRecord>> {
    Ok(sqlx::query_as::<_, BattleRecord>(
        "SELECT id, scenario_id, repository, start_commit, status, started_at,
         completed_at, duration_ms FROM battle_runs ORDER BY started_at DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

pub async fn battle_results(pool: &SqlitePool, battle_id: &str) -> Result<Vec<ContenderRecord>> {
    Ok(sqlx::query_as::<_, ContenderRecord>(
        "SELECT * FROM contender_results WHERE battle_id = ? ORDER BY contender_id",
    )
    .bind(battle_id)
    .fetch_all(pool)
    .await?)
}
