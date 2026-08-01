import { useMemo, useState, type Dispatch, type ReactNode, type SetStateAction } from 'react';
import { AlertTriangle, Ban, BarChart3, Check, CheckCircle2, ChevronDown, Clock, Inbox, Layers, ListChecks, Maximize, Network, PlayCircle, ShieldAlert, Sparkles, Trash2, X, ZoomIn, ZoomOut } from 'lucide-react';
import { apiSurface, localSetup, mcpSurface, type Artifact, type Provider, type Reminder, type Session, type SurfaceMode, type Task, type Workspace } from '../data';
import { canMoveTask, isTaskBlocked, taskWorkflowState, type TaskFlagState } from '../lib/taskBoard';
import { PanelHeader } from './WorkspacePrimitives';
import { WorkspaceOverview } from './WorkspaceOverview';
import { WorkspaceHeader } from './WorkspaceHeader';
import { WorkspaceManagePanel, WorkspaceSessionPanel } from './WorkspacePanels';
import { buildSavantHeaders } from '../services/httpClient';

type SessionFileGroup = {
  session_id?: string;
  id?: string;
  provider?: string;
  summary?: string;
  file_count?: number;
  files?: { path?: string; name?: string; category?: string; size?: number }[];
};

type SectionHero = {
  eyebrow: string;
  title: string;
  subtitle: string;
};

type WorkspaceSurfaceProps = {
  hero: SectionHero;
  heroFacts: string[];
  workspaceSessions: Session[];
  workspaceTasks: Task[];
  allTasks: Task[];
  workspaceReminders: Reminder[];
  workspaceNotes: { id: string; sessionId: string; title: string; body: string }[];
  workspaceArtifacts: Artifact[];
  workspaceSessionFiles: Record<string, SessionFileGroup>;
  workspaceMergeRequests: { id: string; title: string; status: string; mrId: string; createdAt?: string; updatedAt?: string }[];
  workspaceJiraTickets: { id: string; title: string; status: string; ticketKey: string; createdAt?: string; updatedAt?: string }[];
  activeWorkspace?: Workspace;
  workspaceSummary: {
    knowledgeNodes: number;
    knowledgeEdges: number;
    knowledgeCommittedNodes: number;
    knowledgeStagedNodes: number;
    knowledgeCommittedEdges: number;
    knowledgeStagedEdges: number;
  };
  manageSummary: { todayTasks: number; todayReminders: number; openTasks: number; pendingReminders: number };
  todayTasks: Task[];
  todayReminders: Reminder[];
  activeSection: string;
  onEdit: () => void;
  surfaceMode: SurfaceMode;
  setSurfaceMode: (mode: SurfaceMode) => void;
  activeModeLabel: string;
  surfaceTitle: string;
  workspaceList: Workspace[];
  workspaceIndex: number;
  setWorkspaceIndex: (value: number) => void;
  setSessionIndex: (value: number) => void;
  activeSession: Session;
  workspaceTasks: Task[];
  workspaceReminders: Reminder[];
  sessionNotes: { id: string; title: string; body: string }[];
  sessionArtifacts: Artifact[];
  sessionTimeline: { id: string; time: string; title: string; detail: string; kind: string }[];
  sessionConversation: { id: string; time: string; title: string; detail: string; kind: 'user' | 'assistant' | 'tool' | 'system'; role: string; provider: string }[];
  sessionStats: { label: string; value: string | number }[];
  sessionCheckpoints: string[];
  activeProvider: Provider;
  providers: Provider[];
  setSessionProvider: (providerName: string) => void;
  localSetup: typeof localSetup;
  pushToast: (title: string, detail: string, tone?: 'good' | 'warning' | 'muted') => void;
  setActiveSection: (section: string) => void;
  setManagementDrawer: (drawer: 'tasks' | 'reminders' | null) => void;
  setNoteList: Dispatch<SetStateAction<{ id: string; sessionId: string; title: string; body: string }[]>>;
  onCreateSessionNote: (text: string) => Promise<void> | void;
  setTaskList: Dispatch<SetStateAction<Task[]>>;
  setReminderList: Dispatch<SetStateAction<Reminder[]>>;
  taskFlags: Record<string, TaskFlagState>;
  reminderFlags: Record<string, { done?: boolean }>;
  setTaskFlags: Dispatch<SetStateAction<Record<string, TaskFlagState>>>;
  setReminderFlags: Dispatch<SetStateAction<Record<string, { done?: boolean }>>>;
  workspaceSearch: string;
  setWorkspaceSearch: (value: string) => void;
  setIsTaskDrawerOpen: (value: boolean) => void;
  setIsNotesDrawerOpen: (value: boolean) => void;
  setIsMergeRequestsDrawerOpen: (value: boolean) => void;
  setIsJiraDrawerOpen: (value: boolean) => void;
  setIsArtifactsDrawerOpen: (value: boolean) => void;
  isKnowledgeOpen: boolean;
  setIsKnowledgeOpen: (value: boolean) => void;
  isActivityOpen: boolean;
  setIsActivityOpen: (value: boolean) => void;
  serverBaseUrl: string;
  apiKey: string;
  onOpenTasks: () => void;
  onEditGlobalTask: (task: Task) => void;
  workspaceActivitySummary: {
    total: number;
    detail: string;
    latest: string;
  };
  restoreActivityContext: (item: { section: string; surfaceMode: string; workspaceIndex: number; sessionIndex: number }) => void;
  activeWorkspaceId: string;
  onOpenSessions: () => void;
  rightRailSlot?: ReactNode;
};

export function WorkspaceSurface(props: WorkspaceSurfaceProps) {
  const {
    hero,
    heroFacts,
    workspaceSessions,
    workspaceTasks,
    allTasks,
    workspaceReminders,
    workspaceNotes,
    workspaceArtifacts,
    workspaceSessionFiles,
    workspaceMergeRequests,
    workspaceJiraTickets,
    activeWorkspace,
    workspaceSummary,
    manageSummary,
    todayTasks,
    todayReminders,
    activeSection,
    onEdit,
    surfaceMode,
    setSurfaceMode,
    activeModeLabel,
    surfaceTitle,
    workspaceList,
    workspaceIndex,
    setWorkspaceIndex,
    setSessionIndex,
    activeSession,
    sessionNotes,
    sessionArtifacts,
    sessionTimeline,
    sessionConversation,
    sessionStats,
    sessionCheckpoints,
    activeProvider,
    providers,
    setSessionProvider,
    localSetup,
    pushToast,
    setActiveSection,
    setManagementDrawer,
    setNoteList,
    onCreateSessionNote,
    setTaskList,
    setReminderList,
    taskFlags,
    reminderFlags,
    setTaskFlags,
    setReminderFlags,
    workspaceSearch,
    setWorkspaceSearch,
    setIsTaskDrawerOpen,
    setIsNotesDrawerOpen,
    setIsMergeRequestsDrawerOpen,
    setIsJiraDrawerOpen,
    setIsArtifactsDrawerOpen,
    isKnowledgeOpen,
    setIsKnowledgeOpen,
    isActivityOpen,
    setIsActivityOpen,
    serverBaseUrl,
    apiKey,
    onOpenTasks,
    onEditGlobalTask,
    workspaceActivitySummary,
    restoreActivityContext,
    activeWorkspaceId,
    onOpenSessions,
  } = props;
  const [draggingTaskId, setDraggingTaskId] = useState<string | null>(null);
  const [taskViewMode, setTaskViewMode] = useState<'overview' | 'visualization' | 'analytics'>('overview');
  const [visualZoom, setVisualZoom] = useState<number>(1);
  const [hideDoneInAnalytics, setHideDoneInAnalytics] = useState<boolean>(true);
  const [isGlobalTaskCreateOpen, setIsGlobalTaskCreateOpen] = useState(false);
  const [taskWorkspaceFilter, setTaskWorkspaceFilter] = useState<string>('');
  const [taskStatusFilter, setTaskStatusFilter] = useState<string>('');
  const [taskPriorityFilter, setTaskPriorityFilter] = useState<string>('');
  const [taskTextFilter, setTaskTextFilter] = useState('');
  const [globalTaskDraft, setGlobalTaskDraft] = useState({
    title: '',
    description: '',
    priority: 'medium' as Task['priority'],
    state: 'backlog' as Task['state'],
    due: '',
    workspaceId: '',
  });
  const headers = buildSavantHeaders(apiKey, true);
  const normalizeTaskFromServer = (task: any, fallback: Task): Task => ({
    ...fallback,
    id: task.task_id ?? task.id ?? fallback.id,
    workspaceId: task.workspace_id ?? fallback.workspaceId,
    title: task.title ?? fallback.title,
    description: task.description ?? fallback.description ?? '',
    priority: task.priority ?? fallback.priority,
    state: (() => {
      const s = task.status ?? fallback.state;
      if (s === 'todo') return 'backlog';
      if (s === 'code-review') return 'review';
      return s;
    })(),
    due: task.date ?? fallback.due,
    dependsOn: task.depends_on ?? task.dependencies ?? fallback.dependsOn ?? [],
    createdAt: task.created_at ?? task.createdAt ?? fallback.createdAt,
    updatedAt: task.updated_at ?? task.updatedAt ?? fallback.updatedAt,
    comments: task.comments ?? fallback.comments ?? [],
    colosseumConfig: task.colosseum_config ?? fallback.colosseumConfig,
  });
  const moveGlobalTask = (taskId: string, state: Task['state']) => {
    const currentTask = allTasks.find((task) => task.id === taskId);
    if (currentTask && currentTask.state !== state) {
      pushToast('Lifecycle controlled', 'Open the ticket to submit it for grooming or complete human review.', 'warning');
      setDraggingTaskId(null);
      return;
    }
    if (currentTask && !canMoveTask(currentTask, state, taskFlags)) {
      pushToast('Task blocked', `${currentTask.title} must be unblocked before changing status.`, 'warning');
      setDraggingTaskId(null);
      return;
    }
    const previousState = currentTask ? taskWorkflowState(currentTask, taskFlags) : 'backlog';
    const movedAt = new Date().toISOString();
    setTaskList((current) => current.map((task) => (task.id === taskId ? { ...task, state } : task)));
    setTaskFlags((current) => ({ ...current, [taskId]: { ...(current[taskId] ?? {}), done: state === 'done', lastMovedAt: movedAt, lastMovedFrom: previousState, lastMovedTo: state } }));
    setDraggingTaskId(null);
    pushToast('Task moved', `Task moved to ${state}.`, 'good');
    void fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(taskId)}`, {
      method: 'PUT',
      headers,
      body: JSON.stringify({ status: state }),
    }).catch(() => undefined);
  };
  const toggleTaskBlocked = (task: Task) => {
    const isCurrentlyBlocked = isTaskBlocked(task, taskFlags);
    if (!isCurrentlyBlocked && (task.state === 'human-review' || task.state === 'approved' || task.state === 'done')) {
      pushToast('Cannot block task', `Tasks in ${task.state} status cannot be blocked.`, 'warning');
      return;
    }
    const blocked = !isCurrentlyBlocked;
    const workflowState = taskWorkflowState(task, taskFlags);
    setTaskFlags((current) => ({ ...current, [task.id]: { ...(current[task.id] ?? {}), blocked } }));
    if (task.state === 'blocked') {
      setTaskList((current) => current.map((item) => item.id === task.id ? { ...item, state: workflowState } : item));
    }
    pushToast(blocked ? 'Task blocked' : 'Task unblocked', `${task.title} remains in ${workflowState}.`, blocked ? 'warning' : 'good');
  };
  const removeGlobalTask = (taskId: string) => {
    const task = allTasks.find((item) => item.id === taskId);
    setTaskList((current) => current.filter((item) => item.id !== taskId));
    setTaskFlags((current) => {
      const next = { ...current };
      delete next[taskId];
      return next;
    });
    pushToast('Task removed', task ? `${task.title} removed from the global board.` : 'Task removed.', 'warning');
    void fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(taskId)}`, {
      method: 'DELETE',
      headers,
    }).catch(() => undefined);
  };
  const filteredGlobalTasks = useMemo(() => {
    return allTasks.filter((task) => {
      const doneState = taskFlags[task.id]?.done ? 'done' : task.state;
      const searchText = taskTextFilter.trim().toLowerCase();
      if (searchText && !`${task.title} ${task.description ?? ''}`.toLowerCase().includes(searchText)) return false;
      if (taskWorkspaceFilter && task.workspaceId !== taskWorkspaceFilter) return false;
      if (taskStatusFilter && doneState !== taskStatusFilter) return false;
      if (taskPriorityFilter && task.priority !== taskPriorityFilter) return false;
      return true;
    });
  }, [allTasks, taskFlags, taskPriorityFilter, taskStatusFilter, taskTextFilter, taskWorkspaceFilter]);
  const filteredTaskColumns = useMemo(() => {
    return (['backlog', 'grooming', 'ready', 'in-progress', 'review', 'human-review'] as const).map((state) => ({
      state,
      tasks: filteredGlobalTasks.filter((task) => {
        const isDone = task.state === 'done' || Boolean(taskFlags[task.id]?.done);
        if (isDone) return false;
        return task.state === state || (state === 'human-review' && task.state === 'approved');
      }),
    }));
  }, [filteredGlobalTasks, taskFlags]);
  const globalTaskStats = useMemo(() => {
    const completed = filteredGlobalTasks.filter((task) => (taskFlags[task.id]?.done ? 'done' : task.state) === 'done').length;
    const blocked = filteredGlobalTasks.filter((task) => isTaskBlocked(task, taskFlags) && !taskFlags[task.id]?.done).length;
    const active = filteredGlobalTasks.filter((task) => taskWorkflowState(task, taskFlags) === 'in-progress' && !taskFlags[task.id]?.done).length;
    const backlog = filteredGlobalTasks.filter((task) => taskWorkflowState(task, taskFlags) === 'backlog' && !taskFlags[task.id]?.done).length;
    const ready = filteredGlobalTasks.filter((task) => taskWorkflowState(task, taskFlags) === 'ready' && !taskFlags[task.id]?.done).length;
    const review = filteredGlobalTasks.filter((task) => taskWorkflowState(task, taskFlags) === 'review' && !taskFlags[task.id]?.done).length;
    const critical = filteredGlobalTasks.filter((task) => task.priority === 'critical' && !taskFlags[task.id]?.done && task.state !== 'done').length;
    const workspaceCount = new Set(filteredGlobalTasks.map((task) => task.workspaceId).filter(Boolean)).size;
    const completionRate = filteredGlobalTasks.length ? Math.round((completed / filteredGlobalTasks.length) * 100) : 0;
    return { total: filteredGlobalTasks.length, backlog, ready, active, review, blocked, critical, completed, workspaceCount, completionRate };
  }, [filteredGlobalTasks, taskFlags]);
  const globalTaskWorkflow = useMemo(() => {
    const tasksById = new Map(filteredGlobalTasks.map((task) => [task.id, task]));
    const parents = new Map<string, string[]>();
    filteredGlobalTasks.forEach((task) => {
      parents.set(task.id, (task.dependsOn ?? []).filter((id) => tasksById.has(id)));
    });

    const levelCache = new Map<string, number>();
    const getLevel = (taskId: string, seen = new Set<string>()): number => {
      if (levelCache.has(taskId)) return levelCache.get(taskId)!;
      if (seen.has(taskId)) return 0;
      const nextSeen = new Set(seen).add(taskId);
      const taskParents = parents.get(taskId) ?? [];
      const level = taskParents.length ? Math.max(...taskParents.map((id) => getLevel(id, nextSeen))) + 1 : 0;
      levelCache.set(taskId, level);
      return level;
    };

    const groups = new Map<number, Task[]>();
    filteredGlobalTasks.forEach((task) => {
      const level = getLevel(task.id);
      groups.set(level, [...(groups.get(level) ?? []), task]);
    });
    const widestLevel = Math.max(1, ...Array.from(groups.values()).map((tasks) => tasks.length));
    const width = Math.max(1000, widestLevel * 210 + 180);
    const nodes = Array.from(groups.entries()).flatMap(([level, levelTasks]) => {
      const sorted = levelTasks.slice().sort((a, b) => a.title.localeCompare(b.title));
      const spacing = 210;
      const startX = width / 2 - ((sorted.length - 1) * spacing) / 2;
      return sorted.map((task, index) => ({ task, x: startX + index * spacing, y: 80 + level * 130 }));
    });
    const placement = new Map(nodes.map((node) => [node.task.id, node]));
    const edges = filteredGlobalTasks.flatMap((task) => (parents.get(task.id) ?? []).map((parentId) => ({
      id: `${parentId}->${task.id}`,
      source: placement.get(parentId),
      target: placement.get(task.id),
    }))).filter((edge): edge is { id: string; source: NonNullable<typeof edge.source>; target: NonNullable<typeof edge.target> } => Boolean(edge.source && edge.target));
    const depth = Math.max(0, ...nodes.map((node) => getLevel(node.task.id)));
    return { nodes, edges, width, height: Math.max(380, depth * 130 + 170) };
  }, [filteredGlobalTasks]);
  const visualTimeStats = useMemo(() => {
    const tasksWithTime = filteredGlobalTasks.filter(t => t.timeSpent !== undefined && t.timeSpent > 0);
    if (tasksWithTime.length === 0) {
      return { total: 0, average: 0, max: 0 };
    }
    const times = tasksWithTime.map(t => t.timeSpent!);
    const total = times.reduce((sum, t) => sum + t, 0);
    const average = Math.round((total / times.length) * 10) / 10;
    const max = Math.max(...times);
    return { total, average, max };
  }, [filteredGlobalTasks]);
  const workspaceTaskAnalytics = useMemo(() => workspaceList.map((workspace) => {
    const scoped = filteredGlobalTasks.filter((task) => task.workspaceId === workspace.id);
    const statusCounts = (['todo', 'in-progress', 'review', 'done'] as const).map((status) => ({
      status,
      count: scoped.filter((task) => (taskFlags[task.id]?.done ? 'done' : taskWorkflowState(task, taskFlags)) === status).length,
    }));
    const createdTimes = scoped.map((task) => Date.parse(task.createdAt ?? '')).filter(Number.isFinite);
    const latestActivity = scoped
      .map((task) => ({ task, timestamp: taskFlags[task.id]?.lastMovedAt ?? task.updatedAt ?? task.createdAt ?? '' }))
      .filter((item) => item.timestamp)
      .sort((a, b) => Date.parse(b.timestamp) - Date.parse(a.timestamp))[0];
    const times = scoped.map(t => t.timeSpent!).filter(t => typeof t === 'number' && t > 0);
    const totalTimeSpent = times.reduce((sum, t) => sum + t, 0);
    const averageTimeSpent = times.length ? Math.round((totalTimeSpent / times.length) * 10) / 10 : 0;
    const maxTimeSpent = times.length ? Math.max(...times) : 0;

    const complexityCounts = {
      simple: scoped.filter(t => t.complexity === 'simple').length,
      moderate: scoped.filter(t => t.complexity === 'moderate').length,
      complex: scoped.filter(t => t.complexity === 'complex').length,
      extreme: scoped.filter(t => t.complexity === 'extreme').length,
    };

    return {
      workspace,
      tasks: scoped,
      statusCounts,
      blocked: scoped.filter(isTaskBlocked).length,
      firstCreated: createdTimes.length ? new Date(Math.min(...createdTimes)).toLocaleDateString() : 'No timestamp',
      latestActivity,
      totalTimeSpent,
      averageTimeSpent,
      maxTimeSpent,
      complexityCounts,
    };
  }).filter((item) => item.tasks.length > 0), [filteredGlobalTasks, taskFlags, workspaceList]);
  const openGlobalTaskCreator = () => {
    setGlobalTaskDraft({
      title: '',
      description: '',
      priority: 'medium',
      state: 'todo',
      due: '',
      workspaceId: '',
    });
    setIsGlobalTaskCreateOpen(true);
  };
  const createGlobalTask = async () => {
    const title = globalTaskDraft.title.trim();
    if (!title || !globalTaskDraft.workspaceId) return;
    const localTask: Task = {
      id: `task-${Date.now().toString(36)}`,
      workspaceId: globalTaskDraft.workspaceId,
      title,
      description: globalTaskDraft.description,
      priority: globalTaskDraft.priority,
      state: globalTaskDraft.state,
      owner: 'operator',
      due: globalTaskDraft.due || undefined,
      dependsOn: [],
      comments: [],
    };
    let task = localTask;
    try {
      const response = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks`, {
        method: 'POST',
        headers,
        body: JSON.stringify({
          task_id: localTask.id,
          workspace_id: localTask.workspaceId,
          title,
          description: globalTaskDraft.description,
          status: globalTaskDraft.state,
          priority: globalTaskDraft.priority,
          date: globalTaskDraft.due || undefined,
        }),
      });
      if (response.ok) task = normalizeTaskFromServer(await response.json(), localTask);
    } catch {
      // Keep the local optimistic task when the server is unavailable.
    }
    setTaskList((current) => [task, ...current.filter((item) => item.id !== task.id)]);
    pushToast('Task created', `${title} added to ${workspaceList.find((workspace) => workspace.id === task.workspaceId)?.name ?? task.workspaceId}.`, 'good');
    setIsGlobalTaskCreateOpen(false);
  };
  return (
    <main className="workspace">
      {activeSection !== 'tasks' && <WorkspaceHeader
        eyebrow={hero.eyebrow}
        title={hero.title}
        subtitle={hero.subtitle}
        workspaceColor={activeWorkspace?.color}
        workspacePriority={activeWorkspace?.priority}
        workspaceStatus={activeWorkspace?.status}
        facts={heroFacts}
        showFacts={activeSection !== 'workspace'}
        showActions={activeSection !== 'workspace'}
        onEdit={onEdit}
        onAddNote={() => {
          void onCreateSessionNote('Session note captured from the Sanctum shell.');
          setActiveSection('notes');
        }}
        onOpenKnowledge={() => setIsKnowledgeOpen(true)}
        onOpenActivity={() => setIsActivityOpen(true)}
        workspaceId={activeWorkspaceId}
      />}
      {activeSection === 'workspace' ? (
        <WorkspaceOverview
          workspaceSessions={workspaceSessions}
          workspaceTasks={workspaceTasks}
          workspaceNotes={workspaceNotes}
          workspaceArtifacts={workspaceArtifacts}
          workspaceMergeRequests={workspaceMergeRequests}
          workspaceJiraTickets={workspaceJiraTickets}
          workspaceSummary={workspaceSummary}
        workspaceActivitySummary={workspaceActivitySummary}
        taskFlags={taskFlags}
        onTaskSelect={(task) => pushToast('Task selected', `${task.title} · ${task.state}.`, task.state === 'in-progress' ? 'good' : 'muted')}
        onOpenSessions={onOpenSessions}
        onOpenTasks={() => setIsTaskDrawerOpen(true)}
        onOpenNotes={() => setIsNotesDrawerOpen(true)}
        onOpenActivity={() => setIsActivityOpen(true)}
        onOpenArtifacts={() => setIsArtifactsDrawerOpen(true)}
        onOpenKnowledge={() => setIsKnowledgeOpen(true)}
        onOpenMergeRequests={() => setIsMergeRequestsDrawerOpen(true)}
        onOpenJira={() => setIsJiraDrawerOpen(true)}
        />
      ) : activeSection === 'tasks' ? (
        <section className="task-manager-shell" style={{ paddingTop: 8 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12, padding: '10px 14px', border: '1px solid rgba(0,229,255,0.18)', background: 'rgba(0,229,255,0.03)' }}>
            <div>
              <div style={{ fontFamily: "'Share Tech Mono', monospace", letterSpacing: '0.18em', textTransform: 'uppercase', color: 'var(--cyan)', fontSize: 10 }}>Task Manager</div>
              <h2 style={{ margin: '4px 0 0', fontFamily: "'Orbitron', monospace", fontSize: 18, color: 'var(--text)' }}>Cross-workspace board</h2>
            </div>
          </div>
          <section className="panel panel-spanning task-manager-panel" style={{ minHeight: 'calc(100vh - 220px)' }}>
            <div className="task-dashboard-filters mb-4">
              <label className="task-editor-field">
                <span>Search</span>
                <input
                  type="search"
                  value={taskTextFilter}
                  onChange={(event) => setTaskTextFilter(event.target.value)}
                  placeholder="Title or description..."
                />
              </label>
              <label className="task-editor-field">
                <span>Workspace</span>
                <select value={taskWorkspaceFilter} onChange={(event) => setTaskWorkspaceFilter(event.target.value)}>
                  <option value="">All workspaces</option>
                  {workspaceList.map((workspace) => <option key={workspace.id} value={workspace.id}>{workspace.name}</option>)}
                </select>
              </label>
              <label className="task-editor-field">
                <span>Status</span>
                <select value={taskStatusFilter} onChange={(event) => setTaskStatusFilter(event.target.value)}>
                  <option value="">All statuses</option>
                  {(['backlog', 'grooming', 'ready', 'in-progress', 'review', 'human-review', 'approved', 'done'] as const).map((state) => <option key={state} value={state}>{state}</option>)}
                </select>
              </label>
              <label className="task-editor-field">
                <span>Priority</span>
                <select value={taskPriorityFilter} onChange={(event) => setTaskPriorityFilter(event.target.value)}>
                  <option value="">All priorities</option>
                  {(['critical', 'high', 'medium', 'low'] as const).map((priority) => <option key={priority} value={priority}>{priority}</option>)}
                </select>
              </label>
            </div>
            <div className="task-view-toggle mb-4" role="tablist" aria-label="Task view">
              <button type="button" className={taskViewMode === 'overview' ? 'is-active' : ''} onClick={() => setTaskViewMode('overview')}><ListChecks size={13} /> Overview</button>
              <button type="button" className={taskViewMode === 'analytics' ? 'is-active' : ''} onClick={() => setTaskViewMode('analytics')}><BarChart3 size={13} /> Analytics</button>
              <button type="button" className={taskViewMode === 'visualization' ? 'is-active' : ''} onClick={() => setTaskViewMode('visualization')}><Network size={13} /> Visual</button>
            </div>
            {taskViewMode === 'overview' ? (
              <div className="task-kanban-grid task-manager-kanban-grid">
                {filteredTaskColumns.map((column) => (
                  <section key={column.state} className="task-kanban-column">
                    <div className="task-kanban-head">
                      <span>{column.state}</span>
                      <strong>{column.tasks.length}</strong>
                    </div>
                    <div
                      className={`task-kanban-list ${draggingTaskId ? 'is-drop-ready' : ''}`}
                      onDragOver={(event) => event.preventDefault()}
                      onDrop={(event) => {
                        event.preventDefault();
                        const taskId = event.dataTransfer.getData('text/plain') || draggingTaskId;
                        if (taskId) moveGlobalTask(taskId, column.state);
                      }}
                    >
                      {column.tasks.map((task) => (
                        <div
                          key={task.id}
                          className="task-kanban-card"
                          draggable={false}
                          onDragStart={(event) => {
                            if (isTaskBlocked(task, taskFlags)) {
                              event.preventDefault();
                              return;
                            }
                            setDraggingTaskId(task.id);
                            event.dataTransfer.setData('text/plain', task.id);
                            event.dataTransfer.effectAllowed = 'move';
                          }}
                          onDragEnd={() => setDraggingTaskId(null)}
                          onClick={() => onEditGlobalTask(task)}
                          role="button"
                          tabIndex={0}
                        >
                          <div className="task-card-title">{task.title}</div>
                          <div className="task-card-description">{task.description || 'No description.'}</div>
                          <div className="task-card-meta">
                            <span>{task.priority}</span>
                            <span>{workspaceList.find((workspace) => workspace.id === task.workspaceId)?.name ?? task.workspaceId}</span>
                          </div>
                          <div className="task-card-footer">
                            <div className="task-card-footer-state">
                              <div className="task-card-state">{taskFlags[task.id]?.done ? 'done' : taskWorkflowState(task, taskFlags)}</div>
                              {isTaskBlocked(task, taskFlags) && <div className="task-blocked-badge"><Ban size={11} /> Blocked</div>}
                            </div>
                            <div className="task-card-actions">
                              {isTaskBlocked(task, taskFlags) ? (
                                <button
                                  type="button"
                                  className="task-card-block is-unblock-btn"
                                  aria-label={`Unblock ${task.title}`}
                                  title="Unblock task"
                                  onClick={(event) => { event.stopPropagation(); toggleTaskBlocked(task); }}
                                >
                                  Unblock
                                </button>
                              ) : (task.state !== 'human-review' && task.state !== 'approved' && task.state !== 'done') ? (
                                <button
                                  type="button"
                                  className="task-card-block is-block-btn"
                                  aria-label={`Block ${task.title}`}
                                  title="Block task"
                                  onClick={(event) => { event.stopPropagation(); toggleTaskBlocked(task); }}
                                >
                                  Block
                                </button>
                              ) : null}
                              <button
                                type="button"
                                className="task-card-remove"
                                aria-label={`Delete ${task.title}`}
                                title="Delete task"
                                onClick={(event) => { event.stopPropagation(); removeGlobalTask(task.id); }}
                              >
                                <Trash2 size={14} />
                              </button>
                            </div>
                          </div>
                        </div>
                      ))}
                    </div>
                  </section>
                ))}
              </div>
            ) : taskViewMode === 'visualization' ? (
              <div className="task-workflow-shell">
                {filteredGlobalTasks.length === 0 ? (
                  <div className="activity-empty">No tasks to visualize.</div>
                ) : (
                  <>
                    <div className="task-dashboard-stats" style={{ marginBottom: 20, gridTemplateColumns: 'repeat(3, minmax(0, 1fr))' }}>
                      <div className="task-dashboard-stat task-dashboard-stat-active">
                        <span>Total Time Spent</span>
                        <strong>{visualTimeStats.total} hrs</strong>
                      </div>
                      <div className="task-dashboard-stat task-dashboard-stat-review">
                        <span>Average Time Spent</span>
                        <strong>{visualTimeStats.average} hrs</strong>
                      </div>
                      <div className="task-dashboard-stat task-dashboard-stat-critical">
                        <span>Max Time Spent</span>
                        <strong>{visualTimeStats.max} hrs</strong>
                      </div>
                    </div>
                    <div
                      className="relative overflow-auto"
                      onWheel={(event) => {
                        event.preventDefault();
                        const zoomDelta = event.deltaY < 0 ? 0.1 : -0.1;
                        setVisualZoom((z) => Math.min(3, Math.max(0.3, +(z + zoomDelta).toFixed(2))));
                      }}
                    >
                      <div className="sticky top-2 right-2 ml-auto w-fit flex items-center gap-1 z-10 bg-black/70 border border-white/10 p-1 rounded-md backdrop-blur-md">
                        <button
                          type="button"
                          onClick={() => setVisualZoom((z) => Math.min(3, +(z + 0.15).toFixed(2)))}
                          title="Zoom In"
                          aria-label="Zoom in visualization"
                          className="p-1.5 hover:bg-white/10 rounded text-cyan-400 transition-colors"
                        >
                          <ZoomIn size={14} />
                        </button>
                        <span className="text-[10px] font-mono px-1 text-white/70 min-w-[36px] text-center">
                          {Math.round(visualZoom * 100)}%
                        </span>
                        <button
                          type="button"
                          onClick={() => setVisualZoom((z) => Math.max(0.3, +(z - 0.15).toFixed(2)))}
                          title="Zoom Out"
                          aria-label="Zoom out visualization"
                          className="p-1.5 hover:bg-white/10 rounded text-cyan-400 transition-colors"
                        >
                          <ZoomOut size={14} />
                        </button>
                        <button
                          type="button"
                          onClick={() => setVisualZoom(1)}
                          title="Reset Zoom"
                          aria-label="Reset visualization zoom"
                          className="p-1.5 hover:bg-white/10 rounded text-cyan-400 transition-colors"
                        >
                          <Maximize size={14} />
                        </button>
                      </div>
                      <div
                        style={{
                          transform: `scale(${visualZoom})`,
                          transformOrigin: 'top left',
                          transition: 'transform 0.1s ease-out',
                        }}
                      >
                        <svg
                          className="task-workflow-svg task-workflow-svg-contained"
                          viewBox={`0 0 ${globalTaskWorkflow.width} ${globalTaskWorkflow.height}`}
                          preserveAspectRatio="xMidYMin meet"
                          role="img"
                          aria-label="Global task dependency map"
                        >
                          <defs>
                            <marker id="task-arrow-global" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto" markerUnits="strokeWidth">
                              <path d="M0,0 L0,6 L9,3 z" fill="rgba(0,229,255,0.7)" />
                            </marker>
                          </defs>
                          {globalTaskWorkflow.edges.map((edge) => (
                            <path
                              key={edge.id}
                              className="task-workflow-edge"
                              markerEnd="url(#task-arrow-global)"
                              d={`M${edge.source.x},${edge.source.y + 30} C${edge.source.x},${edge.source.y + 58} ${edge.target.x},${edge.target.y - 58} ${edge.target.x},${edge.target.y - 30}`}
                            />
                          ))}
                          {globalTaskWorkflow.nodes.map(({ task, x, y }) => (
                              <g key={task.id} className="task-workflow-node" role="button" tabIndex={0} onClick={() => pushToast('Task selected', `${task.title} · ${workspaceList.find((workspace) => workspace.id === task.workspaceId)?.name ?? task.workspaceId}.`, 'muted')}>
                                <rect x={x - 84} y={y - 28} width="168" height="56" rx="0" className={`task-workflow-card priority-${task.priority}`} />
                                <text x={x} y={y - 7} textAnchor="middle" className="task-workflow-title">{task.title.length > 28 ? `${task.title.slice(0, 27)}…` : task.title}</text>
                                <text x={x} y={y + 13} textAnchor="middle" className="task-workflow-meta">{task.priority} · {workspaceList.find((workspace) => workspace.id === task.workspaceId)?.name ?? task.workspaceId}</text>

                                {/* Complexity Badge */}
                                {task.complexity && (
                                  <g>
                                    <rect
                                      x={x + 35}
                                      y={y - 24}
                                      width="45"
                                      height="12"
                                      rx="2"
                                      fill={
                                        task.complexity === 'extreme' ? 'rgba(255, 0, 85, 0.15)' :
                                        task.complexity === 'complex' ? 'rgba(255, 170, 0, 0.15)' :
                                        task.complexity === 'moderate' ? 'rgba(0, 229, 255, 0.15)' :
                                        'rgba(0, 230, 118, 0.15)'
                                      }
                                      stroke={
                                        task.complexity === 'extreme' ? '#ff0055' :
                                        task.complexity === 'complex' ? '#ffaa00' :
                                        task.complexity === 'moderate' ? '#00e5ff' :
                                        '#00e676'
                                      }
                                      strokeWidth="0.5"
                                    />
                                    <text
                                      x={x + 57.5}
                                      y={y - 15}
                                      textAnchor="middle"
                                      style={{
                                        fontSize: '7px',
                                        fill: '#fff',
                                        fontWeight: 'bold',
                                        fontFamily: "'Share Tech Mono', monospace",
                                        textTransform: 'uppercase',
                                        letterSpacing: '0.05em'
                                      }}
                                    >
                                      {task.complexity}
                                    </text>
                                  </g>
                                )}

                                {(task.dependsOn ?? []).some((id) => filteredGlobalTasks.some((candidate) => candidate.id === id)) && <circle cx={x + 70} cy={y + 15} r="4" className="task-workflow-link-dot" />}
                              </g>
                          ))}
                        </svg>
                      </div>
                    </div>
                  </>
                )}
              </div>
            ) : (
              <>
                <section className="task-status-overview" aria-label="Task Status Overview" style={{ marginBottom: 20 }}>
                  <div className="task-status-overview-head">
                    <div>
                      <div className="eyebrow">Analytics Overview</div>
                      <h3 style={{ margin: 0, fontSize: 16 }}>Task Status Overview</h3>
                    </div>
                    <div className="flex items-center gap-3">
                      <button
                        type="button"
                        onClick={() => setHideDoneInAnalytics((prev) => !prev)}
                        className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-mono border transition-all ${
                          hideDoneInAnalytics
                            ? 'bg-amber-950/40 border-amber-500/40 text-amber-300 hover:bg-amber-950/60'
                            : 'bg-emerald-950/40 border-emerald-500/40 text-emerald-300 hover:bg-emerald-950/60'
                        }`}
                        title={hideDoneInAnalytics ? 'Click to unhide done tasks' : 'Click to hide done tasks'}
                      >
                        {hideDoneInAnalytics ? 'Unhide Done' : 'Hide Done'}
                      </button>
                      <div className="task-overview-summary flex items-center gap-3 bg-cyan-950/30 border border-cyan-500/20 px-3 py-1.5 rounded-md">
                        <span className="text-cyan-300 font-mono text-xs">{globalTaskStats.workspaceCount} workspaces</span>
                        <span className="text-white/20">|</span>
                        <strong className="text-cyan-400 font-mono text-sm">{globalTaskStats.completionRate}% complete</strong>
                      </div>
                    </div>
                  </div>
                  <div className="grid grid-cols-8 gap-2 mt-3">
                    {[
                      { label: 'All tasks', value: globalTaskStats.total, tone: 'all', icon: Layers, border: 'border-cyan-500/20', bg: 'bg-cyan-950/20', text: 'text-cyan-400' },
                      { label: 'Backlog', value: globalTaskStats.backlog, tone: 'backlog', icon: Inbox, border: 'border-slate-500/20', bg: 'bg-slate-900/40', text: 'text-slate-300' },
                      { label: 'Ready', value: globalTaskStats.ready, tone: 'ready', icon: Clock, border: 'border-indigo-500/20', bg: 'bg-indigo-950/20', text: 'text-indigo-400' },
                      { label: 'In progress', value: globalTaskStats.active, tone: 'active', icon: PlayCircle, border: 'border-amber-500/20', bg: 'bg-amber-950/20', text: 'text-amber-400' },
                      { label: 'Review', value: globalTaskStats.review, tone: 'review', icon: Sparkles, border: 'border-purple-500/20', bg: 'bg-purple-950/20', text: 'text-purple-400' },
                      { label: 'Blocked', value: globalTaskStats.blocked, tone: 'blocked', icon: AlertTriangle, border: 'border-rose-500/20', bg: 'bg-rose-950/20', text: 'text-rose-400' },
                      { label: 'Done', value: globalTaskStats.completed, tone: 'done', icon: CheckCircle2, border: 'border-emerald-500/20', bg: 'bg-emerald-950/20', text: 'text-emerald-400' },
                      { label: 'Critical open', value: globalTaskStats.critical, tone: 'critical', icon: ShieldAlert, border: 'border-red-500/30', bg: 'bg-red-950/30', text: 'text-red-400' },
                    ].filter(({ tone }) => !hideDoneInAnalytics || tone !== 'done').map(({ label, value, icon: Icon, border, bg, text }) => (
                      <div
                        key={label}
                        className={`flex flex-col justify-between p-2.5 rounded border ${border} ${bg} transition-all hover:border-white/20`}
                      >
                        <div className="flex items-center justify-between gap-1 text-[10px] text-white/50 uppercase tracking-wider font-mono">
                          <span className="truncate">{label}</span>
                          <Icon size={12} className={`${text} shrink-0 opacity-80`} />
                        </div>
                        <div className="mt-1 flex items-baseline justify-between">
                          <span className={`text-xl font-bold font-mono ${text}`}>{value}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </section>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4" aria-label="Tasks by workspace analytics">
                {workspaceTaskAnalytics.map(({ workspace, tasks: scopedTasks, statusCounts, blocked, firstCreated, latestActivity, totalTimeSpent, averageTimeSpent, maxTimeSpent, complexityCounts }) => {
                  const doneCount = statusCounts.find((s) => s.status === 'done')?.count ?? 0;
                  const pct = scopedTasks.length ? Math.round((doneCount / scopedTasks.length) * 100) : 0;

                  return (
                    <section key={workspace.id} className="p-4 rounded-xl border border-cyan-500/20 bg-gradient-to-b from-slate-900/90 to-slate-950/95 backdrop-blur flex flex-col justify-between gap-3 shadow-lg shadow-black/40 transition-all hover:border-cyan-500/40 hover:shadow-cyan-950/20">
                      {/* Workspace Header & Total */}
                      <div className="flex flex-col gap-2 border-b border-white/10 pb-3">
                        <div className="flex items-center justify-between">
                          <span className="px-2 py-0.5 rounded text-[9px] uppercase font-mono tracking-widest bg-cyan-950/80 border border-cyan-500/30 text-cyan-300 font-semibold">
                            Workspace
                          </span>
                          <span className="text-2xl font-black font-mono text-cyan-400 bg-cyan-950/40 border border-cyan-500/20 px-3 py-0.5 rounded-lg shadow-inner">
                            {scopedTasks.length}
                          </span>
                        </div>
                        <h3 className="text-base font-bold text-white tracking-wide truncate" title={workspace.name}>{workspace.name}</h3>
                        <div className="flex items-center justify-between text-xs font-mono mt-1">
                          <span className="text-white/40">Completion:</span>
                          <div className="flex items-center gap-2">
                            <span className="text-emerald-400 font-bold text-sm">{pct}%</span>
                            <span className="text-white/30 text-[10px]">({doneCount}/{scopedTasks.length})</span>
                          </div>
                        </div>
                        <div className="w-full bg-slate-950 h-2 rounded-full overflow-hidden flex border border-white/10 mt-1">
                          {statusCounts.map(({ status, count }) => {
                            if (!count || !scopedTasks.length) return null;
                            const widthPct = (count / scopedTasks.length) * 100;
                            const barBg: Record<string, string> = {
                              todo: 'bg-slate-500',
                              backlog: 'bg-slate-500',
                              ready: 'bg-indigo-500',
                              'in-progress': 'bg-amber-500',
                              review: 'bg-purple-500',
                              'human-review': 'bg-sky-500',
                              done: 'bg-emerald-500',
                            };
                            return (
                              <div
                                key={`bar-${status}`}
                                style={{ width: `${widthPct}%` }}
                                className={`h-full ${barBg[status] || 'bg-cyan-500'}`}
                                title={`${status}: ${count} (${Math.round(widthPct)}%)`}
                              />
                            );
                          })}
                        </div>
                      </div>

                      {/* Status Distribution Grid */}
                      <div className="grid grid-cols-2 gap-1.5 py-1">
                        {statusCounts.filter(({ status }) => !hideDoneInAnalytics || status !== 'done').map(({ status, count }) => {
                          const statusColors: Record<string, { bg: string; border: string; text: string }> = {
                            todo: { bg: 'bg-slate-900/60', border: 'border-slate-800', text: 'text-slate-300' },
                            backlog: { bg: 'bg-slate-900/60', border: 'border-slate-800', text: 'text-slate-300' },
                            ready: { bg: 'bg-indigo-950/40', border: 'border-indigo-500/30', text: 'text-indigo-300' },
                            'in-progress': { bg: 'bg-amber-950/40', border: 'border-amber-500/30', text: 'text-amber-300' },
                            review: { bg: 'bg-purple-950/40', border: 'border-purple-500/30', text: 'text-purple-300' },
                            'human-review': { bg: 'bg-sky-950/40', border: 'border-sky-500/30', text: 'text-sky-300' },
                            done: { bg: 'bg-emerald-950/40', border: 'border-emerald-500/30', text: 'text-emerald-300' },
                          };
                          const style = statusColors[status] || { bg: 'bg-slate-900/60', border: 'border-slate-800', text: 'text-slate-300' };

                          return (
                            <div key={status} className={`flex items-center justify-between px-2.5 py-1.5 rounded-md border ${style.border} ${style.bg}`}>
                              <span className="text-[10px] font-mono uppercase text-white/50">{status}</span>
                              <span className={`text-xs font-mono font-bold ${style.text}`}>{count}</span>
                            </div>
                          );
                        })}
                      </div>

                      {/* Key Indicators: Time Metrics & Complexity */}
                      <div className="flex flex-col gap-2 bg-black/40 border border-white/5 p-2.5 rounded-lg text-[11px] font-mono">
                        <div className="flex items-center justify-between border-b border-white/5 pb-1.5">
                          <span className="text-white/40 text-[9px] uppercase tracking-wider">Time Metrics</span>
                          <div className="flex items-center gap-2">
                            <span className="text-cyan-400 font-bold">{totalTimeSpent}h total</span>
                            <span className="text-white/20">|</span>
                            <span className="text-white/40 text-[10px]">avg {averageTimeSpent}h</span>
                            <span className="text-white/20">|</span>
                            <span className="text-white/40 text-[10px]">max {maxTimeSpent}h</span>
                          </div>
                        </div>
                        <div className="flex items-center justify-between pt-0.5">
                          <span className="text-white/40 text-[9px] uppercase tracking-wider">Complexity</span>
                          <div className="flex items-center gap-1">
                            <span className="px-1.5 py-0.5 rounded bg-emerald-950/40 border border-emerald-500/20 text-emerald-400 text-[10px]">Simple:{complexityCounts.simple}</span>
                            <span className="px-1.5 py-0.5 rounded bg-cyan-950/40 border border-cyan-500/20 text-cyan-400 text-[10px]">Mod:{complexityCounts.moderate}</span>
                            <span className="px-1.5 py-0.5 rounded bg-amber-950/40 border border-amber-500/20 text-amber-400 text-[10px]">Comp:{complexityCounts.complex}</span>
                            <span className="px-1.5 py-0.5 rounded bg-rose-950/40 border border-rose-500/20 text-rose-400 text-[10px]">Extr:{complexityCounts.extreme}</span>
                          </div>
                        </div>
                      </div>

                      {/* Card Footer: Metadata & Latest Event */}
                      <div className="flex flex-col gap-1.5 pt-2 border-t border-white/10 text-[11px]">
                        <div className="flex items-center justify-between text-[10px] text-white/40 font-mono">
                          <span>Blocked: <strong className={blocked > 0 ? 'text-rose-400' : 'text-slate-400'}>{blocked}</strong></span>
                          <span>First created: <strong className="text-white/70">{firstCreated}</strong></span>
                        </div>
                        <div className="flex flex-col gap-0.5 text-white/50 pt-1">
                          <span className="text-[9px] uppercase font-mono text-white/30">Latest Lifecycle Event</span>
                          {latestActivity ? (
                            <div className="flex flex-col">
                              <span className="font-semibold text-white truncate text-[11px]">{latestActivity.task.title}</span>
                              <span className="text-cyan-300/80 font-mono text-[9px]">
                                {taskFlags[latestActivity.task.id]?.lastMovedAt
                                  ? `Moved ${taskFlags[latestActivity.task.id]?.lastMovedFrom} → ${taskFlags[latestActivity.task.id]?.lastMovedTo}`
                                  : latestActivity.task.updatedAt ? 'Last updated' : 'Created'}
                                {' · '}{new Date(latestActivity.timestamp).toLocaleString()}
                              </span>
                            </div>
                          ) : <em className="text-white/30 text-[10px]">No activity recorded</em>}
                        </div>
                      </div>
                    </section>
                  );
                })}
                </div>
                {workspaceTaskAnalytics.length === 0 && <div className="activity-empty">No workspace task statistics match these filters.</div>}
              </>
            )}
          </section>
        </section>
      ) : (
        <section className="content-grid">
          <WorkspaceSessionPanel
            surfaceTitle={surfaceTitle}
            activeModeLabel={activeModeLabel}
            activeSection={activeSection}
            activeSession={activeSession}
            workspaceList={workspaceList}
            workspaceIndex={workspaceIndex}
            setWorkspaceIndex={setWorkspaceIndex}
            setSessionIndex={setSessionIndex}
            workspaceSessions={workspaceSessions}
            sessionStats={sessionStats}
            sessionCheckpoints={sessionCheckpoints}
            sessionTimeline={sessionTimeline}
            sessionNotes={sessionNotes}
            sessionArtifacts={sessionArtifacts}
            sessionFileGroup={workspaceSessionFiles[activeSession.id]}
            sessionConversation={sessionConversation}
            setActiveSection={setActiveSection}
            pushToast={pushToast}
            taskFlags={taskFlags}
          />
          <section className="panel">
            <PanelHeader title="Tasks" action="Prioritize" />
            <div className="list">{workspaceTasks.map((task) => <button key={task.id} type="button" className="row row-button" onClick={() => pushToast('Task selected', `${task.title} · ${task.state}.`, task.state === 'in-progress' ? 'good' : 'muted')}><div><div className="row-title">{task.title}</div><div className="row-meta">{`${task.priority} · ${task.owner}`}</div></div><div className="row-detail">{taskFlags[task.id]?.done ? 'done' : task.state}</div></button>)}</div>
          </section>
        </section>
      )}

      {isGlobalTaskCreateOpen && (
        <div className="modal-backdrop" role="presentation" onClick={() => setIsGlobalTaskCreateOpen(false)}>
          <form className="modal-card task-info-modal" role="dialog" aria-modal="true" aria-labelledby="global-task-title" onSubmit={(event) => { event.preventDefault(); void createGlobalTask(); }} onClick={(event) => event.stopPropagation()}>
            <div className="modal-head">
              <div>
                <div className="eyebrow">Add task</div>
                <h2 id="global-task-title">New cross-workspace task</h2>
              </div>
              <button type="button" className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsGlobalTaskCreateOpen(false)}><X size={14} /></button>
            </div>

            <div className="task-editor-grid">
              <div className="task-editor-main">
                <label className="task-editor-field">
                  <span>Title</span>
                  <input value={globalTaskDraft.title} onChange={(event) => setGlobalTaskDraft((current) => ({ ...current, title: event.target.value }))} placeholder="Task title" />
                </label>
                <label className="task-editor-field">
                  <span>Detail</span>
                  <textarea value={globalTaskDraft.description} onChange={(event) => setGlobalTaskDraft((current) => ({ ...current, description: event.target.value }))} placeholder="Task detail, acceptance notes, or implementation context." />
                </label>
              </div>
              <aside className="task-editor-side">
                <label className="task-editor-field">
                  <span>Workspace</span>
                  <select required value={globalTaskDraft.workspaceId} onChange={(event) => setGlobalTaskDraft((current) => ({ ...current, workspaceId: event.target.value }))}>
                    <option value="">Select workspace...</option>
                    {workspaceList.map((workspace) => <option key={workspace.id} value={workspace.id}>{workspace.name}</option>)}
                  </select>
                </label>
                <label className="task-editor-field">
                  <span>Status</span>
                  <select value={globalTaskDraft.state} onChange={(event) => setGlobalTaskDraft((current) => ({ ...current, state: event.target.value as Task['state'] }))}>
                    <option value="backlog">backlog</option>
                    <option value="ready">ready</option>
                    <option value="in-progress">in progress</option>
                    <option value="review">review</option>
                    <option value="done">done</option>
                  </select>
                </label>
                <label className="task-editor-field">
                  <span>Priority</span>
                  <select value={globalTaskDraft.priority} onChange={(event) => setGlobalTaskDraft((current) => ({ ...current, priority: event.target.value as Task['priority'] }))}>
                    <option value="critical">critical</option>
                    <option value="high">high</option>
                    <option value="medium">medium</option>
                    <option value="low">low</option>
                  </select>
                </label>
                <label className="task-editor-field">
                  <span>Date</span>
                  <input type="date" value={globalTaskDraft.due} onChange={(event) => setGlobalTaskDraft((current) => ({ ...current, due: event.target.value }))} />
                </label>
              </aside>
            </div>

            <div className="modal-actions">
              <button type="submit" className="ghost-btn action-save icon-only" aria-label="Create task" title="Create task"><Check size={14} /></button>
              <button type="button" className="ghost-btn action-close icon-only" aria-label="Cancel" title="Cancel" onClick={() => setIsGlobalTaskCreateOpen(false)}><X size={14} /></button>
            </div>
          </form>
        </div>
      )}
      {activeSection === 'manage' && <WorkspaceManagePanel manageSummary={manageSummary} />}
    </main>
  );
}
