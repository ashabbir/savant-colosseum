# Savant Colosseum

Savant Colosseum is the headless managed-worker executor for opted-in Savant tasks. A worker claims ready work, creates its isolated Git worktree, runs the configured provider, validates and publishes the result, and leaves durable JSONL evidence. The public CLI is `savant-colosseum`; `once` and `worker` remain compatibility aliases for existing automation.

## Prerequisites and installation

macOS and Linux are supported. Install a Rust toolchain to build from this checkout, configure a reachable Savant Server, and supply `SAVANT_API_KEY` (or `SAVANT_API_KEY_FILE`) when executing work.

```sh
bash install.sh
savant-colosseum help
```

The installer builds the release binary and safely replaces `~/.local/bin/savant-colosseum` (override with `SAVANT_COLOSSEUM_BIN_DIR`). It reports the installed version and the PATH action if needed. Upgrade by running it again. Uninstall the binary with `bash uninstall.sh`; registry records and logs are intentionally retained. Remove the data directory manually only when that history is no longer needed.

## Commands

```sh
savant-colosseum help
savant-colosseum start --workspace 2539163563543949210
savant-colosseum start --workspace 2539163563543949210 --daemon
savant-colosseum ps
savant-colosseum logs 01J...
savant-colosseum stop 01J...

# compatibility aliases
savant-colosseum once
savant-colosseum worker --poll-seconds 15
```

`start` creates a ULID worker ID. Attached workers stream JSONL lifecycle events to stdout until stopped; daemon workers detach and can be inspected using `ps`, `logs`, and `stop`. Only one active managed worker may target a given workspace (including the all-workspaces scope). `logs` prints the complete JSONL file. `stop` sends a graceful termination request and records it before signaling the worker.

## JSON contract and locations

Every CLI result/event is one JSON object on stdout with `timestamp` (RFC 3339 UTC), `event`, `worker_id`, `workspace_id`, `status`, `message`, `data`, and `error`. Stderr is reserved for diagnostics. Worker events are append-only JSONL at:

```text
~/.savant/colosseum/workers/<workspace-id>/<worker-id>/events.jsonl
~/.savant/colosseum/workers/registry.json
```

Override the root with `SAVANT_EXECUTIONER_HOME`. Completed records and logs are retained by default; there is no automatic deletion. Per-task execution evidence remains under `~/.savant/colosseum/logs/` and task worktrees under `~/.savant/colosseum/worktrees/`.

Exit codes: `0` success, `1` execution/worker failure, `2` invalid command or argument, `3` configuration/workspace resolution failure, `4` API/network/service dependency failure, and `5` unknown/unavailable/already-stopped worker.

## Diagnosing workers

Run `ps` to get a worker ID and its log path. Use `logs <id>` to read each lifecycle event, including configuration load, idle polling, task completion, failure, and shutdown. A missing log or invalid worker ID produces a JSON failure with exit code `5`. If a worker fails while calling Savant, its final JSONL event retains the cause.

Legacy service and execution logs remain available through the helper:

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

## Continuation and review contract

Colosseum treats a task worktree as durable state. When the expected registered
worktree or a legacy registered `<task-id>-N` worktree already exists, the next
agent resumes its branch, files, commits, and uncommitted changes. Colosseum
never creates a bypass worktree over an unknown existing directory; ambiguous
state fails closed for human inspection.

Every architect, coder, and reviewer receives the same structured task dossier:
ticket requirements, phase configuration, dependencies, substantive activity,
complete run/decision history, publication metadata, worktree/branch identity,
and the stable whole-MR base-to-HEAD range. Coding runs must preserve prior work
and incorporate every valid review finding. Reviews inspect the complete MR,
verify the base branch is contained in HEAD, and publish actionable structured
findings.

A failed independent review permits one automatic, fully contextualized repair
handback. If the complete second review still fails, Colosseum stops automatic
execution at human review instead of cycling indefinitely. A failed work run is
blocked and also requires explicit intervention.
