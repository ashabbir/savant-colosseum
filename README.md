# Savant Executioner

Headless Savant development-task worker. It has no UI and no benchmark mode.

It polls a workspace for `todo` tasks that contain an explicit execution block,
marks a selected task `in_progress`, creates `savant-execution/<task-id>` in an
isolated Git worktree, runs the selected coding agent, writes JSONL logs, and
then marks the task `done` or `blocked`.

```text
<!-- savant-execution
{
  "repository": "/Users/home/code/project-x/savant-server",
  "agent": { "program": "codex", "args": ["exec", "--full-auto"] },
  "revision": "HEAD",
  "setup": "npm install",
  "validate": "npm test",
  "timeout_seconds": 3600
}
-->
```

Run one task:

```sh
SAVANT_WORKSPACE_ID=<workspace-id> SAVANT_API_KEY=<key> \
  ./run.sh once
```

Or keep working:

```sh
SAVANT_WORKSPACE_ID=<workspace-id> SAVANT_API_KEY=<key> \
  ./run.sh worker --poll-seconds 15
```

`SAVANT_SERVER_URL` defaults to `http://127.0.0.1:8090` and can be overridden
for Docker or remote Savant Server deployments.

Logs are preserved at `.savant-executioner/logs/<task-id>/<run-id>.jsonl`; worktrees
are preserved at `.savant-executioner/worktrees/<task-id>` for review and handoff.
