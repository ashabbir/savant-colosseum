# Known limits

- Worker exclusion is local to one Colosseum data directory. It prevents
  overlapping workspace and all-workspaces workers on that host, but it is not
  a distributed lease across independently installed machines.
- All execution failures converge on `blocked`; the exact reason is in the JSONL
  log rather than a structured task failure field.
- Project validation is heuristic (`cargo test`, `npm test`, `pytest`, or diff
  check). Per-task explicit validation commands are not currently supported.
- `push` is currently informational config; publication is mandatory for review.
- Review is remote-agnostic: Colosseum requires only the configured Git remote
  to accept the branch push. It does not create PRs or comments.
- Worktrees and logs have no automatic retention policy.

Historical notes record provider-argument and review-path issues in earlier
versions. The current code enforces changed files, validation, commit, push, and
review URL before `code-review`, but provider CLI changes and publication should
always be re-proven with a controlled live task.
