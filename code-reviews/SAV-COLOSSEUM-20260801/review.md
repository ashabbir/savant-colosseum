# Code review: refactor-main-cli

Decision: **fail**

Base: `7a5fe9d`

Head: `703f1fa`

## Validation

- `cargo test --all-targets`: passed (31 tests).
- `cargo build --release`: passed.
- Release CLI smoke checks: `help`, unavailable `logs`, and invalid workspace `start`.
- Savant Context research/structure search used for the managed CLI and registry paths.

## Findings

### [blocking] Successful workers never reach the required `succeeded` state

`src/main.rs:279-324` loops forever after each `run_next` result and only transitions the registry to `Stopped` or `Failed`. `WorkerStatus::Succeeded` exists, but no managed execution path writes it or emits a terminal success event. Therefore `ps` cannot distinguish a completed worker from a running worker, violating the required running/stopped/succeeded/failed state contract. Define the worker completion condition and persist a terminal `succeeded` record/event, with a regression test that observes it through `ps` and `logs`.

### [blocking] Stop is not atomic and reports success before finalization

`src/managed.rs:226-259` reads the record without the registry lock, appends `worker.stop_requested`, sends `SIGTERM`, and returns while the registry still says `running`. A second `stop` can race and signal the same PID again; a new `start` can also observe the worker as active until the child handles the signal. If the child is stuck or dies before its handler runs, no final `worker.stopped` event is guaranteed. The stop operation needs a locked state transition/stop-request marker and a defined finalization path, plus a test for concurrent/repeated stop and unavailable workers.

### [blocking] PID-only signaling can terminate an unrelated process

`src/managed.rs:239-244,275-287` treats a numeric PID as the worker identity. After a worker exits and the PID is reused, `stop` can send `SIGTERM` to the unrelated process; `reconcile_locked` can likewise mark the worker alive. Store and verify process identity (for example, a platform-specific start time or child identity/handshake) before signaling, and test the stale/reused-PID case. This is a safety issue for a user-facing daemon manager.

### [important] Attached mode does not stream every worker event it writes

`WorkerRegistry::create_locked` writes `worker.created` at `src/managed.rs:114-121`, but attached `start` begins stdout emission only with `worker.starting` at `src/main.rs:142-152`. Thus `logs <id>` contains an event that attached stdout never streams, contrary to the requirement that attached mode streams the worker's JSONL events. Return/emit the creation event from creation, or otherwise replay the complete log before entering the loop, and add an end-to-end assertion that stdout and the worker log contain the same lifecycle events.

### [important] Invalid workspace IDs use the wrong documented exit-code class

`src/managed.rs:90-96` rejects traversal/empty IDs, but `src/main.rs:187-189` wraps the error as `LIFECYCLE`, and `error_code` maps every `LIFECYCLE:` error to exit `5`. The README/help contract says exit `3` is configuration/workspace resolution failure and exit `5` is not-found/unavailable/invalid lifecycle state. A malformed workspace identifier is therefore reported as the wrong stable exit code. Classify validation separately and add CLI assertions for empty and traversal IDs.

### [important] Help does not document all public flags

`help_event` at `src/main.rs:427-435` omits the public `--api-key` global flag and does not expose `--version`; it also lists no command flags for `logs`/`stop` beyond their positional IDs. The ticket requires help to show all commands and flags, while the implementation exposes more options through Clap. Generate this section from the Clap definition or keep it synchronized with a test covering every public option.

### [suggestion] Core lifecycle and installer behavior remains under-tested

The tests cover registry serialization and a few status cases, but do not exercise the installed binary's attached stream, terminal success, stop finalization, PID identity, stable invalid-ID exit codes, daemon detachment, or install/upgrade/uninstall/PATH behavior. These are the acceptance-critical public interfaces; add focused integration tests on macOS and Linux before accepting the implementation.

## Conclusion

The revision improves task-event forwarding, local duplicate exclusion, liveness reconciliation, and daemon secret transport, and the Rust suite/release build pass. The missing successful terminal state, non-atomic stop lifecycle, PID safety, incomplete attached stream, and contract mismatches still prevent the managed-worker acceptance criteria from being satisfied.
