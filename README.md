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
