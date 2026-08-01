# Code review: refactor-main-cli

Decision: **fail**

Base: `6e1bfd216d9d866874a120204d0f2459a4d67df9`

Head: `b0e0a5bccfa4daa22fe2771cbf6547db284db05f`

## Validation performed

- `cargo test --all-targets`: 22 passed.
- `cargo build --release`: passed.
- Release binary smoke-tested for `help`, daemon start, `ps`, `logs`, and an unavailable/already-failed `stop`.
- Savant Context diff analysis reported the CLI change increased complexity from 10 to 44 and added medium dead-code findings plus long-line findings.

## Findings

### [blocking] Worker logs do not contain the required per-task lifecycle events

`src/main.rs:265-292` records only `task.completed`, `worker.idle`, and a high-level `worker.failed` event in the managed worker log. `ExecutionRunner::run_next` writes the actual task lifecycle/API/provider/validation events to the separate `RunnerConfig.log_root` tree (`logs/<task-id>/<run-id>.jsonl`), and those events are neither copied nor forwarded into the worker log or attached stdout. Consequently `logs <worker-id>` cannot show the required creation, task/work, progress, API, validation, completion, and failure evidence, and attached mode does not stream the worker's actual JSONL execution events. The README also documents the separate per-task evidence, confirming the two streams are split. The worker log needs a correlation/forwarding mechanism (or a single authoritative append-only log) and tests proving the required event classes appear through `logs` and attached stdout.

### [blocking] Registry updates are not safe across concurrent CLI processes

`src/managed.rs:47-62` implements read/modify/write through one shared `registry.json` and a fixed `registry.json.tmp` without an inter-process lock. `start` checks for an existing worker at `src/main.rs:140-146` and creates later; two concurrent starts for the same workspace can both observe no active worker and both become active. Concurrent writes can also race on the fixed temporary file or lose records. This violates the duplicate-worker acceptance criterion and can make `ps`, `stop`, and `logs` inconsistent. Use an atomic lock/lease around the check-plus-create and all registry mutations, with a test that starts two processes concurrently.

### [important] Dead daemon processes remain permanently `running`

`WorkerRegistry::active_for_workspace` and `all` trust the persisted status (`src/managed.rs:109-112`), and `ps` simply serializes that registry (`src/main.rs:112-115`). If a daemon is killed with `SIGKILL`, crashes outside the handled error path, or is otherwise reaped without writing its final event, the registry retains `running` and its PID forever. Subsequent starts are rejected as duplicates and `ps` does not distinguish the dead worker from an active one, contrary to the required lifecycle/status behavior. Reconcile persisted running records using PID liveness/worker identity and append a terminal failure/stopped event before reporting state.

### [important] Stop request ordering does not match the documented contract

The implementation sends `SIGTERM` in `src/managed.rs:156-169`, then appends `worker.stop_requested` in `src/main.rs:124-133`. The README claims the request is recorded before signaling, and the acceptance criteria require the stop request and final outcome to be recorded. The worker can append `worker.stopped` between the signal and the later stop-request write, producing an incorrectly ordered log; a process that exits immediately can also leave the request absent if the follow-up write fails. Append/flush the request first, then signal, and test event ordering.

### [important] Failure events do not use the structured `error` field

`emit_event` always passes `None` for `error` (`src/main.rs:326-334`), while configuration and execution failures put the cause in `data` (`src/main.rs:242-251`, `282-292`). The CLI error envelope has a structured error, but the durable worker JSONL failure records do not. Consumers cannot reliably classify worker failures using the documented stable error contract. Populate `error` with a stable code/message object and add negative-path assertions.

### [suggestion] CLI and installer acceptance paths lack automated coverage

The patch adds only two `main.rs` unit tests and two registry tests. There are no tests for argument parsing/exit codes, duplicate starts, stop ordering, daemon detachment/reconciliation, JSON schema, log-unavailable behavior, installer upgrade/PATH handling, or uninstall behavior. These are the core new public interfaces and should be covered before acceptance, including macOS/Linux shell checks where practical.

## Conclusion

The implementation compiles and the existing Rust suite passes, but the authoritative managed-worker log is incomplete and registry coordination cannot guarantee the central duplicate-worker/lifecycle contract. These are acceptance-level defects, so this change is not ready to pass review.
