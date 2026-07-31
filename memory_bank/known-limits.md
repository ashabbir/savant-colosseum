# Known limits

- There is no durable worker singleton/lease. Task claim protects one task, but
  stale workers can still poll other tasks; stop old workers before a release test.
- All execution failures converge on `blocked`; the exact reason is in the JSONL
  log rather than a structured task failure field.
- Project validation is heuristic (`cargo test`, `npm test`, `pytest`, or diff
  check). Per-task explicit validation commands are not currently supported.
- `push` is currently informational config; publication is mandatory for review.
- Review support is GitHub CLI only, not GitLab/MR.
- `gh pr create` failure is ignored before later comment/view commands; existing
  PR and authentication edge cases need live verification.
- Worktrees and logs have no automatic retention policy.

Historical notes record provider-argument and review-path issues in earlier
versions. The current code enforces changed files, validation, commit, push, and
review URL before `code-review`, but provider CLI changes and publication should
always be re-proven with a controlled live task.
