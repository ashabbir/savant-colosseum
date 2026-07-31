# Operations

## Prerequisites

- Rust/Cargo to build or run from source.
- Reachable Savant Server plus `SAVANT_API_KEY`.
- Selected coding provider and Git on the service `PATH`.
- Git credentials that can push the target repository.
- macOS `script` and launchd for intended PTY/service operation.

## Common commands

```sh
cargo test
cargo clippy -- -D warnings
cargo build --release

SAVANT_API_KEY='…' ./run.sh once
SAVANT_API_KEY='…' ./run.sh worker --poll-seconds 15
SAVANT_API_KEY='…' bash install.sh

SAVANT_API_KEY='…' colosseum-runner
SAVANT_API_KEY_FILE="$HOME/.savant/colosseum/api-key" colosseum-runner
bash logs.sh status
bash logs.sh runs
bash logs.sh run <task-id-or-run-uuid>
bash logs.sh last 100
```

`runner.sh` defaults to the Colosseum workspace and `~/.savant/colosseum`.
Override `SAVANT_WORKSPACE_ID`, `SAVANT_SERVER_URL`, or
`SAVANT_EXECUTIONER_HOME` for a different deployment.

## Service control

```sh
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.savant.colosseum.plist
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.savant.colosseum.plist
launchctl kickstart -k gui/$(id -u)/com.savant.colosseum
```

`uninstall.sh` removes the service, installed binary, and managed-service API
key file; task worktrees and logs stay intact for diagnostics. Delete retained
task data only as a separate, explicit action.

## Diagnosis order

1. Confirm the Savant task is `todo`, ready, and has a valid config.
2. Read current service status and the newest JSONL log.
3. Stop stale workers; confirm the installed binary is the expected build.
4. Verify provider and Git credentials from the service environment.
5. Inspect the preserved worktree, status, and branch.
6. Identify whether the failure happened during ability resolution, validation,
   commit/push; each maps to `blocked`.
