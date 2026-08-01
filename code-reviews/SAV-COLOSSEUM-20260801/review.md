# Code review: refactor-main-cli

Decision: **fail**

Reviewed HEAD `8454d8633e4df5533a6ebc9a4c589b4302e1a967` against base `fd655dd932a9fcd251d4e16c808e57155986a44b`. The Rust test suite, formatting, Clippy, release build, shell syntax checks, and installer regression script pass, but the managed-worker lifecycle still misses blocking acceptance criteria.

## Findings

### [blocking] Stop is not atomic and can report success before finalization

`src/managed.rs:226-259` performs `get`, appends `worker.stop_requested`, signals the stored PID, and only finalizes the record in the unavailable branch. For a live process, `stop` returns the still-running record and leaves final `worker.stopped` logging to a separate worker process. Concurrent `stop` calls can both observe `Running`, append duplicate requests, and signal the same process. If the worker crashes, is killed, or fails before handling SIGTERM, the CLI invocation that requested the stop has no guaranteed final outcome. This violates the required graceful-stop/final-outcome contract and stable already-stopped behavior. The read/check/update/signal operation needs an explicit lifecycle state or an atomic stop transition, with the worker-owned finalization guarded against races.

### [blocking] Stop and liveness use an unsafe PID-only identity

`src/managed.rs:190-197` and `:239-243` use `kill -0`/`kill -TERM` against the numeric PID without validating that the process is the worker recorded for this worker ID. After a daemon exits and its PID is reused, `ps` can classify the old record as live and `stop <old-id>` can terminate an unrelated process. A durable worker ID does not make a PID durable. The registry needs a process identity check (for example, a start-time token captured at spawn and verified before liveness/signaling), or an equivalent ownership mechanism.

### [blocking] Attached start does not stream the complete worker event stream

`src/main.rs:142-152` creates and logs `worker.created` through `registry.event`, then emits only `worker.starting` to stdout. The attached contract says the worker's JSONL events are streamed to stdout; therefore the first lifecycle event in the log is missing from attached stdout. This is observable by comparing `start` output with `logs <id>` and also means creation is not present in the attached machine-readable stream. The creation event should be emitted at creation time (or the attached path should replay/forward it consistently).

### [blocking] Managed workers never transition to `succeeded`

`WorkerStatus` includes `Succeeded`, but `src/main.rs:246-325` only updates workers to `Failed` or `Stopped`; successful task execution leaves the registry `Running` indefinitely. `ps` therefore cannot distinguish a successfully completed worker from an active one, and the required `running`, `stopped`, `succeeded`, and `failed` states are not all reachable through the managed CLI. The worker lifecycle needs an explicit successful terminal path and corresponding JSONL event, with tests covering it.

## Validation performed

- `cargo test --all-targets`: passed (31 tests)
- `cargo fmt --check`: passed
- `cargo clippy --all-targets -- -D warnings`: passed
- `cargo build --release`: passed
- `bash -n install.sh uninstall.sh`: passed
- `bash tests/install.sh`: passed, including failed atomic replacement preserving the old binary
- `git diff --check`: passed

## Recommendation

Do not merge until stop ownership/identity and finalization are made race-safe, attached output includes every worker event, and a successful terminal transition is implemented and tested. The installer changes in this HEAD are reasonable and the validation gates are green, but they do not compensate for the lifecycle contract failures.
