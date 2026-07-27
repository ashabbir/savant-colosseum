# Savant Colosseum

Headless multi-agent benchmark runner for executing the same coding scenario
against multiple CLI agents in isolated Git worktrees.

## What works

- JSON scenario validation
- identical start-commit resolution
- concurrent contender execution
- isolated `colosseum/<scenario>/<agent>` branches and worktrees
- prompt delivery over stdin
- streamed JSON events and per-contender JSONL logs
- validation commands executed inside each worktree
- duration, token, cost, test, lint, and Git-change metrics
- SQLite battle/result persistence
- machine-readable `run`, `list`, and `show` commands

## Run

The repository uses the same pinned Rust channel as Vibe Kanban.

```sh
cargo run -- run examples/scenario.json
cargo run -- list
cargo run -- show <battle-id>
```

Use `--data-dir` and `--worktree-dir` to override the default local paths.
Each contender is an executable plus an argument array; Colosseum does not
shell-concatenate agent commands. The scenario prompt is written to stdin.

## Scenario

See [`examples/scenario.json`](examples/scenario.json). At least two contenders
are required. The validation command is intentionally a shell command because
scenarios need to run native repository test commands; it always runs with the
contender worktree as its current directory.

## Verify

```sh
cargo fmt -- --check
cargo test
```

The end-to-end suite creates a real temporary Git repository with a known bug,
runs two deterministic agents, validates worktree isolation and branch naming,
and reads the persisted SQLite results.

