# Managed CLI workers implementation plan

Task 1: Add durable worker registry and JSONL event contract.
File(s): `src/managed.rs`, `src/lib.rs`
What to do: persist worker metadata under the configured data directory, append lifecycle events, and cover persistence with unit tests.
Verify: `cargo test managed`.

Task 2: Make managed commands canonical while preserving aliases.
File(s): `src/main.rs`
What to do: implement `start`, `ps`, `logs`, `stop`, stable JSON failures and internal daemon execution.
Verify: `cargo test` and manual help/invalid-command smoke tests.

Task 3: Update installation and operator documentation.
File(s): `README.md`, `install.sh`, `uninstall.sh`
What to do: install `savant-colosseum` safely on macOS/Linux and document retention, compatibility, logs, and exit codes.
Verify: shell syntax checks and release build.
