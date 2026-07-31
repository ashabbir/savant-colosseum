use anyhow::Result;

use super::{ExecutionOutcome, ExecutionRunner};

pub(super) async fn run_next(
    runner: &ExecutionRunner,
    workspace_id: Option<&str>,
) -> Result<Option<ExecutionOutcome>> {
    let Some(task) = runner.claim_next(workspace_id).await? else {
        return Ok(None);
    };
    match runner.execute_task(task.clone()).await {
        Ok(outcome) => Ok(Some(outcome)),
        Err(error) => {
            runner
                .savant
                .update_status(&task.task_id, "blocked")
                .await?;
            Err(error.context(format!("execution for task {} was blocked", task.task_id)))
        }
    }
}
