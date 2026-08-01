# Sanctum Roadmap PRD

## 1. Product Summary

Savant Sanctum is a new Electron desktop application for AI-session-centric work management. It unifies user-scoped workspaces, sessions, tasks, reminders, notes, files, Jira tickets, GitLab merge requests, provider configuration, and local AI tool setup into one desktop experience.

Sanctum is designed to use both:

- `API` for app UI, persistence, and server-backed domain data
- `MCP` for agent-facing tool access to the same domain

The product replaces the prototype-era monolith with a cleaner app boundary and a more deliberate information architecture, while preserving the useful workflow depth that exists today.

## 2. Product Goals

- Provide a clear, user-scoped workspace and session model.
- Make sessions the center of work, not just logs.
- Surface notes, files, Jira tickets, and merge requests directly within sessions and workspaces.
- Support both human UI flows and agent/tool flows through the same underlying model.
- Preserve local install/config capability for AI providers and local tooling.
- Keep the app fast, discoverable, and maintainable by separating core domain, integrations, and local setup.

## 3. Non-Goals

- Building a generic multi-tenant collaboration platform.
- Owning server-side business logic that already belongs in `savant-server`.
- Replacing `savant-server` with local-only storage.
- Rewriting the prototype wholesale without reducing scope.
- Making MCP the only interaction model.

## 4. Product Principles

1. User-scoped first. Everything belongs to the active user boundary.
2. API and MCP are both first-class.
3. Server truth wins for shared data and linkage.
4. Local setup is separate from core domain data.
5. Sessions are work containers, not just conversation logs.
6. The UI should surface relationships, not hide them behind tabs.
7. Reuse what is still valid, but rebuild the shell and contracts where the prototype is too coupled.

## 5. Core Domain Model

### 5.1 Primary Entities

- `User`
- `Workspace`
- `Session`
- `Task`
- `Reminder`

### 5.2 Session-Attached Entities

- `Note`
- `File`
- `JiraTicket`
- `MergeRequest`
- `ConversationEvent`
- `Metadata`
- `Stats`
- `Tree`
- `Checkpoint`
- `ResearchArtifact`

### 5.3 Support Entities

- `Provider`
- `ProviderConfig`
- `MCPServerConfig`
- `Skill`
- `Tool`
- `Notification`
- `SyncQueueItem`
- `SystemStatus`

## 6. Relationship Model

### 6.1 Ownership and Scope

- Every entity is user-scoped.
- Workspaces are owned by a user.
- Sessions are owned by a user and linked to a workspace.
- Tasks are owned by a user and usually organized under a workspace.
- Reminders are owned by a user and may be workspace-relevant.
- Notes, files, Jira tickets, and merge requests are attached to sessions and visible in workspace rollups.

### 6.2 Key Relationships

- `User -> Workspaces`
- `Workspace -> Sessions`
- `Workspace -> Tasks`
- `Workspace -> Reminders`
- `Session -> Notes`
- `Session -> Files`
- `Session -> JiraTickets`
- `Session -> MergeRequests`
- `Session -> Metadata`
- `Session -> Stats`
- `Session -> Tree`
- `Session -> Checkpoints`
- `Session -> ResearchArtifacts`
- `Session -> WorkspaceLink`

### 6.3 Source of Truth

- Workspace-session linkage is stored in `savant-server`.
- Shared workspace/task/note/Jira/MR data is server-backed.
- Local session files and provider session roots remain the source for raw session discovery and file-level inspection.
- Local preferences, install state, and outbox state remain local.

## 7. System Architecture

### 7.1 Layers

1. Electron shell
2. Renderer UI
3. Local services
4. Server API
5. MCP tool surface
6. Local config/install surface

### 7.2 Interaction Paths

- UI reads and writes domain data through the API.
- Agents read and write domain data through MCP.
- The desktop app configures local agent MCP files when the user enables or changes provider setup.
- The desktop app installs local AI features like skills and tools.

### 7.3 Architectural Constraint

Sanctum should not collapse API and MCP into one abstraction. They share the same domain model, but they are different surfaces with different consumers.

## 8. Feature Inventory

### 8.1 Core Work Management

- Workspace creation, editing, deletion
- Session listing and detail
- Session assignment to workspace
- Task creation, editing, status changes, prioritization
- Reminder creation, editing, completion, dismissal

### 8.2 Session Experience

- Session discovery across providers
- Session cache for fast rendering
- Session detail timeline
- Session rename
- Session star/unstar
- Session archive/unarchive
- Session delete
- Session notes
- Session files
- Session-linked Jira tickets
- Session-linked merge requests
- Session metadata editing
- Session stats and usage summaries
- Session tree browsing
- Checkpoints and research artifacts

### 8.3 Workspace Experience

- Workspace overview
- Workspace session list
- Workspace task list
- Workspace reminder list
- Workspace note aggregation
- Workspace file aggregation
- Workspace Jira list
- Workspace merge request list
- Workspace summary and counts

### 8.4 Provider and Session Adapter System

- Provider adapter pattern
- Copilot adapter
- Claude adapter
- Codex adapter
- Gemini adapter
- Savant adapter
- Provider registry
- Canonical session normalization
- Provider-specific roots and file formats
- Provider metadata overlays

### 8.5 API Surface

- Health checks
- Auth validation
- Workspace endpoints
- Session-link endpoints
- Task endpoints
- Reminder endpoints
- Notes endpoints
- Jira endpoints
- Merge request endpoints
- Knowledge/context/abilities endpoints where applicable

### 8.6 MCP Surface

- `savant-workspace`
- `savant-abilities`
- `savant-context`
- `savant-knowledge`
- `savant-reminders`
- Workspace/session/task/note/Jira/MR tools
- Knowledge graph tools
- Ability/prompt asset tools
- Context/code search tools

### 8.7 Local Setup and Install

- Install provider-specific MCP config entries
- Detect installed provider configs
- Add SSE or stdio MCP entries as needed
- Install local skills
- Install local tools
- Patch provider configs safely
- Keep credentials out of config files when possible

### 8.8 Preferences and Settings

- Auth and profile
- Server URL
- API key
- Provider enablement
- MCP setup status
- Skills preferences
- Tools preferences
- Theme/layout preferences
- Terminal preferences
- Sync/offline preferences
- Diagnostics/system info

### 8.9 Notifications and Feedback

- Toast notifications
- Success and error banners
- Sync status indicators
- Server online/offline indicators
- MCP setup feedback
- Install feedback

### 8.10 Terminal and Local Execution

- Embedded terminal
- PTY bridge
- Open external terminal
- Run in new tab
- Run in fresh tab
- Resize and drawer behavior

### 8.11 Knowledge and Analysis

- Knowledge graph browsing
- Search
- Edit
- Commit/uncommit
- Link/unlink workspace
- Prune
- Aggregate workspace context
- AST/code analysis
- Repository and file navigation

## 9. MVP Scope

The first Sanctum release should include:

- User auth and profile
- Server configuration
- Workspace list and workspace detail
- Session list and session detail
- Session-to-workspace assignment
- Notes
- Files
- Tasks
- Reminders
- Jira ticket links
- Merge request links
- Provider adapter-based session discovery
- Local provider config setup
- Local skills/tools install surface
- API and MCP support for the shared domain
- Toast notifications and sync/status feedback

## 10. Phase Plan

### Phase 0: Discovery and Contract Freeze

Deliverables:

- final entity list
- relationship matrix
- API vs MCP ownership matrix
- local vs server truth matrix
- prototype feature inventory

Exit criteria:

- all current prototype features are classified
- all core entities have a target home

### Phase 1: App Scaffold

Deliverables:

- new `savant-sanctum` app scaffold
- shell and styling aligned to Olympus/Quorum family language
- local persistence baseline
- settings and auth foundation

Exit criteria:

- app launches cleanly
- server auth works
- basic navigation exists

### Phase 2: Core Domain

Deliverables:

- workspace model
- session model
- task model
- reminder model
- notes
- files
- session linkage

Exit criteria:

- workspaces and sessions are navigable
- session detail shows linked artifacts

### Phase 3: Integration Surfaces

Deliverables:

- API integrations
- MCP integrations
- provider configs
- local install/setup flows
- notifications

Exit criteria:

- same data can be accessed from UI and MCP
- local setup works on a clean machine

### Phase 4: Deep Session Features

Deliverables:

- stats
- trees
- checkpoints
- research artifacts
- Jira/MR workflows
- terminal integration

Exit criteria:

- session detail becomes a real work cockpit

### Phase 5: Knowledge and Power Features

Deliverables:

- knowledge graph
- context aggregation
- abilities
- provider/tool management
- diagnostics

Exit criteria:

- Sanctum can support advanced agent workflows without leaving the app

## 11. Detailed Roadmap

### 11.1 Workspace Roadmap

1. Create workspace list and detail pages.
2. Read workspace-session linkage from server.
3. Render session summaries per workspace.
4. Add workspace task and reminder panels.
5. Add Jira and MR workspace rollups.
6. Add workspace note/file aggregation.

### 11.2 Session Roadmap

1. Normalize provider session discovery.
2. Build session cache for fast load.
3. Add session detail timeline.
4. Add session notes.
5. Add session file browser and file modal.
6. Add Jira and MR panels.
7. Add metadata editing.
8. Add stats and tree views.

### 11.3 Task and Reminder Roadmap

1. Task CRUD.
2. Reminder CRUD.
3. Workspace-scoped lists.
4. Session linking where appropriate.
5. Notification toasts for lifecycle actions.

### 11.4 Integration Roadmap

1. API auth and health.
2. MCP tool surfaces for workspace/session/task/reminder/note/Jira/MR.
3. Agent config setup.
4. Local install of skills and tools.
5. Provider-specific config detection and patching.

### 11.5 Local Setup Roadmap

1. Detect provider config files.
2. Surface config status in UI.
3. Patch MCP config entries.
4. Install local skills.
5. Install local tools.
6. Show status and errors in toasts and diagnostics.

## 12. Migration Strategy

### 12.1 Reuse

- provider adapters where the format still matches
- session parsing and metadata extraction where valid
- notes/file traversal logic
- local install and config detection logic
- local persistence patterns where still useful

### 12.2 Rebuild

- app shell
- navigation
- styling
- workspace/session presentation
- settings and preferences UX
- API/MCP documentation and affordances

### 12.3 Drop or Defer

- prototype-only debug utilities
- redundant surfaces not tied to the Sanctum model
- legacy layout assumptions that no longer fit the new product

## 13. User Experience Requirements

- The app must clearly separate:
  - core work
  - session detail
  - preferences
  - local setup
  - agent/tool configuration
- Session detail should make relationships obvious.
- Workspaces should act as work hubs, not just filters.
- Local setup should be explainable and actionable.
- API and MCP capabilities should be discoverable from the UI.

## 14. Technical Requirements

- Electron desktop app
- API-driven UI
- MCP-driven agent tools
- local SQLite or equivalent for user prefs/cache/outbox
- support for server health/auth
- support for session discovery across providers
- support for local file inspection within session roots
- safe handling of local configuration files
- no direct server imports from renderer code

## 15. Performance Requirements

- First paint should use cached session/workspace data where available.
- Session scans should not block UI responsiveness.
- Large provider trees should be paginated or lazily loaded.
- Metadata and stats should be cached where possible.
- Local install/config checks should not freeze the interface.

## 16. Quality Requirements

- API actions should be testable.
- MCP tool contracts should be testable.
- Session parsing should be covered by unit tests.
- Workspace/session relationship behavior should be integration-tested.
- UI should be covered by contract and smoke tests.

## 17. Risks

- Scope creep from prototype feature parity pressure.
- Mixing API and MCP responsibilities.
- Over-coupling to local file layouts.
- Stale provider configs or agent setup assumptions.
- Large session directories causing slow scans.
- Divergence between UI and agent-visible data.

## 18. Open Questions

- Which prototype features are v1 versus later?
- Which Jira/MR fields are mandatory in Sanctum?
- Which session attachments are writable in Sanctum?
- How much knowledge-graph capability should v1 include?
- Which provider configuration flows must be automatic versus manual?
- Should tools and skills be first-class settings or a separate admin section?

## 19. Acceptance Criteria

- Sanctum can launch and authenticate against `savant-server`.
- Workspaces and sessions are visible and linked correctly.
- Session detail shows notes, files, Jira tickets, and merge requests.
- Tasks and reminders are manageable from the UI.
- MCP tools expose the same core domain operations.
- Local provider config and install flows work on a fresh machine.
- Toasts and diagnostics reflect success and failure states.
- The app is easier to reason about than the prototype.

## 20. Delivery Definition

Sanctum is ready for initial release when:

- the core domain model is implemented
- the API and MCP surfaces are aligned
- the local setup/install layer works
- the prototype’s useful session/workspace features are ported or intentionally deferred
- the UX is coherent and not just a cleaned-up clone

