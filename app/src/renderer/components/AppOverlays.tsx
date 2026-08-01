import { Ban, Check, ChevronDown, Circle, FileCode, FileText, FolderOpen, GitBranch, History, ListChecks, Maximize, MessageSquare, Network, Plus, Timer, Trash2, User, X, Zap, Copy, Loader2, Send, Sparkles, ZoomIn, ZoomOut } from 'lucide-react';
import { useEffect, useMemo, useState, type Dispatch, type SetStateAction } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { KnowledgeGraph } from '../KnowledgeGraph';
import { buildColosseumPhaseConfigs, SettingsModal, type ColosseumSettings } from './SettingsModal';
import type { Artifact, Note, Provider, Reminder, Session, Task, TaskComment, Workspace } from '../data';
import { canMoveTask, canSubmitForGrooming, isTaskBlocked, taskBoardState, taskWorkflowState, type TaskFlagState } from '../lib/taskBoard';
import { buildSavantHeaders } from '../services/httpClient';
import { WorkspaceSessionsDrawer } from './WorkspaceSessionsDrawer';
import { WorkspaceSessionDetailsDrawer } from './WorkspaceSessionDetailsDrawer';
import { ColosseumPhaseLedger, ColosseumRunLedger } from './ColosseumTaskEvidence';
import { getSessionAdapter, inferSessionProvider, type SessionConversationMessage, type SessionFileGroup } from '../services/sessionAdapters';

type WorkspaceMergeRequest = {
  id: string;
  workspaceId: string;
  mrId: string;
  title: string;
  status: string;
  url?: string;
  createdAt?: string;
  updatedAt?: string;
};

type WorkspaceJiraTicket = {
  id: string;
  workspaceId: string;
  ticketId: string;
  ticketKey: string;
  title: string;
  status: string;
  createdAt?: string;
  updatedAt?: string;
};

type EntityEditorState =
  | { kind: 'merge-request'; mode: 'create' | 'edit'; item?: WorkspaceMergeRequest; title: string; url: string; status: string }
  | { kind: 'jira'; mode: 'create' | 'edit'; item?: WorkspaceJiraTicket; ticketKey: string; title: string; status: string };

type ActivityItem = {
  id: string;
  label: string;
  detail: string;
  section: string;
  surfaceMode: string;
  workspaceIndex: number;
  sessionIndex: number;
};

type AppOverlaysProps = {
  isActivityOpen: boolean;
  setIsActivityOpen: Dispatch<SetStateAction<boolean>>;
  isAlertsOpen: boolean;
  setIsAlertsOpen: Dispatch<SetStateAction<boolean>>;
  isKnowledgeOpen: boolean;
  setIsKnowledgeOpen: Dispatch<SetStateAction<boolean>>;
  isTaskDrawerOpen: boolean;
  setIsTaskDrawerOpen: Dispatch<SetStateAction<boolean>>;
  taskCreateRequest: number;
  isNotesDrawerOpen: boolean;
  setIsNotesDrawerOpen: Dispatch<SetStateAction<boolean>>;
  isMergeRequestsDrawerOpen: boolean;
  setIsMergeRequestsDrawerOpen: Dispatch<SetStateAction<boolean>>;
  isJiraDrawerOpen: boolean;
  setIsJiraDrawerOpen: Dispatch<SetStateAction<boolean>>;
  isArtifactsDrawerOpen: boolean;
  setIsArtifactsDrawerOpen: Dispatch<SetStateAction<boolean>>;
  isSessionsDrawerOpen: boolean;
  setIsSessionsDrawerOpen: Dispatch<SetStateAction<boolean>>;
  setSessionIndex: Dispatch<SetStateAction<number>>;
  activeSessionId: string;
  workspaceSessionFileGroups: Record<string, SessionFileGroup>;
  sessionConversationMap: Record<string, SessionConversationMessage[]>;
  settingsOpen: boolean;
  setSettingsOpen: Dispatch<SetStateAction<boolean>>;
  isEditOpen: boolean;
  setIsEditOpen: Dispatch<SetStateAction<boolean>>;
  workspaceEditorMode: 'create' | 'edit';
  isHelpOpen: boolean;
  setIsHelpOpen: Dispatch<SetStateAction<boolean>>;
  authUser: string;
  authDraft: string;
  serverDraft: string;
  gatewayDraft: string;
  hasApiKey: boolean;
  setAuthUser: (value: string) => void;
  setAuthDraft: (value: string) => void;
  setServerDraft: (value: string) => void;
  setGatewayDraft: (value: string) => void;
  selectedProvider: string;
  selectedModel: string;
  setSelectedProvider: (value: string) => void;
  setSelectedModel: (value: string) => void;
  gatewayProviders: Array<{ id: string; label: string; models: string[] }>;
  onSaveSettings: () => Promise<void> | void;
  onLogout: () => Promise<void> | void;
  onRefreshProviders?: () => Promise<void> | void;
  colosseumGroomingSettings: ColosseumSettings;
  colosseumReadySettings: ColosseumSettings;
  colosseumReviewSettings: ColosseumSettings;
  onColosseumGroomingSettingsChange: (val: ColosseumSettings) => void;
  onColosseumReadySettingsChange: (val: ColosseumSettings) => void;
  onColosseumReviewSettingsChange: (val: ColosseumSettings) => void;
  onDeleteSession?: (sessionId: string) => void;
  recentAlerts: {
    id: string;
    title: string;
    detail: string;
    tone: 'good' | 'warning' | 'muted';
    createdAt: number;
  }[];
  editDraft: {
    workspaceName: string;
    workspaceDescription: string;
    workspacePriority: 'critical' | 'high' | 'medium' | 'low';
    workspaceStatus: 'open' | 'closed';
    workspaceColor: string;
  };
  setEditDraft: Dispatch<SetStateAction<{
    workspaceName: string;
    workspaceDescription: string;
    workspacePriority: 'critical' | 'high' | 'medium' | 'low';
    workspaceStatus: 'open' | 'closed';
    workspaceColor: string;
  }>>;
  saveEditDraft: () => void;
  providers: Provider[];
  activityFeed: ActivityItem[];
  restoreActivityContext: (item: ActivityItem) => void;
  managementDrawer: 'tasks' | 'reminders' | null;
  setManagementDrawer: Dispatch<SetStateAction<'tasks' | 'reminders' | null>>;
  workspaceTasks: Task[];
  taskDrawerScope: 'global' | 'workspace';
  workspaceList: Workspace[];
  taskToEdit: Task | null;
  onTaskEditOpened: () => void;
  setTaskList: Dispatch<SetStateAction<Task[]>>;
  workspaceNotes: Note[];
  workspaceArtifacts: Artifact[];
  workspaceSessions: Session[];
  workspaceReminders: Reminder[];
  displayedWorkspaceName: string;
  taskFlags: Record<string, TaskFlagState>;
  setTaskFlags: Dispatch<SetStateAction<Record<string, TaskFlagState>>>;
  reminderFlags: Record<string, { done?: boolean }>;
  setReminderFlags: Dispatch<SetStateAction<Record<string, { done?: boolean }>>>;
  pushToast: (title: string, detail: string, tone?: 'good' | 'warning' | 'muted') => void;
  activeWorkspaceId: string;
  serverBaseUrl: string;
  apiKey: string;
  taskAthenaChats?: Record<string, { messages: any[]; isLoading: boolean; error: string; input: string }>;
  onSendTaskAthenaMessage?: (taskId: string, text: string) => Promise<void> | void;
  onDeleteTaskAthenaMessage?: (taskId: string, messageId: string) => void;
  onClearTaskAthenaChat?: (taskId: string) => void;
  onChangeTaskAthenaInput?: (taskId: string, text: string) => void;
};

export function AppOverlays(props: AppOverlaysProps) {
  const {
    isActivityOpen,
    setIsActivityOpen,
    isAlertsOpen,
    setIsAlertsOpen,
    isKnowledgeOpen,
    setIsKnowledgeOpen,
    isTaskDrawerOpen,
    setIsTaskDrawerOpen,
    taskCreateRequest,
    isNotesDrawerOpen,
    setIsNotesDrawerOpen,
    isMergeRequestsDrawerOpen,
    setIsMergeRequestsDrawerOpen,
    isJiraDrawerOpen,
    setIsJiraDrawerOpen,
    isArtifactsDrawerOpen,
    setIsArtifactsDrawerOpen,
    isSessionsDrawerOpen,
    setIsSessionsDrawerOpen,
    setSessionIndex,
    activeSessionId,
    workspaceSessionFileGroups,
    sessionConversationMap,
    settingsOpen,
    setSettingsOpen,
    isEditOpen,
    setIsEditOpen,
    workspaceEditorMode,
    isHelpOpen,
    setIsHelpOpen,
    authUser,
    authDraft,
    serverDraft,
    gatewayDraft,
    hasApiKey,
    setAuthUser,
    setAuthDraft,
    setServerDraft,
    setGatewayDraft,
    selectedProvider,
    selectedModel,
    setSelectedProvider,
    setSelectedModel,
    gatewayProviders,
    onSaveSettings,
    onLogout,
    onRefreshProviders,
    colosseumGroomingSettings,
    colosseumReadySettings,
    colosseumReviewSettings,
    onColosseumGroomingSettingsChange,
    onColosseumReadySettingsChange,
    onColosseumReviewSettingsChange,
    onDeleteSession,
    recentAlerts,
    editDraft,
    setEditDraft,
    saveEditDraft,
    providers,
    activityFeed,
    restoreActivityContext,
    managementDrawer,
    setManagementDrawer,
    workspaceTasks,
    taskDrawerScope,
    workspaceList,
    taskToEdit,
    onTaskEditOpened,
    setTaskList,
    workspaceNotes,
    workspaceArtifacts,
    workspaceSessions,
    workspaceReminders,
    displayedWorkspaceName,
    taskFlags,
    setTaskFlags,
    reminderFlags,
    setReminderFlags,
    pushToast,
    activeWorkspaceId,
    serverBaseUrl,
    apiKey,
    taskAthenaChats = {},
    onSendTaskAthenaMessage,
    onDeleteTaskAthenaMessage,
    onClearTaskAthenaChat,
    onChangeTaskAthenaInput,
  } = props;
  const [selectedNote, setSelectedNote] = useState<Note | null>(null);
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [isTaskModalOpen, setIsTaskModalOpen] = useState(false);
  const [taskEditorTab, setTaskEditorTab] = useState<'details' | 'activity' | 'diff' | 'comments' | 'athena'>('details');
  const [taskDiffData, setTaskDiffData] = useState<{ diff: string; files: Array<{ status: string; path: string }>; worktree_path?: string } | null>(null);
  const [loadingTaskDiff, setLoadingTaskDiff] = useState(false);
  const [taskViewMode, setTaskViewMode] = useState<'board' | 'visualization' | 'report'>('board');
  const [taskVisualZoom, setTaskVisualZoom] = useState<number>(1);
  const [draggingTaskId, setDraggingTaskId] = useState<string | null>(null);
  const [taskWorkspaceId, setTaskWorkspaceId] = useState('');
  const [taskSearchQuery, setTaskSearchQuery] = useState('');
  const [isTaskDropdownOpen, setIsTaskDropdownOpen] = useState(false);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [mergeRequests, setMergeRequests] = useState<WorkspaceMergeRequest[]>([]);
  const [jiraTickets, setJiraTickets] = useState<WorkspaceJiraTicket[]>([]);
  const [entityEditor, setEntityEditor] = useState<EntityEditorState | null>(null);
  const [statusDropdownOpen, setStatusDropdownOpen] = useState<'merge-request' | 'jira' | null>(null);
  const [taskEditor, setTaskEditor] = useState<{
    title: string;
    description: string;
    priority: Task['priority'];
    state: Task['state'];
    due: string;
    dependencyId: string;
    comment: string;
    repository: string;
    workType: 'development' | 'research';
    autopilot: boolean;
  }>({
    title: '',
    description: '',
    priority: 'medium',
    state: 'backlog',
    due: '',
    dependencyId: '',
    comment: '',
    repository: '',
    workType: 'development',
    autopilot: false,
  });
  const [colosseumProviders, setColosseumProviders] = useState<Array<{ id: string; label: string }>>([]);
  const isTaskDone = (task: Task) => task.state === 'done' || Boolean(taskFlags[task.id]?.done);
  const taskDisplayState = (task: Task) => taskBoardState(task, taskFlags);
  const taskColumns = [
    { id: 'backlog', title: 'Backlog', state: 'backlog' as const, tasks: workspaceTasks.filter((task) => !isTaskDone(task) && taskDisplayState(task) === 'backlog') },
    { id: 'grooming', title: 'Grooming', state: 'grooming' as const, tasks: workspaceTasks.filter((task) => !isTaskDone(task) && taskDisplayState(task) === 'grooming') },
    { id: 'ready', title: 'Ready', state: 'ready' as const, tasks: workspaceTasks.filter((task) => !isTaskDone(task) && taskDisplayState(task) === 'ready') },
    { id: 'active', title: 'In progress', state: 'in-progress' as const, tasks: workspaceTasks.filter((task) => !isTaskDone(task) && taskDisplayState(task) === 'in-progress') },
    { id: 'review', title: 'Review', state: 'review' as const, tasks: workspaceTasks.filter((task) => !isTaskDone(task) && taskDisplayState(task) === 'review') },
    { id: 'human-review', title: 'Human review', state: 'human-review' as const, tasks: workspaceTasks.filter((task) => !isTaskDone(task) && ['human-review', 'approved'].includes(taskDisplayState(task))) },
  ];
  const dependencyParents = new Map<string, string[]>();
  const dependencyChildren = new Map<string, string[]>();
  workspaceTasks.forEach((task) => {
    (task.dependsOn ?? []).forEach((dependsOn) => {
      if (!dependencyChildren.has(dependsOn)) dependencyChildren.set(dependsOn, []);
      dependencyChildren.get(dependsOn)!.push(task.id);
      if (!dependencyParents.has(task.id)) dependencyParents.set(task.id, []);
      dependencyParents.get(task.id)!.push(dependsOn);
    });
  });
  const levelCache = new Map<string, number>();
  const getTaskLevel = (taskId: string, seen = new Set<string>()): number => {
    if (levelCache.has(taskId)) return levelCache.get(taskId)!;
    if (seen.has(taskId)) return 0;
    seen.add(taskId);
    const parents = dependencyParents.get(taskId) ?? [];
    const level = parents.length === 0 ? 0 : Math.max(...parents.map((parentId) => getTaskLevel(parentId, new Set(seen)))) + 1;
    levelCache.set(taskId, level);
    return level;
  };
  const workflowDepth = Math.max(0, ...workspaceTasks.map((task) => getTaskLevel(task.id)));
  const workflowHeight = Math.max(380, workflowDepth * 132 + 140);
  const workflowCenterX = 500;
  const workflowWidth = 820;
  const depthGroups = new Map<number, Task[]>();
  workspaceTasks
    .slice()
    .sort((a, b) => getTaskLevel(a.id) - getTaskLevel(b.id) || a.title.localeCompare(b.title))
    .forEach((task) => {
      const level = getTaskLevel(task.id);
      if (!depthGroups.has(level)) depthGroups.set(level, []);
      depthGroups.get(level)!.push(task);
    });
  const workflowNodes = Array.from(depthGroups.entries()).flatMap(([level, tasksAtLevel]) => {
    const count = tasksAtLevel.length;
    const usableWidth = Math.min(workflowWidth, 220 + Math.max(0, count - 1) * 220);
    const laneWidth = count > 1 ? usableWidth / (count - 1) : 0;
    const startX = workflowCenterX - usableWidth / 2;
    return tasksAtLevel.map((task, index) => ({
      task,
      level,
      x: count > 1 ? startX + laneWidth * index : workflowCenterX,
      y: 90 + level * 132,
    }));
  });
  const workflowNodePlacement = new Map(workflowNodes.map((node) => [node.task.id, node]));
  const workflowEdges = workspaceTasks.flatMap((task) => (task.dependsOn ?? []).map((dependsOn) => ({
    id: `${dependsOn}->${task.id}`,
    source: workflowNodePlacement.get(dependsOn),
    target: workflowNodePlacement.get(task.id),
  }))).filter((edge): edge is { id: string; source: NonNullable<typeof edge.source>; target: NonNullable<typeof edge.target> } => Boolean(edge.source && edge.target));
  const visualTimeStats = useMemo(() => {
    const tasksWithTime = workspaceTasks.filter(t => t.timeSpent !== undefined && t.timeSpent > 0);
    if (tasksWithTime.length === 0) {
      return { total: 0, average: 0, max: 0 };
    }
    const times = tasksWithTime.map(t => t.timeSpent!);
    const total = times.reduce((sum, t) => sum + t, 0);
    const average = Math.round((total / times.length) * 10) / 10;
    const max = Math.max(...times);
    return { total, average, max };
  }, [workspaceTasks]);
  const truncateTaskTitle = (title: string) => title.length > 28 ? `${title.slice(0, 27)}…` : title;
  const selectedSession = selectedSessionId ? workspaceSessions.find((session) => session.id === selectedSessionId) ?? null : null;
  const selectedSessionFiles = selectedSession ? workspaceSessionFileGroups[selectedSession.id] : undefined;
  const selectedSessionConversation = selectedSession ? sessionConversationMap[selectedSession.id] ?? [] : [];
  const selectedSessionNotes = selectedSession ? workspaceNotes.filter((note) => note.sessionId === selectedSession.id) : [];
  const selectedSessionArtifacts = selectedSession ? workspaceArtifacts.filter((artifact) => artifact.sessionId === selectedSession.id) : [];
  const selectedSessionProviderKey = selectedSession ? inferSessionProvider(selectedSession.provider, selectedSession as unknown as Record<string, any>, selectedSessionFiles) : '';
  const selectedSessionView = selectedSession ? { ...selectedSession, provider: getSessionAdapter(selectedSessionProviderKey).displayName } : null;
  const selectedSessionProviderMode = providers.find((provider) => provider.name.toLowerCase() === selectedSessionProviderKey)?.mode ?? 'unknown';
  const selectedSessionProviderCount = selectedSession ? workspaceSessions.filter((session) => inferSessionProvider(session.provider, session as unknown as Record<string, any>, workspaceSessionFileGroups[session.id]) === selectedSessionProviderKey).length : 0;
  const selectedSessionProviderShare = selectedSession && workspaceSessions.length > 0 ? Math.round((selectedSessionProviderCount / workspaceSessions.length) * 100) : 0;
  const selectedSessionToolCalls = selectedSessionConversation.filter((message) => message.kind === 'tool').length;
  const selectedSessionStats = selectedSession ? [
    { label: 'Files', value: selectedSession.files },
    { label: 'Session files', value: selectedSession.files },
    { label: 'Linked', value: selectedSession.linked },
    { label: 'Notes', value: selectedSession.notes },
    { label: 'Jira', value: selectedSession.jira },
    { label: 'Merge requests', value: selectedSession.mergeRequests },
    { label: 'Tool calls', value: selectedSessionToolCalls },
    { label: 'Mode', value: selectedSessionProviderMode },
    { label: 'Usage', value: `${selectedSessionProviderShare}%` },
    { label: 'Updated', value: selectedSession.updated || selectedSession.updatedAt || 'server' },
  ] : [];

  useEffect(() => {
    setSelectedSessionId(null);
  }, [activeWorkspaceId]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      if (entityEditor) {
        setEntityEditor(null);
        return;
      }
      if (isTaskModalOpen) {
        setIsTaskModalOpen(false);
        return;
      }
      if (isKnowledgeOpen) {
        setIsKnowledgeOpen(false);
        return;
      }
      if (isActivityOpen) {
        setIsActivityOpen(false);
        return;
      }
      if (isAlertsOpen) {
        setIsAlertsOpen(false);
        return;
      }
      if (isTaskDrawerOpen) {
        setIsTaskDrawerOpen(false);
        return;
      }
      if (isNotesDrawerOpen) {
        setIsNotesDrawerOpen(false);
        return;
      }
      if (isMergeRequestsDrawerOpen) {
        setIsMergeRequestsDrawerOpen(false);
        return;
      }
      if (isJiraDrawerOpen) {
        setIsJiraDrawerOpen(false);
        return;
      }
      if (isArtifactsDrawerOpen) {
        setIsArtifactsDrawerOpen(false);
        return;
      }
      if (isSessionsDrawerOpen) {
        setIsSessionsDrawerOpen(false);
        return;
      }
      if (managementDrawer) {
        setManagementDrawer(null);
        return;
      }
      if (selectedSessionId) {
        setSelectedSessionId(null);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    entityEditor,
    isActivityOpen,
    isArtifactsDrawerOpen,
    isAlertsOpen,
    isJiraDrawerOpen,
    isKnowledgeOpen,
    isMergeRequestsDrawerOpen,
    isNotesDrawerOpen,
    isSessionsDrawerOpen,
    selectedSessionId,
    isTaskDrawerOpen,
    isTaskModalOpen,
    managementDrawer,
    setManagementDrawer,
  ]);

  const getNoteSessionTitle = (note: Note) => workspaceSessions.find((session) => session.id === note.sessionId)?.title ?? 'Workspace session';
  const headers = buildSavantHeaders(apiKey, true);
  const authHeaders = buildSavantHeaders(apiKey);
  const taskStatusForServer = (state: Task['state']) => (state === 'backlog' ? 'todo' : state);
  const taskStateFromServer = (status: string | undefined, fallback: Task['state']): Task['state'] => {
    if (status === 'todo' || status === 'blocked') return 'backlog';
    if (status === 'code-review') return 'review';
    if (status === 'backlog' || status === 'grooming' || status === 'ready' || status === 'in-progress' || status === 'review' || status === 'human-review' || status === 'approved' || status === 'done') return status;
    return fallback;
  };
  const normalizeTaskFromServer = (task: any, fallback: Task): Task => ({
    ...fallback,
    id: task.task_id ?? task.id ?? fallback.id,
    workspaceId: task.workspace_id ?? fallback.workspaceId,
    title: task.title ?? fallback.title,
    description: task.description ?? fallback.description ?? '',
    priority: task.priority ?? fallback.priority,
    state: taskStateFromServer(task.status, fallback.state),
    due: task.date ?? fallback.due,
    dependsOn: task.depends_on ?? task.dependencies ?? fallback.dependsOn ?? [],
    complexity: task.complexity ?? fallback.complexity,
    comments: task.comments ?? fallback.comments ?? [],
    colosseumConfig: task.colosseum_config ?? fallback.colosseumConfig,
  });
  const colosseumPayload = () => {
    const phaseConfigs = buildColosseumPhaseConfigs(
      colosseumGroomingSettings,
      colosseumReadySettings,
      colosseumReviewSettings,
    );
    return {
      config: {
        repository: taskEditor.repository.trim(),
        work_type: taskEditor.workType,
        autopilot: taskEditor.autopilot,
        provider: phaseConfigs.ready.provider,
        model: phaseConfigs.ready.model,
        persona: phaseConfigs.ready.persona,
        tags: phaseConfigs.ready.tags,
        phase_configs: phaseConfigs,
      },
    };
  };
  const chooseColosseumRepository = async () => {
    try {
      const repository = await window.sanctum?.pickRepository?.(taskEditor.repository);
      if (repository) setTaskEditor((current) => ({ ...current, repository }));
    } catch (error) {
      pushToast('Repository required', error instanceof Error ? error.message : 'Choose a Git repository.', 'warning');
    }
  };
  useEffect(() => {
    void window.sanctum?.listColosseumProviders?.().then((available) => {
      setColosseumProviders(available.map(({ id, label }) => ({ id, label })));
    }).catch(() => setColosseumProviders([]));
  }, []);
  const normalizeMergeRequest = (raw: Record<string, any>): WorkspaceMergeRequest | null => {
    const id = String(raw.mr_id ?? raw.id ?? '').trim();
    if (!id) return null;
    return {
      id,
      workspaceId: String(raw.workspace_id ?? raw.workspaceId ?? activeWorkspaceId),
      mrId: id,
      title: String(raw.title ?? raw.url ?? id),
      status: String(raw.status ?? 'open'),
      url: String(raw.url ?? ''),
      createdAt: String(raw.created_at ?? ''),
      updatedAt: String(raw.updated_at ?? raw.created_at ?? ''),
    };
  };
  const normalizeJiraTicket = (raw: Record<string, any>): WorkspaceJiraTicket | null => {
    const id = String(raw.ticket_id ?? raw.id ?? '').trim();
    if (!id) return null;
    return {
      id,
      workspaceId: String(raw.workspace_id ?? raw.workspaceId ?? activeWorkspaceId),
      ticketId: id,
      ticketKey: String(raw.ticket_key ?? raw.ticketKey ?? id),
      title: String(raw.title ?? id),
      status: String(raw.status ?? 'open'),
      createdAt: String(raw.created_at ?? ''),
      updatedAt: String(raw.updated_at ?? raw.created_at ?? ''),
    };
  };
  const statusLabel = (status?: string) => {
    const normalized = String(status ?? '').toLowerCase();
    if (normalized === 'merged') return 'merged';
    if (normalized === 'closed' || normalized === 'done') return 'closed';
    if (normalized === 'open') return 'open';
    return normalized || 'open';
  };
  const statusTone = (status?: string) => {
    const normalized = statusLabel(status);
    if (normalized === 'merged') return 'green';
    if (normalized === 'closed') return 'grey';
    return 'yellow';
  };
  const refreshWorkspaceEntities = async () => {
    if (!activeWorkspaceId) return;
    try {
      const [mrsResponse, jiraResponse] = await Promise.all([
        fetch(`${serverBaseUrl}/api/merge-requests?workspace_id=${encodeURIComponent(activeWorkspaceId)}`, { headers: authHeaders }),
        fetch(`${serverBaseUrl}/api/jira-tickets?workspace_id=${encodeURIComponent(activeWorkspaceId)}`, { headers: authHeaders }),
      ]);
      if (mrsResponse.ok) {
        const payload = (await mrsResponse.json()) as Record<string, any>[];
        setMergeRequests(payload.map(normalizeMergeRequest).filter((item): item is WorkspaceMergeRequest => Boolean(item)));
      }
      if (jiraResponse.ok) {
        const payload = (await jiraResponse.json()) as Record<string, any>[];
        setJiraTickets(payload.map(normalizeJiraTicket).filter((item): item is WorkspaceJiraTicket => Boolean(item)));
      }
    } catch {
      // Keep the previous entity lists if the server is unavailable.
    }
  };
  const openMergeRequestEditor = (item?: WorkspaceMergeRequest) => {
    setEntityEditor({
      kind: 'merge-request',
      mode: item ? 'edit' : 'create',
      item,
      title: item?.title ?? '',
      url: item?.url ?? '',
      status: item?.status ?? 'open',
    });
  };
  const openJiraEditor = (item?: WorkspaceJiraTicket) => {
    setEntityEditor({
      kind: 'jira',
      mode: item ? 'edit' : 'create',
      item,
      ticketKey: item?.ticketKey ?? '',
      title: item?.title ?? '',
      status: item?.status ?? 'open',
    });
  };
  const closeEntityEditor = () => setEntityEditor(null);
  const saveEntityEditor = async () => {
    if (!activeWorkspaceId || !entityEditor) return;
    try {
      if (entityEditor.kind === 'merge-request') {
        const payload = {
          workspace_id: activeWorkspaceId,
          title: entityEditor.title.trim(),
          status: entityEditor.status.trim() || 'open',
          url: entityEditor.url.trim(),
        };
        if (!payload.url) return;
        const endpoint = entityEditor.mode === 'edit' && entityEditor.item
          ? `${serverBaseUrl}/api/merge-requests/${encodeURIComponent(entityEditor.item.id)}`
          : `${serverBaseUrl}/api/merge-requests`;
        const response = await fetch(endpoint, {
          method: entityEditor.mode === 'edit' ? 'PUT' : 'POST',
          headers,
          body: JSON.stringify(payload),
        });
        if (!response.ok) return;
      } else {
        const payload = {
          workspace_id: activeWorkspaceId,
          ticket_key: entityEditor.ticketKey.trim(),
          title: entityEditor.title.trim(),
          status: entityEditor.status.trim() || 'open',
        };
        if (!payload.ticket_key) return;
        const endpoint = entityEditor.mode === 'edit' && entityEditor.item
          ? `${serverBaseUrl}/api/jira-tickets/${encodeURIComponent(entityEditor.item.id)}`
          : `${serverBaseUrl}/api/jira-tickets`;
        const response = await fetch(endpoint, {
          method: entityEditor.mode === 'edit' ? 'PUT' : 'POST',
          headers,
          body: JSON.stringify(payload),
        });
        if (!response.ok) return;
      }
      closeEntityEditor();
      await refreshWorkspaceEntities();
    } catch {
      // Leave the editor open if the save failed.
    }
  };
  const renderStatusDropdown = (kind: 'merge-request' | 'jira') => {
    if (!entityEditor || entityEditor.kind !== kind) return null;
    const options = kind === 'merge-request'
      ? ['open', 'closed', 'merged']
      : ['open', 'closed'];

    return (
      <div className="entity-status-dropdown">
        <button
          type="button"
          className="entity-status-button"
          aria-haspopup="listbox"
          aria-expanded={statusDropdownOpen === kind}
          onClick={() => setStatusDropdownOpen((current) => current === kind ? null : kind)}
        >
          <span className={`workspace-card-badge workspace-card-badge-${statusTone(entityEditor.status)}`}>{statusLabel(entityEditor.status)}</span>
          <ChevronDown size={12} />
        </button>
        {statusDropdownOpen === kind && (
          <div className="entity-status-menu" role="listbox" aria-label="Status options">
            {options.map((option) => (
              <button
                key={option}
                type="button"
                className={`entity-status-option ${entityEditor.status === option ? 'is-selected' : ''}`}
                onClick={() => {
                  setEntityEditor((current) => current && current.kind === kind ? { ...current, status: option } : current);
                  setStatusDropdownOpen(null);
                }}
              >
                <span>{option}</span>
                <span className={`workspace-card-badge workspace-card-badge-${statusTone(option)}`}>{statusLabel(option)}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    );
  };
  const openTaskCreator = () => {
    setSelectedTask(null);
    setTaskEditorTab('details');
    setTaskEditor({
      title: '',
      description: '',
      priority: 'medium',
      state: 'backlog',
      due: '',
      dependencyId: '',
      comment: '',
      repository: '',
      workType: 'development',
      autopilot: false,
    });
    setTaskSearchQuery('');
    setIsTaskDropdownOpen(false);
    setTaskWorkspaceId(taskDrawerScope === 'workspace' ? activeWorkspaceId : '');
    setIsTaskModalOpen(true);
  };
  useEffect(() => {
    if (taskCreateRequest <= 0) return;
    openTaskCreator();
  }, [taskCreateRequest]);
  const openTaskEditor = (task: Task) => {
    setSelectedTask(task);
    setTaskEditorTab('details');
    setTaskSearchQuery('');
    setIsTaskDropdownOpen(false);
    setTaskEditor({
      title: task.title,
      description: task.description ?? '',
      priority: task.priority,
      state: task.state,
      due: task.due ?? '',
      dependencyId: '',
      comment: '',
      repository: task.colosseumConfig?.repository ?? '',
      workType: task.colosseumConfig?.work_type ?? 'development',
      autopilot: Boolean(task.colosseumConfig?.autopilot),
    });
    setTaskWorkspaceId(task.workspaceId);
    setIsTaskModalOpen(true);
    setTaskDiffData(null);
    setLoadingTaskDiff(true);
    fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(task.id)}/diff`, { headers })
      .then((res) => res.json())
      .then((data) => setTaskDiffData(data))
      .catch(() => setTaskDiffData(null))
      .finally(() => setLoadingTaskDiff(false));
  };
  useEffect(() => {
    if (!taskToEdit) return;
    openTaskEditor(taskToEdit);
    onTaskEditOpened();
  }, [taskToEdit]);
  const moveTask = (taskId: string, state: Task['state']) => {
    const currentTask = workspaceTasks.find((task) => task.id === taskId);
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
    setTaskList((current) => current.map((task) => (task.id === taskId ? { ...task, state } : task)));
    setTaskFlags((current) => ({
      ...current,
      [taskId]: {
        ...(current[taskId] ?? {}),
        done: state === 'done',
        lastMovedAt: new Date().toISOString(),
        lastMovedFrom: current[taskId]?.lastMovedFrom ?? previousState,
        lastMovedTo: state,
      },
    }));
    setDraggingTaskId(null);
    pushToast('Task moved', `Task moved to ${state}.`, 'good');
    void fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(taskId)}`, {
      method: 'PUT',
      headers,
      body: JSON.stringify({ status: taskStatusForServer(state) }),
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
    setTaskFlags((current) => ({
      ...current,
      [task.id]: {
        ...(current[task.id] ?? {}),
        blocked,
        lastMovedFrom: current[task.id]?.lastMovedFrom ?? workflowState,
      },
    }));
    if (task.state === 'blocked' && !blocked) {
      setTaskList((current) => current.map((item) => item.id === task.id ? { ...item, state: workflowState } : item));
    }
    pushToast(blocked ? 'Task blocked' : 'Task unblocked', `${task.title} remains in ${workflowState}.`, blocked ? 'warning' : 'good');
  };
  const removeTask = (taskId: string) => {
    setTaskList((current) => current.filter((task) => task.id !== taskId));
    setTaskFlags((current) => {
      const next = { ...current };
      delete next[taskId];
      return next;
    });
    pushToast('Task removed', 'Task removed from the workspace board.', 'warning');
    void fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(taskId)}`, {
      method: 'DELETE',
      headers,
    }).catch(() => undefined);
  };
  const createTask = async () => {
    const title = taskEditor.title.trim();
    const workspaceId = taskDrawerScope === 'global' ? taskWorkspaceId : activeWorkspaceId;
    if (!title || !workspaceId) return;
    const executionConfig = colosseumPayload().config;
    const localTask: Task = {
      id: `task-${Date.now().toString(36)}`,
      workspaceId,
      title,
      description: taskEditor.description,
      priority: taskEditor.priority,
      state: 'backlog',
      owner: 'operator',
      due: taskEditor.due || undefined,
      dependsOn: [],
      comments: taskEditor.comment.trim() ? [taskEditor.comment.trim()] : [],
      colosseumConfig: executionConfig,
    };
    let task = localTask;
    try {
      const response = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks`, {
        method: 'POST',
        headers,
        body: JSON.stringify({
          task_id: localTask.id,
          workspace_id: workspaceId,
          title,
          description: taskEditor.description,
          status: taskStatusForServer('backlog'),
          priority: taskEditor.priority,
          date: taskEditor.due || undefined,
        }),
      });
      if (response.ok) task = normalizeTaskFromServer(await response.json(), localTask);
      {
        const readyResp = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(task.id)}/colosseum-metadata`, {
          method: 'PUT',
          headers,
          body: JSON.stringify(colosseumPayload().config),
        });
        if (readyResp.ok) task = normalizeTaskFromServer(await readyResp.json(), task);
      }
    } catch {
      // Local optimistic task remains available when the server is offline.
    }
    setTaskList((current) => [task, ...current.filter((item) => item.id !== task.id)]);
    setSelectedTask(task);
    setIsTaskModalOpen(false);
    pushToast('Task created', `${title} added to ${workspaceList.find((workspace) => workspace.id === workspaceId)?.name ?? workspaceId}.`, 'good');
  };
  const saveTask = async (closeModal = true): Promise<Task | null> => {
    if (!selectedTask) return null;
    const title = taskEditor.title.trim();
    if (!title) return null;
    const blockedState = isTaskBlocked(selectedTask, taskFlags) ? taskWorkflowState(selectedTask, taskFlags) : null;
    const workspaceId = taskDrawerScope === 'global' ? taskWorkspaceId : selectedTask.workspaceId;

    const updatedLocal: Task = {
      ...selectedTask,
      title,
      description: taskEditor.description,
      priority: taskEditor.priority,
      state: blockedState ?? taskEditor.state,
      due: taskEditor.due || undefined,
      workspaceId,
      colosseumConfig: { ...selectedTask.colosseumConfig, ...colosseumPayload().config },
    };

    let updated = updatedLocal;
    try {
      const response = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(selectedTask.id)}`, {
        method: 'PUT',
        headers,
        body: JSON.stringify({
          title,
          description: taskEditor.description,
          status: taskStatusForServer(blockedState ?? taskEditor.state),
          priority: taskEditor.priority,
          date: taskEditor.due || undefined,
          workspace_id: workspaceId,
        }),
      });
      if (response.ok) updated = normalizeTaskFromServer(await response.json(), updatedLocal);
      {
        const readyResp = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(selectedTask.id)}/colosseum-metadata`, {
          method: 'PUT',
          headers,
          body: JSON.stringify(colosseumPayload().config),
        });
        if (readyResp.ok) updated = normalizeTaskFromServer(await readyResp.json(), updated);
      }
    } catch {
      // Keep local edit when the server is unavailable.
    }

    setTaskList((current) => current.map((task) => (task.id === selectedTask.id ? updated : task)));
    setSelectedTask(updated);
    if (closeModal) setIsTaskModalOpen(false);
    pushToast('Task saved', `${title} updated.`, 'good');
    return updated;
  };
  const saveTaskModal = () => {
    if (selectedTask) {
      void saveTask();
      return;
    }
    createTask();
  };
  const addDependency = async () => {
    if (!selectedTask) return;
    const dependsOn = taskEditor.dependencyId.trim();
    if (!dependsOn || dependsOn === selectedTask.id) return;
    try {
      await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(selectedTask.id)}/deps`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ depends_on: dependsOn }),
      });
    } catch {
      // Local linkage still updates below.
    }
    const nextDepends = Array.from(new Set([...(selectedTask.dependsOn ?? []), dependsOn]));
    const updated = { ...selectedTask, dependsOn: nextDepends };
    setTaskList((current) => current.map((task) => (task.id === selectedTask.id ? updated : task)));
    setSelectedTask(updated);
    setTaskEditor((current) => ({ ...current, dependencyId: '' }));
  };
  const removeDependency = async (dependsOn: string) => {
    if (!selectedTask) return;
    try {
      await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(selectedTask.id)}/deps/${encodeURIComponent(dependsOn)}`, {
        method: 'DELETE',
        headers,
      });
    } catch {
      // Local unlink still updates below.
    }
    const updated = { ...selectedTask, dependsOn: (selectedTask.dependsOn ?? []).filter((id) => id !== dependsOn) };
    setTaskList((current) => current.map((task) => (task.id === selectedTask.id ? updated : task)));
    setSelectedTask(updated);
  };
  const addTaskComment = async () => {
    if (!selectedTask || !taskEditor.comment.trim()) return;
    const authorName = authUser?.username || authUser?.name || 'User';
    const newCommentObj: TaskComment = {
      id: `comment-${Date.now()}-${Math.random().toString(36).substr(2, 4)}`,
      author: authorName,
      text: taskEditor.comment.trim(),
      createdAt: new Date().toISOString(),
      role: 'user',
    };
    let persistedComment = newCommentObj;
    try {
      const response = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(selectedTask.id)}/comments`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ text: newCommentObj.text, author: newCommentObj.author, role: newCommentObj.role }),
      });
      if (!response.ok) throw new Error('Comment was not accepted by the server.');
      persistedComment = await response.json();
    } catch (error) {
      pushToast('Comment not saved', error instanceof Error ? error.message : 'Try again.', 'warning');
      return;
    }
    const updated = { ...selectedTask, comments: [...(selectedTask.comments ?? []), persistedComment] };
    setTaskList((current) => current.map((task) => (task.id === selectedTask.id ? updated : task)));
    setSelectedTask(updated);
    setTaskEditor((current) => ({ ...current, comment: '' }));
  };
  const submitForGrooming = async () => {
    if (!selectedTask) return;
    const executionConfig = colosseumPayload().config;
    if (!executionConfig.provider) {
      pushToast('Ready provider required', 'Choose a provider in Settings → Colosseum → Ready Status Config before grooming.', 'warning');
      return;
    }
    const candidate: Task = {
      ...selectedTask,
      colosseumConfig: { ...selectedTask.colosseumConfig, ...executionConfig },
    };
    if (!canSubmitForGrooming(candidate)) {
      pushToast('Repository required', 'Development tickets need a local Git repository before grooming.', 'warning');
      return;
    }
    const saved = await saveTask(false);
    if (!saved) return;
    try {
      const statusResponse = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(saved.id)}`, {
        method: 'PUT',
        headers,
        body: JSON.stringify({ status: 'grooming' }),
      });
      if (!statusResponse.ok) throw new Error('The ticket could not enter grooming.');
      let updated = normalizeTaskFromServer(await statusResponse.json(), { ...saved, state: 'grooming' });
      const readyResponse = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(saved.id)}/colosseum-ready-state`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ ready: true }),
      });
      if (!readyResponse.ok) throw new Error('Colosseum could not be activated for this ticket.');
      updated = normalizeTaskFromServer(await readyResponse.json(), updated);
      setTaskList((current) => current.map((task) => task.id === updated.id ? updated : task));
      setSelectedTask(updated);
      setTaskEditor((current) => ({ ...current, state: 'grooming' }));
      pushToast('Sent to grooming', 'Colosseum can now inspect the ticket and ask for missing details.', 'good');
    } catch (error) {
      pushToast('Grooming submission failed', error instanceof Error ? error.message : 'Try again.', 'warning');
    }
  };
  const submitHumanDecision = async (decision: 'approve' | 'reject') => {
    if (!selectedTask) return;
    const comment = taskEditor.comment.trim();
    if (decision === 'reject' && !comment) {
      pushToast('Reason required', 'Explain what Colosseum must change before resubmitting.', 'warning');
      setTaskEditorTab('comments');
      return;
    }
    try {
      const response = await fetch(`${serverBaseUrl.replace(/\/+$/, '')}/api/tasks/${encodeURIComponent(selectedTask.id)}/approval`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ decision, comment }),
      });
      const payload = await response.json();
      if (!response.ok) throw new Error(payload.error || 'The review decision was not accepted.');
      const updated = normalizeTaskFromServer(payload, selectedTask);
      setTaskList((current) => current.map((task) => task.id === updated.id ? updated : task));
      setSelectedTask(updated);
      setTaskEditor((current) => ({ ...current, state: updated.state, comment: '' }));
      pushToast(decision === 'approve' ? 'Approved for merge' : 'Changes requested', decision === 'approve' ? 'Colosseum will merge and close the merge request.' : 'The ticket is back in the execution queue.', decision === 'approve' ? 'good' : 'warning');
    } catch (error) {
      pushToast('Review decision failed', error instanceof Error ? error.message : 'Try again.', 'warning');
    }
  };

  useEffect(() => {
    void refreshWorkspaceEntities();
    const interval = window.setInterval(() => {
      void refreshWorkspaceEntities();
    }, 30000);
    return () => window.clearInterval(interval);
  }, [activeWorkspaceId, apiKey, serverBaseUrl]);

  return (
    <>
      {isActivityOpen && (
        <div className="activity-drawer-backdrop" onClick={() => setIsActivityOpen(false)}>
          <div className="knowledge-drawer" onClick={(e) => e.stopPropagation()}>
            <div className="workspace-drawer-head">
              <div className="flex items-center gap-3">
                <History size={18} className="text-cyan-400" />
                <h2 className="brand-title !text-base !tracking-wider">Recent Activity</h2>
              </div>
              <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsActivityOpen(false)}><X size={14} /></button>
            </div>
            <div className="workspace-drawer-body">
              {activityFeed.length === 0 ? (
                <div className="activity-empty">No recent changes yet.</div>
              ) : (
                activityFeed.map((item) => (
                  <button key={item.id} type="button" className="activity-item !border-0 !border-b !border-white/5 !px-5 !py-4" onClick={() => { restoreActivityContext(item); setIsActivityOpen(false); }}>
                    <div className="flex flex-col">
                      <strong className="text-[13px]">{item.label}</strong>
                      <span className="text-[11px] opacity-50">{item.detail}</span>
                    </div>
                  </button>
                ))
              )}
            </div>
          </div>
        </div>
      )}

      {isAlertsOpen && (
        <div className="modal-backdrop" role="presentation" onClick={() => setIsAlertsOpen(false)}>
          <div className="modal-card" role="dialog" aria-modal="true" aria-labelledby="alerts-title" onClick={(event) => event.stopPropagation()} style={{ display: 'flex', flexDirection: 'column', maxHeight: '80vh' }}>
            <div className="modal-head">
              <div>
                <div className="eyebrow">Alerts</div>
                <h2 id="alerts-title">Recent alerts</h2>
              </div>
              <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsAlertsOpen(false)}><X size={14} /></button>
            </div>
            <div className="workspace-drawer-body" style={{ padding: 0 }}>
              {recentAlerts.length === 0 ? (
                <div className="activity-empty">No recent alerts.</div>
              ) : (
                <div className="management-drawer-list">
                  {recentAlerts.map((alert) => (
                    <div key={alert.id} className="management-drawer-row">
                      <div className="flex flex-col min-w-0 flex-1">
                        <strong className="truncate">{alert.title}</strong>
                        <span className="text-xs opacity-60 truncate">{alert.detail}</span>
                      </div>
                      <div className={`workspace-card-badge workspace-card-badge-${alert.tone === 'good' ? 'green' : alert.tone === 'warning' ? 'yellow' : 'grey'}`}>
                        {new Date(alert.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {isKnowledgeOpen && (
        <div className="activity-drawer-backdrop" onClick={() => setIsKnowledgeOpen(false)}>
          <div className="knowledge-drawer" onClick={(e) => e.stopPropagation()}>
            <div className="flex-1 overflow-hidden">
              <KnowledgeGraph
                workspaceId={activeWorkspaceId}
                baseUrl={serverBaseUrl}
                apiKey={apiKey}
                onClose={() => setIsKnowledgeOpen(false)}
                selectedProvider={selectedProvider}
                selectedModel={selectedModel}
              />
            </div>
          </div>
        </div>
      )}

      <WorkspaceSessionsDrawer
        open={isSessionsDrawerOpen && !selectedSessionId}
        workspaceName={displayedWorkspaceName}
        sessions={workspaceSessions}
        sessionFileGroups={workspaceSessionFileGroups}
        activeSessionId={activeSessionId}
        onSelectSession={(sessionId) => {
          const index = workspaceSessions.findIndex((session) => session.id === sessionId);
          if (index >= 0) setSessionIndex(index);
          setSelectedSessionId(sessionId);
          setIsSessionsDrawerOpen(false);
        }}
        onDeleteSession={onDeleteSession}
        onClose={() => {
          setSelectedSessionId(null);
          setIsSessionsDrawerOpen(false);
        }}
      />

      <WorkspaceSessionDetailsDrawer
        open={Boolean(selectedSessionView)}
        workspaceName={displayedWorkspaceName}
        session={selectedSessionView}
        stats={selectedSessionStats}
        files={selectedSessionFiles}
        conversation={selectedSessionConversation}
        notes={selectedSessionNotes}
        artifacts={selectedSessionArtifacts}
        onSelectNote={setSelectedNote}
        onBack={() => {
          setSelectedSessionId(null);
          setIsSessionsDrawerOpen(true);
        }}
        onClose={() => {
          setSelectedSessionId(null);
          setIsSessionsDrawerOpen(false);
        }}
      />

      {isTaskDrawerOpen && (
        <div className="activity-drawer-backdrop" onClick={() => setIsTaskDrawerOpen(false)}>
          <div className="knowledge-drawer task-drawer" onClick={(e) => e.stopPropagation()}>
            <div className="workspace-drawer-head">
              <div className="flex items-center gap-3">
                <ListChecks size={18} className="text-cyan-400" />
                <div>
                  <h2 className="brand-title !text-base !tracking-wider">Tasks</h2>
                  <div className="workspace-drawer-subtitle">{workspaceTasks.length} tasks · {taskDrawerScope === 'global' ? 'All workspaces' : displayedWorkspaceName}</div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <div className="task-view-toggle" role="tablist" aria-label="Task view">
                  <button type="button" className={taskViewMode === 'board' ? 'is-active' : ''} onClick={() => setTaskViewMode('board')}><ListChecks size={13} /> Board</button>
                  <button type="button" className={taskViewMode === 'visualization' ? 'is-active' : ''} onClick={() => setTaskViewMode('visualization')}><Network size={13} /> Visualization</button>
                  <button type="button" className={taskViewMode === 'report' ? 'is-active' : ''} onClick={() => setTaskViewMode('report')}><FileText size={13} /> Done Report</button>
                </div>
                <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsTaskDrawerOpen(false)}><X size={14} /></button>
              </div>
            </div>
            <div className="workspace-drawer-body">
              {taskViewMode === 'board' ? (
                <div className="task-kanban-grid task-manager-kanban-grid">
                  {taskColumns.map((column) => (
                    <section key={column.id} className="task-kanban-column">
                      <div className="task-kanban-head">
                        <span>{column.title}</span>
                        <strong>{column.tasks.length}</strong>
                      </div>
                      <div
                        className={`task-kanban-list ${draggingTaskId ? 'is-drop-ready' : ''}`}
                        onDragOver={(event) => event.preventDefault()}
                        onDrop={(event) => {
                          event.preventDefault();
                          const taskId = event.dataTransfer.getData('text/plain') || draggingTaskId;
                          if (taskId) moveTask(taskId, column.state);
                        }}
                      >
                        {column.tasks.length > 0 ? column.tasks.map((task) => (
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
                            onClick={() => openTaskEditor(task)}
                            onKeyDown={(event) => {
                              if (event.key === 'Enter' || event.key === ' ') {
                                event.preventDefault();
                                openTaskEditor(task);
                              }
                            }}
                            role="button"
                            tabIndex={0}
                          >
                            <div className="task-card-title">{task.title}</div>
                            <div className="task-card-description">{task.description || 'No description.'}</div>
                            <div className="task-card-meta">
                              <span>{task.priority}</span>
                              <span>{taskDrawerScope === 'global' ? (workspaceList.find((workspace) => workspace.id === task.workspaceId)?.name ?? task.workspaceId) : task.owner}</span>
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
                                  onClick={(event) => { event.stopPropagation(); removeTask(task.id); }}
                                >
                                  <Trash2 size={14} />
                                </button>
                              </div>
                            </div>
                          </div>
                        )) : (
                          <div className="workspace-list-empty">No tasks in this lane.</div>
                        )}
                      </div>
                    </section>
                  ))}
                </div>
              ) : taskViewMode === 'visualization' ? (
                <div className="task-workflow-shell">
                  {workspaceTasks.length === 0 ? (
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
                          setTaskVisualZoom((z) => Math.min(3, Math.max(0.3, +(z + zoomDelta).toFixed(2))));
                        }}
                      >
                        <div className="sticky top-2 right-2 ml-auto w-fit flex items-center gap-1 z-10 bg-black/70 border border-white/10 p-1 rounded-md backdrop-blur-md">
                          <button
                            type="button"
                            onClick={() => setTaskVisualZoom((z) => Math.min(3, +(z + 0.15).toFixed(2)))}
                            title="Zoom In"
                            aria-label="Zoom in visualization"
                            className="p-1.5 hover:bg-white/10 rounded text-cyan-400 transition-colors"
                          >
                            <ZoomIn size={14} />
                          </button>
                          <span className="text-[10px] font-mono px-1 text-white/70 min-w-[36px] text-center">
                            {Math.round(taskVisualZoom * 100)}%
                          </span>
                          <button
                            type="button"
                            onClick={() => setTaskVisualZoom((z) => Math.max(0.3, +(z - 0.15).toFixed(2)))}
                            title="Zoom Out"
                            aria-label="Zoom out visualization"
                            className="p-1.5 hover:bg-white/10 rounded text-cyan-400 transition-colors"
                          >
                            <ZoomOut size={14} />
                          </button>
                          <button
                            type="button"
                            onClick={() => setTaskVisualZoom(1)}
                            title="Reset Zoom"
                            aria-label="Reset visualization zoom"
                            className="p-1.5 hover:bg-white/10 rounded text-cyan-400 transition-colors"
                          >
                            <Maximize size={14} />
                          </button>
                        </div>
                        <div
                          style={{
                            transform: `scale(${taskVisualZoom})`,
                            transformOrigin: 'top left',
                            transition: 'transform 0.15s ease-out',
                          }}
                        >
                          <svg className="task-workflow-svg" viewBox={`0 0 1000 ${workflowHeight}`} role="img" aria-label="Task workflow graph">
                            <defs>
                              <marker id="task-arrow" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto" markerUnits="strokeWidth">
                                <path d="M0,0 L0,6 L9,3 z" fill="rgba(0,229,255,0.7)" />
                              </marker>
                            </defs>
                            {workflowEdges.map((edge) => (
                              <path
                                key={edge.id}
                                className="task-workflow-edge"
                                markerEnd="url(#task-arrow)"
                                d={`M${edge.source.x},${edge.source.y + 30} C${edge.source.x},${edge.source.y + 56} ${edge.target.x},${edge.target.y - 56} ${edge.target.x},${edge.target.y - 30}`}
                              />
                            ))}
                            {workflowNodes.map(({ task, x, y }) => (
                              <g key={task.id} className="task-workflow-node" role="button" tabIndex={0} onClick={() => openTaskEditor(task)}>
                                <rect x={x - 84} y={y - 28} width="168" height="56" rx="0" className={`task-workflow-card priority-${task.priority}`} />
                                <text x={x} y={y - 7} textAnchor="middle" className="task-workflow-title">{truncateTaskTitle(task.title)}</text>
                                <text x={x} y={y + 13} textAnchor="middle" className="task-workflow-meta">{task.priority} · {taskFlags[task.id]?.done ? 'done' : task.state}</text>

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

                                {(task.dependsOn ?? []).length > 0 && <circle cx={x + 70} cy={y + 15} r="4" className="task-workflow-link-dot" />}
                              </g>
                            ))}
                          </svg>
                        </div>
                      </div>
                    </>
                  )}
                </div>
              ) : (
                <div className="task-workflow-shell p-6">
                  <div className="flex items-center justify-between mb-6 pb-4 border-b border-white/10">
                    <div>
                      <h3 className="brand-title text-lg tracking-wider text-cyan-400">Completed Tasks Report</h3>
                      <p className="text-xs text-white/50 mt-1">Summary of all completed deliverables and closed work items</p>
                    </div>
                    <div className="text-right">
                      <span className="text-2xl font-mono font-bold text-cyan-400">{workspaceTasks.filter((t) => t.state === 'done' || taskFlags[t.id]?.done).length}</span>
                      <span className="text-xs text-white/50 block">Tasks Completed</span>
                    </div>
                  </div>
                  {workspaceTasks.filter((t) => t.state === 'done' || taskFlags[t.id]?.done).length === 0 ? (
                    <div className="activity-empty py-12 text-center text-white/40">No completed tasks in this workspace.</div>
                  ) : (
                    <div className="space-y-3 max-h-[60vh] overflow-y-auto pr-2">
                      {workspaceTasks.filter((t) => t.state === 'done' || taskFlags[t.id]?.done).map((task) => (
                        <div key={task.id} className="task-card-block p-4 rounded-lg bg-black/40 border border-emerald-500/30 hover:border-emerald-400/60 transition-all cursor-pointer" onClick={() => openTaskEditor(task)}>
                          <div className="flex items-start justify-between gap-4">
                            <div className="space-y-1">
                              <div className="flex items-center gap-2">
                                <Check size={14} className="text-emerald-400" />
                                <span className="font-semibold text-sm text-white/90">{task.title}</span>
                                <span className={`text-[10px] px-2 py-0.5 rounded font-mono font-bold uppercase ${task.priority === 'critical' ? 'bg-rose-500/20 text-rose-300 border border-rose-500/40' : task.priority === 'high' ? 'bg-amber-500/20 text-amber-300 border border-amber-500/40' : 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/40'}`}>{task.priority}</span>
                              </div>
                              {task.description && <p className="text-xs text-white/60 line-clamp-2 pl-6">{task.description}</p>}
                            </div>
                            <div className="text-right shrink-0 text-xs text-white/40 font-mono">
                              {task.updatedAt ? new Date(task.updatedAt).toLocaleDateString() : 'Completed'}
                            </div>
                          </div>
                          {task.colosseumConfig?.repository && (
                            <div className="mt-2 pl-6 text-[11px] font-mono text-cyan-400/70 flex items-center gap-1.5">
                              <GitBranch size={11} /> {task.colosseumConfig.repository}
                            </div>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {isNotesDrawerOpen && (
        <div className="activity-drawer-backdrop" onClick={() => setIsNotesDrawerOpen(false)}>
          <div className="knowledge-drawer" onClick={(e) => e.stopPropagation()}>
            <div className="workspace-drawer-head">
              <div className="flex items-center gap-3">
                <FileText size={18} className="text-cyan-400" />
                <div>
                  <h2 className="brand-title !text-base !tracking-wider">Notes</h2>
                  <div className="workspace-drawer-subtitle">{workspaceNotes.length} notes · {displayedWorkspaceName}</div>
                </div>
              </div>
              <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsNotesDrawerOpen(false)}><X size={14} /></button>
            </div>
            <div className="workspace-drawer-body">
              <div className="notes-drawer-grid">
                {workspaceNotes.length > 0 ? workspaceNotes.map((note) => (
                  <button key={note.id} type="button" className="note-drawer-card" onClick={() => setSelectedNote(note)}>
                    <div className="note-drawer-title">{note.title}</div>
                    <div className="note-drawer-meta">{getNoteSessionTitle(note)}</div>
                    <p>{note.body}</p>
                  </button>
                )) : (
                  <div className="activity-empty">No notes for this workspace.</div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {isMergeRequestsDrawerOpen && (
        <div className="activity-drawer-backdrop" onClick={() => setIsMergeRequestsDrawerOpen(false)}>
          <div className="knowledge-drawer" onClick={(e) => e.stopPropagation()}>
            <div className="workspace-drawer-head">
              <div className="flex items-center gap-3">
                <GitBranch size={18} className="text-cyan-400" />
                <div>
                  <h2 className="brand-title !text-base !tracking-wider">Merge requests</h2>
                  <div className="workspace-drawer-subtitle">{mergeRequests.length} items · {displayedWorkspaceName}</div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <button className="ghost-btn accent icon-only" aria-label="Add merge request" title="Add merge request" onClick={() => openMergeRequestEditor()}>
                  <Plus size={14} />
                </button>
                <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsMergeRequestsDrawerOpen(false)}><X size={14} /></button>
              </div>
            </div>
            <div className="workspace-drawer-body">
              <div className="workspace-list">
                {mergeRequests.length > 0 ? mergeRequests.map((mr) => (
                  <button key={mr.id} type="button" className="workspace-list-item workspace-list-button workspace-list-item-stack" onClick={() => openMergeRequestEditor(mr)}>
                    <div className="workspace-list-main">
                      <div className="row-title">{mr.title}</div>
                      <div className="row-meta">{mr.mrId}</div>
                    </div>
                    <div className={`workspace-card-badge workspace-card-badge-${statusTone(mr.status)}`}>{statusLabel(mr.status)}</div>
                  </button>
                )) : (
                  <div className="activity-empty">No merge requests in this workspace.</div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {isJiraDrawerOpen && (
        <div className="activity-drawer-backdrop" onClick={() => setIsJiraDrawerOpen(false)}>
          <div className="knowledge-drawer" onClick={(e) => e.stopPropagation()}>
            <div className="workspace-drawer-head">
              <div className="flex items-center gap-3">
                <Zap size={18} className="text-cyan-400" />
                <div>
                  <h2 className="brand-title !text-base !tracking-wider">Jira tickets</h2>
                  <div className="workspace-drawer-subtitle">{jiraTickets.length} items · {displayedWorkspaceName}</div>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <button className="ghost-btn accent icon-only" aria-label="Add Jira ticket" title="Add Jira ticket" onClick={() => openJiraEditor()}>
                  <Plus size={14} />
                </button>
                <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsJiraDrawerOpen(false)}><X size={14} /></button>
              </div>
            </div>
            <div className="workspace-drawer-body">
              <div className="workspace-list">
                {jiraTickets.length > 0 ? jiraTickets.map((ticket) => (
                  <button key={ticket.id} type="button" className="workspace-list-item workspace-list-button workspace-list-item-stack" onClick={() => openJiraEditor(ticket)}>
                    <div className="workspace-list-main">
                      <div className="row-title">{ticket.title}</div>
                      <div className="row-meta">{ticket.ticketKey}</div>
                    </div>
                    <div className={`workspace-card-badge workspace-card-badge-${statusTone(ticket.status)}`}>{statusLabel(ticket.status)}</div>
                  </button>
                )) : (
                  <div className="activity-empty">No Jira tickets in this workspace.</div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {isArtifactsDrawerOpen && (
        <div className="activity-drawer-backdrop" onClick={() => setIsArtifactsDrawerOpen(false)}>
          <div className="knowledge-drawer" onClick={(e) => e.stopPropagation()}>
            <div className="workspace-drawer-head">
              <div className="flex items-center gap-3">
                <FileCode size={18} className="text-cyan-400" />
                <div>
                  <h2 className="brand-title !text-base !tracking-wider">Artifacts</h2>
                  <div className="workspace-drawer-subtitle">{workspaceArtifacts.length} items · {displayedWorkspaceName}</div>
                </div>
              </div>
              <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsArtifactsDrawerOpen(false)}><X size={14} /></button>
            </div>
            <div className="workspace-drawer-body">
              <div className="artifact-group-grid">
                <div className="artifact-group-card">
                  <div className="task-editor-section-head">Files</div>
                  <div className="workspace-list">
                    {workspaceArtifacts.filter((artifact) => artifact.kind === 'file').length > 0 ? workspaceArtifacts.filter((artifact) => artifact.kind === 'file').map((artifact) => (
                      <div key={artifact.id} className="workspace-list-item">
                        <div>
                          <div className="row-title">{artifact.title}</div>
                          <div className="row-meta">{workspaceSessions.find((session) => session.id === artifact.sessionId)?.title ?? 'Workspace session'}</div>
                        </div>
                        <div className="workspace-list-mini">{artifact.count}</div>
                      </div>
                    )) : (
                      <div className="activity-empty">No file artifacts in this workspace.</div>
                    )}
                  </div>
                </div>
                <div className="artifact-group-card">
                  <div className="task-editor-section-head">Merge requests</div>
                  <div className="workspace-list">
                    {mergeRequests.length > 0 ? mergeRequests.map((mr) => (
                      <div key={mr.id} className="workspace-list-item">
                        <div className="min-w-0">
                          <div className="row-title">{mr.title}</div>
                          <div className="row-meta">{mr.mrId}</div>
                        </div>
                        <div className={`workspace-card-badge workspace-card-badge-${statusTone(mr.status)}`}>{statusLabel(mr.status)}</div>
                      </div>
                    )) : (
                      <div className="activity-empty">No merge requests in this workspace.</div>
                    )}
                  </div>
                </div>
                <div className="artifact-group-card">
                  <div className="task-editor-section-head">Jira tickets</div>
                  <div className="workspace-list">
                    {jiraTickets.length > 0 ? jiraTickets.map((ticket) => (
                      <div key={ticket.id} className="workspace-list-item">
                        <div className="min-w-0">
                          <div className="row-title">{ticket.title}</div>
                          <div className="row-meta">{ticket.ticketKey}</div>
                        </div>
                        <div className={`workspace-card-badge workspace-card-badge-${statusTone(ticket.status)}`}>{statusLabel(ticket.status)}</div>
                      </div>
                    )) : (
                      <div className="activity-empty">No Jira tickets in this workspace.</div>
                    )}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {entityEditor && (
        <div className="modal-backdrop" role="presentation" onClick={closeEntityEditor}>
          <form className="modal-card task-info-modal" role="dialog" aria-modal="true" aria-labelledby="entity-editor-title" onSubmit={(event) => { event.preventDefault(); void saveEntityEditor(); }} onClick={(event) => event.stopPropagation()}>
            <div className="modal-head">
              <div>
                <div className="eyebrow">{entityEditor.mode === 'edit' ? 'Edit' : 'Add'} {entityEditor.kind === 'merge-request' ? 'merge request' : 'Jira ticket'}</div>
                <h2 id="entity-editor-title">{entityEditor.kind === 'merge-request' ? (entityEditor.mode === 'edit' ? entityEditor.item?.title ?? 'Merge request' : 'New merge request') : (entityEditor.mode === 'edit' ? entityEditor.item?.title ?? 'Jira ticket' : 'New Jira ticket')}</h2>
              </div>
              <button type="button" className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={closeEntityEditor}><X size={14} /></button>
            </div>

            {entityEditor.kind === 'merge-request' ? (
              <div className="entity-editor-grid">
                <label className="task-editor-field">
                  <span>Title</span>
                  <input value={entityEditor.title} onChange={(event) => setEntityEditor((current) => current && current.kind === 'merge-request' ? { ...current, title: event.target.value } : current)} placeholder="Merge request title" />
                </label>
                <label className="task-editor-field">
                  <span>Status</span>
                  {renderStatusDropdown('merge-request')}
                </label>
                <label className="task-editor-field entity-editor-span-2">
                  <span>URL</span>
                  <input value={entityEditor.url} onChange={(event) => setEntityEditor((current) => current && current.kind === 'merge-request' ? { ...current, url: event.target.value } : current)} placeholder="https://..." />
                </label>
                {entityEditor.item && <div className="detail-row entity-editor-span-2"><strong>Sanctum MR ID</strong><span>{entityEditor.item.id}</span></div>}
              </div>
            ) : (
              <div className="entity-editor-grid">
                <label className="task-editor-field">
                  <span>Jira ticket key</span>
                  <input value={entityEditor.ticketKey} onChange={(event) => setEntityEditor((current) => current && current.kind === 'jira' ? { ...current, ticketKey: event.target.value } : current)} placeholder="PROJ-1234" />
                </label>
                <label className="task-editor-field">
                  <span>Status</span>
                  {renderStatusDropdown('jira')}
                </label>
                <label className="task-editor-field entity-editor-span-2">
                  <span>Title</span>
                  <input value={entityEditor.title} onChange={(event) => setEntityEditor((current) => current && current.kind === 'jira' ? { ...current, title: event.target.value } : current)} placeholder="Jira ticket title" />
                </label>
                {entityEditor.item && <div className="detail-row entity-editor-span-2"><strong>Sanctum Jira ID</strong><span>{entityEditor.item.id}</span></div>}
              </div>
            )}

            <div className="modal-actions">
              <button type="submit" className="ghost-btn action-save icon-only" aria-label="Save" title="Save"><Check size={14} /></button>
              <button type="button" className="ghost-btn action-close icon-only" aria-label="Cancel" title="Cancel" onClick={closeEntityEditor}><X size={14} /></button>
            </div>
          </form>
        </div>
      )}

      {isTaskModalOpen && (
        <div className="modal-backdrop" role="presentation" onClick={() => setIsTaskModalOpen(false)}>
          <form className="modal-card task-info-modal" role="dialog" aria-modal="true" aria-labelledby="task-info-title" onSubmit={(event) => { event.preventDefault(); saveTaskModal(); }} onClick={(event) => event.stopPropagation()}>
            <div className="modal-head">
              <div>
                <div className="eyebrow">{selectedTask ? 'Edit task' : 'Add task'}</div>
                <h2 id="task-info-title">{selectedTask ? selectedTask.title : 'New workspace task'}</h2>
              </div>
              <button type="button" className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsTaskModalOpen(false)}><X size={14} /></button>
            </div>

            {selectedTask && (
              <ColosseumPhaseLedger state={selectedTask.state} />
            )}

            {selectedTask && (
              <div className="flex justify-between items-center border-b border-white/10 px-6 py-2">
                <div className="flex gap-4">
                  <button
                    type="button"
                    onClick={() => setTaskEditorTab('details')}
                    className={`pb-1 text-sm font-semibold border-b-2 transition-all cursor-pointer ${taskEditorTab === 'details' ? 'border-cyan-400 text-cyan-400' : 'border-transparent text-white/50 hover:text-white'}`}
                  >
                    Details
                  </button>
                  <button
                    type="button"
                    onClick={() => setTaskEditorTab('activity')}
                    className={`pb-1 text-sm font-semibold border-b-2 transition-all cursor-pointer flex items-center gap-1.5 ${taskEditorTab === 'activity' ? 'border-cyan-400 text-cyan-400' : 'border-transparent text-white/50 hover:text-white'}`}
                  >
                    <History size={13} />
                    <span>Activity</span>
                    {(selectedTask.colosseumConfig?.runs ?? []).length > 0 && <span className="colosseum-run-count">{selectedTask.colosseumConfig?.runs?.length}</span>}
                  </button>
                  <button
                    type="button"
                    onClick={() => setTaskEditorTab('diff')}
                    className={`pb-1 text-sm font-semibold border-b-2 transition-all cursor-pointer ${taskEditorTab === 'diff' ? 'border-cyan-400 text-cyan-400' : 'border-transparent text-white/50 hover:text-white'}`}
                  >
                    Diff
                  </button>
                  <button
                    type="button"
                    onClick={() => setTaskEditorTab('comments')}
                    className={`pb-1 text-sm font-semibold border-b-2 transition-all cursor-pointer flex items-center gap-1.5 ${taskEditorTab === 'comments' ? 'border-cyan-400 text-cyan-400' : 'border-transparent text-white/50 hover:text-white'}`}
                  >
                    <MessageSquare size={13} />
                    <span>Comments</span>
                    {(selectedTask.comments ?? []).length > 0 && (
                      <span className="px-1.5 py-0.2 text-[10px] rounded-full bg-cyan-950 text-cyan-300 border border-cyan-800/60 font-mono">
                        {(selectedTask.comments ?? []).length}
                      </span>
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={() => setTaskEditorTab('athena')}
                    className={`pb-1 border-b-2 transition-all cursor-pointer flex items-center justify-center ${taskEditorTab === 'athena' ? 'border-cyan-400 text-cyan-400' : 'border-transparent text-white/50 hover:text-white'}`}
                    title="Ask Athena"
                    aria-label="Ask Athena"
                  >
                    <Sparkles size={14} />
                  </button>
                </div>
                {taskEditorTab === 'athena' && (
                  <span
                    className="px-2 py-0.5 border border-cyan-500/30 text-cyan-400 bg-cyan-950/20 text-[10px] font-mono tracking-wider rounded uppercase whitespace-nowrap cursor-default"
                    title={`Active Model: ${selectedProvider} (${selectedModel})`}
                  >
                    {selectedProvider}:{selectedModel}
                  </span>
                )}
              </div>
            )}

            {(!selectedTask || taskEditorTab === 'details') ? (
              <div className="task-editor-grid">
                <div className="task-editor-main">
                  <label className="task-editor-field">
                    <span>Title</span>
                    <input value={taskEditor.title} onChange={(event) => setTaskEditor((current) => ({ ...current, title: event.target.value }))} placeholder="Task title" />
                  </label>
                  <label className="task-editor-field">
                    <span>Detail</span>
                    <textarea value={taskEditor.description} onChange={(event) => setTaskEditor((current) => ({ ...current, description: event.target.value }))} placeholder="Task detail, acceptance notes, or implementation context." />
                  </label>
                  <div className="task-editor-section">
                    <div className="task-editor-section-head">Colosseum execution</div>
                    <label className="task-editor-field">
                      <span>Work type</span>
                      <select value={taskEditor.workType} onChange={(event) => setTaskEditor((current) => ({ ...current, workType: event.target.value as 'development' | 'research' }))}>
                        <option value="development">Development</option>
                        <option value="research">Research / information</option>
                      </select>
                    </label>
                    {taskEditor.workType === 'development' && (
                      <label className="task-editor-field"><span>Repository</span><div className="task-inline-control"><input value={taskEditor.repository} onChange={(event) => setTaskEditor((current) => ({ ...current, repository: event.target.value }))} placeholder="Choose a local Git repository" /><button type="button" className="ghost-btn" onClick={() => void chooseColosseumRepository()}><FolderOpen size={14} /> Browse…</button></div><span className="task-editor-hint">Required before the ticket can enter grooming.</span></label>
                    )}
                    <label className="colosseum-autopilot-control">
                      <input type="checkbox" checked={taskEditor.autopilot} onChange={(event) => setTaskEditor((current) => ({ ...current, autopilot: event.target.checked }))} />
                      <span><strong>Autopilot</strong><small>Merge automatically after an independent Colosseum review passes.</small></span>
                    </label>
                  </div>
                  {!selectedTask && (
                    <div className="task-editor-section">
                      <div className="task-editor-section-head">Initial Comment</div>
                      <div className="task-inline-control">
                        <input value={taskEditor.comment} onChange={(event) => setTaskEditor((current) => ({ ...current, comment: event.target.value }))} placeholder="Initial comment..." />
                      </div>
                    </div>
                  )}
                </div>

                <aside className="task-editor-side">
                  <label className="task-editor-field">
                    <span>Status</span>
                    <div className="colosseum-status-readout">{selectedTask ? selectedTask.state.replace('-', ' ') : 'backlog'}</div>
                    <span className="task-editor-hint">Lifecycle transitions are recorded by Colosseum and human review actions.</span>
                  </label>
                  <label className="task-editor-field">
                    <span>Priority</span>
                    <select value={taskEditor.priority} onChange={(event) => setTaskEditor((current) => ({ ...current, priority: event.target.value as Task['priority'] }))}>
                      <option value="critical">critical</option>
                      <option value="high">high</option>
                      <option value="medium">medium</option>
                      <option value="low">low</option>
                    </select>
                  </label>
                  <label className="task-editor-field">
                    <span>Date</span>
                    <input type="date" value={taskEditor.due} onChange={(event) => setTaskEditor((current) => ({ ...current, due: event.target.value }))} />
                  </label>
                  {taskDrawerScope === 'global' ? (
                    <label className="task-editor-field">
                      <span>Workspace</span>
                      <select required value={taskWorkspaceId} onChange={(event) => setTaskWorkspaceId(event.target.value)}>
                        <option value="">Choose workspace...</option>
                        {workspaceList.map((workspace) => <option key={workspace.id} value={workspace.id}>{workspace.name}</option>)}
                      </select>
                    </label>
                  ) : selectedTask ? (
                    <div className="detail-row"><strong>Workspace</strong><span>{workspaceList.find((workspace) => workspace.id === selectedTask.workspaceId)?.name ?? selectedTask.workspaceId}</span></div>
                  ) : (
                    <div className="detail-row"><strong>Workspace</strong><span>{displayedWorkspaceName}</span></div>
                  )}
                  {selectedTask && <div className="detail-row"><strong>Task ID</strong><span>{selectedTask.id}</span></div>}

                  <div className="task-editor-section">
                    <div className="task-editor-section-head">Linked tasks</div>
                    <div className="task-link-list">
                      {(selectedTask?.dependsOn ?? []).length > 0 ? (selectedTask?.dependsOn ?? []).map((taskId) => {
                        const linkedTask = workspaceTasks.find((task) => task.id === taskId);
                        return (
                          <div key={taskId} className="task-link-row">
                            <span>{linkedTask?.title ?? taskId}</span>
                            <button type="button" onClick={() => removeDependency(taskId)}><Trash2 size={12} /> Remove</button>
                          </div>
                        );
                      }) : (
                        <div className="workspace-list-empty">No linked tasks.</div>
                      )}
                    </div>
                    <div className="task-inline-control relative">
                      <div className="relative flex-1">
                        <input
                          type="text"
                          value={taskSearchQuery}
                          disabled={!selectedTask}
                          onChange={(event) => {
                            setTaskSearchQuery(event.target.value);
                            setIsTaskDropdownOpen(true);
                          }}
                          onFocus={() => setIsTaskDropdownOpen(true)}
                          placeholder="Type to search task..."
                          className="w-full bg-slate-900 border border-white/15 px-2.5 py-1 text-xs rounded text-white focus:outline-none focus:border-cyan-400 placeholder:text-white/30"
                        />
                        {isTaskDropdownOpen && selectedTask && (
                          <div className="absolute left-0 right-0 top-full mt-1 max-h-48 overflow-y-auto bg-slate-900 border border-cyan-500/30 rounded-md shadow-2xl z-50">
                            {workspaceTasks
                              .filter((task) => task.id !== selectedTask?.id && !(selectedTask?.dependsOn ?? []).includes(task.id))
                              .filter((task) => task.title.toLowerCase().includes(taskSearchQuery.toLowerCase()))
                              .map((task) => (
                                <button
                                  key={task.id}
                                  type="button"
                                  className="w-full text-left px-3 py-2 text-xs hover:bg-cyan-500/20 text-white/90 border-b border-white/5 last:border-0 flex flex-col gap-0.5"
                                  onClick={() => {
                                    setTaskEditor((current) => ({ ...current, dependencyId: task.id }));
                                    setTaskSearchQuery(task.title);
                                    setIsTaskDropdownOpen(false);
                                  }}
                                >
                                  <span className="font-semibold text-cyan-300">{task.title}</span>
                                  <span className="text-[10px] text-white/50">{task.priority} · {task.state}</span>
                                </button>
                              ))}
                            {workspaceTasks
                              .filter((task) => task.id !== selectedTask?.id && !(selectedTask?.dependsOn ?? []).includes(task.id))
                              .filter((task) => task.title.toLowerCase().includes(taskSearchQuery.toLowerCase())).length === 0 && (
                              <div className="px-3 py-2 text-xs text-white/40 italic">No matching tasks found.</div>
                            )}
                          </div>
                        )}
                      </div>
                      <button
                        type="button"
                        className="ghost-btn"
                        onClick={() => {
                          void addDependency();
                          setTaskSearchQuery('');
                          setIsTaskDropdownOpen(false);
                        }}
                        disabled={!selectedTask || !taskEditor.dependencyId}
                      >
                        Link
                      </button>
                    </div>
                  </div>
                </aside>
              </div>
            ) : taskEditorTab === 'activity' ? (
              <ColosseumRunLedger runs={selectedTask?.colosseumConfig?.runs ?? []} />
            ) : taskEditorTab === 'diff' ? (
              <div className="flex flex-col h-[500px] bg-slate-950 p-4 font-mono text-xs overflow-hidden">
                <div className="flex items-center justify-between pb-3 border-b border-white/10 mb-3">
                  <div className="flex items-center gap-2 text-cyan-400 font-semibold">
                    <span>Git Worktree Diff</span>
                    {taskDiffData?.worktree_path && <span className="text-[10px] text-white/40">({taskDiffData.worktree_path})</span>}
                  </div>
                  <div className="text-[11px] text-white/50">
                    {(taskDiffData?.files || []).length} file(s) changed
                  </div>
                </div>
                {loadingTaskDiff ? (
                  <div className="flex-1 flex items-center justify-center text-cyan-400/70 animate-pulse">
                    Loading git diff...
                  </div>
                ) : !taskDiffData || (!taskDiffData.diff && (taskDiffData.files || []).length === 0) ? (
                  <div className="flex-1 flex flex-col items-center justify-center text-white/40 gap-2">
                    <span>No uncommitted or committed changes found in worktree.</span>
                    <span className="text-[11px] text-white/20">Worktrees are populated when Colosseum processes a task.</span>
                  </div>
                ) : (
                  <div className="flex-1 flex gap-4 min-h-0">
                    <div className="w-1/4 border-r border-white/10 pr-2 overflow-y-auto flex flex-col gap-1">
                      <div className="text-[10px] uppercase font-bold text-white/40 mb-1">Changed Files</div>
                      {(taskDiffData.files || []).map((file, idx) => (
                        <div key={idx} className="flex items-center gap-2 text-white/80 hover:text-white text-[11px] py-1 px-1.5 rounded bg-white/5 truncate">
                          <span className={`text-[10px] font-bold px-1 rounded ${file.status === 'M' ? 'bg-amber-500/20 text-amber-300' : file.status === 'A' ? 'bg-emerald-500/20 text-emerald-300' : 'bg-rose-500/20 text-rose-300'}`}>
                            {file.status}
                          </span>
                          <span className="truncate">{file.path}</span>
                        </div>
                      ))}
                    </div>
                    <div className="flex-1 bg-black/40 p-3 rounded border border-white/10 overflow-auto whitespace-pre leading-relaxed text-slate-200">
                      {taskDiffData.diff.split('\n').map((line, idx) => {
                        let colorClass = 'text-slate-300';
                        if (line.startsWith('+') && !line.startsWith('+++')) colorClass = 'text-emerald-400 bg-emerald-950/30';
                        else if (line.startsWith('-') && !line.startsWith('---')) colorClass = 'text-rose-400 bg-rose-950/30';
                        else if (line.startsWith('@@')) colorClass = 'text-cyan-400 font-bold';
                        else if (line.startsWith('diff ') || line.startsWith('index ')) colorClass = 'text-white/40 font-bold';
                        return (
                          <div key={idx} className={`${colorClass} px-1 rounded-sm`}>
                            {line || ' '}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                )}
              </div>
            ) : taskEditorTab === 'comments' ? (
              <div className="flex flex-col h-[500px] bg-slate-950/80 p-4 font-sans text-xs overflow-hidden">
                <div className="flex items-center justify-between pb-3 border-b border-white/10 mb-3">
                  <div className="flex items-center gap-2 text-cyan-400 font-semibold">
                    <MessageSquare size={14} />
                    <span>Ticket Discussion & Activity</span>
                  </div>
                  <div className="text-[11px] text-white/50">
                    {(selectedTask?.comments ?? []).length} comment(s)
                  </div>
                </div>

                <div className="flex-1 overflow-y-auto flex flex-col gap-3 pr-2 mb-3">
                  {(selectedTask?.comments ?? []).length === 0 ? (
                    <div className="flex-1 flex flex-col items-center justify-center text-white/30 gap-1">
                      <MessageSquare size={24} className="opacity-40" />
                      <span>No comments posted yet.</span>
                      <span className="text-[11px] text-white/20">Start the conversation below.</span>
                    </div>
                  ) : (
                    (selectedTask?.comments ?? []).map((c, index) => {
                      const isObj = typeof c === 'object' && c !== null;
                      const author = isObj ? c.author : 'User';
                      const text = isObj ? c.text : c;
                      const dateStr = isObj && c.createdAt ? new Date(c.createdAt).toLocaleString() : '';
                      const role = isObj && c.role ? c.role : 'user';

                      const isCurrentAuthor = author.toLowerCase() === (authUser?.username || authUser?.name || 'user').toLowerCase();

                      return (
                        <div
                          key={`comment-${index}`}
                          className={`flex flex-col max-w-[80%] rounded-lg p-3 border ${
                            isCurrentAuthor
                              ? 'self-end bg-cyan-950/40 border-cyan-500/30 text-cyan-50'
                              : role === 'agent'
                              ? 'self-start bg-purple-950/40 border-purple-500/30 text-purple-50'
                              : 'self-start bg-slate-900 border-white/10 text-slate-200'
                          }`}
                        >
                          <div className="flex items-center justify-between gap-3 border-b border-white/5 pb-1.5 mb-1.5 text-[10px]">
                            <div className="flex items-center gap-1.5 font-medium">
                              {role === 'agent' ? <Sparkles size={11} className="text-purple-400" /> : <User size={11} className="text-cyan-400" />}
                              <span className={isCurrentAuthor ? 'text-cyan-300 font-semibold' : 'text-slate-300'}>{author}</span>
                              {role && role !== 'user' && (
                                <span className="px-1 py-0.2 text-[9px] uppercase tracking-wider rounded bg-white/10 font-mono">
                                  {role}
                                </span>
                              )}
                            </div>
                            {dateStr && <span className="text-white/40 font-mono text-[9px]">{dateStr}</span>}
                          </div>
                          <div className="whitespace-pre-wrap text-[12px] leading-relaxed select-text">
                            {text}
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>

                <div className="flex gap-2 pt-2 border-t border-white/10">
                  <input
                    type="text"
                    value={taskEditor.comment}
                    onChange={(event) => setTaskEditor((current) => ({ ...current, comment: event.target.value }))}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && !e.shiftKey) {
                        e.preventDefault();
                        addTaskComment();
                      }
                    }}
                    placeholder="Type a comment or note (press Enter to send)..."
                    className="flex-1 bg-black/50 border border-white/15 rounded px-3 py-2 text-xs text-white placeholder:text-white/30 focus:outline-none focus:border-cyan-400"
                  />
                  <button
                    type="button"
                    onClick={addTaskComment}
                    disabled={!taskEditor.comment.trim()}
                    className="px-4 py-2 bg-cyan-600 hover:bg-cyan-500 disabled:opacity-40 disabled:hover:bg-cyan-600 text-white rounded font-medium text-xs flex items-center gap-1.5 transition-colors"
                  >
                    <Send size={13} />
                    <span>Send</span>
                  </button>
                </div>
              </div>
            ) : (() => {
              const activeTaskChat = selectedTask ? taskAthenaChats[selectedTask.id] || { messages: [], isLoading: false, error: '', input: '' } : { messages: [], isLoading: false, error: '', input: '' };
              const handleTaskAthenaSend = (e: React.FormEvent | React.KeyboardEvent) => {
                e.preventDefault();
                if (!selectedTask || !onSendTaskAthenaMessage) return;
                const text = (activeTaskChat.input || '').trim();
                if (!text) return;
                onSendTaskAthenaMessage(selectedTask.id, text);
              };
               return (
                <div className="task-athena-container">
                  <div className="px-4 py-2 bg-cyan-950/20 border-b border-cyan-500/10 flex flex-col gap-0.5">
                    <div className="text-[9px] font-bold text-cyan-400 uppercase tracking-wider">Athena Context: Edit Task</div>
                    <div className="text-[12px] font-semibold text-white/90 truncate">
                      {selectedTask?.title || 'Unknown Task'}
                    </div>
                    <div className="text-[10px] text-white/50 flex gap-2">
                      <span>Status: <strong className="text-cyan-300/80">{selectedTask?.state}</strong></span>
                      <span>Priority: <strong className="text-cyan-300/80">{selectedTask?.priority}</strong></span>
                    </div>
                  </div>
                  <div className="task-athena-messages">
                    {activeTaskChat.messages.length === 0 ? (
                      <div className="task-athena-empty">
                        <Sparkles size={28} />
                        <span>Ask Athena about this task</span>
                      </div>
                    ) : activeTaskChat.messages.map((message: any) => (
                      <article key={message.id} className={`task-athena-message task-athena-message-${message.sender}`}>
                        <div className="task-athena-message-body">
                          {message.sender === 'assistant' ? (
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.text}</ReactMarkdown>
                          ) : message.text}
                        </div>
                        <footer>
                          <span>{message.sender === 'user' ? 'Operator' : 'Athena'} · {new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                          <div>
                            <button type="button" onClick={() => { void navigator.clipboard?.writeText(message.text); }} title="Copy message" aria-label="Copy message"><Copy size={12} /></button>
                            <button type="button" onClick={() => onDeleteTaskAthenaMessage && onDeleteTaskAthenaMessage(selectedTask.id, message.id)} title="Delete message" aria-label="Delete message"><Trash2 size={12} /></button>
                          </div>
                        </footer>
                      </article>
                    ))}
                    {activeTaskChat.isLoading && (
                      <div className="task-athena-loading">
                        <Loader2 size={14} />
                        <span>Reading task context...</span>
                      </div>
                    )}
                    {activeTaskChat.error && <div className="task-athena-error">{activeTaskChat.error}</div>}
                  </div>

                  <div className="task-athena-composer">
                    <input
                      type="text"
                      value={activeTaskChat.input || ''}
                      onChange={(event) => onChangeTaskAthenaInput && onChangeTaskAthenaInput(selectedTask.id, event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') {
                          event.preventDefault();
                          event.stopPropagation();
                          handleTaskAthenaSend(event);
                        }
                      }}
                      placeholder={activeTaskChat.isLoading ? "Athena is thinking... send message to steer" : "Ask Athena about this task..."}
                    />
                    <button type="button" onClick={handleTaskAthenaSend} disabled={!(activeTaskChat.input || '').trim()} title="Send" aria-label="Send">
                      <Send size={15} />
                    </button>
                    {activeTaskChat.messages.length > 0 && onClearTaskAthenaChat && (
                      <button
                        type="button"
                        onClick={() => { if (window.confirm('Clear all Athena chat history for this task?')) onClearTaskAthenaChat(selectedTask.id); }}
                        title="Clear chat history"
                        aria-label="Clear chat"
                        className="ghost-btn action-close icon-only"
                        style={{ opacity: 0.5 }}
                      >
                        <Trash2 size={13} />
                      </button>
                    )}
                  </div>
                </div>
              );
            })()}

            <div className="modal-actions">
              {(selectedTask?.state === 'backlog' || selectedTask?.state === 'grooming') && taskEditorTab === 'details' && (
                <button type="button" className="ghost-btn colosseum-submit-btn" onClick={() => void submitForGrooming()}><Zap size={14} /> {selectedTask.state === 'grooming' ? 'Retry grooming' : 'Send to grooming'}</button>
              )}
              {selectedTask?.state === 'human-review' && (
                <>
                  <button type="button" className="ghost-btn colosseum-reject-btn" onClick={() => void submitHumanDecision('reject')}>Request changes</button>
                  <button type="button" className="ghost-btn colosseum-approve-btn" onClick={() => void submitHumanDecision('approve')}><Check size={14} /> Approve</button>
                </>
              )}
              {(!selectedTask || taskEditorTab === 'details') && (
                <button type="submit" className="ghost-btn action-save icon-only" aria-label={selectedTask ? 'Save task' : 'Create task'} title={selectedTask ? 'Save task' : 'Create task'}><Check size={14} /></button>
              )}
              <button type="button" className="ghost-btn action-close icon-only" aria-label="Cancel" title="Cancel" onClick={() => setIsTaskModalOpen(false)}><X size={14} /></button>
            </div>
          </form>
        </div>
      )}

      {selectedNote && (
        <div className="modal-backdrop" role="presentation" onClick={() => setSelectedNote(null)}>
          <div className="modal-card note-info-modal" role="dialog" aria-modal="true" aria-labelledby="note-info-title" onClick={(event) => event.stopPropagation()}>
            <div className="modal-head">
              <div>
                <div className="eyebrow">Note info</div>
                <h2 id="note-info-title">{selectedNote.title}</h2>
              </div>
              <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setSelectedNote(null)}><X size={14} /></button>
            </div>
            <div className="note-info-body">
              <div className="detail-row"><strong>Session</strong><span>{getNoteSessionTitle(selectedNote)}</span></div>
              <div className="detail-row"><strong>Note ID</strong><span>{selectedNote.id}</span></div>
              <p>{selectedNote.body}</p>
            </div>
          </div>
        </div>
      )}

      {isEditOpen && (
        <div className="modal-backdrop" role="presentation" onClick={() => setIsEditOpen(false)}>
          <div className="modal-card" role="dialog" aria-modal="true" aria-labelledby="edit-title" onClick={(event) => event.stopPropagation()}>
            <div className="modal-head">
              <div>
                <div className="eyebrow">{workspaceEditorMode === 'create' ? 'Add workspace' : 'Quick edit'}</div>
                <h2 id="edit-title">{workspaceEditorMode === 'create' ? 'Create workspace' : 'Workspace details'}</h2>
              </div>
              <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsEditOpen(false)}><X size={14} /></button>
            </div>
            <div className="entity-editor-grid">
              <label className="task-editor-field entity-editor-span-2"><span>Workspace name</span><input value={editDraft.workspaceName} onChange={(event) => setEditDraft((current) => ({ ...current, workspaceName: event.target.value }))} /></label>
              <label className="task-editor-field entity-editor-span-2"><span>Description</span><textarea value={editDraft.workspaceDescription} onChange={(event) => setEditDraft((current) => ({ ...current, workspaceDescription: event.target.value }))} placeholder="Workspace summary, scope, or operating notes." /></label>
              <div className="workspace-editor-inline-row entity-editor-span-2">
                <label className="task-editor-field"><span>Priority</span><select value={editDraft.workspacePriority} onChange={(event) => setEditDraft((current) => ({ ...current, workspacePriority: event.target.value as 'critical' | 'high' | 'medium' | 'low' }))}><option value="critical">critical</option><option value="high">high</option><option value="medium">medium</option><option value="low">low</option></select></label>
                <label className="task-editor-field"><span>Status</span><select value={editDraft.workspaceStatus} onChange={(event) => setEditDraft((current) => ({ ...current, workspaceStatus: event.target.value as 'open' | 'closed' }))}><option value="open">open</option><option value="closed">closed</option></select></label>
                <label className="task-editor-field"><span>Color</span><input type="color" value={editDraft.workspaceColor || '#00e5ff'} onChange={(event) => setEditDraft((current) => ({ ...current, workspaceColor: event.target.value }))} /></label>
              </div>
            </div>
            <div className="modal-actions">
              <button className="ghost-btn action-save icon-only" aria-label={workspaceEditorMode === 'create' ? 'Create workspace' : 'Save workspace'} title={workspaceEditorMode === 'create' ? 'Create workspace' : 'Save workspace'} onClick={saveEditDraft}><span aria-hidden="true">✓</span></button>
              <button className="ghost-btn action-close icon-only" aria-label="Cancel" title="Cancel" onClick={() => setIsEditOpen(false)}><span aria-hidden="true">✕</span></button>
            </div>
          </div>
        </div>
      )}

      {isHelpOpen && (
        <div className="modal-backdrop" role="presentation" onClick={() => setIsHelpOpen(false)}>
          <div className="modal-card help-card" role="dialog" aria-modal="true" aria-labelledby="help-title" onClick={(event) => event.stopPropagation()}>
            <div className="modal-head">
              <div>
                <div className="eyebrow">Keyboard help</div>
                <h2 id="help-title">Sanctum shortcuts</h2>
              </div>
              <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={() => setIsHelpOpen(false)}><X size={14} /></button>
            </div>
            <div className="shortcut-grid">
              {[
                ['W', 'Workspace'], ['S', 'Session'], ['T', 'Tasks'], ['R', 'Reminders'], ['N', 'Notes'], ['F', 'Files'], ['J', 'Jira'], ['M', 'Merge requests'], ['K', 'Knowledge'], ['P', 'Providers'], ['G', 'Settings'], ['?', 'Toggle this help'], ['Esc', 'Close overlays'],
              ].map(([key, label]) => (
                <div key={label} className="shortcut-row"><span className="shortcut-key">{key}</span><span className="shortcut-label">{label}</span></div>
              ))}
            </div>
          </div>
        </div>
      )}

      <SettingsModal
        open={settingsOpen}
        authUser={authUser}
        authDraft={authDraft}
        serverDraft={serverDraft}
        gatewayDraft={gatewayDraft}
        selectedProvider={selectedProvider}
        selectedModel={selectedModel}
        providers={gatewayProviders}
        hasApiKey={hasApiKey}
        onClose={() => setSettingsOpen(false)}
        onAuthUserChange={setAuthUser}
        onAuthDraftChange={setAuthDraft}
        onServerDraftChange={setServerDraft}
        onGatewayDraftChange={setGatewayDraft}
        onSelectedProviderChange={setSelectedProvider}
        onSelectedModelChange={setSelectedModel}
        onSave={onSaveSettings}
        onLogout={onLogout}
        onRefreshProviders={onRefreshProviders}
        colosseumGroomingSettings={colosseumGroomingSettings}
        colosseumReadySettings={colosseumReadySettings}
        colosseumReviewSettings={colosseumReviewSettings}
        onColosseumGroomingSettingsChange={onColosseumGroomingSettingsChange}
        onColosseumReadySettingsChange={onColosseumReadySettingsChange}
        onColosseumReviewSettingsChange={onColosseumReviewSettingsChange}
      />

      {managementDrawer && (
        <div className="management-drawer-backdrop" onClick={() => setManagementDrawer(null)}>
          <div className="management-drawer" onClick={(e) => e.stopPropagation()}>
            <div style={{ background: 'var(--cp-bg-1)', borderBottom: '1px solid var(--cp-border)' }} className="flex items-center justify-between px-5 h-12 shrink-0">
              <div className="flex items-center gap-3">
                <span style={{ color: 'var(--cp-cyan)' }}>{managementDrawer === 'tasks' ? <ListChecks size={18} /> : <Timer size={18} />}</span>
                <span style={{ fontFamily: "'Orbitron', monospace", color: 'var(--cp-cyan)', letterSpacing: '0.15em' }} className="text-sm font-bold uppercase tracking-widest">
                  {managementDrawer === 'tasks' ? 'Task Queue' : 'Reminder Schedule'}
                </span>
                <span style={{ color: 'var(--muted-foreground)', fontFamily: "'Share Tech Mono', monospace" }} className="text-xs opacity-60">
                  {managementDrawer === 'tasks' ? `${workspaceTasks.length} tasks · ${displayedWorkspaceName}` : `${workspaceReminders.length} reminders · ${displayedWorkspaceName}`}
                </span>
              </div>
              <button onClick={() => setManagementDrawer(null)} style={{ color: 'var(--cp-cyan)', border: '1px solid var(--cp-border)', background: 'transparent', fontFamily: "'Share Tech Mono', monospace" }} className="px-3 py-1 text-xs hover:brightness-125 transition-all">
                close
              </button>
            </div>
            <div className="p-5 overflow-auto flex-1">
              {managementDrawer === 'tasks' && (
                <div className="management-drawer-list">
                  {workspaceTasks.map((task) => (
                    <button key={task.id} type="button" className="management-drawer-row" onClick={() => { setTaskFlags((current) => ({ ...current, [task.id]: { done: !(current[task.id]?.done) } })); pushToast('Task updated', `${task.title} toggled.`, 'good'); }}>
                      <div className="flex items-center gap-3 flex-1 min-w-0"><Circle size={8} style={{ color: task.priority === 'critical' ? '#ff2244' : task.priority === 'high' ? '#ffe600' : '#00e5ff', fill: task.priority === 'critical' ? '#ff2244' : task.priority === 'high' ? '#ffe600' : '#00e5ff', filter: `drop-shadow(0 0 4px ${task.priority === 'critical' ? '#ff2244' : task.priority === 'high' ? '#ffe600' : '#00e5ff'})`, flexShrink: 0 }} /><span style={{ color: 'var(--foreground)' }} className="text-sm truncate">{task.title}</span></div>
                      <div className="flex items-center gap-3"><span style={{ fontFamily: "'Share Tech Mono', monospace", color: task.state === 'in-progress' ? 'var(--cp-cyan)' : task.state === 'done' ? 'var(--cp-green)' : 'var(--muted-foreground)', border: '1px solid var(--cp-border)' }} className="text-[10px] px-2 py-0.5 uppercase">{taskFlags[task.id]?.done ? 'done' : task.state}</span><span style={{ color: 'var(--muted-foreground)' }} className="text-xs">{task.priority}</span><span style={{ color: 'var(--muted-foreground)' }} className="text-xs">{task.owner}</span></div>
                    </button>
                  ))}
                </div>
              )}
              {managementDrawer === 'reminders' && (
                <div className="management-drawer-list">
                  {workspaceReminders.map((reminder) => (
                    <button key={reminder.id} type="button" className="management-drawer-row" onClick={() => { setReminderFlags((current) => ({ ...current, [reminder.id]: { done: !(current[reminder.id]?.done) } })); pushToast('Reminder updated', `${reminder.title} toggled.`, 'good'); }}>
                      <div className="flex items-center gap-3 flex-1 min-w-0"><Circle size={8} style={{ color: reminderFlags[reminder.id]?.done ? 'var(--muted-foreground)' : '#00ff88', fill: reminderFlags[reminder.id]?.done ? 'var(--muted-foreground)' : '#00ff88', filter: reminderFlags[reminder.id]?.done ? 'none' : 'drop-shadow(0 0 4px #00ff88)', flexShrink: 0 }} /><span style={{ color: reminderFlags[reminder.id]?.done ? 'var(--muted-foreground)' : 'var(--foreground)', textDecoration: reminderFlags[reminder.id]?.done ? 'line-through' : 'none' }} className="text-sm truncate">{reminder.title}</span></div>
                      <div className="flex items-center gap-3"><span style={{ fontFamily: "'Share Tech Mono', monospace", color: reminderFlags[reminder.id]?.done ? 'var(--muted-foreground)' : 'var(--cp-cyan)', border: '1px solid var(--cp-border)' }} className="text-[10px] px-2 py-0.5 uppercase">{reminderFlags[reminder.id]?.done ? 'done' : reminder.state}</span><span style={{ color: 'var(--muted-foreground)', fontFamily: "'Share Tech Mono', monospace" }} className="text-xs whitespace-nowrap">{reminder.due}</span></div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
