# Savant Colosseum · v6.0.0

Savant Colosseum is the managed worker execution engine, multi-agent pipeline orchestrator, and interactive TUI dashboard for AI coding tasks in the Savant ecosystem.

A Colosseum worker claims ready work from `savant-server`, provisions an isolated Git worktree, executes the configured provider adapter (e.g. `claude`, `codex`, `copilot`, `gemini`, `agy`), validates the work, publishes commits/MRs, and leaves durable JSONL evidence.

---

## Key Capabilities

### 1. Managed Worker Engine
* **Isolated Worktrees**: Creates and reuses task worktrees under `~/.savant/colosseum/worktrees/<task-id>`, preserving branch history across multi-attempt runs.
* **Provider Support**: Seamless execution via specialist personas (`persona.architect`, `persona.coder`, `persona.reviewer`) using `claude`, `codex`, `copilot`, `gemini`, or `agy`.
* **Bounded Repair & Review Loop**: Automatically allows one contextualized repair handback on review failure before safely escalating to human review to prevent infinite loops.
* **Pipeline-Aware Execution**: Workers spawned with `--pipeline` inherit the pipeline's agent config (provider, model, persona, prompt, pickup/working/drop locations) automatically.

### 2. Agent Configuration & Library
* **Agent Attributes**: Define granular agent specifications with:
  * `name`: Human-readable identifier.
  * `persona`: AI role (e.g., `persona.coder`, `persona.reviewer`, `persona.architect`).
  * `prompt`: Custom task or system instructions (multi-line supported).
  * `provider`: AI engine provider (`claude`, `codex`, `copilot`, `gemini`, `agy`).
  * `model`: Specific model (`claude-3-5-sonnet`, `gpt-4o`, `gemini-1.5-pro`).
  * `pickup_location`: Task status queue the agent claims work from (e.g. `ready`, `review`).
  * `working_location`: Status the task is moved to while the agent is actively working (e.g. `in-progress`).
  * `drop_location`: Status the task is set to after the agent completes (e.g. `review`, `done`).
* **Working Lock**: When a worker claims a task, it automatically transitions the task to `working_location`, locking it from concurrent pickup by other agents.
* **Immutability & Deletion Safeguards**: Agents attached to active pipelines cannot be edited directly or deleted. To customize an attached agent, **clone it** into a decoupled copy.

### 3. Multi-Stage Pipeline DAG & Validation Engine
* **DAG Flow**: Agents connect sequentially when `Agent_A.drop_location == Agent_B.pickup_location`.
* **Full Status Transition Visualization**: Each stage in a pipeline displays its complete status lifecycle:
  ```
  [Pickup: ready] ──► (Working Lock: in-progress) ──► [Drop Target: review]
       ▼
  Handoff: Task transitions to 'review' ──► Picked up by Stage 2: 'Reviewer'
  ```
* **Pipeline Validation Rules**:
  * **Duplicate Pickup Protection**: No two agents in the same pipeline may share a `pickup_location`.
  * **Shared Drop Location**: Multiple agents are permitted to drop to the same destination.
* **Execution Locking**: Running pipelines are locked against modification while active workers execute them.

### 4. Interactive TUI Dashboard (`savant-colosseum tui`)
* **60fps render loop** with non-blocking async I/O — all network fetches (workspaces, skills, gateway health) happen in background threads with zero impact on input responsiveness.
* **Header Tabs**:
  * `[1] Engine`: High-density managed workers dashboard, real-time CPU/RAM/Disk metrics, hardware topology, process controls.
  * `[2] Workspaces`: Workspace list, active task queue, and manual worker launcher.
  * `[3] Agents`: Agent Library, Pipeline DAG Visualizer, active worker execution metrics, and interactive creation/edit wizards.
  * `[4] Diagnostics`: Ecosystem intelligence, abilities, skills, knowledge graph, and context repositories.

---

## Interactive TUI Keyboard Shortcuts

### Global Navigation

| Key | Action | Description |
| :--- | :--- | :--- |
| **`1`**, **`2`**, **`3`**, **`4`** | **Select Tab** | Switch directly to `Engine`, `Workspaces`, `Agents`, or `Diagnostics`. |
| **`Tab`** | **Cycle Tabs** | Advance to next tab in order. |
| **`q`** / **`Esc`** | **Quit** | Exit the TUI. |

### Tab 3 — Agents & Pipelines

| Key | Action | Description |
| :--- | :--- | :--- |
| **`←`** / **`h`** | **Focus Agent Library** | Move keyboard focus to the left Agent Library subpanel. |
| **`→`** / **`l`** | **Focus Pipeline Visualizer** | Move keyboard focus to the right Pipeline Visualizer subpanel. |
| **`↑`** / **`k`** | **Select Previous** | Move selection up in the focused subpanel (highlighted in Cyan/Yellow). |
| **`↓`** / **`j`** | **Select Next** | Move selection down in the focused subpanel. |
| **`Enter`** | **Edit** | Open the selected agent or pipeline in full interactive Edit Mode. |
| **`a`** | **New Agent** | Open interactive 8-step Agent creation wizard. |
| **`p`** | **New Pipeline** | Open interactive Pipeline creation wizard. |
| **`C`** | **Clone Agent** | Duplicate the selected agent as `<name> Copy`. |
| **`d`** / **`Delete`** | **Delete** | Delete the selected agent or pipeline (safety-locked if in use). |

### Agent / Pipeline Wizard Controls

| Key | Field 0 (Name) | Field 1 (Agent List — Last Field) |
| :--- | :--- | :--- |
| **`Enter`** | Advance to next field | **Save & validate** the agent or pipeline |
| **`Space`** | Append space to name text | **Toggle agent selection** (`[✓ Stage N]` / `[  ]`) |
| **`↑`** / **`↓`** | Advance to next field | Navigate available agents list |
| **`Tab`** | Switch fields | Switch fields |
| **`Esc`** | Cancel / close wizard | Cancel / close wizard |

### Worker Actions (Tab 1 & 2)

| Key | Action | Description |
| :--- | :--- | :--- |
| **`s`** / **`S`** | **Start / Restart** | Launch or restart a worker on the selected workspace. |
| **`x`** / **`X`** | **Stop / Kill** | Gracefully terminate or force-kill a running worker. |
| **`d`** / **`D`** | **Purge Record** | Delete worker record and associated logs. |
| **`Enter`** | **Inspect** | Open detailed worker log inspector. |
| **`c`** | **Copy** | Copy selected worker ID or asset to clipboard. |

---

## Installation and Setup

### Prerequisites
macOS or Linux with a Rust toolchain (`cargo`), and a reachable `savant-server` (default `http://127.0.0.1:8090`).

```sh
# Build and install to ~/.local/bin/savant-colosseum
bash install.sh

# Launch TUI dashboard
savant-colosseum tui
```

---

## CLI Command Reference

```sh
# Show help & JSON CLI contract
savant-colosseum help

# Launch interactive terminal UI dashboard
savant-colosseum tui

# Start worker for a specific workspace
savant-colosseum start --workspace <WORKSPACE_ID>

# Start worker in background (daemon mode)
savant-colosseum start --workspace <WORKSPACE_ID> --daemon

# Start worker bound to a specific pipeline
savant-colosseum start --workspace <WORKSPACE_ID> --pipeline <PIPELINE_NAME>

# List running and retained worker records
savant-colosseum ps

# Stream worker JSONL lifecycle logs
savant-colosseum logs <WORKER_ID>

# Gracefully stop a running worker
savant-colosseum stop <WORKER_ID>

# Agent CLI subcommands
savant-colosseum agent list
savant-colosseum agent add --id review-x --name "Review X" --pickup-location "ready" --working-location "in-progress" --drop-location "review"
savant-colosseum agent clone --source-id review-x --new-id review-x-v2 --new-name "Review X V2"

# Pipeline CLI subcommands
savant-colosseum pipeline list
savant-colosseum pipeline add --id pipe-1 --name "Quality Suite" --agent-ids "agent-coder,agent-reviewer"
savant-colosseum pipeline validate --id pipe-1
```

---

## Contract & Storage Locations

### JSON Lifecycle Stream
Attached workers stream single-line JSON objects on `stdout`:

```json
{
  "timestamp": "2026-08-03T00:00:00Z",
  "event": "worker.starting",
  "worker_id": "01J...",
  "workspace_id": "2539163563543949210",
  "status": "running",
  "message": "attached worker starting",
  "data": null,
  "error": null
}
```

### Storage Paths
* **Agent & Pipeline Registry**: `~/.savant/colosseum/pipelines.json`
* **Worker Logs & Registry**: `~/.savant/colosseum/workers/<workspace-id>/<worker-id>/events.jsonl`
* **Per-Task Execution Evidence**: `~/.savant/colosseum/logs/<task-id>/<run-id>.jsonl`
* **Worktree Checkouts**: `~/.savant/colosseum/worktrees/<task-id>`

### Exit Codes
* `0`: Success
* `1`: Execution/worker failure
* `2`: Invalid command or argument
* `3`: Configuration/workspace resolution failure
* `4`: API/network/service dependency failure
* `5`: Unknown, unavailable, or already-stopped worker

---

## Performance Architecture (v6.0.0)

| Subsystem | Interval | Mechanism |
| :--- | :--- | :--- |
| Worker CPU/RAM metrics | 500ms | `sysinfo::refresh_processes` — process-scoped only |
| Workspace & task list | 500ms | Non-blocking fire-and-forget tokio spawn + `try_recv` |
| Gateway health | 500ms | Non-blocking fire-and-forget tokio spawn + `try_recv` |
| Skills & abilities scan | 5s | Throttled slow tick |
| Disk I/O stats | 5s | Throttled slow tick |
| TUI render framerate | ~60fps (16ms) | Immediate key-drain loop per tick |

All HTTP calls (workspaces, skills, gateway health) run in background tokio tasks. Results are collected via non-blocking `try_recv()` on each frame. The main event loop never blocks on network I/O.
