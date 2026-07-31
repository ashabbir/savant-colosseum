# Savant task execution flows

## Normal lifecycle

```text
todo + colosseum_ready
  -> GET next -> POST claim -> in_progress
  -> decode config -> provision/reuse task worktree
  -> resolve persona.engineer
  -> optional setup -> coding provider
  -> git diff --check -> project validation
  -> git add/commit/push -> JSONL commit/remote evidence
  -> code-review
```

`ExecutionRunner::run_next` owns claim and execution. Its error path performs a
best-effort `blocked` update for a task already claimed.

## Detailed flow

1. `next_colosseum_task` fetches a ready `todo` task, optionally restricted to a
   workspace; no `task_id` means idle.
2. `claim` atomically moves the task into execution. A 409 conflict is harmless.
3. `ExecutionSpec::from_task` rejects a task that is not ready and decodes its
   Colosseum configuration.
4. `provision_task` resolves the requested Git revision and creates/reuses the
   stable task worktree and branch.
5. The repository directory name is sent to Savant ability resolution with
   `persona.engineer` and engineering/execution/code-review tags. The returned
   prompt is included in the provider task prompt.
6. Optional setup runs through `/bin/sh -lc`; failure or timeout blocks the task.
7. The provider runs in the worktree through a PTY on macOS, with the task prompt
   on stdin and both streams captured.
8. Successful provider exit triggers `git diff --check`, then one default project
   test: `cargo test`, `npm test`, `pytest`, or a fallback diff check.
9. Verified work must be non-empty. Colosseum stages everything, commits
   `colosseum: <title>`, pushes the task branch, and records the commit and
   remote in JSONL.
10. Only the complete publication tuple produces `code-review`; all other
    completed attempts are `blocked`.

## Failure outcomes

| Failure | Result |
| --- | --- |
| No task / claim conflict | Idle, no task transition. |
| Invalid config, ability failure, worktree error, provider launch error | Claimed task becomes `blocked`. |
| Setup, provider, or validation failure/timeout | No publication; task becomes `blocked`. |
| No changes or commit/push failure | No `code-review`; task becomes `blocked`. |

## Provider profiles

| Provider | Program arguments |
| --- | --- |
| `codex` | `exec --dangerously-bypass-approvals-and-sandbox` |
| `claude` | `-p --dangerously-skip-permissions` |
| `copilot` | `-p --allow-all-tools` |
| `hermes` | `--yes` |
| `agy` | `--dangerously-skip-permissions --print` |

Unknown providers fail explicitly. Provider argument ordering is operationally
significant, particularly for Agy.
