# Code review: refactor-main-cli

Decision: **fail**

Base: `b0e0a5bccfa4daa22fe2771cbf6547db284db05f`

Head: `2abc9a6fa78be3dfd0b161e49c5fd0fbb89c0c6d`

## Validation performed

- `cargo test --all-targets`: 26 passed.
- `cargo build --release`: passed.
- Release binary smoke-tested for `help`, unavailable `logs`, and the daemon argument path.
- Savant Context diff analysis reported increased complexity and new medium dead-code findings in `src/main.rs`.

## Findings

### [blocking] Worker logs still omit the required task lifecycle evidence

`src/main.rs:265-292` records only high-level `task.completed`, `worker.idle`, and `worker.failed` events in the managed worker log. `ExecutionRunner::run_next` writes task/API/provider/setup/validation/publication events to the separate `RunnerConfig.log_root` tree, and those events are neither forwarded into the worker log nor streamed by attached mode. Consequently `logs <worker-id>` and attached `start` cannot provide the required creation, startup, task/work, progress, API, validation, completion, failure, and shutdown evidence. The README explicitly documents the separate per-task log tree, confirming that the worker log is not authoritative. Use one authoritative append-only log or correlate/forward all task events, with an integration test asserting the required event classes through `logs` and attached stdout.

### [blocking] Duplicate-worker prevention and registry writes are not process-safe

`src/main.rs:140-146` checks `active_for_workspace` and calls `create` as separate operations. `src/managed.rs:47-62` performs unsynchronized read/modify/write operations using a shared fixed `registry.json.tmp`. Two concurrent `start` processes can both pass the duplicate check, and concurrent writes can overwrite records or race on the temporary file. This violates the duplicate-workspace acceptance criterion and can make `ps`, `stop`, and `logs` inconsistent. Protect the check-plus-create and all registry mutations with an inter-process lock/atomic update, then add a concurrent-process regression test.

### [blocking] The `help` command does not satisfy its documented command contract

The custom `Command::Help` response in `src/main.rs:103-107` lists command names and exit codes, but does not show command flags, examples, compatibility behavior, or the detailed log locations/retention and cleanup guidance required by the ticket. The smoke output confirms this is the installed binary behavior. Either render complete help content from the CLI definition or include all required sections in the structured `data` payload and test them.

### [important] Crashed or unspawned daemon records remain permanently `running`

`WorkerRegistry::active_for_workspace` and `all` trust persisted status, while `ps` serializes the registry without checking PID liveness. A `SIGKILL`, crash, or daemon spawn failure therefore leaves a worker marked `running`; later starts are rejected and `ps` reports a dead worker as active. In particular, `start_daemon` creates the record before `spawn`, and an error from `spawn` does not transition that record to `failed`. Reconcile stale PIDs and record a terminal failure/stopped event before reporting state.

### [important] Daemon startup exposes API secrets in process arguments

`src/main.rs:177-180` forwards `cli.api_key` using `--api-key` to the detached child. That value is visible to process-list tooling and can leak to other local users or diagnostics. Pass secrets through an inherited environment/file descriptor instead, and add a test ensuring the child command line does not contain the API key.

### [important] Failure JSONL events do not populate the structured `error` field

`emit_event` always passes `None` for `error` (`src/main.rs:326-334`), while configuration and execution failures put their causes in `data` (`src/main.rs:242-251`, `282-292`). Consumers cannot classify durable worker failures through the documented stable error contract. Populate `error` with a stable code/message object and add negative-path assertions.

### [suggestion] Public lifecycle and installer behavior lacks automated coverage

The patch adds only parser/version and registry unit coverage. There are no tests for JSON schema and exit codes, duplicate starts, stale daemon reconciliation, attached event streaming, unavailable logs, stop finalization, installer upgrade/PATH handling, or uninstall behavior. These are the core public interfaces and should be covered before acceptance on supported macOS/Linux environments.

## Conclusion

The latest commit correctly adds machine-readable version output and records stop requests before signaling, and the Rust suite/build pass. However, the authoritative managed-worker log, registry concurrency/lifecycle handling, and required help contract remain incomplete, with an additional local secret exposure in daemon startup. The implementation does not satisfy the ticket acceptance criteria, so it is not ready to pass review.
