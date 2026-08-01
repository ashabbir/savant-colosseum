import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

vi.mock('lucide-react', () => {
  return new Proxy({}, {
    get: (target, prop) => {
      return (props: any) => `Icon-${String(prop)}`;
    }
  });
});

import { WorkspaceHeader } from '../components/WorkspaceHeader';
import { SessionConversation } from '../components/SessionConversation';
import { WorkspaceManagePanel, WorkspaceSessionPanel } from '../components/WorkspacePanels';
import { WorkspaceSessionDetailsDrawer } from '../components/WorkspaceSessionDetailsDrawer';
import { WorkspaceSessionsDrawer } from '../components/WorkspaceSessionsDrawer';
import { Metric, PanelHeader } from '../components/WorkspacePrimitives';
import {
  buildWorkspaceEditorPayload,
  createWorkspaceEditorDraft,
  createWorkspaceEditorDraftFromWorkspace,
} from '../components/workspaceEditor';
import { WorkspaceOverview } from '../components/WorkspaceOverview';
import { WorkspaceSessionCard } from '../components/WorkspaceSessionCard';
import { DEFAULT_COLOSSEUM_READY_SETTINGS, SettingsModal, withDefaultColosseumSettings } from '../components/SettingsModal';
import { isAbortError } from '../components/BottomBar';
import { LoginScreen } from '../components/LoginScreen';
import { ShellChrome } from '../components/ShellChrome';
import { WorkspaceSurface } from '../components/WorkspaceSurface';
import { WorkspaceAthenaDrawer } from '../components/WorkspaceAthenaDrawer';
import type { Session, Task, Workspace, Artifact, Reminder } from '../data';

const workspace: Workspace = {
  id: 'ws-test',
  name: 'Test Workspace',
  sessions: 2,
  tasks: 1,
  reminders: 1,
  status: 'active',
  summary: 'A fixture workspace.',
  files: 3,
  knowledgeNodes: 4,
  knowledgeEdges: 5,
  aiIntelligence: { providers: [] },
};

const session: Session = {
  id: 'session-test',
  workspaceId: 'ws-test',
  title: 'Fixture session',
  provider: 'Codex',
  model: 'Llama 3.3',
  updated: 'just now',
  files: 1,
  linked: 2,
  notes: 1,
  jira: 0,
  mergeRequests: 1,
  tree: 'workspace → session → provider config',
};

const taskFlags: Record<string, { done?: boolean }> = {};
const noop = () => {};

describe('renderer components', () => {
  it('renders the reusable metric primitive', () => {
    const html = renderToStaticMarkup(<Metric label="Sessions" value={12} />);
    expect(html).toContain('Sessions');
    expect(html).toContain('12');
  });

  it('renders the shared panel header', () => {
    const html = renderToStaticMarkup(<PanelHeader title="Tasks" actionLabel="Prioritize" />);
    expect(html).toContain('Tasks');
    expect(html).toContain('aria-label="Prioritize"');
  });

  it('renders the workspace header actions and facts', () => {
    const html = renderToStaticMarkup(
      <WorkspaceHeader
        eyebrow="Workspace"
        title="Sanctum"
        subtitle="1 sessions · 5 tasks"
        workspaceColor="#12ab34"
        workspacePriority="high"
        workspaceStatus="open"
        facts={['Open', 'Nodes 4', 'Edges 3', 'Density sparse']}
        onEdit={noop}
        onAddNote={noop}
        onOpenKnowledge={noop}
        onOpenActivity={noop}
        workspaceId="test-workspace-id"
      />,
    );

    expect(html).toContain('Sanctum');
    expect(html).toContain('>high<');
    expect(html).toContain('>open<');
    expect(html).toContain('Density sparse');
    expect(html).toContain('Edit workspace');
    expect(html).toContain('color:#12ab34');
    expect(html).toContain('aria-label="Copy workspace ID"');
  });

  it('creates a blank workspace draft for add', () => {
    const draft = createWorkspaceEditorDraft();
    expect(draft).toEqual({
      workspaceName: '',
      workspaceDescription: '',
      workspacePriority: 'medium',
      workspaceStatus: 'open',
      workspaceColor: '',
    });
  });

  it('creates an edit draft from the active workspace', () => {
    const draft = createWorkspaceEditorDraftFromWorkspace({
      id: 'ws-test',
      name: 'Workspace A',
      description: 'Workspace description',
      priority: 'high',
      status: 'closed',
      color: '#123456',
      sessions: 0,
      tasks: 0,
      reminders: 0,
      summary: '',
      files: 0,
      knowledgeNodes: 0,
      knowledgeEdges: 0,
      aiIntelligence: { providers: [] },
    });

    expect(draft).toEqual({
      workspaceName: 'Workspace A',
      workspaceDescription: 'Workspace description',
      workspacePriority: 'high',
      workspaceStatus: 'closed',
      workspaceColor: '#123456',
    });
  });

  it('builds the workspace payload without session fields', () => {
    const payload = buildWorkspaceEditorPayload({
      workspaceName: '  Workspace B  ',
      workspaceDescription: '  Summary  ',
      workspacePriority: 'critical',
      workspaceStatus: 'open',
      workspaceColor: '  #abcdef  ',
    });

    expect(payload).toEqual({
      name: 'Workspace B',
      description: 'Summary',
      priority: 'critical',
      status: 'open',
      color: '#abcdef',
    });
  });

  it('renders the session panel with session and attachment details', () => {
    const html = renderToStaticMarkup(
      <WorkspaceSessionPanel
        surfaceTitle="Workspace overview"
        activeModeLabel="Overview"
        activeSection="session"
        activeSession={session}
        workspaceList={[workspace]}
        workspaceIndex={0}
        setWorkspaceIndex={() => {}}
        setSessionIndex={() => {}}
        workspaceSessions={[session]}
        sessionStats={[{ label: 'Notes', value: 1 }]}
        sessionCheckpoints={['root → session']}
        sessionTimeline={[{ id: 'event-1', time: 'now', title: 'Synced', detail: 'ok', kind: 'sync' }]}
        sessionConversation={[{ id: 'chat-1', time: 'now', title: 'Assistant', detail: 'Working on it', kind: 'assistant', role: 'assistant_message', provider: 'Codex' }]}
        sessionNotes={[{ id: 'note-1', title: 'Note', body: 'Body' }]}
        sessionArtifacts={[{ id: 'artifact-1', sessionId: 'session-test', title: 'File', kind: 'file', count: 1 } as Artifact]}
        setActiveSection={() => {}}
        pushToast={() => {}}
        taskFlags={taskFlags}
      />,
    );

    expect(html).toContain('Workspace overview');
    expect(html).toContain('Fixture session');
    expect(html).toContain('Notes');
    expect(html).toContain('Merge requests');
  });

  it('renders full session conversation roles', () => {
    const html = renderToStaticMarkup(
      <SessionConversation
        messages={[
          { id: 'chat-1', time: 'now', title: 'User', detail: 'Build the feature', kind: 'user', role: 'user', provider: 'Codex' },
          { id: 'chat-2', time: 'now', title: 'Assistant', detail: 'Working on it', kind: 'assistant', role: 'assistant', provider: 'Codex' },
          { id: 'chat-3', time: 'now', title: 'Terminal', detail: 'npm test', kind: 'tool', role: 'tool', provider: 'Codex' },
          { id: 'chat-4', time: 'now', title: 'Reasoning', detail: 'Planning the fix', kind: 'system', role: 'system', provider: 'Codex' },
        ]}
      />,
    );

    expect(html).toContain('Build the feature');
    expect(html).toContain('Working on it');
    expect(html).toContain('npm test');
    expect(html).toContain('Planning the fix');
  });

  it('renders the sessions drawer list', () => {
    const html = renderToStaticMarkup(
      <WorkspaceSessionsDrawer
        open
        workspaceName="Test Workspace"
        sessions={[session]}
        sessionFileGroups={{ [session.id]: { filePath: '/tmp/workspaces/test/session-test/session.jsonl' } }}
        activeSessionId={session.id}
        onSelectSession={() => {}}
        onClose={() => {}}
      />,
    );

    expect(html).toContain('Sessions');
    expect(html).toContain('Fixture session');
    expect(html).toContain('Test Workspace');
    expect(html).toContain('Copy resume command');
    expect(html).toContain('--yolo');
  });

  it('renders the session details drawer stats', () => {
    const html = renderToStaticMarkup(
      <WorkspaceSessionDetailsDrawer
        open
        workspaceName="Test Workspace"
        session={session}
        stats={[
          { label: 'Files', value: 1 },
          { label: 'Notes', value: 2 },
        ]}
        files={{ files: [{ path: 'chat.jsonl', category: 'file', size: 42 }] }}
        conversation={[]}
        notes={[{ id: 'note-1', title: 'Note', body: 'Body' }]}
        artifacts={[{ id: 'artifact-1', sessionId: 'session-test', title: 'File', kind: 'file', count: 1 } as Artifact]}
        onSelectNote={() => {}}
        onBack={() => {}}
        onClose={() => {}}
      />,
    );

    expect(html).toContain('Overview');
    expect(html).toContain('Session stats');
    expect(html).toContain('Files');
    expect(html).toContain('Fixture session');
    expect(html).toContain('Test Workspace');
  });

  it('renders the manage panel summary', () => {
    const html = renderToStaticMarkup(<WorkspaceManagePanel manageSummary={{ todayTasks: 3, todayReminders: 2, openTasks: 4, pendingReminders: 1 }} />);
    expect(html).toContain('Today&#x27;s work');
    expect(html).toContain('5');
  });

  it('renders the WorkspaceOverview board with sessions, tasks, notes and live entity stats', () => {
    const html = renderToStaticMarkup(
      <WorkspaceOverview
        workspaceSessions={[
          { ...session, createdAt: '2026-06-20T10:00:00Z', updatedAt: '2026-06-21T10:00:00Z' }
        ]}
        workspaceTasks={[
          { id: 'task-1', title: 'Task 1', priority: 'high', state: 'todo' }
        ]}
        workspaceNotes={[
          { id: 'note-1', sessionId: 'session-test', title: 'A Note', body: 'Notes content', createdAt: '2026-06-20T11:00:00Z' }
        ]}
        workspaceArtifacts={[
          { id: 'art-1', sessionId: 'session-test', title: 'artifact.json', kind: 'file', updated: 'yesterday' } as Artifact
        ]}
        workspaceMergeRequests={[
          { id: 'mr-1', title: 'Fix auth bug', status: 'open', mrId: '123' }
        ]}
        workspaceJiraTickets={[
          { id: 'jira-1', title: 'Jira issue', status: 'closed', ticketKey: 'PROJ-123' }
        ]}
        workspaceSummary={{ knowledgeNodes: 10, knowledgeEdges: 5 }}
        workspaceActivitySummary={{ total: 15 }}
        taskFlags={{}}
        onTaskSelect={() => {}}
        onOpenSessions={() => {}}
        onOpenTasks={() => {}}
        onOpenNotes={() => {}}
        onOpenActivity={() => {}}
        onOpenArtifacts={() => {}}
        onOpenKnowledge={() => {}}
        onOpenMergeRequests={() => {}}
        onOpenJira={() => {}}
      />
    );

    expect(html).toContain('Merge request');
    expect(html).toContain('Jira tickets');
    expect(html).toContain('artifact.json');
    expect(html).toContain('Knowledge graph');
    expect(html).toContain('10'); // Nodes
    expect(html).toContain('5'); // Edges
    expect(html).toContain('15'); // Activity total
    expect(html).not.toContain('Workspace ID');
    expect(html).not.toContain('Copy ID');
  });

  it('renders WorkspaceSessionCard and generates the correct resume command', () => {
    const html = renderToStaticMarkup(
      <WorkspaceSessionCard
        session={session}
        active={true}
        files={{ filePath: '/tmp/workspaces/test/session-test/session.jsonl' }}
        onSelect={() => {}}
      />
    );

    expect(html).toContain('Fixture session');
    expect(html).toContain('is-active');
    expect(html).toContain('Copy resume command');
    // Verify it generates correct resume command in the title attribute
    expect(html).toContain('title="cd /tmp/workspaces/test/session-test &amp;&amp; codex start --session session-test --yolo"');
  });

  it('renders SettingsModal when open and shows error if no providers', () => {
    const html = renderToStaticMarkup(
      <SettingsModal
        open={true}
        authUser="Alice"
        authDraft="sk-key"
        serverDraft="http://localhost:8090"
        gatewayDraft="http://localhost:3100"
        selectedProvider=""
        selectedModel=""
        providers={[]}
        hasApiKey={true}
        onClose={() => {}}
        onAuthUserChange={() => {}}
        onAuthDraftChange={() => {}}
        onServerDraftChange={() => {}}
        onGatewayDraftChange={() => {}}
        onSelectedProviderChange={() => {}}
        onSelectedModelChange={() => {}}
        onSave={() => {}}
        onLogout={() => {}}
      />
    );

    expect(html).toContain('Settings');
    expect(html).toContain('User preferences');
    expect(html).toContain('GATEWAY ERROR: NO PROVIDERS DETECTED');
  });

  it('uses Colosseum defaults when settings have not been persisted yet', () => {
    expect(withDefaultColosseumSettings(undefined, DEFAULT_COLOSSEUM_READY_SETTINGS).persona).toBe('persona.engineer');
  });

  it('recognizes the expected BottomBar timeout abort', () => {
    const controller = new AbortController();
    controller.abort();
    expect(isAbortError(new DOMException('aborted', 'AbortError'), controller.signal)).toBe(true);
  });

  it('renders SettingsModal with providers', () => {
    const html = renderToStaticMarkup(
      <SettingsModal
        open={true}
        authUser="Alice"
        authDraft="sk-key"
        serverDraft="http://localhost:8090"
        gatewayDraft="http://localhost:3100"
        selectedProvider="gemini"
        selectedModel="gemini-1.5-pro"
        providers={[{ id: 'gemini', label: 'Gemini Provider', models: ['gemini-1.5-pro'] }]}
        hasApiKey={true}
        onClose={() => {}}
        onAuthUserChange={() => {}}
        onAuthDraftChange={() => {}}
        onServerDraftChange={() => {}}
        onGatewayDraftChange={() => {}}
        onSelectedProviderChange={() => {}}
        onSelectedModelChange={() => {}}
        onSave={() => {}}
        onLogout={() => {}}
      />
    );

    expect(html).toContain('Gemini Provider');
    expect(html).toContain('gemini-1.5-pro');
  });

  it('renders LoginScreen', () => {
    const html = renderToStaticMarkup(
      <LoginScreen onLogin={async () => {}} />
    );

    expect(html).toContain('Savant Sanctum');
    expect(html).toContain('Server URL');
    expect(html).toContain('value="http://127.0.0.1:8090"');
    expect(html).toContain('Savant API Key');
    expect(html).toContain('placeholder="sk-..."');
  });

  it('renders ShellChrome with navigation and workspace search bar', () => {
    const html = renderToStaticMarkup(
      <ShellChrome
        sanctumMark="sanctum.svg"
        navigation={[{ id: 'tasks', label: 'Tasks', icon: 'TasksIcon' }]}
        sectionIcons={{}}
        activeSection="tasks"
        activityCount={2}
        toasts={[{ id: 't1', title: 'Error', detail: 'Fail', tone: 'warning' }]}
        workspaceList={[workspace]}
        workspaceSearch="Test"
        onWorkspaceSearchChange={() => {}}
        workspaceIndex={0}
        onWorkspaceSelect={() => {}}
        isWorkspacePaneOpen={true}
        toggleWorkspacePane={() => {}}
        workspaceSessions={[]}
        onAlerts={() => {}}
        onHelp={() => {}}
        onLogout={() => {}}
        onWorkspaceCreate={() => {}}
        onWorkspaceSelected={() => {}}
        onSettings={() => {}}
        rightRailItems={[]}
        footerItems={[{ label: 'Status', status: 'OK', detail: 'Good' }]}
      >
        <div>Content Area</div>
      </ShellChrome>
    );

    expect(html).toContain('sanctum');
    expect(html).toContain('Content Area');
    expect(html).toContain('Tasks');
    expect(html).toContain('Fail');
    expect(html).not.toContain('Search workspaces...');
  });

  it('renders the cross-workspace Task Dashboard as a complete top-level surface', () => {
    const globalTasks: Task[] = [
      { id: 'task-global', workspaceId: workspace.id, title: 'Global task', priority: 'critical', state: 'in-progress', owner: 'operator' },
    ];
    const html = renderToStaticMarkup(
      <WorkspaceSurface
        hero={{ eyebrow: 'Tasks', title: 'Task manager', subtitle: 'All workspaces' }}
        heroFacts={[]}
        workspaceSessions={[session]}
        workspaceTasks={globalTasks}
        allTasks={globalTasks}
        workspaceReminders={[]}
        workspaceNotes={[]}
        workspaceArtifacts={[]}
        workspaceSessionFiles={{}}
        workspaceMergeRequests={[]}
        workspaceJiraTickets={[]}
        activeWorkspace={workspace}
        workspaceSummary={{ knowledgeNodes: 0, knowledgeEdges: 0, knowledgeCommittedNodes: 0, knowledgeStagedNodes: 0, knowledgeCommittedEdges: 0, knowledgeStagedEdges: 0 }}
        manageSummary={{ todayTasks: 0, todayReminders: 0, openTasks: 1, pendingReminders: 0 }}
        todayTasks={[]}
        todayReminders={[]}
        activeSection="tasks"
        onEdit={() => {}}
        surfaceMode="dashboard"
        setSurfaceMode={() => {}}
        activeModeLabel="Dashboard"
        surfaceTitle="Tasks"
        workspaceList={[workspace]}
        workspaceIndex={0}
        setWorkspaceIndex={() => {}}
        setSessionIndex={() => {}}
        activeSession={session}
        sessionNotes={[]}
        sessionArtifacts={[]}
        sessionTimeline={[]}
        sessionConversation={[]}
        sessionStats={[]}
        sessionCheckpoints={[]}
        activeProvider={{ id: 'p1', label: 'P1', models: [] }}
        providers={[]}
        setSessionProvider={() => {}}
        localSetup={{ server: 'http://localhost:8090', key: '123', initialized: true }}
        pushToast={() => {}}
        setActiveSection={() => {}}
        setManagementDrawer={() => {}}
        setNoteList={() => {}}
        onCreateSessionNote={() => {}}
        setTaskList={() => {}}
        setReminderList={() => {}}
        taskFlags={{}}
        reminderFlags={{}}
        setTaskFlags={() => {}}
        setReminderFlags={() => {}}
        workspaceSearch=""
        setWorkspaceSearch={() => {}}
        setIsTaskDrawerOpen={() => {}}
        setIsNotesDrawerOpen={() => {}}
        setIsMergeRequestsDrawerOpen={() => {}}
        setIsJiraDrawerOpen={() => {}}
        setIsArtifactsDrawerOpen={() => {}}
        isKnowledgeOpen={false}
        setIsKnowledgeOpen={() => {}}
        isActivityOpen={false}
        setIsActivityOpen={() => {}}
        serverBaseUrl="http://localhost:8090"
        apiKey="123"
        onOpenTasks={() => {}}
        onEditGlobalTask={() => {}}
        workspaceActivitySummary={{ total: 0, detail: 'none', latest: 'never' }}
        restoreActivityContext={() => {}}
        activeWorkspaceId={workspace.id}
        onOpenSessions={() => {}}
      />
    );

    expect(html).toContain('Cross-workspace board');
    expect(html).toContain('Task Status Overview');
    expect(html).toContain('In progress');
    expect(html).toContain('All workspaces');
    expect(html).toContain('All statuses');
    expect(html).toContain('All priorities');
    expect(html).toContain('Board');
    expect(html).toContain('Visual');
    expect(html).toContain('Global task');
  });

  it('renders WorkspaceSurface for different modes', () => {
    const html = renderToStaticMarkup(
      <WorkspaceSurface
        hero={{ eyebrow: 'Sanctum', title: 'Sanctum Title', subtitle: 'Sanctum Sub' }}
        heroFacts={['Fact 1']}
        workspaceSessions={[session]}
        workspaceTasks={[]}
        workspaceReminders={[]}
        workspaceNotes={[]}
        workspaceArtifacts={[]}
        workspaceSessionFiles={{}}
        workspaceMergeRequests={[]}
        workspaceJiraTickets={[]}
        activeWorkspace={workspace}
        workspaceSummary={{
          knowledgeNodes: 5,
          knowledgeEdges: 4,
          knowledgeCommittedNodes: 3,
          knowledgeStagedNodes: 2,
          knowledgeCommittedEdges: 2,
          knowledgeStagedEdges: 2,
        }}
        manageSummary={{ todayTasks: 1, todayReminders: 1, openTasks: 1, pendingReminders: 1 }}
        todayTasks={[]}
        todayReminders={[]}
        activeSection="workspace"
        onEdit={() => {}}
        surfaceMode="dashboard"
        setSurfaceMode={() => {}}
        activeModeLabel="Dashboard"
        surfaceTitle="Surface Title"
        workspaceList={[workspace]}
        workspaceIndex={0}
        setWorkspaceIndex={() => {}}
        setSessionIndex={() => {}}
        activeSession={session}
        sessionNotes={[]}
        sessionArtifacts={[]}
        sessionTimeline={[]}
        sessionConversation={[]}
        sessionStats={[]}
        sessionCheckpoints={[]}
        activeProvider={{ id: 'p1', label: 'P1', models: [] }}
        providers={[]}
        setSessionProvider={() => {}}
        localSetup={{ server: 'http://localhost:8090', key: '123', initialized: true }}
        pushToast={() => {}}
        setActiveSection={() => {}}
        setManagementDrawer={() => {}}
        setNoteList={() => {}}
        onCreateSessionNote={() => {}}
        setTaskList={() => {}}
        setReminderList={() => {}}
        taskFlags={{}}
        reminderFlags={{}}
        setTaskFlags={() => {}}
        setReminderFlags={() => {}}
        workspaceSearch=""
        setWorkspaceSearch={() => {}}
        setIsTaskDrawerOpen={() => {}}
        setIsNotesDrawerOpen={() => {}}
        setIsMergeRequestsDrawerOpen={() => {}}
        setIsJiraDrawerOpen={() => {}}
        setIsArtifactsDrawerOpen={() => {}}
        isKnowledgeOpen={false}
        setIsKnowledgeOpen={() => {}}
        isActivityOpen={false}
        setIsActivityOpen={() => {}}
        serverBaseUrl="http://localhost:8090"
        apiKey="123"
        onOpenTasks={() => {}}
        onEditGlobalTask={() => {}}
        workspaceActivitySummary={{ total: 0, detail: 'none', latest: 'never' }}
        restoreActivityContext={() => {}}
        activeWorkspaceId="ws-test"
        onOpenSessions={() => {}}
      />
    );

    expect(html).toContain('Sanctum Title');
    expect(html).toContain('Sanctum Sub');
  });

  it('renders WorkspaceAthenaDrawer', () => {
    const html = renderToStaticMarkup(
      <WorkspaceAthenaDrawer
        isOpen={true}
        onClose={() => {}}
        activeWorkspace={workspace}
        activeWorkspaceId="ws-test"
        messages={[{ id: '1', sender: 'assistant', text: 'Hello, how can I help?', timestamp: 'now' }]}
        isLoading={false}
        error=""
        persistedInput=""
        onSendMessage={() => {}}
        onDeleteMessage={() => {}}
        onChangeInput={() => {}}
        workspaceSessions={[]}
        workspaceTasks={[]}
        workspaceNotes={[]}
        workspaceArtifacts={[]}
        workspaceMergeRequests={[]}
        workspaceJiraTickets={[]}
        workspaceActivitySummary={{ total: 0, detail: 'none', latest: 'never' }}
      />
    );

    expect(html).toContain('ATHENA');
    expect(html).toContain('Hello, how can I help?');
  });
});
