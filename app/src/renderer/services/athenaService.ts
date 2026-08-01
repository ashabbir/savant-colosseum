import type { Artifact, Session, Task, Workspace } from '../data';
import { buildAthenaPromptSections, formatWorkspaceKnowledgeGraph, TASK_ATHENA_SYSTEM_DIRECTIVE } from '../lib/athenaContext';

export type ChatMessage = {
  id: string;
  sender: 'user' | 'assistant';
  text: string;
  timestamp: string;
};

export function buildTaskAthenaPrompt(args: {
  task: Task;
  workspace: Workspace | undefined;
  messages: ChatMessage[];
  userText: string;
  mcpData: any;
}) {
  const { task, workspace, messages, userText, mcpData } = args;

  const linkedRepo = task.colosseumConfig?.repository ? task.colosseumConfig.repository.trim() : '';

  const taskContextLines = [
    `Title: ${task.title}`,
    `ID: ${task.id}`,
    `State: ${task.state}`,
    `Priority: ${task.priority}`,
    linkedRepo ? `Linked Target Repository: ${linkedRepo}` : '',
    task.owner ? `Owner: ${task.owner}` : '',
    task.due ? `Due: ${task.due}` : '',
    task.description ? `Description:\n${task.description}` : '',
    task.dependsOn?.length ? `Depends On: ${task.dependsOn.join(', ')}` : '',
    task.comments?.length ? `Comments:\n${task.comments.map((c) => `- ${c}`).join('\n')}` : '',
  ].filter(Boolean).join('\n');

  const workspaceContextLines = workspace
    ? [
        `Workspace Name: ${workspace.name}`,
        `Workspace ID: ${workspace.id}`,
        `Status: ${workspace.status}`,
        workspace.summary ? `Summary: ${workspace.summary}` : '',
      ].filter(Boolean).join('\n')
    : `Workspace ID: ${task.workspaceId}`;

  return buildAthenaPromptSections(
    [
      ['TASK CONTEXT', taskContextLines],
      ['WORKSPACE SUMMARY', workspaceContextLines],
      ['AVAILABLE MCPS', formatGatewayMCPs(mcpData)],
      ['CONVERSATION HISTORY', formatHistory(messages.slice(0, -1))],
      ['NEW USER QUESTION', userText],
    ],
    TASK_ATHENA_SYSTEM_DIRECTIVE
  );
}

export function buildWorkspaceAthenaPrompt(args: {
  workspace: Workspace | undefined;
  workspaceId: string;
  graph: { nodes: any[]; edges: any[] };
  sessions: Session[];
  fileGroups: Record<string, any>;
  conversations: Record<string, any[]>;
  tasks: Task[];
  notes: { id: string; sessionId: string; title: string; body: string; createdAt?: string }[];
  mergeRequests: { id: string; title: string; status: string; mrId: string; createdAt?: string; updatedAt?: string }[];
  jiraTickets: { id: string; title: string; status: string; ticketKey: string; createdAt?: string; updatedAt?: string }[];
  artifacts: Artifact[];
  activitySummary: { total: number; detail: string; latest: string };
  messages: ChatMessage[];
  userText: string;
  mcpData: any;
  athenaType?: 'workspace' | 'task_manager';
}) {
  const {
    workspace,
    workspaceId,
    graph,
    sessions,
    fileGroups,
    conversations,
    tasks,
    notes,
    mergeRequests,
    jiraTickets,
    artifacts,
    activitySummary,
    messages,
    userText,
    mcpData,
    athenaType = 'workspace',
  } = args;

  return buildAthenaPromptSections([
    ...(athenaType === 'task_manager'
      ? [['TASK CONTEXT', formatTasks(tasks)]]
      : [
          ['WORKSPACE', formatWorkspace(workspace, workspaceId)],
          ['WORKSPACE KNOWLEDGE GRAPH', formatWorkspaceKnowledgeGraph(graph.nodes, graph.edges)],
          ['LINKED SESSIONS', formatSessionsFull(sessions, fileGroups, conversations)],
          ['TASK CONTEXT', formatTasks(tasks)],
          ['NOTES', formatNotes(notes, sessions)],
          ['MERGE REQUESTS', formatMergeRequests(mergeRequests)],
          ['JIRA TICKETS', formatJiraTickets(jiraTickets)],
          ['ARTIFACTS', formatArtifacts(artifacts, sessions)],
          ['ACTIVITY SUMMARY', `${activitySummary.detail}\nLatest: ${activitySummary.latest}\nTotal signals: ${activitySummary.total}`],
        ]),
    ['AVAILABLE MCPS', formatGatewayMCPs(mcpData)],
    ['CONVERSATION HISTORY', formatHistory(messages.slice(0, -1))],
    ['NEW USER QUESTION', userText],
  ]);
}

function formatWorkspace(workspace: Workspace | undefined, workspaceId: string) {
  if (!workspace) return `Workspace ID: ${workspaceId}`;
  return [
    `Name: ${workspace.name}`,
    `Workspace ID: ${workspaceId}`,
    `Status: ${workspace.status}`,
    `Priority: ${workspace.priority || 'unknown'}`,
    `Summary: ${workspace.summary || workspace.description || 'No summary available.'}`,
  ].join('\n');
}

function formatSessionsFull(sessions: Session[], fileGroups: Record<string, any>, conversations: Record<string, any[]>) {
  if (!sessions.length) return 'No sessions are linked to this workspace.';
  return sessions.map((session) => {
    const lines: string[] = [
      `### Session: ${session.title}`,
      `ID: ${session.id}`,
      `Provider: ${session.provider} / Model: ${session.model}`,
      `Updated: ${session.updatedAt || session.updated || 'unknown'}`,
      `Stats: files=${session.files} notes=${session.notes} jira=${session.jira} mrs=${session.mergeRequests}`,
    ];
    if (session.tree) lines.push(`Directory: ${session.tree}`);
    const fg = fileGroups[session.id];
    if (fg) {
      const allFiles: string[] = [];
      if (Array.isArray(fg)) {
        fg.forEach((f: any) => allFiles.push(f.path || f.name || String(f)));
      } else if (typeof fg === 'object') {
        Object.values(fg).forEach((group: any) => {
          if (Array.isArray(group)) group.forEach((f: any) => allFiles.push(f.path || f.name || String(f)));
        });
      }
      if (allFiles.length) lines.push(`Files (${allFiles.length}):\n${allFiles.slice(0, 60).map(f => `  ${f}`).join('\n')}`);
    }
    const convo = conversations[session.id];
    if (convo && convo.length) {
      lines.push(`Conversation (last ${Math.min(6, convo.length)} turns):`);
      convo.slice(-6).forEach((msg: any) => {
        const role = msg.role || msg.sender || 'unknown';
        const content = String(msg.content || msg.text || '').slice(0, 200);
        lines.push(`  [${role.toUpperCase()}] ${content}`);
      });
    }
    return lines.join('\n');
  }).join('\n\n---\n\n');
}

function formatTasks(tasks: Task[]) {
  if (!tasks.length) return 'No tasks are currently loaded for this workspace.';
  return [
    `Total tasks: ${tasks.length}`,
    '',
    ...tasks.map((task) => {
      const flags = [
        task.state ? `state=${task.state}` : '',
        task.priority ? `priority=${task.priority}` : '',
        task.owner ? `owner=${task.owner}` : '',
        task.due ? `due=${task.due}` : '',
        task.dependsOn?.length ? `dependsOn=${task.dependsOn.join(',')}` : '',
        task.comments?.length ? `comments=${task.comments.slice(-3).join(' || ')}` : '',
        task.description ? `description=${task.description}` : '',
        task.createdAt ? `createdAt=${task.createdAt}` : '',
        task.updatedAt ? `updatedAt=${task.updatedAt}` : '',
      ].filter(Boolean).join(' | ');
      return `- ${task.title} (${task.id})${flags ? ` :: ${flags}` : ''}`;
    }),
  ].join('\n');
}

function formatNotes(notes: { id: string; sessionId: string; title: string; body: string; createdAt?: string }[], sessions: Session[]) {
  if (!notes.length) return 'No notes are currently loaded for this workspace.';
  return notes.slice(0, 40).map((note) => {
    const session = sessions.find((candidate) => candidate.id === note.sessionId);
    return `- ${note.title} [${session?.title || note.sessionId}] ${note.body}`;
  }).join('\n');
}

function formatMergeRequests(items: { id: string; title: string; status: string; mrId: string }[]) {
  if (!items.length) return 'No merge requests are registered for this workspace.';
  return items.map((item) => `- ${item.mrId}: ${item.title} (${item.status})`).join('\n');
}

function formatJiraTickets(items: { id: string; title: string; status: string; ticketKey: string }[]) {
  if (!items.length) return 'No Jira tickets are registered for this workspace.';
  return items.map((item) => `- ${item.ticketKey}: ${item.title} (${item.status})`).join('\n');
}

function formatArtifacts(artifacts: Artifact[], sessions: Session[]) {
  if (!artifacts.length) return 'No artifacts are currently loaded for this workspace.';
  return artifacts.map((artifact) => {
    const session = sessions.find((candidate) => candidate.id === artifact.sessionId);
    return `- ${artifact.title} (${artifact.kind}, count=${artifact.count}) session=${session?.title || artifact.sessionId}`;
  }).join('\n');
}

function formatHistory(messages: ChatMessage[]) {
  if (!messages.length) return 'No previous messages in this workspace Athena chat.';
  return messages.map((message) => `${message.sender === 'user' ? 'User' : 'Athena'}: ${message.text}`).join('\n');
}

function formatGatewayMCPs(mcpData: any): string {
  if (!mcpData) return 'No external MCP configuration detected on the gateway.';
  try {
    return typeof mcpData === 'object' ? JSON.stringify(mcpData, null, 2) : String(mcpData);
  } catch {
    return String(mcpData);
  }
}
