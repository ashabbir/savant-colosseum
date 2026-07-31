# Code conventions

## Implementation conventions

- Use Rust 2024, Tokio async functions, typed serde models, and contextual
  `anyhow` errors at external boundaries.
- Keep HTTP in `savant.rs`/`savant/response.rs`, Git operations in
  `worktree.rs`/`worktree/publication.rs`, process mechanics in `executor.rs`,
  lifecycle gates in focused `execution/` modules, and CLI setup in `main.rs`.
- Return structured `ProcessOutcome` / `ExecutionOutcome` values; do not rely on
  unstructured console logs as the execution contract.
- Model timeout directly (`timed_out` plus exit code 124), and kill child
  processes on timeout/drop.

## Executioner safety rules

- Fail closed for ability, configuration, validation, Git, and review failures.
- Never delete an unregistered worktree directory.
- Never regard provider exit 0 as sufficient proof of a reviewable task result.
- Preserve final JSONL evidence before changing the remote task status.
- Send both Savant headers; an API key alone can still be denied.

## Tests and validation

The current suite includes focused unit tests plus local-Git integration tests
for worktree reuse, refusal to overwrite, and commit/push to a bare remote.
Add deterministic tests beside the affected module and run:

```sh
cargo test
cargo clippy -- -D warnings
cargo build --release
```

For lifecycle, worktree, provider, or publication work, also perform a
controlled task run. Local remotes prove Git publication only; review-path
evidence additionally requires an authenticated real GitHub PR/comment URL
before calling it complete.

## Documentation maintenance

Update this memory bank in the same change as any task transition, endpoint,
configuration key, path, provider profile, or install-behaviour change. Separate
source-level guarantees from behaviour that has not been freshly proven live.
