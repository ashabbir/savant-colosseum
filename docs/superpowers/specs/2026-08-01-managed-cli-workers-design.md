# Managed CLI workers design

The CLI owns durable worker lifecycle state while the existing execution runner
continues to own task claiming, worktrees, validation, and publication.

Options considered:

1. Reuse the existing launchd service for each worker. This is macOS-specific
   and makes `start`, `ps`, and `stop` hard to make portable.
2. Store managed worker records in a local registry and spawn the same binary
   for daemon workers. This works on macOS and Linux and preserves the existing
   runner implementation. Chosen.
3. Add a database-backed coordinator. This would introduce a service dependency
   and is unnecessary for one-machine worker management.

The registry uses an atomic JSON replacement protected by a directory lock;
each worker has a ULID, a workspace-scoped JSONL log, PID, timestamps, and a
terminal lifecycle status. `start` rejects conflicting active scopes, invokes
the hidden managed-worker command when detached, and leaves `once`/`worker`
unchanged as compatibility aliases. All public results use the stable JSON
envelope described in the README.
