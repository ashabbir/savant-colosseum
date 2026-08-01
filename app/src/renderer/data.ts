export type SectionId =
  | 'workspace'
  | 'manage'
  | 'session'
  | 'tasks'
  | 'reminders'
  | 'notes'
  | 'files'
  | 'jira'
  | 'merge-requests'
  | 'knowledge'
  | 'providers'
  | 'settings';

export type SurfaceMode = 'overview' | 'sessions' | 'artifacts' | 'setup';

export type Workspace = {
  id: string;
  name: string;
  description?: string;
  priority?: 'critical' | 'high' | 'medium' | 'low';
  status: 'active' | 'online' | 'syncing' | 'open' | 'closed';
  color?: string | null;
  createdAt?: string;
  updatedAt?: string;
  sessions: number;
  counts?: {
    total: number;
    copilot?: number;
    claude?: number;
    codex?: number;
    gemini?: number;
    savant?: number;
  };
  tasks: number;
  notes?: number;
  reminders: number;
  summary: string;
  files: number;
  knowledgeNodes: number;
  knowledgeEdges: number;
  nodeDensity?: number;
  kg_stats?: {
    total_nodes: number;
    total_edges: number;
    staged_count: number;
    nodes_by_type: Record<string, number>;
  };
  aiIntelligence: {
    providers: { name: string; models: string[]; usage: string }[];
  };
};

export type Session = {
  id: string;
  workspaceId: string;
  title: string;
  provider: string;
  agentType?: string;
  model: string;
  createdAt?: string;
  updatedAt?: string;
  updated: string;
  files: number;
  linked: number;
  notes: number;
  jira: number;
  mergeRequests: number;
  tree: string;
};

export type TaskComment = {
  id?: string;
  author: string;
  text: string;
  createdAt: string;
  role?: 'user' | 'agent' | 'system';
};

export type ColosseumRun = {
  run_id: string;
  phase: 'grooming' | 'work' | 'review' | 'merge';
  status: string;
  summary?: string;
  rationale?: string;
  questions?: string[];
  provider?: string;
  model?: string;
  persona?: string;
  created_at?: string;
  duration_ms?: number;
  log_path?: string;
  worktree_path?: string;
  branch?: string;
  base_commit?: string;
  commit?: string;
  merge_commit?: string;
  mr_id?: string;
};

export type ColosseumTaskConfig = {
  repository?: string;
  provider?: string;
  model?: string;
  persona?: string;
  tags?: string[];
  phase_configs?: Partial<Record<'grooming' | 'ready' | 'review', {
    persona: string;
    tags: string[];
    provider: string;
    model: string;
  }>>;
  setup?: string;
  timeout_seconds?: number;
  work_type?: 'development' | 'research';
  autopilot?: boolean;
  runs?: ColosseumRun[];
  worktree_path?: string;
  branch?: string;
  base_branch?: string;
  base_commit?: string;
  commit?: string;
  remote?: string;
  mr_id?: string;
  log_path?: string;
};

export type Task = {
  id: string;
  workspaceId: string;
  title: string;
  description?: string;
  priority: 'critical' | 'high' | 'medium' | 'low';
  state: 'backlog' | 'grooming' | 'ready' | 'in-progress' | 'review' | 'human-review' | 'approved' | 'done';
  owner: string;
  due?: string;
  comments?: Array<string | TaskComment>;
  dependsOn?: string[];
  createdAt?: string;
  updatedAt?: string;
  timeSpent?: number;
  complexity?: 'simple' | 'moderate' | 'complex' | 'extreme';
  colosseumConfig?: ColosseumTaskConfig;
};

export type Reminder = {
  id: string;
  workspaceId: string;
  title: string;
  due: string;
  state: 'scheduled' | 'done';
  dueLabel?: string;
};

export type Note = {
  id: string;
  sessionId: string;
  title: string;
  body: string;
  createdAt?: string;
};

export type Artifact = {
  id: string;
  sessionId: string;
  title: string;
  kind: 'file' | 'jira' | 'merge-request';
  count: number;
};

export type SessionEvent = {
  id: string;
  sessionId: string;
  time: string;
  title: string;
  detail: string;
  kind: 'note' | 'artifact' | 'task' | 'sync';
};

export type Provider = {
  id: string;
  name: string;
  mode: string;
  state: 'ready' | 'configured' | 'syncing' | 'online';
  scope: string;
};

export type LocalSetupItem = {
  id: string;
  label: string;
  value: string;
  tone: 'good' | 'muted' | 'warning';
};

export type SurfaceItem = {
  id: string;
  label: string;
  detail: string;
  tone?: 'good' | 'muted' | 'warning';
};

export type SectionProfile = {
  id: SectionId;
  eyebrow: string;
  title: string;
  subtitle: string;
  blurb: string;
  modeLabel: string;
};

export const navigation = [
  { id: 'workspace', short: 'W', icon: '◫', label: 'Workspaces' },
  { id: 'manage', short: 'M', icon: '⧉', label: 'Manage' },
  { id: 'session', short: 'S', icon: '◔', label: 'Sessions' },
  { id: 'tasks', short: 'T', icon: '≡', label: 'Tasks' },
  { id: 'reminders', short: 'R', icon: '◷', label: 'Reminders' },
  { id: 'notes', short: 'N', icon: '⋯', label: 'Notes' },
  { id: 'files', short: 'F', icon: '▤', label: 'Files' },
  { id: 'jira', short: 'J', icon: '⌁', label: 'Jira' },
  { id: 'merge-requests', short: 'M', icon: '⇄', label: 'Merge Requests' },
  { id: 'knowledge', short: 'K', icon: '⟡', label: 'Knowledge' },
  { id: 'settings', short: 'G', icon: '⚙', label: 'Settings' },
] as const;

export const workspaces: Workspace[] = [
  {
    id: 'ws-olympus',
    name: 'Olympus',
    sessions: 21,
    tasks: 8,
    reminders: 1,
    status: 'online',
    summary: 'Renderer and control-surface work with authenticated API and MCP integration.',
    files: 142,
    knowledgeNodes: 64,
    knowledgeEdges: 112,
    aiIntelligence: {
      providers: [
        { name: 'Claude', models: ['Sonnet 3.7', 'Haiku'], usage: 'High' },
        { name: 'Gemini', models: ['Flash 2.0', 'Pro 1.5'], usage: 'Medium' },
      ],
    },
  },
  {
    id: 'ws-sanctum',
    name: 'Sanctum',
    sessions: 18,
    tasks: 14,
    reminders: 5,
    status: 'active',
    summary: 'Workspace/session operations console with local setup and artifact management.',
    files: 89,
    knowledgeNodes: 42,
    knowledgeEdges: 78,
    aiIntelligence: {
      providers: [
        { name: 'Codex', models: ['Llama 3.3', 'Qwen 2.5'], usage: 'High' },
        { name: 'Savant', models: ['Vision 1.0'], usage: 'Low' },
      ],
    },
  },
];

export const sessions: Session[] = [
  {
    id: 'session-auth',
    workspaceId: 'ws-sanctum',
    title: 'Auth relay stabilization',
    provider: 'Codex',
    model: 'Llama 3.3',
    updated: '2m ago',
    files: 6,
    linked: 4,
    notes: 3,
    jira: 1,
    mergeRequests: 1,
    tree: 'workspace → session → provider config',
  },
  {
    id: 'session-tray',
    workspaceId: 'ws-sanctum',
    title: 'Tray asset refinement',
    provider: 'Gemini',
    model: 'Flash 2.0',
    updated: '29m ago',
    files: 2,
    linked: 2,
    notes: 1,
    jira: 0,
    mergeRequests: 0,
    tree: 'tray.svg → sanctum.svg → shell',
  },
  {
    id: 'session-linkage',
    workspaceId: 'ws-olympus',
    title: 'Workspace linkage audit',
    provider: 'Savant',
    model: 'Vision 1.0',
    updated: '1h ago',
    files: 8,
    linked: 9,
    notes: 4,
    jira: 1,
    mergeRequests: 2,
    tree: 'workspace → session → audit trail',
  },
];

export const tasks: Task[] = [
  {
    id: 'task-1',
    workspaceId: 'ws-sanctum',
    title: 'Wire workspace/session contract',
    priority: 'critical',
    state: 'in-progress',
    owner: 'ahmed',
    due: 'Today',
    timeSpent: 12,
    complexity: 'extreme',
  },
  {
    id: 'task-2',
    workspaceId: 'ws-sanctum',
    title: 'Add reminder CRUD surface',
    priority: 'high',
    state: 'backlog',
    owner: 'ahmed',
    due: 'Today',
    timeSpent: 4,
    complexity: 'moderate',
  },
  {
    id: 'task-3',
    workspaceId: 'ws-quorum',
    title: 'Implement provider config panels',
    priority: 'high',
    state: 'backlog',
    owner: 'ahmed',
    due: 'Tomorrow',
    timeSpent: 8,
    complexity: 'complex',
  },
  {
    id: 'task-4',
    workspaceId: 'ws-olympus',
    title: 'Surface MCP status in chrome',
    priority: 'medium',
    state: 'review',
    owner: 'ahmed',
    due: 'Today',
    timeSpent: 2,
    complexity: 'simple',
  },
];

export const reminders: Reminder[] = [
  { id: 'rem-1', workspaceId: 'ws-sanctum', title: 'Review workspace linkage audit', due: 'Today 4:00 PM', state: 'scheduled', dueLabel: 'Today' },
  { id: 'rem-2', workspaceId: 'ws-sanctum', title: 'Sync provider config on launch', due: 'Tomorrow 9:30 AM', state: 'scheduled', dueLabel: 'Tomorrow' },
  { id: 'rem-3', workspaceId: 'ws-quorum', title: 'Publish knowledge graph update', due: 'Friday 2:00 PM', state: 'scheduled', dueLabel: 'Friday' },
];

export const notes: Note[] = [
  {
    id: 'note-1',
    sessionId: 'session-auth',
    title: 'Server truth wins',
    body: 'Server truth wins for shared data and linkage.',
  },
  {
    id: 'note-2',
    sessionId: 'session-graph',
    title: 'Sessions as work containers',
    body: 'Sessions are work containers, not just logs.',
  },
  {
    id: 'note-3',
    sessionId: 'session-linkage',
    title: 'Right rail should inspect',
    body: 'The right rail should show inspection detail, not duplicate navigation.',
  },
];

export const artifacts: Artifact[] = [
  { id: 'art-1', sessionId: 'session-auth', title: 'Session notes', kind: 'file', count: 12 },
  { id: 'art-2', sessionId: 'session-auth', title: 'Files', kind: 'file', count: 18 },
  { id: 'art-3', sessionId: 'session-graph', title: 'Jira tickets', kind: 'jira', count: 7 },
  { id: 'art-4', sessionId: 'session-linkage', title: 'Merge requests', kind: 'merge-request', count: 4 },
];

export const sessionEvents: SessionEvent[] = [
  {
    id: 'event-1',
    sessionId: 'session-auth',
    time: '2m ago',
    title: 'Auth relay verified',
    detail: 'API key persisted and workspace/session linkage confirmed.',
    kind: 'sync',
  },
  {
    id: 'event-2',
    sessionId: 'session-auth',
    time: '6m ago',
    title: 'Session notes updated',
    detail: 'Server truth note added to the session rollup.',
    kind: 'note',
  },
  {
    id: 'event-3',
    sessionId: 'session-auth',
    time: '18m ago',
    title: 'Merge request attached',
    detail: 'MR surface linked to the active session.',
    kind: 'artifact',
  },
  {
    id: 'event-4',
    sessionId: 'session-graph',
    time: '11m ago',
    title: 'Knowledge export refreshed',
    detail: 'Graph cleanup output made available to the agent.',
    kind: 'artifact',
  },
  {
    id: 'event-5',
    sessionId: 'session-linkage',
    time: '1h ago',
    title: 'Workspace linkage audited',
    detail: 'Session-to-workspace links were validated from the server.',
    kind: 'task',
  },
];

export const providers: Provider[] = [
  { id: 'prov-codex', name: 'Codex', mode: 'MCP + API', state: 'ready', scope: 'workspace' },
  { id: 'prov-claude', name: 'Claude', mode: 'MCP', state: 'configured', scope: 'session' },
  { id: 'prov-gemini', name: 'Gemini', mode: 'Local cache', state: 'syncing', scope: 'local' },
  { id: 'prov-savant', name: 'Savant', mode: 'Internal', state: 'online', scope: 'system' },
];

export const localSetup: LocalSetupItem[] = [
  { id: 'setup-1', label: 'Provider MCP', value: 'Ready', tone: 'good' },
  { id: 'setup-2', label: 'Skills', value: 'Installed', tone: 'good' },
  { id: 'setup-3', label: 'Tools', value: 'Installed', tone: 'good' },
  { id: 'setup-4', label: 'Quarantine', value: 'Clear', tone: 'muted' },
  { id: 'setup-5', label: 'API key', value: 'Loaded', tone: 'warning' },
];

export const apiSurface: SurfaceItem[] = [
  { id: 'api-1', label: 'Health', detail: '/api/health · online', tone: 'good' },
  { id: 'api-2', label: 'Auth', detail: '/api/auth/validate · ready', tone: 'good' },
  { id: 'api-3', label: 'Workspace', detail: '/api/workspaces · server-backed', tone: 'muted' },
  { id: 'api-4', label: 'Sessions', detail: '/api/sessions · linked', tone: 'muted' },
  { id: 'api-5', label: 'Notes', detail: '/api/notes · writable', tone: 'muted' },
];

export const mcpSurface: SurfaceItem[] = [
  { id: 'mcp-1', label: 'savant-workspace', detail: 'Workspace/session/task/reminder tools', tone: 'good' },
  { id: 'mcp-2', label: 'savant-knowledge', detail: 'Knowledge graph and context tools', tone: 'good' },
  { id: 'mcp-3', label: 'savant-abilities', detail: 'Ability and prompt assets', tone: 'good' },
  { id: 'mcp-4', label: 'savant-context', detail: 'Context and code search', tone: 'muted' },
  { id: 'mcp-5', label: 'savant-reminders', detail: 'Reminder tooling', tone: 'muted' },
];

export const sectionProfiles: SectionProfile[] = [
  {
    id: 'workspace',
    eyebrow: 'Workspace',
    title: 'Workspace overview',
    subtitle: 'Sessions, tasks, reminders, and linked artifacts',
    blurb: 'A dense workspace summary with session-linked artifacts, reminders, and system state.',
    modeLabel: 'Inspect workspace',
  },
  {
    id: 'manage',
    eyebrow: 'Manage',
    title: 'Operational Hub',
    subtitle: 'Tasks, reminders, and workspace management',
    blurb: 'Central command for tracking progress and managing workspace-scoped activities.',
    modeLabel: 'Manage active work',
  },
  {
    id: 'session',
    eyebrow: 'Session',
    title: 'Session timeline',
    subtitle: 'Notes, files, Jira, and merge requests attached to the selected session',
    blurb: 'Timeline, notes, files, linked Jira, and merge requests stay attached to the selected session.',
    modeLabel: 'Inspect session',
  },
  {
    id: 'tasks',
    eyebrow: 'Tasks',
    title: 'Task queue',
    subtitle: 'Ordering, ownership, and status across the active workspace',
    blurb: 'Use this surface for ordering, ownership, and status changes across the workspace.',
    modeLabel: 'Inspect queue',
  },
  {
    id: 'reminders',
    eyebrow: 'Reminders',
    title: 'Reminder schedule',
    subtitle: 'Due dates and workspace-relevant followups',
    blurb: 'Reminder scheduling stays visible alongside the workspace it affects.',
    modeLabel: 'Inspect reminders',
  },
  {
    id: 'notes',
    eyebrow: 'Notes',
    title: 'Session notes',
    subtitle: 'Attached notes grouped by the active session',
    blurb: 'Notes remain session-scoped and visible in workspace rollups.',
    modeLabel: 'Inspect notes',
  },
  {
    id: 'files',
    eyebrow: 'Files',
    title: 'Session files',
    subtitle: 'Attached files and file-backed work artifacts',
    blurb: 'Files remain attached to the selected session and available in workspace rollups.',
    modeLabel: 'Inspect files',
  },
  {
    id: 'jira',
    eyebrow: 'Jira',
    title: 'Linked issues',
    subtitle: 'Server-backed issue tracking tied to the session',
    blurb: 'Jira stays linked to the selected session and organized by workspace context.',
    modeLabel: 'Sync Jira',
  },
  {
    id: 'merge-requests',
    eyebrow: 'Merge requests',
    title: 'Code review surface',
    subtitle: 'Session-linked merge requests and review state',
    blurb: 'Merge requests stay attached to the active session and visible in the workspace context.',
    modeLabel: 'Review MRs',
  },
  {
    id: 'knowledge',
    eyebrow: 'Knowledge',
    title: 'Context surfaces',
    subtitle: 'Exports, search, and MCP knowledge tools',
    blurb: 'Knowledge stays discoverable as an operational surface for the coding agent.',
    modeLabel: 'Export context',
  },
  {
    id: 'providers',
    eyebrow: 'Providers',
    title: 'Local setup',
    subtitle: 'MCP, API, and install state',
    blurb: 'Keep provider config, local setup, and sync state separate from the core domain.',
    modeLabel: 'Configure setup',
  },
  {
    id: 'settings',
    eyebrow: 'Settings',
    title: 'Preferences',
    subtitle: 'Install paths, API keys, and sync defaults',
    blurb: 'Settings stay local while domain data stays server-backed.',
    modeLabel: 'Save defaults',
  },
];
