# Savant Executioner 1.0.4

Headless Savant development-task worker. It has no UI and no benchmark mode.

It polls **all workspaces** (or a specific workspace if configured) for `todo`
tasks marked **Ready for Colosseum** in Sanctum, marks a selected task
`in_progress`, creates `savant-execution/<task-id>` in an isolated Git worktree,
runs the selected installed provider with its non-interactive full-permission
profile, writes JSONL logs, performs a final `git diff --check`, and then moves
the task to `code-review` or `blocked`.

Sanctum stores the repository and provider (`hermes`, `codex`, `claude`,
`copilot`, or `agy`) in the dedicated `colosseum_tasks` server table. Colosseum
instructs the provider to determine and run the relevant project validation;
there is no task-supplied shell command to execute.

---

## Installation (macOS launchd service)

The recommended way to run Colosseum is as an always-on background service:

```sh
SAVANT_API_KEY='sk-your-key' bash install.sh
```

This will:
1. Build the release binary via `cargo build --release`
2. Install the binary to `~/.local/bin/savant-executioner`
3. Create data directories at `~/.savant/colosseum/`
4. Register a launchd agent that starts on login and restarts on crash

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `SAVANT_API_KEY` | *(required)* | API key for authenticating with Savant Server |
| `SAVANT_SERVER_URL` | `http://127.0.0.1:8090` | Savant Server URL |
| `SAVANT_WORKSPACE_ID` | *(none — scans all)* | Optional workspace filter |
| `COLOSSEUM_POLL_SECONDS` | `15` | Polling interval in worker mode |

### Service management

```sh
# Stop the service
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.savant.colosseum.plist

# Start the service
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.savant.colosseum.plist

# Restart the service
launchctl kickstart -k gui/$(id -u)/com.savant.colosseum
```

### Uninstall

```sh
bash uninstall.sh
```

---

## Viewing Logs

Use the `logs.sh` script to inspect service and execution logs:

```sh
bash logs.sh              # live-tail the service log
bash logs.sh last 50      # show last 50 lines
bash logs.sh runs         # list all execution run logs
bash logs.sh run <ID>     # view a run log by task ID or run UUID
bash logs.sh status       # show launchd service status
bash logs.sh clear        # truncate the service log
```

### Log locations

| Log | Path |
|---|---|
| Service log (stdout/stderr) | `~/.savant/colosseum.log` |
| Per-run execution logs | `~/.savant/colosseum/logs/<task-id>/<run-id>.jsonl` |
| Worktree checkouts | `~/.savant/colosseum/worktrees/<task-id>` |

---

## Manual usage

Run one task and exit:

```sh
SAVANT_API_KEY=<key> ./run.sh once
```

Keep working (foreground worker):

```sh
SAVANT_API_KEY=<key> ./run.sh worker --poll-seconds 15
```

Optionally scope to a single workspace:

```sh
SAVANT_WORKSPACE_ID=<workspace-id> SAVANT_API_KEY=<key> ./run.sh worker
```

`SAVANT_SERVER_URL` defaults to `http://127.0.0.1:8090` and can be overridden
for Docker or remote Savant Server deployments.
