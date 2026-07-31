# Architecture

## System boundary

Colosseum is not a UI, benchmark arena, scheduler database, or coding provider.
It is a Rust task executioner between Savant Server, a checked-out Git repository,
installed coding agents, and the repository remote.

```text
Savant ready task + engineer ability API
                 |
                 v
      savant-executioner (CLI / launchd worker)
          | worktree             | JSONL evidence
          v                      v
 Git checkout + remote   ~/.savant/colosseum/logs
          |
          v
 provider -> validation -> commit/push -> JSONL evidence -> task status
```

## Components

| Module | Owns |
| --- | --- |
| `src/main.rs` | CLI flags/env, `once` and polling `worker` modes. |
| `src/savant.rs` | Task/ability HTTP contract and Savant headers. |
| `src/execution.rs` | Task policy: resolve ability, execute, validate, publish, transition status. |
| `src/executor.rs` | Timed child process execution, macOS PTY, stdout/stderr capture. |
| `src/worktree.rs` | Isolated Git worktrees and verified commit/push. |
| Shell scripts and plist | Build/install a macOS LaunchAgent and inspect retained logs. |

## Executioner invariants

- Only `colosseum_ready` tasks can execute.
- Each task maps to `<data-dir>/worktrees/<task-id>` and branch
  `savant-execution/<task-id>`.
- `persona.engineer` ability resolution is mandatory before the provider runs.
- Provider success is insufficient: Colosseum runs independent validation.
- `code-review` requires changed files, successful commit, successful remote
  push, and retained JSONL commit/remote evidence.
- A claimed task encountering an error is transitioned to `blocked`, never
  deliberately left `in_progress`.

## Concurrency and recovery

The server claim call arbitrates competing workers; HTTP 409 means another
worker won. A keyed async mutex also serializes worktree creation in one process.
Registered task worktrees are safely reused after restart. An existing directory
not registered as a Git worktree is refused rather than overwritten.

Launchd uses `RunAtLoad` and `KeepAlive`, but it does not prevent separately
installed stale workers from polling; stop old workers before a release test.
