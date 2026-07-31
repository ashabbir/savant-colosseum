# Savant Colosseum memory bank

This directory documents `savant-executioner`: the headless executioner for
Savant tasks. It reflects the current Rust implementation and should be updated
with any change to task execution, verification, publication, or operations.

## What this service does

For a task explicitly marked ready by Savant, Colosseum claims it, creates or
reuses a task-owned Git worktree, resolves the Savant engineer persona, runs the
chosen coding provider, independently validates the change, commits and pushes
the branch, records publication evidence in JSONL, and only then moves the task
to `code-review`. A failed prerequisite leaves the task `blocked`.

## Contents

- [Architecture](architecture.md): process boundaries and invariants.
- [Task execution flows](flows.md): normal and failure lifecycle.
- [Components and features](components-and-features.md): module ownership.
- [Storage and contracts](storage-and-contracts.md): APIs, state, filesystem,
  Git, and secrets.
- [Operations](operations.md): build, service, logs, and diagnosis.
- [Code conventions](code-conventions.md): implementation and validation rules.
- [Known limits](known-limits.md): gaps that require care or live evidence.

The source code is authoritative; these documents include the responsible
module names to make re-verification straightforward.
