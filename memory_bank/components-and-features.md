# Components and features

## Module responsibilities

### `src/main.rs`

Provides `savant-executioner once` and `savant-executioner worker`. The worker
polls every 15 seconds by default. Raw CLI data defaults to `.savant-executioner`;
the installed runner/service explicitly uses `~/.savant/colosseum`.

### `src/savant.rs`

Defines `Task` and `SavantClient`: find next task, claim, status update, and
ability resolution. It sends `X-App-Name: savant-server` on every request and
adds `X-API-Key` when provided.

### `src/execution.rs`

The execution policy centre. It owns JSONL writer setup, engineer-prompt
composition, setup/agent/validation ordering, publication gates, and final
task status. `ExecutionOutcome` returns run, worktree, log, and process data.

### `src/executor.rs`

Runs programs with captured stdin/stdout/stderr, timeout enforcement and
line-oriented log events. macOS calls `script -q /dev/null` to provide a PTY;
other targets use normal pipes.

### `src/worktree.rs`

Provisions a task worktree and prevents accidental overwrite. It also validates
dirty state, stages/commits/pushes changes, and invokes `gh` for GitHub review
metadata. Worktrees intentionally remain for review; `cleanup` is available but
not part of the normal successful flow.

## Delivered behaviour

- Continuous or one-shot execution of Savant-ready tasks.
- Per-task Git isolation and restart-safe reuse.
- Five explicit noninteractive coding-provider profiles.
- Structured, retained JSONL evidence for every run.
- Independent whitespace and project-test validation.
- Branch commit/push and GitHub review metadata before review state.
- Build/install/logging support for a macOS background LaunchAgent.
