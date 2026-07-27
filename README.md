# Savant Executioner

Headless Savant development-task worker. It has no UI and no benchmark mode.

It polls a workspace for `todo` tasks marked **Ready for Colosseum** in Sanctum,
marks a selected task `in_progress`, creates `savant-execution/<task-id>` in an
isolated Git worktree, runs the selected installed provider with its
non-interactive full-permission profile, writes JSONL logs, performs a final
`git diff --check`, and then moves the task to `code-review` or `blocked`.

Sanctum stores the repository and provider (`hermes`, `codex`, `claude`,
`copilot`, or `agy`) in the dedicated `colosseum_tasks` server table. Colosseum
instructs the provider to determine and run the relevant project validation;
there is no task-supplied shell command to execute.

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

For the installed worker, `runner.sh` defaults to the `savant-colosseum`
workspace and keeps all worktrees and JSONL logs in
`/Users/home/.savant/colosseum`:

```sh
SAVANT_API_KEY='your-savant-api-key' colosseum-runner
```

`SAVANT_SERVER_URL` defaults to `http://127.0.0.1:8090` and can be overridden
for Docker or remote Savant Server deployments.

Logs are preserved at `.savant-executioner/logs/<task-id>/<run-id>.jsonl`; worktrees
are preserved at `.savant-executioner/worktrees/<task-id>` for review and handoff.
