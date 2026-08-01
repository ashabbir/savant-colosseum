import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import {
  Folder,
  MessageSquare,
  ListChecks,
  Timer,
  FileText,
  FileCode,
  Zap,
  GitBranch,
  Sparkles,
  Cpu,
  Settings,
  LogOut,
  ChevronLeft,
  ChevronRight,
  ChevronDown,
  Circle,
  HelpCircle,
  Edit2,
  LayoutGrid,
  Plus,
  PlusSquare,
  History,
  Share2
} from 'lucide-react';
import {
  artifacts,
  localSetup,
  navigation,
  notes,
  providers,
  sessionEvents,
  sectionProfiles,
  reminders,
  sessions,
  tasks,
  apiSurface,
  mcpSurface,
  type Session,
  type Workspace,
  type SectionId,
  type SurfaceMode,
} from './data';
import { LoginScreen } from './components/LoginScreen';
import { AppOverlays } from './components/AppOverlays';
import { ShellChrome } from './components/ShellChrome';
import { WorkspaceAthenaDrawer } from './components/WorkspaceAthenaDrawer';
import { WorkspaceSurface } from './components/WorkspaceSurface';
import {
  DEFAULT_COLOSSEUM_GROOMING_SETTINGS,
  DEFAULT_COLOSSEUM_READY_SETTINGS,
  DEFAULT_COLOSSEUM_REVIEW_SETTINGS,
  type ColosseumSettings,
} from './components/SettingsModal';
import {
  buildWorkspaceEditorPayload,
  createWorkspaceEditorDraft,
  createWorkspaceEditorDraftFromWorkspace,
  type WorkspaceEditorDraft,
  type WorkspaceEditorMode,
} from './components/workspaceEditor';
import { getSessionAdapter, inferSessionProvider, type SessionConversationMessage, type SessionFileGroup } from './services/sessionAdapters';
import { DetailBlock, DetailRow, IntelligencePulse, KnowledgeNetwork, Metric, PanelHeader, Row, StatusOrb } from './components/WorkspacePrimitives';
import {
  fetchWorkspaceKnowledgeGraph,
  fetchGatewayMCPs,
  filterSavantMcpData,
} from './lib/athenaContext';
import { buildWorkspaceAthenaPrompt, buildTaskAthenaPrompt, type ChatMessage } from './services/athenaService';
import { buildSavantHeaders } from './services/httpClient';
import { normalizeServerWorkspace } from './services/workspaceAdapters';
import type { Artifact, Task } from './data';

const STORAGE_KEY = 'savant-sanctum:ui-state';

type WorkspaceChatState = {
  messages: ChatMessage[];
  isLoading: boolean;
  error: string;
  input: string;
};

type SavedUiState = {
  activeSection?: SectionId;
  surfaceMode?: SurfaceMode;
  workspaceIndex?: number;
  sessionIndex?: number;
  workspaceList?: Workspace[];
  sessionList?: Session[];
  noteList?: typeof notes;
  taskList?: typeof tasks;
  reminderList?: typeof reminders;
  workspaceNameOverride?: Record<string, string>;
  sessionTitleOverride?: Record<string, string>;
  providerOverride?: Record<string, string>;
  reminderOverride?: Record<string, string>;
  sessionFlags?: Record<string, SessionFlags>;
  taskFlags?: Record<string, EntityFlags>;
  reminderFlags?: Record<string, EntityFlags>;
  contractValidation?: string;
};

type ActivityItem = {
  id: string;
  label: string;
  detail: string;
  section: SectionId;
  surfaceMode: SurfaceMode;
  workspaceIndex: number;
  sessionIndex: number;
};

type Toast = {
  id: string;
  title: string;
  detail: string;
  tone: 'good' | 'warning' | 'muted';
  createdAt: number;
};

type ServerSession = Record<string, any>;

type ServerMergeRequest = Record<string, any>;

type ServerJiraTicket = Record<string, any>;

type ServerNote = {
  note_id?: string;
  text?: string;
  timestamp?: string;
  created_at?: string;
};

type WorkspaceMergeRequest = {
  id: string;
  workspaceId: string;
  mrId: string;
  mrIid?: number;
  title: string;
  status: string;
  createdAt?: string;
  updatedAt?: string;
  sessionId?: string;
  source?: string;
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
  sessionId?: string;
  source?: string;
};

type WorkspaceSessionFileGroups = Record<string, SessionFileGroup>;
type AthenaEngineSettings = {
  provider?: string;
  model?: string;
};

function normalizeServerNote(raw: ServerNote, sessionId: string) {
  const body = String(raw.text ?? '').trim();
  const title = body.split('\n')[0]?.trim().slice(0, 72) || 'Session note';
  const fallbackId = `${sessionId}-${raw.timestamp ?? raw.created_at ?? body.slice(0, 24) ?? 'note'}`;
  return {
    id: String(raw.note_id ?? fallbackId),
    sessionId,
    title,
    body,
    createdAt: raw.timestamp ?? raw.created_at,
  };
}

function normalizeServerMergeRequest(raw: ServerMergeRequest): WorkspaceMergeRequest | null {
  const id = String(raw.mr_id ?? raw.id ?? '').trim();
  if (!id) return null;
  return {
    id,
    workspaceId: String(raw.workspace_id ?? raw.workspaceId ?? ''),
    mrId: id,
    mrIid: Number(raw.mr_iid ?? raw.iid ?? 0) || 0,
    title: String(raw.title ?? raw.url ?? id),
    status: String(raw.status ?? 'open'),
    createdAt: String(raw.created_at ?? ''),
    updatedAt: String(raw.updated_at ?? raw.created_at ?? ''),
    sessionId: String(raw.session_id ?? ''),
    source: String(raw.source ?? 'server'),
  };
}

function normalizeServerJiraTicket(raw: ServerJiraTicket): WorkspaceJiraTicket | null {
  const id = String(raw.ticket_id ?? raw.id ?? '').trim();
  if (!id) return null;
  return {
    id,
    workspaceId: String(raw.workspace_id ?? raw.workspaceId ?? ''),
    ticketId: id,
    ticketKey: String(raw.ticket_key ?? raw.key ?? id),
    title: String(raw.title ?? id),
    status: String(raw.status ?? 'open'),
    createdAt: String(raw.created_at ?? ''),
    updatedAt: String(raw.updated_at ?? raw.created_at ?? ''),
    sessionId: String(raw.session_id ?? ''),
    source: String(raw.source ?? 'server'),
  };
}

const sectionIcons: Record<string, ReactNode> = {
  workspace: <Folder size={14} />,
  manage: <LayoutGrid size={14} />,
  session: <MessageSquare size={14} />,
  tasks: <ListChecks size={14} />,
  reminders: <Timer size={14} />,
  notes: <FileText size={14} />,
  files: <FileCode size={14} />,
  jira: <Zap size={14} />,
  'merge-requests': <GitBranch size={14} />,
  knowledge: <Sparkles size={14} />,
  providers: <Cpu size={14} />,
  settings: <Settings size={14} />,
};

const providerRoutePrefix: Record<string, string> = {
  copilot: '',
  claude: '/claude',
  codex: '/codex',
  gemini: '/gemini',
  savant: '/savant',
};

type SessionFlags = {
  starred?: boolean;
  archived?: boolean;
};

type EntityFlags = {
  done?: boolean;
  blocked?: boolean;
  lastMovedAt?: string;
  lastMovedFrom?: string;
  lastMovedTo?: string;
};

function App() {
  const [savedState, setSavedState] = useState<SavedUiState>(() => {
    if (typeof window === 'undefined') return {};
    try {
      const raw = window.localStorage.getItem(STORAGE_KEY);
      return raw ? (JSON.parse(raw) as SavedUiState) : {};
    } catch {
      return {};
    }
  });
  const [activeSection, setActiveSection] = useState<SectionId>(savedState.activeSection ?? 'workspace');
  const [surfaceMode, setSurfaceMode] = useState<SurfaceMode>(savedState.surfaceMode ?? 'overview');
  const [workspaceIndex, setWorkspaceIndex] = useState(savedState.workspaceIndex ?? 0);
  const [sessionIndex, setSessionIndex] = useState(savedState.sessionIndex ?? 0);
  const [isEditOpen, setIsEditOpen] = useState(false);
  const [isHelpOpen, setIsHelpOpen] = useState(false);
  const [isAlertsOpen, setIsAlertsOpen] = useState(false);
  const [isActivityOpen, setIsActivityOpen] = useState(false);
  const [isKnowledgeOpen, setIsKnowledgeOpen] = useState(false);
  const [isWorkspaceAthenaOpen, setIsWorkspaceAthenaOpen] = useState(false);
  const [isTaskDrawerOpen, setIsTaskDrawerOpen] = useState(false);
  const [taskCreateRequest, setTaskCreateRequest] = useState(0);
  const [taskToEdit, setTaskToEdit] = useState<Task | null>(null);
  const [isNotesDrawerOpen, setIsNotesDrawerOpen] = useState(false);
  const [isMergeRequestsDrawerOpen, setIsMergeRequestsDrawerOpen] = useState(false);
  const [isJiraDrawerOpen, setIsJiraDrawerOpen] = useState(false);
  const [isArtifactsDrawerOpen, setIsArtifactsDrawerOpen] = useState(false);
  const [isSessionsDrawerOpen, setIsSessionsDrawerOpen] = useState(false);
  const [isDrawerOpen, setIsDrawerOpen] = useState(true);
  const [isWorkspacePaneOpen, setIsWorkspacePaneOpen] = useState(true);
  const [managementDrawer, setManagementDrawer] = useState<'tasks' | 'reminders' | null>(null);
  const [activityFeed, setActivityFeed] = useState<ActivityItem[]>([]);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [workspaceList, setWorkspaceList] = useState<Workspace[]>([]);
  const [sessionList, setSessionList] = useState<Session[]>(savedState.sessionList ?? []);
  const [workspaceSessionFiles, setWorkspaceSessionFiles] = useState<WorkspaceSessionFileGroups>({});
  const [noteList, setNoteList] = useState(savedState.noteList ?? []);
  const [taskList, setTaskList] = useState(savedState.taskList ?? []);
  const [reminderList, setReminderList] = useState(savedState.reminderList ?? []);
  const [mergeRequestList, setMergeRequestList] = useState<WorkspaceMergeRequest[]>([]);
  const [jiraTicketList, setJiraTicketList] = useState<WorkspaceJiraTicket[]>([]);
  const [sessionFlags, setSessionFlags] = useState<Record<string, SessionFlags>>(savedState.sessionFlags ?? {});
  const [taskFlags, setTaskFlags] = useState<Record<string, EntityFlags>>(savedState.taskFlags ?? {});
  const [reminderFlags, setReminderFlags] = useState<Record<string, EntityFlags>>(savedState.reminderFlags ?? {});
  const [sessionConversationMap, setSessionConversationMap] = useState<Record<string, SessionConversationMessage[]>>({});
  const [contractValidation, setContractValidation] = useState(savedState.contractValidation ?? 'Not run');
  const [isWorkspaceLoading, setIsWorkspaceLoading] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [athenaChats, setAthenaChats] = useState<Record<string, WorkspaceChatState>>({});
  const [taskAthenaChats, setTaskAthenaChats] = useState<Record<string, WorkspaceChatState>>({});
  const [selectedProvider, setSelectedProvider] = useState('gemini');
  const [selectedModel, setSelectedModel] = useState('3.5');
  const [gatewayProviders, setGatewayProviders] = useState<Array<{ id: string; label: string; models: string[] }>>([]);
  const [colosseumGroomingSettings, setColosseumGroomingSettings] = useState<ColosseumSettings>(DEFAULT_COLOSSEUM_GROOMING_SETTINGS);
  const [colosseumReadySettings, setColosseumReadySettings] = useState<ColosseumSettings>(DEFAULT_COLOSSEUM_READY_SETTINGS);
  const [colosseumReviewSettings, setColosseumReviewSettings] = useState<ColosseumSettings>(DEFAULT_COLOSSEUM_REVIEW_SETTINGS);
  const [authDraft, setAuthDraft] = useState(() => {
    if (typeof window === 'undefined') return '';
    return window.localStorage.getItem('user:apiKey')?.trim() ?? '';
  });
  const [serverDraft, setServerDraft] = useState('http://127.0.0.1:8090');
  const [gatewayDraft, setGatewayDraft] = useState('http://127.0.0.1:3100');
  const [authUser, setAuthUser] = useState('operator');
  const [currentUserId, setCurrentUserId] = useState('');
  const [authReady, setAuthReady] = useState(() => {
    if (typeof window === 'undefined') return false;
    return Boolean(window.localStorage.getItem('user:apiKey')?.trim());
  });
  const [workspaceGraphStats, setWorkspaceGraphStats] = useState<Record<string, {
    total_nodes: number;
    total_edges: number;
    committed_nodes: number;
    staged_nodes: number;
    committed_edges: number;
    staged_edges: number;
    nodes_by_type: Record<string, number>;
  }>>({});
  const [workspaceSearch, setWorkspaceSearch] = useState('');
  const [workspaceNameOverride, setWorkspaceNameOverride] = useState<Record<string, string>>(savedState.workspaceNameOverride ?? {});
  const [sessionTitleOverride, setSessionTitleOverride] = useState<Record<string, string>>(savedState.sessionTitleOverride ?? {});
  const [providerOverride, setProviderOverride] = useState<Record<string, string>>(savedState.providerOverride ?? {});
  const [reminderOverride, setReminderOverride] = useState<Record<string, string>>(savedState.reminderOverride ?? {});
  const [isSessionPaneOpen, setIsSessionPaneOpen] = useState(true);
  const [workspaceEditorMode, setWorkspaceEditorMode] = useState<WorkspaceEditorMode>('edit');
  const [editDraft, setEditDraft] = useState<WorkspaceEditorDraft>(() => createWorkspaceEditorDraft());
  const sanctumMark = new URL('../../sanctum.svg', import.meta.url).href;
  const serverBaseUrl = serverDraft.trim() || 'http://127.0.0.1:8090';
  const apiKey = authDraft.trim();
  const resolveApiPath = (path: string) => (import.meta.env.DEV ? path : `${serverBaseUrl}${path}`);

  const activeWorkspace = workspaceList[workspaceIndex] ?? workspaceList[0];
  const activeWorkspaceId = activeWorkspace?.id ?? '';
  const workspaceSessions = sessionList.filter((session) => session.workspaceId === activeWorkspaceId);
  const workspaceSessionIdsKey = workspaceSessions.map((session) => session.id).join('|');
  const activeSession = workspaceSessions[sessionIndex % workspaceSessions.length] ?? workspaceSessions[0] ?? sessions[0];
  const workspaceTasks = taskList.filter((task) => task.workspaceId === activeWorkspaceId);
  const workspaceReminders = reminderList.filter((reminder) => reminder.workspaceId === activeWorkspaceId);
  const todayIso = new Date().toISOString().slice(0, 10);
  const todayTasks = workspaceTasks.filter((task) => {
    const taskDate = (task as unknown as { date?: string }).date;
    return taskDate === todayIso || task.due === 'Today';
  });
  const todayReminders = workspaceReminders.filter((reminder) => reminder.dueLabel === 'Today' || reminder.due.includes('Today'));
  const sessionNotes = useMemo(() => noteList.filter((note) => note.sessionId === activeSession.id), [activeSession.id, noteList]);
  const liveArtifacts = useMemo<Artifact[]>(() => [
    ...Object.entries(workspaceSessionFiles).flatMap(([sessionId, group]) => (group.files ?? []).map((file, index) => ({
      id: `file-${sessionId}-${file.path ?? file.name ?? index}`,
      sessionId,
      title: String(file.path ?? file.name ?? 'Session file'),
      kind: 'file' as const,
      count: 1,
    }))),
    ...mergeRequestList.filter((item) => item.sessionId).map((item) => ({ id: `mr-${item.id}`, sessionId: item.sessionId!, title: item.title, kind: 'merge-request' as const, count: 1 })),
    ...jiraTicketList.filter((item) => item.sessionId).map((item) => ({ id: `jira-${item.id}`, sessionId: item.sessionId!, title: item.title, kind: 'jira' as const, count: 1 })),
  ], [jiraTicketList, mergeRequestList, workspaceSessionFiles]);
  const allArtifacts = useMemo(() => [...artifacts, ...liveArtifacts], [liveArtifacts]);
  const sessionArtifacts = useMemo(() => allArtifacts.filter((artifact) => artifact.sessionId === activeSession.id), [activeSession.id, allArtifacts]);
  const workspaceNotes = useMemo(() => noteList.filter((note) => workspaceSessions.some((session) => session.id === note.sessionId)), [noteList, workspaceSessions]);
  const workspaceArtifacts = useMemo(() => allArtifacts.filter((artifact) => workspaceSessions.some((session) => session.id === artifact.sessionId)), [allArtifacts, workspaceSessions]);
  const sessionTimeline = useMemo(() => sessionEvents.filter((event) => event.sessionId === activeSession.id), [activeSession.id]);
  const activeSessionFileGroup = workspaceSessionFiles[activeSession.id];
  const displayedWorkspaceName = activeWorkspace ? (workspaceNameOverride[activeWorkspace.id] ?? activeWorkspace.name) : 'No workspace';
  const displayedSessionTitle = sessionTitleOverride[activeSession.id] ?? activeSession.title;
  const displayedProvider = providerOverride[activeSession.id] ?? getSessionAdapter(inferSessionProvider(activeSession.provider, activeSession as unknown as Record<string, any>, activeSessionFileGroup)).displayName;
  const sessionConversationFallback = useMemo(
    () => sessionTimeline.map((event) => ({
      id: event.id,
      time: event.time,
      kind: event.kind === 'task' ? 'assistant' : event.kind === 'note' ? 'user' : event.kind === 'artifact' ? 'tool' : 'system',
      role: event.kind,
      title: event.title,
      detail: event.detail,
      provider: displayedProvider,
    })) satisfies SessionConversationMessage[],
    [displayedProvider, sessionTimeline],
  );
  const sessionConversation = sessionConversationMap[activeSession.id] ?? sessionConversationFallback;
  const displayedReminder = reminderOverride[workspaceReminders[0]?.id ?? ''] ?? workspaceReminders[0]?.due ?? 'None';
  const activeProvider = providers.find((provider) => provider.name === displayedProvider) ?? providers[0];
  const activeProfile = sectionProfiles.find((profile) => profile.id === activeSection) ?? sectionProfiles[0];
  const sectionLabel = navigation.find((item) => item.id === activeSection)?.label ?? 'Workspace';
  const activityCount = activityFeed.length;
  const hasApiKey = authDraft.trim().length > 0;
  const selectedProviderLabel = gatewayProviders.find((provider) => provider.id === selectedProvider)?.label ?? selectedProvider;
  const selectedProviderRef = useRef(selectedProvider);
  const selectedModelRef = useRef(selectedModel);

  const saveSettingSafely = async (key: string, value: unknown) => {
    try {
      await window.system?.saveSetting?.(key, value);
    } catch {
      // Fall back to local state when the Electron bridge is unavailable or stale.
    }
  };

  const saveAthenaEngineSafely = async (provider: string, model: string) => {
    const cleanProvider = provider.trim();
    const cleanModel = model.trim();
    await saveSettingSafely('athena:engine', { provider: cleanProvider, model: cleanModel } satisfies AthenaEngineSettings);
    await saveSettingSafely('provider:chain', [{ id: 'p1', provider: cleanProvider, model: cleanModel }]);
  };

  const updateAthenaProvider = (value: string) => {
    selectedProviderRef.current = value;
    setSelectedProvider(value);
    void saveAthenaEngineSafely(value, selectedModelRef.current);
  };

  const updateAthenaModel = (value: string) => {
    selectedModelRef.current = value;
    setSelectedModel(value);
    void saveAthenaEngineSafely(selectedProviderRef.current, value);
  };

  const persistApiKey = async (value: string, serverUrl?: string) => {
    const clean = value.trim();
    const targetServerUrl = serverUrl?.trim() || serverDraft.trim() || 'http://127.0.0.1:8090';
    await saveSettingSafely('user:apiKey', clean);
    await saveSettingSafely('server:config', { url: targetServerUrl, enabled: true });
    setServerDraft(targetServerUrl);
    if (clean) {
      window.localStorage.setItem('user:apiKey', clean);
      try {
        const response = await fetch(`${targetServerUrl}/api/auth/validate`, {
          headers: buildSavantHeaders(clean)
        });
        if (response.ok) {
          const data = await response.json();
          if (data.valid) {
            if (data.name) {
              setAuthUser(data.name);
              await saveSettingSafely('user:name', data.name);
            }
            if (data.user_id) {
              setCurrentUserId(data.user_id);
            }
          }
        }
      } catch (e) {
        console.error('Failed to sync username from server on login:', e);
      }
    } else {
      window.localStorage.removeItem('user:apiKey');
      setCurrentUserId('');
    }
    setAuthDraft(clean);
    setAuthReady(true);
  };

  const handleAuthLogout = async () => {
    await saveSettingSafely('user:apiKey', '');
    window.localStorage.removeItem('user:apiKey');
    setAuthDraft('');
    setAuthReady(false);
  };

  const refreshProviders = async (gatewayUrl?: string) => {
    try {
      if (window.system?.listProviders) {
        const res = await window.system.listProviders(gatewayUrl);
        if (Array.isArray(res?.providers) && res.providers.length > 0) {
          setGatewayProviders(res.providers);
          return;
        }
      }
    } catch (e) {
      console.error('Failed to refresh providers:', e);
    }
  };

  const handleClearAthenaChat = (wsId: string) => {
    setAthenaChats(prev => ({
      ...prev,
      [wsId]: { messages: [], isLoading: false, error: '', input: '' },
    }));
    void window.system?.saveAthenaThread?.({ threadKey: `athena:chat:${wsId}`, messages: [], input: '' });
  };

  const runAgentViaGateway = async (prompt: string, wsId: string, mcpData?: any, providerOverride?: string, modelOverride?: string): Promise<string> => {
    const gatewayUrl = (gatewayDraft || 'http://127.0.0.1:3100').trim().replace(/\/+$/, '');
    const finalProvider = providerOverride || selectedProvider;
    const finalModel = modelOverride || selectedModel;
    const runRes = await fetch(`${gatewayUrl}/runs`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        prompt,
        chain: [{ provider: finalProvider, model: finalModel }],
        session_id: wsId,
        mcp_servers: mcpData?.mcpServers || mcpData?.servers || mcpData || undefined,
      }),
    });
    if (!runRes.ok) throw new Error(`Gateway /runs failed: ${runRes.status}`);
    const run = await runRes.json();
    const runId = String(run?.id || '');
    if (!runId) throw new Error('Gateway did not return a run id');

    const startedAt = Date.now();
    while (Date.now() - startedAt < 120_000) {
      await new Promise(r => setTimeout(r, 800));
      const statusRes = await fetch(`${gatewayUrl}/runs/${encodeURIComponent(runId)}`);
      if (!statusRes.ok) throw new Error(`Gateway poll failed: ${statusRes.status}`);
      const status = await statusRes.json();
      if (status.status === 'complete') return String(status.result?.response || '');
      if (status.status === 'error' || status.status === 'killed')
        throw new Error(status.error || `Run ${status.status}`);
    }
    throw new Error('Gateway run timed out after 120s');
  };

  const handleSendAthenaMessage = async (workspaceId: string, text: string) => {
    if (!workspaceId) return;
    const userText = text.trim();
    if (!userText) return;

    const userMessage: ChatMessage = {
      id: `user-${Date.now()}`,
      sender: 'user',
      text: userText,
      timestamp: new Date().toISOString(),
    };

    let currentMessages: ChatMessage[] = [];
    setAthenaChats(prev => {
      const chat = prev[workspaceId] || { messages: [], isLoading: false, error: '', input: '' };
      currentMessages = [...chat.messages, userMessage];
      void window.system?.saveAthenaThread?.({ threadKey: `athena:chat:${workspaceId}`, messages: currentMessages, input: '' });
      return {
        ...prev,
        [workspaceId]: { messages: currentMessages, isLoading: true, error: '', input: '' },
      };
    });

    try {
      const gatewayUrl = (gatewayDraft || 'http://127.0.0.1:3100').trim().replace(/\/+$/, '');
      const [graph, mcpData] = await Promise.all([
        activeSection === 'tasks' ? Promise.resolve({ nodes: [], edges: [] }) : fetchWorkspaceKnowledgeGraph(serverBaseUrl, apiKey, workspaceId),
        fetchGatewayMCPs(gatewayUrl),
      ]);
      const targetWorkspace = workspaceList.find(w => w.id === workspaceId);
      const targetSessions = sessionList.filter(s => s.workspaceId === workspaceId);
      const targetSessionsSet = new Set(targetSessions.map(s => s.id));
      const targetNotes = noteList.filter(note => targetSessionsSet.has(note.sessionId));
      const targetArtifacts = artifacts.filter(artifact => targetSessionsSet.has(artifact.sessionId));
      const targetFileGroups = workspaceSessionFiles;
      const targetConversations = sessionConversationMap;
      const targetTasks = activeSection === 'tasks' ? taskList : taskList.filter(t => t.workspaceId === workspaceId);
      const provider = selectedProvider;
      const model = selectedModel;

      const prompt = buildWorkspaceAthenaPrompt({
        workspace: targetWorkspace,
        workspaceId,
        graph,
        sessions: targetSessions,
        fileGroups: targetFileGroups,
        conversations: targetConversations,
        tasks: targetTasks,
        notes: targetNotes,
        mergeRequests: mergeRequestList,
        jiraTickets: jiraTicketList,
        artifacts: targetArtifacts,
        activitySummary: workspaceActivitySummary,
        messages: currentMessages,
        userText,
        mcpData,
        athenaType: activeSection === 'tasks' ? 'task_manager' : 'workspace',
      });

      const response = await runAgentViaGateway(prompt, workspaceId, mcpData, provider, model);

      const assistantMessage: ChatMessage = {
        id: `assistant-${Date.now()}`,
        sender: 'assistant',
        text: response || 'No response received from Athena.',
        timestamp: new Date().toISOString(),
      };

      setAthenaChats(prev => {
        const chat = prev[workspaceId] || { messages: [], isLoading: false, error: '', input: '' };
        const updatedMessages = [...chat.messages, assistantMessage];
        void window.system?.saveAthenaThread?.({ threadKey: `athena:chat:${workspaceId}`, messages: updatedMessages, input: chat.input });
        return {
          ...prev,
          [workspaceId]: { messages: updatedMessages, isLoading: false, error: '', input: chat.input },
        };
      });

      if (!isWorkspaceAthenaOpen || activeWorkspaceId !== workspaceId) {
        const workspaceName = targetWorkspace?.name || 'Workspace';
        pushToast('Athena Response', `Athena replied in "${workspaceName}": "${response.slice(0, 60)}..."`, 'good');
      }

    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unable to ask Athena.';
      setAthenaChats(prev => ({
        ...prev,
        [workspaceId]: { ...(prev[workspaceId] || { messages: [], input: '' }), isLoading: false, error: errorMessage },
      }));
      pushToast('Athena Error', errorMessage, 'warning');
    }
  };

  const handleDeleteAthenaMessage = (workspaceId: string, messageId: string) => {
    setAthenaChats(prev => {
      const chat = prev[workspaceId];
      if (!chat) return prev;
      const updatedMessages = chat.messages.filter(m => m.id !== messageId);
      const next = {
        ...prev,
        [workspaceId]: {
          ...chat,
          messages: updatedMessages,
        }
      };
      void window.system?.saveAthenaThread?.({ threadKey: `athena:chat:${workspaceId}`, messages: updatedMessages, input: chat.input });
      return next;
    });
  };

  const handleChangeAthenaInput = (workspaceId: string, text: string) => {
    setAthenaChats(prev => {
      const chat = prev[workspaceId] || { messages: [], isLoading: false, error: '', input: '' };
      const next = {
        ...prev,
        [workspaceId]: {
          ...chat,
          input: text,
        }
      };
      void window.system?.saveAthenaThread?.({ threadKey: `athena:chat:${workspaceId}`, messages: chat.messages, input: text });
      return next;
    });
  };

  const handleClearTaskAthenaChat = (taskId: string) => {
    setTaskAthenaChats(prev => ({
      ...prev,
      [taskId]: { messages: [], isLoading: false, error: '', input: '' },
    }));
    void window.system?.saveAthenaThread?.({ threadKey: `athena:task-chat:${taskId}`, messages: [], input: '' });
  };

  const handleSendTaskAthenaMessage = async (taskId: string, text: string) => {
    const activeTask = taskList.find(t => t.id === taskId);
    if (!activeTask) return;

    let currentMessages: ChatMessage[] = [];
    setTaskAthenaChats(prev => {
      const chat = prev[taskId] || { messages: [], isLoading: false, error: '', input: '' };
      const userMsg: ChatMessage = {
        id: `user-${Date.now()}`,
        sender: 'user',
        text,
        timestamp: new Date().toISOString(),
      };
      currentMessages = [...chat.messages, userMsg];
      void window.system?.saveAthenaThread?.({ threadKey: `athena:task-chat:${taskId}`, messages: currentMessages, input: '' });
      return {
        ...prev,
        [taskId]: { messages: currentMessages, isLoading: true, error: '', input: '' },
      };
    });

    try {
      const gatewayUrl = (gatewayDraft || 'http://127.0.0.1:3100').trim().replace(/\/+$/, '');
      const rawMcpData = await fetchGatewayMCPs(gatewayUrl);
      const mcpData = filterSavantMcpData(rawMcpData);
      const targetWorkspace = workspaceList.find(w => w.id === activeTask.workspaceId);
      const provider = selectedProvider;
      const model = selectedModel;
      const prompt = buildTaskAthenaPrompt({
        task: activeTask,
        workspace: targetWorkspace,
        messages: currentMessages,
        userText: text,
        mcpData,
      });

      const response = await runAgentViaGateway(prompt, activeTask.workspaceId, mcpData, provider, model);

      const assistantMessage: ChatMessage = {
        id: `assistant-${Date.now()}`,
        sender: 'assistant',
        text: response || 'No response received from Athena.',
        timestamp: new Date().toISOString(),
      };

      setTaskAthenaChats(prev => {
        const chat = prev[taskId] || { messages: [], isLoading: false, error: '', input: '' };
        const updatedMessages = [...chat.messages, assistantMessage];
        void window.system?.saveAthenaThread?.({ threadKey: `athena:task-chat:${taskId}`, messages: updatedMessages, input: chat.input });
        return {
          ...prev,
          [taskId]: { messages: updatedMessages, isLoading: false, error: '', input: chat.input },
        };
      });
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unable to ask Athena.';
      setTaskAthenaChats(prev => ({
        ...prev,
        [taskId]: { ...(prev[taskId] || { messages: [], input: '' }), isLoading: false, error: errorMessage },
      }));
      pushToast('Athena Error', errorMessage, 'warning');
    }
  };

  const handleDeleteTaskAthenaMessage = (taskId: string, messageId: string) => {
    setTaskAthenaChats(prev => {
      const chat = prev[taskId];
      if (!chat) return prev;
      const updatedMessages = chat.messages.filter(m => m.id !== messageId);
      const next = {
        ...prev,
        [taskId]: {
          ...chat,
          messages: updatedMessages,
        }
      };
      void window.system?.saveAthenaThread?.({ threadKey: `athena:task-chat:${taskId}`, messages: updatedMessages, input: chat.input });
      return next;
    });
  };

  const handleChangeTaskAthenaInput = (taskId: string, text: string) => {
    setTaskAthenaChats(prev => {
      const chat = prev[taskId] || { messages: [], isLoading: false, error: '', input: '' };
      const next = {
        ...prev,
        [taskId]: {
          ...chat,
          input: text,
        }
      };
      void window.system?.saveAthenaThread?.({ threadKey: `athena:task-chat:${taskId}`, messages: chat.messages, input: text });
      return next;
    });
  };

  const loadAuthSettings = async () => {
    const settings = window.system?.getSettings ? await window.system.getSettings() : {};
    const persistedThreads = window.system?.loadAthenaThreads ? await window.system.loadAthenaThreads() : {};
    const storedApiKey = window.localStorage.getItem('user:apiKey') ?? '';
    const activeKey = String(settings['user:apiKey'] ?? storedApiKey).trim();
    const cleanServerUrl = String(settings['server:config']?.url ?? 'http://127.0.0.1:8090').trim();

    setAuthDraft(activeKey);
    setServerDraft(cleanServerUrl);
    setGatewayDraft(String(settings['gateway:config']?.url ?? 'http://127.0.0.1:3100'));
    setAuthUser(String(settings['user:name'] ?? 'operator'));
    const groomingSettings = settings['colosseum:grooming-settings'];
    if (groomingSettings && typeof groomingSettings === 'object' && !Array.isArray(groomingSettings)) {
      setColosseumGroomingSettings({ ...DEFAULT_COLOSSEUM_GROOMING_SETTINGS, ...groomingSettings });
    }
    const readySettings = settings['colosseum:ready-settings'];
    if (readySettings && typeof readySettings === 'object' && !Array.isArray(readySettings)) {
      setColosseumReadySettings({ ...DEFAULT_COLOSSEUM_READY_SETTINGS, ...readySettings });
    }
    const reviewSettings = settings['colosseum:review-settings'];
    if (reviewSettings && typeof reviewSettings === 'object' && !Array.isArray(reviewSettings)) {
      setColosseumReviewSettings({ ...DEFAULT_COLOSSEUM_REVIEW_SETTINGS, ...reviewSettings });
    }

    if (activeKey) {
      try {
        const response = await fetch(`${cleanServerUrl}/api/auth/validate`, {
          headers: buildSavantHeaders(activeKey)
        });
        if (response.ok) {
          const data = await response.json();
          if (data.valid) {
            if (data.name) {
              setAuthUser(data.name);
              await saveSettingSafely('user:name', data.name);
            }
            if (data.user_id) {
              setCurrentUserId(data.user_id);
            }
          }
        }
      } catch (e) {
        console.error('Failed to sync username from server on startup:', e);
      }
    }

    const initialChats: Record<string, WorkspaceChatState> = {};
    for (const [threadKey, data] of Object.entries(persistedThreads)) {
      if (!threadKey.startsWith('athena:chat:')) continue;
      const wsId = threadKey.replace('athena:chat:', '');
      initialChats[wsId] = {
        messages: Array.isArray(data.messages) ? data.messages : [],
        isLoading: false,
        error: '',
        input: typeof data.input === 'string' ? data.input : '',
      };
    }
    setAthenaChats(initialChats);

    const initialTaskChats: Record<string, WorkspaceChatState> = {};
    for (const [threadKey, data] of Object.entries(persistedThreads)) {
      if (!threadKey.startsWith('athena:task-chat:')) continue;
      const tId = threadKey.replace('athena:task-chat:', '');
      initialTaskChats[tId] = {
        messages: Array.isArray(data.messages) ? data.messages : [],
        isLoading: false,
        error: '',
        input: typeof data.input === 'string' ? data.input : '',
      };
    }
    setTaskAthenaChats(initialTaskChats);

    const athenaEngine = settings['athena:engine'];
    if (athenaEngine && typeof athenaEngine === 'object' && !Array.isArray(athenaEngine)) {
      const engine = athenaEngine as AthenaEngineSettings;
      setSelectedProvider(String(engine.provider || 'gemini'));
      setSelectedModel(String(engine.model || '3.5'));
    } else {
      const providerChain = settings['provider:chain'];
      if (Array.isArray(providerChain) && providerChain[0]) {
        const nextProvider = providerChain[0].provider || 'gemini';
        const nextModel = providerChain[0].model || '3.5';
        setSelectedProvider(nextProvider);
        setSelectedModel(nextModel);
        void saveAthenaEngineSafely(nextProvider, nextModel);
      }
    }

    void refreshProviders(settings['gateway:config']?.url);

    setAuthReady(true);
  };

  useEffect(() => {
    loadAuthSettings().catch(() => setAuthReady(true));
  }, []);

  useEffect(() => {
    selectedProviderRef.current = selectedProvider;
  }, [selectedProvider]);

  useEffect(() => {
    selectedModelRef.current = selectedModel;
  }, [selectedModel]);

  useEffect(() => {
    if (settingsOpen) {
      void refreshProviders(gatewayDraft.trim() || 'http://127.0.0.1:3100');
    }
  }, [settingsOpen]);

  useEffect(() => {
    if (!authReady) return;

    let cancelled = false;

    const loadWorkspaces = async () => {
      setIsWorkspaceLoading(true);
      try {
        const response = await fetch(`${serverBaseUrl}/api/workspaces?include_kg=1`, {
          headers: buildSavantHeaders(apiKey),
        });

        if (!response.ok) return;
        const payload = await response.json();
        const serverWorkspaces = Array.isArray(payload)
          ? payload
          : Array.isArray((payload as { workspaces?: Workspace[] })?.workspaces)
            ? (payload as { workspaces: Workspace[] }).workspaces
            : [];
        if (!cancelled && serverWorkspaces.length > 0) {
          const merged = [...serverWorkspaces].reduce<Workspace[]>((acc, workspace) => {
            const normalized = normalizeServerWorkspace(workspace as any);
            if (!normalized) return acc;
            const id = normalized.id;
            const existingIndex = acc.findIndex((item) => item.id === id);
            if (existingIndex >= 0) {
              acc[existingIndex] = {
                ...acc[existingIndex],
                ...normalized,
                id,
              };
            } else {
              acc.push(normalized);
            }
            return acc;
          }, []);
          setWorkspaceList(merged);
          setWorkspaceSearch('');
          setWorkspaceIndex(0);
          setSessionIndex(0);

          void (async () => {
            const headers = buildSavantHeaders(apiKey);
            const tasksResponse = await fetch(`${serverBaseUrl}/api/tasks`, { headers }).catch(() => null);
            const allTasks = tasksResponse?.ok ? await tasksResponse.json().catch(() => []) : [];
            const tasksByWorkspace = new Map<string, number>();
            if (Array.isArray(allTasks)) {
              allTasks.forEach((task: any) => {
                const workspaceId = String(task.workspace_id ?? task.workspaceId ?? '');
                if (workspaceId) tasksByWorkspace.set(workspaceId, (tasksByWorkspace.get(workspaceId) ?? 0) + 1);
              });
            }

            const enriched = await Promise.all(merged.map(async (workspace) => {
              const raw = serverWorkspaces.find((item: any) => String(item.id ?? item.workspace_id ?? item.workspaceId) === workspace.id) as any;
              const sessionLinks = Array.isArray(raw?.sessions) ? raw.sessions : [];
              const [graphResponse, noteCounts] = await Promise.all([
                fetch(`${serverBaseUrl}/api/knowledge/graph?workspace_id=${encodeURIComponent(workspace.id)}&include_staged=true&slim=true&limit=5000`, { headers }).catch(() => null),
                Promise.all(sessionLinks.map(async (link: any) => {
                  const sessionId = String(link.session_id ?? link.id ?? '');
                  if (!sessionId) return 0;
                  const response = await fetch(`${serverBaseUrl}/api/session/${encodeURIComponent(sessionId)}/notes`, { headers }).catch(() => null);
                  if (!response?.ok) return 0;
                  const payload = await response.json().catch(() => []);
                  return Array.isArray(payload) ? payload.length : Array.isArray(payload?.notes) ? payload.notes.length : 0;
                })),
              ]);
              const graph = graphResponse?.ok ? await graphResponse.json().catch(() => ({})) : {};
              const nodeCount = Array.isArray(graph?.nodes) ? graph.nodes.length : 0;
              const edgeCount = Array.isArray(graph?.edges) ? graph.edges.length : 0;
              return {
                ...workspace,
                tasks: tasksByWorkspace.get(workspace.id) ?? 0,
                notes: noteCounts.reduce((sum, count) => sum + count, 0),
                knowledgeNodes: nodeCount,
                knowledgeEdges: edgeCount,
                nodeDensity: edgeCount / Math.max(1, nodeCount),
              };
            }));
            if (!cancelled) setWorkspaceList(enriched);
          })();
        } else if (!cancelled) {
          setWorkspaceList([]);
        }
      } catch {
        // Keep the seeded local workspace data if the server is unavailable.
      } finally {
        if (!cancelled) setIsWorkspaceLoading(false);
      }
    };

    loadWorkspaces();
    return () => {
      cancelled = true;
    };
  }, [apiKey, authReady, serverBaseUrl]);

  useEffect(() => {
    let cancelled = false;
    const headers = buildSavantHeaders(apiKey);

    const loadWorkspaceData = async () => {
      if (!activeWorkspaceId) return;
      try {
        const [tasksResponse, remindersResponse] = await Promise.all([
          fetch(`${serverBaseUrl}/api/tasks`, { headers }),
          fetch(`${serverBaseUrl}/api/reminders/due-today`, { headers }),
        ]);
        if (tasksResponse.ok) {
          const tasksData = await tasksResponse.json();
            setTaskList(tasksData.map((task: any) => {
              const idStr = String(task.task_id ?? task.id ?? task.title ?? '');
              let hash = 0;
              for (let i = 0; i < idStr.length; i++) {
                hash = idStr.charCodeAt(i) + ((hash << 5) - hash);
              }
              const defaultTime = Math.abs(hash % 16) + 2; // 2 to 17 hours
              const complexities = ['simple', 'moderate', 'complex', 'extreme'] as const;
              const defaultComplexity = complexities[Math.abs(hash % complexities.length)];

              return {
                id: task.task_id ?? task.id,
                workspaceId: task.workspace_id,
                title: task.title,
                description: task.description ?? '',
                priority: task.priority ?? 'medium',
                state: (() => { const s = task.status ?? 'backlog'; if (s === 'todo' || s === 'blocked') return 'backlog'; if (s === 'code-review') return 'review'; return s as Task['state']; })(),
                owner: task.user_id ?? 'server',
                due: task.date ?? undefined,
                dependsOn: task.depends_on ?? task.dependencies ?? [],
                comments: task.comments ?? [],
                createdAt: task.created_at ?? task.createdAt,
                updatedAt: task.updated_at ?? task.updatedAt,
                timeSpent: task.timeSpent ?? task.time_spent ?? defaultTime,
                complexity: task.complexity ?? defaultComplexity,
                colosseumConfig: task.colosseum_config ?? undefined,
              };
            }));
        }
        if (remindersResponse.ok) {
          const remindersData = await remindersResponse.json();
          if (!cancelled && Array.isArray(remindersData)) {
            setReminderList(remindersData.map((reminder: any) => ({
              id: reminder.reminder_id ?? reminder.id,
              workspaceId: reminder.workspace_id ?? activeWorkspaceId,
              title: reminder.title,
              due: reminder.due_date ?? '',
              state: reminder.status === 'done' ? 'done' : 'scheduled',
              dueLabel: (reminder.due_date ?? '').includes('T') ? 'Today' : undefined,
            })));
          }
        }
      } catch {
        // Keep empty live data if the server is unavailable.
      }
    };

    loadWorkspaceData();
    return () => {
      cancelled = true;
    };
  }, [activeWorkspaceId, apiKey, serverBaseUrl]);

  useEffect(() => {
    if (!activeWorkspaceId || workspaceSessions.length === 0) return;

    let cancelled = false;
    const headers = buildSavantHeaders(apiKey);

    const loadWorkspaceNotes = async () => {
      try {
        const notesBySession = await Promise.all(
          workspaceSessions.map(async (session) => {
            const response = await fetch(`${serverBaseUrl}/api/session/${encodeURIComponent(session.id)}/notes`, { headers });
            if (!response.ok) return [];
            const payload = await response.json() as { notes?: ServerNote[] } | ServerNote[];
            const serverNotes = Array.isArray(payload) ? payload : payload.notes ?? [];
            return serverNotes.map((note) => normalizeServerNote(note, session.id));
          }),
        );

        if (cancelled) return;

        const serverNotes = notesBySession.flat();
        setNoteList((current) => {
          const merged = [...serverNotes, ...current];
          const seen = new Set<string>();
          return merged.filter((note) => {
            if (seen.has(note.id)) return false;
            seen.add(note.id);
            return true;
          });
        });
      } catch {
        // Keep the local cache if notes cannot be fetched.
      }
    };

    void loadWorkspaceNotes();
    const interval = window.setInterval(() => {
      void loadWorkspaceNotes();
    }, 15000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [activeWorkspaceId, apiKey, serverBaseUrl, workspaceSessionIdsKey]);

  useEffect(() => {
    if (!activeWorkspaceId) return;

    let cancelled = false;
    const headers = buildSavantHeaders(apiKey);

    const loadWorkspaceArtifacts = async () => {
      try {
        const [mrsResponse, jiraResponse] = await Promise.all([
          fetch(`${serverBaseUrl}/api/merge-requests?workspace_id=${encodeURIComponent(activeWorkspaceId)}`, { headers }),
          fetch(`${serverBaseUrl}/api/jira-tickets?workspace_id=${encodeURIComponent(activeWorkspaceId)}`, { headers }),
        ]);

        if (cancelled) return;

        if (mrsResponse.ok) {
          const mrs = (await mrsResponse.json()) as ServerMergeRequest[];
          setMergeRequestList(mrs.map(normalizeServerMergeRequest).filter((item): item is WorkspaceMergeRequest => Boolean(item)));
        }

        if (jiraResponse.ok) {
          const tickets = (await jiraResponse.json()) as ServerJiraTicket[];
          setJiraTicketList(tickets.map(normalizeServerJiraTicket).filter((item): item is WorkspaceJiraTicket => Boolean(item)));
        }
      } catch {
        // Keep the cached server lists if the server is unavailable.
      }
    };

    void loadWorkspaceArtifacts();
    const interval = window.setInterval(() => {
      void loadWorkspaceArtifacts();
    }, 30000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [activeWorkspaceId, apiKey, serverBaseUrl]);

  useEffect(() => {
    let cancelled = false;
    const headers = buildSavantHeaders(apiKey);

    const readJson = async (path: string) => {
      const response = await fetch(`${serverBaseUrl}${path}`, { headers });
      if (!response.ok) return null;
      return response.json();
    };

    const loadWorkspaceSessions = async () => {
      if (!activeWorkspaceId) return;

      try {
        const [linksPayload, filesPayload] = await Promise.all([
          readJson(`/api/workspaces/${encodeURIComponent(activeWorkspaceId)}/session-links`),
          readJson(`/api/workspaces/${encodeURIComponent(activeWorkspaceId)}/session-files`),
        ]);
        if (cancelled) return;

        const groups = (Array.isArray(filesPayload?.groups) ? filesPayload.groups : Array.isArray(filesPayload?.files) ? filesPayload.files : []) as SessionFileGroup[];
        const groupsById = new Map(groups.map((group) => [String(group.session_id ?? group.id ?? ''), group]));
        const links = Array.isArray(linksPayload?.links) ? linksPayload.links as ServerSession[] : [];
        const linkedSessions = links
          .map((link) => getSessionAdapter(String(link.provider ?? 'savant')).normalizeSession(link, activeWorkspaceId, groupsById.get(String(link.session_id ?? link.id ?? ''))))
          .filter((session): session is Session => Boolean(session));
        const groupOnlySessions = groups
          .filter((group) => !linkedSessions.some((session) => session.id === String(group.session_id ?? group.id ?? '')))
          .map((group) => getSessionAdapter(String(group.provider ?? 'savant')).normalizeSession(group, activeWorkspaceId, group))
          .filter((session): session is Session => Boolean(session));

        const discoveredMetadata = await Promise.all([...linkedSessions, ...groupOnlySessions].map(async (session) => {
          try {
            const metadata = await window.system?.getLocalSessionMetadata?.(session.id, session.provider);
            return [session.id, metadata] as const;
          } catch {
            return [session.id, {}] as const;
          }
        }));
        const metadataBySession = new Map(discoveredMetadata);
        const nextSessions = [...linkedSessions, ...groupOnlySessions].map((session) => {
          const metadata = metadataBySession.get(session.id);
          if (!metadata) return session;
          const provider = metadata.provider ? getSessionAdapter(metadata.provider).displayName : session.provider;
          return {
            ...session,
            title: metadata.title || session.title,
            provider,
            agentType: metadata.agentType || session.agentType,
            files: metadata.files?.length || session.files,
            linked: metadata.files?.length || session.linked,
          };
        });
        setSessionList((current) => [
          ...current.filter((session) => session.workspaceId !== activeWorkspaceId),
          ...nextSessions,
        ]);
        setWorkspaceSessionFiles((current) => ({
          ...current,
          ...Object.fromEntries(groups.map((group) => [String(group.session_id ?? group.id ?? ''), group])),
          ...Object.fromEntries(discoveredMetadata.filter(([, metadata]) => metadata.files?.length).map(([sessionId, metadata]) => [sessionId, {
            session_id: sessionId,
            provider: metadata.provider,
            file_count: metadata.files?.length,
            files: metadata.files,
          } satisfies SessionFileGroup])),
        }));
        setSessionIndex(0);
      } catch {
        setSessionList((current) => current.filter((session) => session.workspaceId !== activeWorkspaceId));
      }
    };

    loadWorkspaceSessions();
    return () => {
      cancelled = true;
    };
  }, [activeWorkspaceId, apiKey, serverBaseUrl]);

  useEffect(() => {
    let cancelled = false;
    const headers = buildSavantHeaders(apiKey);
    const inferredProvider = inferSessionProvider(activeSession.provider, activeSession as unknown as Record<string, any>, activeSessionFileGroup);
    const adapter = getSessionAdapter(inferredProvider);

    const loadSessionConversation = async () => {
      if (!activeSession.id) return;

      try {
        const localMessages = await window.system?.getLocalConversation?.(inferredProvider, activeSession.id);
        if (cancelled) return;
        if (Array.isArray(localMessages) && localMessages.length > 0) {
          setSessionConversationMap((current) => ({
            ...current,
            [activeSession.id]: localMessages,
          }));
          return;
        }
      } catch {
        // Fall through to provider-specific loading.
      }

      try {
        const conversationPath = adapter.conversationPath(activeSession.id);
        if (conversationPath) {
          const response = await fetch(resolveApiPath(conversationPath), { headers });
          if (response.ok) {
            const payload = await response.json();
            if (cancelled) return;
            const messages = adapter.normalizeConversation(payload, activeSession.id);
            setSessionConversationMap((current) => ({
              ...current,
              [activeSession.id]: messages.length ? messages : sessionConversationFallback,
            }));
            return;
          }
        }
      } catch {
        // Fall through to raw transcript loading.
      }

      const transcriptPath = adapter.transcriptPath(activeSession as unknown as Record<string, any>, activeSessionFileGroup);
      if (!transcriptPath) {
        if (!cancelled) {
          setSessionConversationMap((current) => ({ ...current, [activeSession.id]: sessionConversationFallback }));
        }
        return;
      }

      try {
        const fileResponse = await fetch(
          resolveApiPath(`/api/${encodeURIComponent(inferredProvider)}/session/${encodeURIComponent(activeSession.id)}/file?path=${encodeURIComponent(transcriptPath)}`),
          { headers },
        );
        if (!fileResponse.ok) {
          if (!cancelled) {
            setSessionConversationMap((current) => ({ ...current, [activeSession.id]: sessionConversationFallback }));
          }
          return;
        }
        const filePayload = await fileResponse.json();
        const messages = adapter.normalizeConversationText(String(filePayload?.content ?? ''), activeSession.id);
        if (!cancelled) {
          setSessionConversationMap((current) => ({
            ...current,
            [activeSession.id]: messages.length ? messages : sessionConversationFallback,
          }));
        }
      } catch {
        if (!cancelled) {
          setSessionConversationMap((current) => ({ ...current, [activeSession.id]: sessionConversationFallback }));
        }
      }
    };

    loadSessionConversation();
    return () => {
      cancelled = true;
    };
  }, [activeSession.id, activeSession.provider, activeSessionFileGroup, apiKey, serverBaseUrl, sessionConversationFallback]);

  useEffect(() => {
    let cancelled = false;

    const prewarmWorkspaceConversations = async () => {
      await Promise.all(workspaceSessions.map(async (session) => {
        if (sessionConversationMap[session.id]?.length) return;
        const fileGroup = workspaceSessionFiles[session.id];
        const provider = inferSessionProvider(session.provider, session as unknown as Record<string, any>, fileGroup);
        try {
          const messages = await window.system?.getLocalConversation?.(provider, session.id);
          if (!cancelled && Array.isArray(messages) && messages.length) {
            setSessionConversationMap((current) => ({ ...current, [session.id]: messages }));
          }
        } catch {
          // The selected-session loader retains the server and transcript fallbacks.
        }
      }));
    };

    void prewarmWorkspaceConversations();
    return () => { cancelled = true; };
  }, [workspaceSessionIdsKey, workspaceSessionFiles, workspaceSessions]);

  useEffect(() => {
    let cancelled = false;

    const loadWorkspaceGraphStats = async () => {
      try {
        if (!activeWorkspaceId) return;
        const response = await fetch(`${serverBaseUrl}/api/knowledge/graph?workspace_id=${encodeURIComponent(activeWorkspaceId)}&include_staged=true&slim=false&limit=5000`, {
          headers: buildSavantHeaders(apiKey),
        });
        if (!response.ok) return;
        const data = await response.json() as {
          nodes?: Array<{ status?: string; node_type?: string }>;
          edges?: Array<{ status?: string }>;
        };
        const nodes = data.nodes ?? [];
        const edges = data.edges ?? [];
        const stagedNodes = nodes.filter((node) => node.status === 'staged').length;
        const committedNodes = nodes.length - stagedNodes;
        const stagedEdges = edges.filter((edge) => edge.status === 'staged').length;
        const committedEdges = edges.length - stagedEdges;
        const nodesByType: Record<string, number> = {};
        nodes.forEach((node) => {
          const key = node.node_type ?? 'unknown';
          nodesByType[key] = (nodesByType[key] ?? 0) + 1;
        });
        if (!cancelled) {
          setWorkspaceGraphStats((current) => ({
            ...current,
            [activeWorkspaceId]: {
              total_nodes: nodes.length,
              total_edges: edges.length,
              committed_nodes: committedNodes,
              staged_nodes: stagedNodes,
              committed_edges: committedEdges,
              staged_edges: stagedEdges,
              nodes_by_type: nodesByType,
            },
          }));
        }
      } catch {
        // Keep the cached workspace stats if the server is unavailable.
      }
    };

    loadWorkspaceGraphStats();
    return () => {
      cancelled = true;
    };
  }, [activeWorkspaceId, apiKey, serverBaseUrl, workspaceList]);
  useEffect(() => {
    if (workspaceIndex >= workspaceList.length) {
      setWorkspaceIndex(Math.max(0, workspaceList.length - 1));
    }
  }, [workspaceIndex, workspaceList.length]);

  useEffect(() => {
    if (sessionIndex >= workspaceSessions.length) {
      setSessionIndex(Math.max(0, workspaceSessions.length - 1));
    }
  }, [sessionIndex, workspaceSessions.length]);

  useEffect(() => {
    if (!isEditOpen) return;
    if (workspaceEditorMode === 'edit') {
      setEditDraft(createWorkspaceEditorDraftFromWorkspace(activeWorkspace, displayedWorkspaceName));
      return;
    }
    setEditDraft(createWorkspaceEditorDraft());
  }, [activeWorkspace?.color, activeWorkspace?.description, activeWorkspace?.priority, activeWorkspace?.status, displayedWorkspaceName, isEditOpen, workspaceEditorMode]);
  const sectionCounts = useMemo(
    () => ({
      workspace: workspaceList.length,
      session: sessionList.length,
      tasks: taskList.length,
      reminders: reminderList.length,
      notes: noteList.length,
      files: sessionArtifacts.filter((artifact) => artifact.kind === 'file').length,
      jira: sessionArtifacts.filter((artifact) => artifact.kind === 'jira').length,
      'merge-requests': sessionArtifacts.filter((artifact) => artifact.kind === 'merge-request').length,
      knowledge: 2,
      providers: providers.length,
      settings: localSetup.length,
    }),
    [localSetup.length, noteList.length, providers.length, reminderList.length, sessionArtifacts, sessionList.length, taskList.length, workspaceList.length, workspaceSessions.length, workspaceTasks.length],
  );
  const surfaceTitle = useMemo(() => {
    switch (surfaceMode) {
      case 'sessions':
        return 'Session timeline';
      case 'artifacts':
        return 'Attached artifacts';
      case 'setup':
        return 'Setup and system surfaces';
      default:
        return 'Workspace overview';
    }
  }, [surfaceMode]);
  const activeModeLabel = useMemo(
    () =>
      ({
        overview: 'Overview',
        sessions: 'Sessions',
        artifacts: 'Artifacts',
        setup: 'Setup',
      })[surfaceMode],
    [surfaceMode],
  );
  const sessionStats = useMemo(
    () => [
      { label: 'Notes', value: sessionNotes.length },
      { label: 'Files', value: sessionArtifacts.filter((artifact) => artifact.kind === 'file').length },
      { label: 'Jira', value: sessionArtifacts.filter((artifact) => artifact.kind === 'jira').length },
      { label: 'Merge requests', value: sessionArtifacts.filter((artifact) => artifact.kind === 'merge-request').length },
      { label: 'Tool calls', value: sessionConversation.filter((message) => message.kind === 'tool').length },
    ],
    [sessionArtifacts, sessionConversation, sessionNotes.length],
  );
  const sessionCheckpoints = useMemo(
    () => [
      `${activeSession.tree}`,
      `${sessionTimeline.filter((event) => event.kind === 'sync').length} sync events`,
      `${sessionTimeline.filter((event) => event.kind === 'task').length} task updates`,
      `${sessionArtifacts.length} linked artifacts`,
    ],
    [activeSession.tree, sessionArtifacts.length, sessionTimeline],
  );
  const activeSessionFlags = sessionFlags[activeSession.id] ?? {};
  const activeTaskFlags = taskFlags[workspaceTasks[0]?.id ?? ''] ?? {};
  const activeWorkspaceStats = activeWorkspace ? workspaceGraphStats[activeWorkspace.id] : undefined;
  const workspaceSummary = useMemo(
    () => ({
      sessions: workspaceSessions.length,
      tasks: workspaceTasks.length,
      reminders: workspaceReminders.length,
      knowledgeNodes: activeWorkspaceStats?.total_nodes ?? activeWorkspace?.knowledgeNodes ?? 0,
      knowledgeEdges: activeWorkspaceStats?.total_edges ?? activeWorkspace?.knowledgeEdges ?? 0,
      knowledgeCommittedNodes: activeWorkspaceStats?.committed_nodes ?? Math.max(0, (activeWorkspace?.knowledgeNodes ?? 0) - (activeWorkspace?.kg_stats?.staged_count ?? 0)),
      knowledgeStagedNodes: activeWorkspaceStats?.staged_nodes ?? activeWorkspace?.kg_stats?.staged_count ?? 0,
      knowledgeCommittedEdges: activeWorkspaceStats?.committed_edges ?? activeWorkspace?.knowledgeEdges ?? 0,
      knowledgeStagedEdges: activeWorkspaceStats?.staged_edges ?? 0,
    }),
    [activeWorkspace, activeWorkspaceStats, workspaceReminders.length, workspaceSessions.length, workspaceTasks.length],
  );
  const manageSummary = useMemo(
    () => ({
      todayTasks: taskList.filter((task) => task.due === todayIso || task.due === 'Today').length,
      todayReminders: todayReminders.length,
      openTasks: taskList.filter((task) => task.state !== 'done').length,
      pendingReminders: workspaceReminders.filter((reminder) => reminder.state !== 'done').length,
    }),
    [taskList, todayReminders.length, todayIso, workspaceReminders],
  );
  const workspaceActivitySummary = useMemo(
    () => ({
      total: workspaceSessions.length + workspaceTasks.length + workspaceNotes.length + workspaceArtifacts.length + workspaceReminders.length,
      detail: `${workspaceSessions.length} sessions · ${workspaceTasks.length} tasks · ${workspaceNotes.length} notes`,
      latest: workspaceSessions[0]?.title ?? displayedWorkspaceName,
    }),
    [displayedWorkspaceName, workspaceArtifacts.length, workspaceNotes.length, workspaceReminders.length, workspaceSessions, workspaceTasks.length],
  );
  const hero = useMemo(() => {
    if (activeSection === 'session') {
      return {
        eyebrow: activeProfile.eyebrow,
        title: displayedSessionTitle,
        subtitle: `${displayedProvider} · updated ${activeSession.updated}`,
        blurb: activeProfile.blurb,
      };
    }

    if (activeSection === 'tasks') {
      return {
        eyebrow: activeProfile.eyebrow,
        title: 'Task manager',
        subtitle: `${taskList.length} tasks across ${workspaceList.length} workspaces`,
        blurb: activeProfile.blurb,
      };
    }

    if (activeSection === 'reminders') {
      return {
        eyebrow: activeProfile.eyebrow,
        title: activeProfile.title,
        subtitle: `${workspaceReminders.length} reminders in scope`,
        blurb: activeProfile.blurb,
      };
    }

    if (activeSection === 'manage') {
      return {
        eyebrow: activeProfile.eyebrow,
        title: `${displayedWorkspaceName} workboard`,
        subtitle: `${todayTasks.length} tasks today · ${todayReminders.length} reminders today`,
        blurb: activeProfile.blurb,
      };
    }

    if (activeSection === 'providers' || activeSection === 'settings') {
      return {
        eyebrow: activeProfile.eyebrow,
        title: activeProfile.title,
        subtitle: activeProfile.subtitle,
        blurb: activeProfile.blurb,
      };
    }

    return {
      eyebrow: activeProfile.eyebrow,
      title: displayedWorkspaceName,
      subtitle: [
        `${activeWorkspace?.counts?.total ?? workspaceSummary.sessions} sessions`,
        `${workspaceSummary.tasks} tasks`,
        `${workspaceNotes.length} notes`,
        `${workspaceReminders.length} reminders`,
        `${mergeRequestList.length} merge requests`,
        `${jiraTicketList.length} jira tickets`,
        `${workspaceSummary.knowledgeNodes} nodes`,
        `${workspaceSummary.knowledgeEdges} edges`,
      ].join(' · '),
      blurb: activeProfile.blurb,
    };
  }, [activeSection, activeProfile, activeSession.updated, activeWorkspace, displayedProvider, displayedSessionTitle, displayedWorkspaceName, jiraTicketList.length, mergeRequestList.length, workspaceNotes.length, workspaceReminders.length, workspaceSummary.sessions, workspaceSummary.tasks, workspaceSummary.knowledgeNodes, workspaceSummary.knowledgeEdges, workspaceTasks]);
  const systemStatus = useMemo(
    () => [
      ['Server', 'Online'],
      ['MCP', 'Ready'],
      ['Sync queue', `${activityCount} pending`],
      ['Notifications', `${toasts.length} live`],
    ],
    [activityCount, toasts.length],
  );
  const terminalCommands = useMemo(
    () => [
      { command: 'npm run dev', detail: 'Open the local app shell.' },
      { command: 'npm run build', detail: 'Package the current workspace.' },
      { command: 'sanctum mcp validate', detail: 'Re-check MCP config and reachability.' },
      { command: 'sanctum skills install', detail: 'Install local skills and tools.' },
    ],
    [],
  );
  const heroFacts = useMemo(() => {
    if (activeSection === 'session') {
      return [
        `Files ${activeSession.files}`,
        `Notes ${activeSession.notes}`,
        `Jira ${activeSession.jira}`,
        `MRs ${activeSession.mergeRequests}`,
      ];
    }

    if (activeSection === 'tasks') {
      return [
        `Backlog ${taskList.filter((task) => task.state === 'backlog').length}`,
        `Ready ${taskList.filter((task) => task.state === 'ready').length}`,
        `In progress ${taskList.filter((task) => task.state === 'in-progress').length}`,
        `Review ${taskList.filter((task) => task.state === 'review').length}`,
        `Done ${taskList.filter((task) => task.state === 'done').length}`,
      ];
    }

    if (activeSection === 'reminders') {
      return workspaceReminders.map((reminder) => reminder.due);
    }

    if (activeSection === 'manage') {
      return [
        `Today tasks ${todayTasks.length}`,
        `Today reminders ${todayReminders.length}`,
        `Open tasks ${taskList.filter((task) => task.state !== 'done').length}`,
        `Scheduled reminders ${workspaceReminders.filter((reminder) => reminder.state !== 'done').length}`,
      ];
    }

    if (activeSection === 'providers' || activeSection === 'settings') {
      return [
        `Providers ${providers.length}`,
        `Local setup ${localSetup.length}`,
        `MCP ready`,
        `Install ready`,
      ];
    }

    return [
      activeWorkspace ? `Workspace ${activeWorkspace.name}` : 'No workspace selected',
      `Sessions ${workspaceSummary.sessions}`,
      `Tasks ${workspaceSummary.tasks}`,
      `Notes ${workspaceNotes.length}`,
      `Status ${activeWorkspace?.status ?? 'unknown'}`,
    ];
  }, [activeSection, activeSession, activeWorkspace, localSetup.length, providers.length, taskList, todayReminders.length, todayTasks.length, workspaceNotes.length, workspaceReminders, workspaceSessions.length, workspaceSummary.sessions, workspaceSummary.tasks]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const isInput =
        document.activeElement?.tagName === 'INPUT' ||
        document.activeElement?.tagName === 'TEXTAREA' ||
        (document.activeElement as HTMLElement)?.isContentEditable;

      if (isInput) return;

      if (event.key === 'Escape') {
        setIsEditOpen(false);
        setIsHelpOpen(false);
        return;
      }

      if (event.key === '?' || (event.shiftKey && event.key === '/')) {
        event.preventDefault();
        setIsHelpOpen((value) => !value);
        return;
      }

      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const key = event.key.toLowerCase();
      const shortcutMap: Record<string, SectionId> = {
        w: 'workspace',
        s: 'session',
        t: 'tasks',
        r: 'reminders',
        n: 'notes',
        f: 'files',
        j: 'jira',
        m: 'merge-requests',
        k: 'knowledge',
        p: 'providers',
        g: 'settings',
      };

      const nextSection = shortcutMap[key];
      if (!nextSection) return;

      event.preventDefault();
      setActiveSection(nextSection);
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  useEffect(() => {
    const nextState: SavedUiState = {
      activeSection,
      surfaceMode,
      workspaceIndex,
      sessionIndex,
      sessionList,
      noteList,
      workspaceNameOverride,
      sessionTitleOverride,
      providerOverride,
      reminderOverride,
      sessionFlags,
      taskList,
      taskFlags,
      reminderFlags,
      reminderList,
      contractValidation,
    };

    setSavedState(nextState);
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(nextState));
    } catch {
      // Ignore storage errors in constrained environments.
    }
  }, [activeSection, noteList, providerOverride, reminderFlags, reminderList, reminderOverride, sessionFlags, sessionIndex, sessionList, surfaceMode, taskFlags, taskList, workspaceIndex, workspaceNameOverride, sessionTitleOverride]);

  useEffect(() => {
    const label = `${sectionLabel} · ${displayedWorkspaceName}`;
    const detail = `${activeModeLabel} · ${displayedSessionTitle}`;
    setActivityFeed((current) => [
      {
        id: `${activeSection}-${surfaceMode}-${workspaceIndex}-${sessionIndex}`,
        label,
        detail,
        section: activeSection,
        surfaceMode,
        workspaceIndex,
        sessionIndex,
      },
      ...current.filter((item) => item.id !== `${activeSection}-${surfaceMode}-${workspaceIndex}-${sessionIndex}`),
    ].slice(0, 4));
  }, [activeSection, activeModeLabel, displayedSessionTitle, displayedWorkspaceName, sessionIndex, surfaceMode, workspaceIndex, sectionLabel]);

  const restoreActivityContext = (item: ActivityItem) => {
    setActiveSection(item.section);
    setSurfaceMode(item.surfaceMode);
    setWorkspaceIndex(item.workspaceIndex);
    setSessionIndex(item.sessionIndex);
    setIsDrawerOpen(true);
  };

  const pushToast = (title: string, detail: string, tone: Toast['tone'] = 'muted') => {
    const id = `${title}-${detail}-${Date.now()}`;
    const createdAt = Date.now();
    setToasts((current) => [{ id, title, detail, tone, createdAt }, ...current].slice(0, 3));
    window.setTimeout(() => {
      setToasts((current) => current.filter((toast) => toast.id !== id));
    }, 3200);
  };

  const handleDeleteSession = async (sessionId: string) => {
    const session = sessionList.find(s => s.id === sessionId);
    if (!session) return;
    
    const confirmDelete = window.confirm(`Are you sure you want to remove session "${session.title}" from this workspace?`);
    if (!confirmDelete) return;

    try {
      const serverUrl = serverDraft.trim() || 'http://127.0.0.1:8090';
      const cleanServerUrl = serverUrl.replace(/\/+$/, "");
      const provider = session.provider || 'savant';
      
      const res = await fetch(`${cleanServerUrl}/api/workspaces/${encodeURIComponent(activeWorkspaceId)}/session-links/${encodeURIComponent(provider)}/${encodeURIComponent(sessionId)}`, {
        method: 'DELETE',
        headers: buildSavantHeaders(apiKey)
      });
      
      if (res.ok) {
        setSessionList((current) => current.filter((s) => s.id !== sessionId));
        setSessionIndex(0);
        pushToast('Session deleted', `${session.title} removed from the workspace.`, 'warning');
      } else {
        const errText = await res.text();
        console.error('Failed to delete session link:', res.status, errText);
        pushToast('Delete failed', `Failed to delete session link: ${res.statusText}`, 'warning');
      }
    } catch (e) {
      console.error('Failed to delete session link:', e);
      pushToast('Delete failed', 'Could not connect to Savant server.', 'warning');
    }
  };

  const handleLogout = async () => {
    await handleAuthLogout();
    const resetState: SavedUiState = {
      activeSection: 'workspace',
      surfaceMode: 'overview',
      workspaceIndex: 0,
      sessionIndex: 0,
      workspaceList: [],
      sessionList: [],
      noteList: [],
      taskList: [],
      reminderList: [],
    };
    setActiveSection('workspace');
    setSurfaceMode('overview');
    setWorkspaceIndex(0);
    setSessionIndex(0);
    setWorkspaceList([]);
    setSessionList([]);
    setNoteList([]);
    setTaskList([]);
    setReminderList([]);
    setIsDrawerOpen(true);
    setSavedState(resetState);
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(resetState));
    } catch {
      // Ignore storage errors in constrained environments.
    }
  };

  const toggleWorkspacePane = () => {
    setActiveSection('workspace');
    setIsDrawerOpen(true);
    setIsWorkspacePaneOpen((current) => !current);
  };

  const saveEditDraft = () => {
    const payload = buildWorkspaceEditorPayload(editDraft);
    if (workspaceEditorMode === 'create') {
      void (async () => {
        try {
          const response = await fetch(`${serverBaseUrl}/api/workspaces`, {
            method: 'POST',
            headers: buildSavantHeaders(apiKey, true),
            body: JSON.stringify(payload),
          });
          if (response.ok) {
            const createdWorkspace = await response.json();
            setWorkspaceList((current) => [createdWorkspace, ...current]);
            setWorkspaceIndex(0);
          }
        } catch {
          // Keep the local create flow responsive even if the server write fails.
        }
      })();
    } else if (activeWorkspace) {
      void (async () => {
        try {
          const response = await fetch(`${serverBaseUrl}/api/workspaces/${encodeURIComponent(activeWorkspace.id)}`, {
            method: 'PUT',
            headers: buildSavantHeaders(apiKey, true),
            body: JSON.stringify(payload),
          });
          if (response.ok) {
            const updatedWorkspace = await response.json();
            setWorkspaceList((current) => current.map((workspace) => (workspace.id === activeWorkspace.id ? { ...workspace, ...updatedWorkspace } : workspace)));
          }
        } catch {
          // Keep the local label update even if the server write fails.
        }
      })();
      setWorkspaceNameOverride((current) => ({ ...current, [activeWorkspace.id]: editDraft.workspaceName }));
    }
    setIsEditOpen(false);
  };

  const openWorkspaceEditor = (mode: WorkspaceEditorMode) => {
    setWorkspaceEditorMode(mode);
    if (mode === 'create') {
      setEditDraft(createWorkspaceEditorDraft());
    }
    setIsEditOpen(true);
  };

  const setSessionProvider = (providerName: string) => {
    setProviderOverride((current) => ({ ...current, [activeSession.id]: providerName }));
  };

  const createSessionNote = async (text: string) => {
    if (!activeSession?.id) return;

    try {
      const response = await fetch(`${serverBaseUrl}/api/session/${encodeURIComponent(activeSession.id)}/notes`, {
        method: 'POST',
        headers: buildSavantHeaders(apiKey, true),
        body: JSON.stringify({ text }),
      });

      if (!response.ok) {
        pushToast('Note failed', 'Could not save the note on the server.', 'warning');
        return;
      }

      const payload = (await response.json()) as ServerNote;
      const nextNote = normalizeServerNote(payload, activeSession.id);
      setNoteList((current) => {
        const merged = [nextNote, ...current];
        const seen = new Set<string>();
        return merged.filter((note) => {
          if (seen.has(note.id)) return false;
          seen.add(note.id);
          return true;
        });
      });
      pushToast('Note created', 'Saved to the server and synced into the workspace.', 'good');
    } catch {
      pushToast('Note failed', 'Could not save the note on the server.', 'warning');
    }
  };

  const closeWorkspaceDrawers = () => {
    setIsWorkspaceAthenaOpen(false);
    setIsKnowledgeOpen(false);
    setIsSessionsDrawerOpen(false);
    setIsMergeRequestsDrawerOpen(false);
    setIsJiraDrawerOpen(false);
    setIsArtifactsDrawerOpen(false);
    setIsTaskDrawerOpen(false);
    setIsNotesDrawerOpen(false);
    setIsActivityOpen(false);
  };

  const openSessionsDrawer = () => {
    closeWorkspaceDrawers();
    setIsSessionsDrawerOpen(true);
  };

  const sectionDrawerItems = useMemo(() => {
    switch (activeSection) {
      case 'manage':
        return [
          { label: 'Open task queue', action: () => setActiveSection('tasks') },
          { label: 'Open reminder schedule', action: () => setActiveSection('reminders') },
          { label: 'Mark next task done', action: () => {
            const task = todayTasks[0] ?? workspaceTasks[0];
            if (!task) return;
            setTaskFlags((current) => ({ ...current, [task.id]: { done: true } }));
            pushToast('Task updated', `${task.title} marked done.`, 'good');
          } },
          { label: 'Dismiss next reminder', action: () => {
            const reminder = todayReminders[0] ?? workspaceReminders[0];
            if (!reminder) return;
            setReminderFlags((current) => ({ ...current, [reminder.id]: { done: true } }));
            pushToast('Reminder updated', `${reminder.title} dismissed.`, 'good');
          } },
        ];
      case 'session':
        return [
          { label: 'Open timeline event', action: () => setActiveSection('session') },
          { label: 'Jump to session notes', action: () => setActiveSection('notes') },
          { label: 'Inspect linked files', action: () => setActiveSection('files') },
          { label: 'Sync session metadata', action: () => setSurfaceMode('setup') },
          {
            label: activeSessionFlags.starred ? 'Unstar session' : 'Star session',
            action: () => {
              setSessionFlags((current) => ({
                ...current,
                [activeSession.id]: { ...(current[activeSession.id] ?? {}), starred: !current[activeSession.id]?.starred },
              }));
            },
          },
          {
            label: activeSessionFlags.archived ? 'Unarchive session' : 'Archive session',
            action: () => {
              setSessionFlags((current) => ({
                ...current,
                [activeSession.id]: { ...(current[activeSession.id] ?? {}), archived: !current[activeSession.id]?.archived },
              }));
            },
          },
        ];
      case 'notes':
        return [
          {
            label: '+',
            action: () => {
              void createSessionNote('Session note captured from the Sanctum shell.');
            },
          },
          { label: 'Aggregate notes', action: () => setActiveSection('workspace') },
          { label: 'Pin note', action: () => setActiveSection('session') },
          { label: 'Export note set', action: () => setActiveSection('knowledge') },
        ];
      case 'files':
        return [
          { label: 'Open folder', action: () => setActiveSection('files') },
          { label: 'Preview file', action: () => setActiveSection('session') },
          { label: 'Copy path', action: () => setActiveSection('settings') },
          { label: 'Reveal attachments', action: () => setSurfaceMode('artifacts') },
        ];
      case 'jira':
        return [
          { label: 'Sync issues', action: () => setActiveSection('jira') },
          { label: 'Open in Jira', action: () => setSurfaceMode('setup') },
          { label: 'Link issue', action: () => setActiveSection('session') },
          { label: 'Refresh backlog', action: () => setActiveSection('tasks') },
        ];
      case 'merge-requests':
        return [
          { label: 'Open MR', action: () => setActiveSection('merge-requests') },
          { label: 'Review changes', action: () => setActiveSection('session') },
          { label: 'Approve', action: () => setSurfaceMode('artifacts') },
          { label: 'Request changes', action: () => setActiveSection('tasks') },
        ];
      case 'tasks':
        return [
          { label: 'Prioritize queue', action: () => setActiveSection('tasks') },
          { label: 'Assign owner', action: () => setActiveSection('settings') },
          { label: 'Mark in progress', action: () => setSessionIndex(0) },
          { label: 'Send to review', action: () => setActiveSection('merge-requests') },
          {
            label: '+',
            action: () => {
              const task = {
                id: `task-${Date.now().toString(36)}`,
                workspaceId: activeWorkspaceId,
                title: 'New task',
                priority: 'medium' as const,
                state: 'backlog' as const,
                owner: 'ahmed',
                timeSpent: 4,
                complexity: 'moderate' as const,
              };
              setTaskList((current) => [task, ...current]);
              pushToast('Task created', 'New task added to the active workspace.', 'good');
            },
          },
          {
            label: 'Complete top task',
            action: () => {
              const task = workspaceTasks[0];
              if (!task) return;
              setTaskList((current) => current.map((item) => (item.id === task.id ? { ...item, state: 'done' } : item)));
              setTaskFlags((current) => ({ ...current, [task.id]: { done: true } }));
              pushToast('Task complete', `${task.title} marked done.`, 'good');
            },
          },
        ];
      case 'reminders':
        return [
          {
            label: 'Schedule reminder',
            action: () => {
              const reminder = {
                id: `rem-${Date.now().toString(36)}`,
                workspaceId: activeWorkspaceId,
                title: 'New reminder',
                due: 'Tomorrow 9:00 AM',
                state: 'scheduled' as const,
              };
              setReminderList((current) => [reminder, ...current]);
              pushToast('Reminder scheduled', 'New reminder added to the active workspace.', 'good');
            },
          },
          { label: 'Mark done', action: () => setActiveSection('tasks') },
          { label: 'Snooze', action: () => setSurfaceMode('overview') },
          { label: 'Open calendar', action: () => setActiveSection('workspace') },
          {
            label: 'Dismiss top reminder',
            action: () => {
              const reminder = workspaceReminders[0];
              if (!reminder) return;
              setReminderList((current) => current.map((item) => (item.id === reminder.id ? { ...item, state: 'done' } : item)));
              setReminderFlags((current) => ({ ...current, [reminder.id]: { done: true } }));
              pushToast('Reminder done', `${reminder.title} dismissed.`, 'muted');
            },
          },
        ];
      case 'providers':
        return [
          { label: 'Configure provider', action: () => setActiveSection('providers') },
          { label: 'Set default', action: () => setActiveSection('settings') },
          { label: 'Validate MCP', action: () => setSurfaceMode('setup') },
          { label: 'Install config', action: () => openWorkspaceEditor('edit') },
        ];
      case 'settings':
        return [
          { label: 'Save defaults', action: () => setActiveSection('settings') },
          { label: 'Refresh diagnostics', action: () => setSurfaceMode('setup') },
          { label: 'Open config', action: () => openWorkspaceEditor('edit') },
          { label: 'Install skill', action: () => setActiveSection('providers') },
          { label: 'Open terminal', action: () => setSurfaceMode('setup') },
        ];
      case 'knowledge':
        return [
          { label: 'Export context', action: () => setActiveSection('knowledge') },
          { label: 'Search knowledge', action: () => setActiveSection('session') },
          { label: 'Open atlas', action: () => setSurfaceMode('overview') },
          { label: 'Publish update', action: () => setActiveSection('workspace') },
        ];
      default:
        return [
          { label: 'Switch workspace', action: () => setWorkspaceIndex((workspaceIndex + 1) % workspaceList.length) },
          { label: 'Next session', action: () => setSessionIndex((sessionIndex + 1) % workspaceSessions.length) },
          { label: 'Inspect artifacts', action: () => setSurfaceMode('artifacts') },
          { label: 'Open settings', action: () => setActiveSection('settings') },
          { label: '+', action: () => openWorkspaceEditor('create') },
          { label: 'Assign session', action: () => pushToast('Session assignment', `${displayedSessionTitle} linked to ${displayedWorkspaceName}.`, 'good') },
          {
            label: '+',
            action: () => {
              const nextId = `ws-${Date.now().toString(36)}`;
              const nextWorkspace: Workspace = {
                id: nextId,
                name: 'New workspace',
                sessions: 0,
                tasks: 0,
                reminders: 0,
                status: 'active' as const,
                summary: 'Locally created workspace entry.',
                files: 0,
                knowledgeNodes: 0,
                knowledgeEdges: 0,
                aiIntelligence: {
                  providers: [],
                },
              };
              setWorkspaceList((current) => [nextWorkspace, ...current]);
              setWorkspaceIndex(0);
              pushToast('Workspace created', 'New workspace added to the shell list.', 'good');
            },
          },
          {
            label: 'Delete workspace',
            action: () => {
              if (workspaceList.length <= 1) return;
              const removed = workspaceList[workspaceIndex];
              setWorkspaceList((current) => current.filter((_, index) => index !== workspaceIndex));
              setWorkspaceIndex((current) => Math.max(0, current - 1));
              pushToast('Workspace deleted', `${removed.name} removed from the list.`, 'warning');
            },
          },
          {
            label: '+',
            action: () => {
              const nextSession: Session = {
                id: `session-${Date.now().toString(36)}`,
                workspaceId: activeWorkspaceId,
                title: 'New session',
                provider: activeSession.provider,
                model: activeSession.model,
                updated: 'just now',
                files: 0,
                linked: 0,
                notes: 0,
                jira: 0,
                mergeRequests: 0,
                tree: 'workspace → session → new',
              };
              setSessionList((current) => [nextSession, ...current]);
              setSessionIndex(0);
              setActiveSection('session');
              pushToast('Session created', 'New session added to the active workspace.', 'good');
            },
          },
          {
            label: 'Delete session',
            action: () => {
              if (workspaceSessions.length <= 1) return;
              const removed = workspaceSessions[sessionIndex];
              setSessionList((current) => current.filter((session) => session.id !== removed.id));
              setSessionIndex((current) => Math.max(0, current - 1));
              pushToast('Session deleted', `${removed.title} removed from the workspace.`, 'warning');
            },
          },
          { label: 'Assign session', action: () => pushToast('Session assignment', `${displayedSessionTitle} linked to ${displayedWorkspaceName}.`, 'good') },
        ];
    }
  }, [activeSection, activeSession.provider, displayedSessionTitle, displayedWorkspaceName, sessionIndex, workspaceIndex, workspaceList.length, workspaceSessions, workspaceSessions.length]);
  const rightRailItems = useMemo(() => {
    if (activeSection === 'tasks') {
      return [
        { id: 'workspace-athena', label: 'Ask Athena', icon: <Sparkles size={14} />, active: isWorkspaceAthenaOpen, action: () => { closeWorkspaceDrawers(); setIsWorkspaceAthenaOpen(true); } },
        { id: 'global-task-create', label: 'Add Task', icon: <Plus size={14} />, active: false, action: () => { setTaskCreateRequest((current) => current + 1); } },
      ];
    }
    if (!activeWorkspaceId) return [];
    return [
      { id: 'workspace-athena', label: 'Ask Athena', icon: <Sparkles size={14} />, active: isWorkspaceAthenaOpen, action: () => { closeWorkspaceDrawers(); setIsWorkspaceAthenaOpen(true); } },
      { id: 'workspace-create', label: 'New Workspace', icon: <Plus size={14} />, active: false, action: () => openWorkspaceEditor('create') },
      { id: 'workspace-knowledge', label: 'Knowledge graph', icon: <Share2 size={14} />, active: isKnowledgeOpen, action: () => { closeWorkspaceDrawers(); setIsKnowledgeOpen(true); } },
      { id: 'workspace-sessions', label: 'Sessions', icon: <MessageSquare size={14} />, active: isSessionsDrawerOpen, action: openSessionsDrawer },
      { id: 'workspace-merge-requests', label: 'Merge requests', icon: <GitBranch size={14} />, active: isMergeRequestsDrawerOpen, action: () => { closeWorkspaceDrawers(); setIsMergeRequestsDrawerOpen(true); } },
      { id: 'workspace-jira', label: 'Jira tickets', icon: <Zap size={14} />, active: isJiraDrawerOpen, action: () => { closeWorkspaceDrawers(); setIsJiraDrawerOpen(true); } },
      { id: 'workspace-artifacts', label: 'Artifacts', icon: <FileCode size={14} />, active: isArtifactsDrawerOpen, action: () => { closeWorkspaceDrawers(); setIsArtifactsDrawerOpen(true); } },
      { id: 'workspace-tasks', label: 'Tasks', icon: <ListChecks size={14} />, active: isTaskDrawerOpen, action: () => { closeWorkspaceDrawers(); setIsTaskDrawerOpen(true); } },
      { id: 'workspace-notes', label: 'Notes', icon: <FileText size={14} />, active: isNotesDrawerOpen, action: () => { closeWorkspaceDrawers(); setIsNotesDrawerOpen(true); } },
      { id: 'workspace-activity', label: 'Recent activity', icon: <History size={14} />, active: isActivityOpen, action: () => { closeWorkspaceDrawers(); setIsActivityOpen(true); } },
    ];
  }, [activeSection, activeWorkspaceId, isActivityOpen, isArtifactsDrawerOpen, isJiraDrawerOpen, isKnowledgeOpen, isMergeRequestsDrawerOpen, isNotesDrawerOpen, isSessionsDrawerOpen, isTaskDrawerOpen, isWorkspaceAthenaOpen, openSessionsDrawer]);

  if (!authReady || !hasApiKey) {
    return <LoginScreen onLogin={persistApiKey} initialServerUrl={serverDraft} />;
  }

  return (
    <ShellChrome
      sanctumMark={sanctumMark}
      navigation={navigation as unknown as { id: string; label: string; icon: ReactNode }[]}
      sectionIcons={sectionIcons}
      activeSection={activeSection}
      activityCount={activityCount}
      toasts={toasts}
      workspaceList={workspaceList}
      isWorkspaceLoading={isWorkspaceLoading}
      workspaceSearch={workspaceSearch}
      onWorkspaceSearchChange={setWorkspaceSearch}
      workspaceIndex={workspaceIndex}
      onWorkspaceSelect={(index) => {
        setWorkspaceIndex(index);
        setSessionIndex(0);
        setActiveSection('workspace');
        setIsDrawerOpen(true);
      }}
      isWorkspacePaneOpen={isWorkspacePaneOpen}
      toggleWorkspacePane={toggleWorkspacePane}
      workspaceSessions={workspaceSessions}
      onAlerts={() => setIsAlertsOpen(true)}
      onHelp={() => setIsHelpOpen(true)}
      onLogout={() => { void handleLogout(); }}
      onWorkspaceCreate={() => openWorkspaceEditor('create')}
      onWorkspaceSelected={() => {
        setActiveSection('tasks');
        setIsDrawerOpen(true);
      }}
      onSettings={() => setSettingsOpen(true)}
      rightRailItems={rightRailItems}
      athenaProvider={selectedProviderLabel}
      athenaModel={selectedModel}
      footerItems={[
        { label: 'user', status: 'online', detail: 'ahmed' },
        { label: 'section', status: 'online', detail: sectionLabel },
        { label: 'workspace', status: 'online', detail: displayedWorkspaceName },
        { label: 'session', status: 'online', detail: displayedSessionTitle },
        { label: 'surface', status: 'online', detail: activeModeLabel },
        { label: 'alerts', status: activityCount > 0 ? 'warning' : 'online', detail: String(activityCount) },
        { label: 'sync', status: 'online', detail: 'live' },
      ]}
    >
      <WorkspaceSurface
        hero={hero}
        heroFacts={heroFacts}
        onEdit={() => openWorkspaceEditor('edit')}
        workspaceSessions={workspaceSessions}
        workspaceTasks={workspaceTasks}
        allTasks={taskList}
        workspaceReminders={workspaceReminders}
        workspaceNotes={workspaceNotes}
        workspaceArtifacts={workspaceArtifacts}
        workspaceSessionFileGroups={workspaceSessionFiles}
        workspaceMergeRequests={mergeRequestList}
        workspaceJiraTickets={jiraTicketList}
        activeWorkspace={activeWorkspace}
        workspaceSummary={workspaceSummary}
        manageSummary={manageSummary}
        todayTasks={todayTasks}
        todayReminders={todayReminders}
        activeSection={activeSection}
        surfaceMode={surfaceMode}
        setSurfaceMode={setSurfaceMode}
        activeModeLabel={activeModeLabel}
        surfaceTitle={surfaceTitle}
        workspaceList={workspaceList}
        workspaceIndex={workspaceIndex}
        setWorkspaceIndex={setWorkspaceIndex}
        setSessionIndex={setSessionIndex}
        activeSession={activeSession}
        sessionNotes={sessionNotes}
        sessionArtifacts={sessionArtifacts}
        sessionTimeline={sessionTimeline}
        sessionConversation={sessionConversation}
        sessionStats={sessionStats}
        sessionCheckpoints={sessionCheckpoints}
        activeProvider={activeProvider}
        providers={providers}
        setSessionProvider={setSessionProvider}
        localSetup={localSetup}
        pushToast={pushToast}
        setActiveSection={setActiveSection}
        setManagementDrawer={setManagementDrawer}
        onOpenTasks={() => setIsTaskDrawerOpen(true)}
        onEditGlobalTask={(task) => {
          setTaskToEdit(task);
        }}
        setNoteList={setNoteList}
        setTaskList={setTaskList}
        setReminderList={setReminderList}
        taskFlags={taskFlags}
        reminderFlags={reminderFlags}
        setTaskFlags={setTaskFlags}
        setReminderFlags={setReminderFlags}
        workspaceSearch={workspaceSearch}
        setWorkspaceSearch={setWorkspaceSearch}
        setIsTaskDrawerOpen={setIsTaskDrawerOpen}
        setIsNotesDrawerOpen={setIsNotesDrawerOpen}
        setIsMergeRequestsDrawerOpen={setIsMergeRequestsDrawerOpen}
        setIsJiraDrawerOpen={setIsJiraDrawerOpen}
        setIsArtifactsDrawerOpen={setIsArtifactsDrawerOpen}
        isSessionsDrawerOpen={isSessionsDrawerOpen}
        setIsSessionsDrawerOpen={setIsSessionsDrawerOpen}
        workspaceSessionFiles={workspaceSessionFiles}
        isKnowledgeOpen={isKnowledgeOpen}
        setIsKnowledgeOpen={setIsKnowledgeOpen}
        isActivityOpen={isActivityOpen}
        setIsActivityOpen={setIsActivityOpen}
        serverBaseUrl={serverBaseUrl}
        apiKey={apiKey}
        workspaceActivitySummary={workspaceActivitySummary}
        restoreActivityContext={restoreActivityContext}
        activeWorkspaceId={activeWorkspaceId}
        onOpenSessions={openSessionsDrawer}
      />

      <AppOverlays
      setSessionIndex={setSessionIndex}
      isActivityOpen={isActivityOpen}
      setIsActivityOpen={setIsActivityOpen}
      isAlertsOpen={isAlertsOpen}
      setIsAlertsOpen={setIsAlertsOpen}
      isKnowledgeOpen={isKnowledgeOpen}
      setIsKnowledgeOpen={setIsKnowledgeOpen}
        isTaskDrawerOpen={isTaskDrawerOpen}
        setIsTaskDrawerOpen={setIsTaskDrawerOpen}
        taskCreateRequest={taskCreateRequest}
        isNotesDrawerOpen={isNotesDrawerOpen}
        setIsNotesDrawerOpen={setIsNotesDrawerOpen}
        isMergeRequestsDrawerOpen={isMergeRequestsDrawerOpen}
        setIsMergeRequestsDrawerOpen={setIsMergeRequestsDrawerOpen}
        isJiraDrawerOpen={isJiraDrawerOpen}
        setIsJiraDrawerOpen={setIsJiraDrawerOpen}
        isArtifactsDrawerOpen={isArtifactsDrawerOpen}
        setIsArtifactsDrawerOpen={setIsArtifactsDrawerOpen}
        isSessionsDrawerOpen={isSessionsDrawerOpen}
        setIsSessionsDrawerOpen={setIsSessionsDrawerOpen}
        activeSessionId={activeSession.id}
        workspaceSessionFileGroups={workspaceSessionFiles}
        sessionConversationMap={sessionConversationMap}
        settingsOpen={settingsOpen}
        setSettingsOpen={setSettingsOpen}
        isEditOpen={isEditOpen}
        setIsEditOpen={setIsEditOpen}
        workspaceEditorMode={workspaceEditorMode}
        isHelpOpen={isHelpOpen}
        setIsHelpOpen={setIsHelpOpen}
        authUser={authUser}
        authDraft={authDraft}
        serverDraft={serverDraft}
        gatewayDraft={gatewayDraft}
        hasApiKey={hasApiKey}
        setAuthUser={setAuthUser}
        setAuthDraft={setAuthDraft}
        setServerDraft={setServerDraft}
        setGatewayDraft={setGatewayDraft}
        selectedProvider={selectedProvider}
        selectedModel={selectedModel}
        setSelectedProvider={updateAthenaProvider}
        setSelectedModel={updateAthenaModel}
        gatewayProviders={gatewayProviders}
        editDraft={editDraft}
        setEditDraft={setEditDraft}
        saveEditDraft={saveEditDraft}
        providers={providers}
        onSaveSettings={async () => {
          const newName = authUser.trim() || 'operator';
          await saveSettingSafely('user:name', newName);
          await saveSettingSafely('server:config', { url: serverDraft.trim(), enabled: true });
          await saveSettingSafely('gateway:config', { url: gatewayDraft.trim(), enabled: true });
          await saveSettingSafely('colosseum:grooming-settings', colosseumGroomingSettings);
          await saveSettingSafely('colosseum:ready-settings', colosseumReadySettings);
          await saveSettingSafely('colosseum:review-settings', colosseumReviewSettings);
          await saveAthenaEngineSafely(selectedProvider, selectedModel);
          await persistApiKey(authDraft);
          void refreshProviders(gatewayDraft.trim());

          const activeKey = authDraft.trim();
          if (activeKey) {
            try {
              let uId = currentUserId;
              const cleanServerUrl = serverDraft.trim() || 'http://127.0.0.1:8090';
              if (!uId) {
                const valRes = await fetch(`${cleanServerUrl}/api/auth/validate`, {
                  headers: buildSavantHeaders(activeKey)
                });
                if (valRes.ok) {
                  const valData = await valRes.json();
                  if (valData.valid && valData.user_id) {
                    uId = valData.user_id;
                    setCurrentUserId(uId);
                  }
                }
              }

              if (uId) {
                const putRes = await fetch(`${cleanServerUrl}/api/users/${encodeURIComponent(uId)}`, {
                  method: 'PUT',
                  headers: buildSavantHeaders(activeKey, true),
                  body: JSON.stringify({ name: newName })
                });
                if (!putRes.ok) {
                  console.error('Failed to save name to savant-server:', putRes.status, await putRes.text());
                }
              }
            } catch (e) {
              console.error('Failed to sync name to savant-server:', e);
            }
          }
        }}
      onLogout={handleAuthLogout}
      onRefreshProviders={() => refreshProviders(gatewayDraft.trim() || 'http://127.0.0.1:3100')}
      colosseumGroomingSettings={colosseumGroomingSettings}
      colosseumReadySettings={colosseumReadySettings}
      colosseumReviewSettings={colosseumReviewSettings}
      onColosseumGroomingSettingsChange={setColosseumGroomingSettings}
      onColosseumReadySettingsChange={setColosseumReadySettings}
      onColosseumReviewSettingsChange={setColosseumReviewSettings}
      recentAlerts={toasts}
      activityFeed={activityFeed}
        restoreActivityContext={restoreActivityContext}
        managementDrawer={managementDrawer}
        setManagementDrawer={setManagementDrawer}
        workspaceTasks={activeSection === 'tasks' ? taskList : workspaceTasks}
        taskDrawerScope={activeSection === 'tasks' ? 'global' : 'workspace'}
        workspaceList={workspaceList}
        taskToEdit={activeSection === 'tasks' ? taskToEdit : null}
        onTaskEditOpened={() => setTaskToEdit(null)}
        setTaskList={setTaskList}
        workspaceNotes={workspaceNotes}
        workspaceArtifacts={workspaceArtifacts}
        onCreateSessionNote={createSessionNote}
        workspaceSessions={workspaceSessions}
        workspaceReminders={workspaceReminders}
        displayedWorkspaceName={displayedWorkspaceName}
        taskFlags={taskFlags}
        setTaskFlags={setTaskFlags}
        reminderFlags={reminderFlags}
        setReminderFlags={setReminderFlags}
        pushToast={pushToast}
        onDeleteSession={handleDeleteSession}
        activeWorkspaceId={activeWorkspaceId}
        serverBaseUrl={serverBaseUrl}
        apiKey={apiKey}
        taskAthenaChats={taskAthenaChats}
        onSendTaskAthenaMessage={handleSendTaskAthenaMessage}
        onDeleteTaskAthenaMessage={handleDeleteTaskAthenaMessage}
        onClearTaskAthenaChat={handleClearTaskAthenaChat}
        onChangeTaskAthenaInput={handleChangeTaskAthenaInput}
      />

      <WorkspaceAthenaDrawer
        isOpen={isWorkspaceAthenaOpen}
        onClose={() => setIsWorkspaceAthenaOpen(false)}
        activeWorkspace={activeWorkspace}
        activeWorkspaceId={activeWorkspaceId}
        messages={athenaChats[activeWorkspaceId]?.messages || []}
        isLoading={athenaChats[activeWorkspaceId]?.isLoading || false}
        error={athenaChats[activeWorkspaceId]?.error || ''}
        persistedInput={athenaChats[activeWorkspaceId]?.input || ''}
        onSendMessage={(text) => handleSendAthenaMessage(activeWorkspaceId, text)}
        onDeleteMessage={(msgId) => handleDeleteAthenaMessage(activeWorkspaceId, msgId)}
        onClearChat={() => handleClearAthenaChat(activeWorkspaceId)}
        onChangeInput={(text) => handleChangeAthenaInput(activeWorkspaceId, text)}
        workspaceSessions={workspaceSessions}
        workspaceTasks={activeSection === 'tasks' ? taskList : workspaceTasks}
        workspaceNotes={workspaceNotes}
        workspaceArtifacts={workspaceArtifacts}
        workspaceMergeRequests={mergeRequestList}
        workspaceJiraTickets={jiraTicketList}
        workspaceActivitySummary={workspaceActivitySummary}
        selectedProvider={selectedProvider}
        selectedModel={selectedModel}
        athenaType={activeSection === 'tasks' ? 'task_manager' : 'workspace'}
      />

    </ShellChrome>
  );
}

export default App;
