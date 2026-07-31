# Components and features

## Module responsibilities

### `src/main.rs`

Provides `savant-executioner once` and `savant-executioner worker`. The worker
polls every 15 seconds by default. Raw CLI data defaults to `.savant-executioner`;
the installed runner/service explicitly uses `~/.savant/colosseum`.

### `src/savant.rs`

Defines `Task` and `SavantClient`: find next task, claim, status update, and
ability resolution. `response.rs` owns common non-success HTTP reporting and
the harmless claim-conflict result. It sends `X-App-Name: savant-server` on
every request and adds `X-API-Key` when provided.

### `src/execution/`

`execution.rs` is a small public coordinator. `lifecycle.rs` owns one task's
orchestration; `setup.rs`, `validation.rs`, and `publication.rs` own their
respective gates; `worker.rs` owns claim/error transition; `event_log.rs`
durably flushes JSONL evidence; `steps.rs` forwards phase-tagged process
events; and `types.rs` defines public runner/result types. `ExecutionOutcome`
returns run, worktree, log, and process data.

### `src/executor.rs`

Runs programs with captured stdin/stdout/stderr, timeout enforcement and
line-oriented log events. macOS calls `script -q /dev/null` to provide a PTY;
other targets use normal pipes.

### `src/worktree/`

`worktree.rs` provisions a task worktree and prevents accidental overwrite.
`locks.rs` serializes each task path in-process and `publication.rs` validates
dirty state, stages/commits/pushes changes, then verifies or creates a GitHub
PR before publishing review metadata. Worktrees intentionally remain for
review; `cleanup` is available but not part of the normal successful flow.

## Delivered behaviour

- Continuous or one-shot execution of Savant-ready tasks.
- Per-task Git isolation and restart-safe reuse.
- Five explicit noninteractive coding-provider profiles.
- Structured, retained JSONL evidence for every run.
- Independent whitespace and project-test validation.
- Branch commit/push and GitHub review metadata before review state.
- Build/install/logging support for a macOS background LaunchAgent.
- Local-Git integration coverage for worktree reuse/refusal and commit/push to
  a bare remote; GitHub review publication still needs authenticated live proof.
