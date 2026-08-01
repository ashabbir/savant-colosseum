# Code review: refactor-main-cli

Decision: **fail**

Base: `2abc9a6fa78be3dfd0b161e49c5fd0fbb89c0c6d`

Head: `08694ccbe5b8070df1982693ea738525bc790f57`

## Validation performed

- `cargo test --all-targets`: 27 passed.
- `cargo build --release`: passed.
- Release binary smoke test: `help`, unavailable `logs`, daemon `start`, and `ps` all emitted JSON; the daemon failure path was persisted as `failed`.
- Savant Context research/analyze was used for the managed-worker implementation, prior findings, and the current `src/main.rs` diff. The current diff increases `main.rs` complexity from 10 to 13 and adds no new correctness tests for the lifecycle contract.

## Findings

### [blocking] Worker logs still omit the required task lifecycle evidence

`src/main.rs:263-290` records only `task.completed`, `worker.idle`, and a high-level `worker.failed` event in the managed worker log. `ExecutionRunner::run_next` writes the task/API/provider/setup/validation/publication events to the separate `RunnerConfig.log_root` tree (`logs/<task-id>/<run-id>.jsonl`), and those events are neither forwarded into the worker log nor streamed by attached mode. Consequently `logs <worker-id>` and attached `start` cannot expose the required creation, startup, task/work, progress, API, validation, completion, failure, and shutdown lifecycle evidence. Use one authoritative append-only log or correlate/forward all task events, with an integration test asserting the required event classes through `logs` and attached stdout.

### [blocking] Duplicate-worker prevention and registry writes are not process-safe

`src/main.rs:139-146` performs `active_for_workspace` and `create` as separate operations. `src/managed.rs:47-62,87-89` performs unsynchronized read/modify/write operations through a shared fixed `registry.json.tmp`. Two concurrent `start` processes can both observe no active worker and create workers for the same workspace; concurrent writers can also race on the temporary file or overwrite records. This violates the duplicate-workspace acceptance criterion and can make `ps`, `stop`, and `logs` inconsistent. Protect the check-plus-create and every registry mutation with an inter-process lock or atomic compare/update, then add a concurrent-process regression test.

### [blocking] Daemon secrets are exposed in the process command line

`src/main.rs:188-200` forwards `cli.api_key` as `--api-key <secret>` to the detached child. On macOS/Linux, process-list tooling and local diagnostics can expose command-line arguments to other users or services. Pass the secret via an inherited environment variable, a protected file, or an inherited descriptor; add a test that the child command line contains no API key.

### [important] Crashed or unspawned daemon records can remain permanently `running`

`src/main.rs:187-215` creates and persists a `running` record before spawning the child, but does not transition it to `failed` if `spawn()` or the subsequent registry update fails. More generally, `WorkerRegistry::active_for_workspace` and `all` (`src/managed.rs:100-112`) trust persisted status without checking PID liveness or worker identity. A crash or `SIGKILL` therefore leaves a dead worker reported as running, blocks later starts for that workspace, and prevents the registry from distinguishing active and completed workers. Reconcile stale PIDs and append a terminal event before reporting `ps`; test crash and spawn-failure paths.

### [important] Durable failure events do not populate the structured `error` field

`src/main.rs:241-249` and `280-290` put causes under `data.cause`, while `emit_event` at `src/main.rs:326-334` always passes `None` for the durable event `error` field. Consumers cannot reliably classify configuration and execution failures using the documented stable error contract. Emit a stable error object with code and message for every failure event, and assert it in negative-path tests.

### [important] Worker lifecycle never records successful completion

`WorkerStatus::Succeeded` exists, but `run_managed` loops indefinitely and only transitions to `Stopped` or `Failed` (`src/main.rs:262-298`). A managed worker that finishes its intended work has no `worker.succeeded` terminal event or `succeeded` registry state. This contradicts the required status distinction for running, stopped, succeeded, and failed workers. Define the worker completion condition and persist/test the successful terminal transition.

### [suggestion] Public CLI and installer behavior is under-tested

The added test only checks part of the `help` payload. There are no automated tests for JSON schema and exit codes, duplicate starts, stale-daemon reconciliation, attached event streaming, stop finalization, unavailable logs, installer upgrade/PATH handling, or uninstall behavior. These are the core new public interfaces and should be covered on supported macOS/Linux environments before acceptance.

## Conclusion

The release build and existing Rust tests pass, and the revision improves the machine-readable help payload. However, the authoritative worker log, cross-process registry/duplicate protection, daemon secret handling, stale-worker lifecycle, structured failure events, and successful terminal state remain incomplete. The implementation does not satisfy the ticket acceptance criteria, so it is not ready to pass review.
