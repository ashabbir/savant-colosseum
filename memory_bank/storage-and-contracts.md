# Storage and external contracts

## Savant task configuration

Colosseum consumes a `todo` task that is marked `colosseum_ready` and has a
JSON `colosseum_config` shaped like:

```json
{
  "repository": "/absolute/path/to/repository",
  "provider": "codex | claude | copilot | hermes | agy",
  "revision": "HEAD",
  "setup": "optional shell command",
  "timeout_seconds": 3600,
  "push": false
}
```

`repository` and `provider` are required. Revision defaults to `HEAD` and timeout
to 3600 seconds. `push` is parsed but current behaviour always pushes as the
required route to `code-review`.

## Server API contract

| Purpose | Method and path |
| --- | --- |
| Find work | `GET /api/tasks/colosseum/next?status=todo[&workspace_id=...]` |
| Claim | `POST /api/tasks/{task_id}/claim` |
| State update | `PUT /api/tasks/{task_id}` with `{ "status": "..." }` |
| Engineer instructions | `POST /api/abilities/resolve` with persona, tags, repo, and trace |

Protected Savant deployments need both `X-API-Key` and `X-App-Name:
savant-server`.

## Persisted local state

| Item | Installed location | Meaning |
| --- | --- | --- |
| Service output | `~/.savant/colosseum.log` | launchd stdout/stderr. |
| Per-run logs | `~/.savant/colosseum/logs/<task-id>/<run-id>.jsonl` | lifecycle and streamed output. |
| Worktrees | `~/.savant/colosseum/worktrees/<task-id>` | retained task checkout. |
| Binary | `~/.local/bin/savant-executioner` | release build copied by install. |
| LaunchAgent | `~/Library/LaunchAgents/com.savant.colosseum.plist` | generated worker service without an inline API key. |
| API key file | `~/.savant/colosseum/api-key` | mode-600 credential loaded by the managed service. |

JSONL includes `started`, `abilities-resolved`, streamed `log`, and `finished`
events, giving reviewers durable evidence without a Colosseum database.

## Git and secrets

The target repository needs a valid `origin` remote. Branches use
`savant-execution/<task-id>`. Publication runs `git add -A`, commit, and
`git push -u origin <branch>`, then records the commit and remote in the
run's JSONL file. Commit or push failures prevent review state.

`SAVANT_API_KEY` is supplied for interactive runs. The managed service uses
`SAVANT_API_KEY_FILE` and a mode-600 file rather than persisting the key in its
LaunchAgent plist. Never commit either secret source or emit it in task logs.
