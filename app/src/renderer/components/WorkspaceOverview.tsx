import type { Artifact, Session, Task } from '../data';
import { PanelHeader } from './WorkspacePrimitives';

type WorkspaceNote = {
  id: string;
  sessionId: string;
  title: string;
  body: string;
  createdAt?: string;
};

type WorkspaceActivitySummary = {
  total: number;
};

type WorkspaceSummary = {
  knowledgeNodes: number;
  knowledgeEdges: number;
};

type WorkspaceOverviewProps = {
  workspaceSessions: Session[];
  workspaceTasks: Task[];
  workspaceNotes: WorkspaceNote[];
  workspaceArtifacts: Artifact[];
  workspaceMergeRequests: { id: string; title: string; status: string; mrId: string; createdAt?: string; updatedAt?: string }[];
  workspaceJiraTickets: { id: string; title: string; status: string; ticketKey: string; createdAt?: string; updatedAt?: string }[];
  workspaceSummary: WorkspaceSummary;
  workspaceActivitySummary: WorkspaceActivitySummary;
  taskFlags: Record<string, { done?: boolean }>;
  onTaskSelect: (task: Task) => void;
  onOpenSessions: () => void;
  onOpenTasks: () => void;
  onOpenNotes: () => void;
  onOpenActivity: () => void;
  onOpenArtifacts: () => void;
  onOpenKnowledge: () => void;
  onOpenMergeRequests: () => void;
  onOpenJira: () => void;
};

export function WorkspaceOverview({
  workspaceSessions,
  workspaceTasks,
  workspaceNotes,
  workspaceArtifacts,
  workspaceMergeRequests,
  workspaceJiraTickets,
  workspaceSummary,
  workspaceActivitySummary,
  taskFlags,
  onTaskSelect,
  onOpenSessions,
  onOpenTasks,
  onOpenNotes,
  onOpenActivity,
  onOpenArtifacts,
  onOpenKnowledge,
  onOpenMergeRequests,
  onOpenJira,
}: WorkspaceOverviewProps) {
  const densityRatio = workspaceSummary.knowledgeEdges / Math.max(1, workspaceSummary.knowledgeNodes);
  const density = densityRatio > 1.5 ? 'dense' : densityRatio > 0.8 ? 'linear' : 'sparse';
  const mrItems = workspaceMergeRequests.slice(0, 4);
  const jiraItems = workspaceJiraTickets.slice(0, 4);
  const artifactRows = workspaceArtifacts.filter((artifact) => artifact.kind === 'file').slice(0, 5);
  const sessionFirstActive = formatDateLabel(getEarliestTimestamp(workspaceSessions.map((session) => session.createdAt ?? session.updatedAt)));
  const sessionLastActive = formatDateLabel(getLatestTimestamp(workspaceSessions.map((session) => session.updatedAt ?? session.createdAt)));
  const noteFirstAdded = formatDateLabel(getEarliestTimestamp(workspaceNotes.map((note) => note.createdAt)));
  const noteLastAdded = formatDateLabel(getLatestTimestamp(workspaceNotes.map((note) => note.createdAt)));
  const taskStatusCounts = workspaceTasks.reduce((acc, task) => {
    const status = taskFlags[task.id]?.done ? 'done' : task.state;
    acc[status] = (acc[status] ?? 0) + 1;
    return acc;
  }, {} as Record<string, number>);
  const taskPriorityCounts = workspaceTasks.reduce((acc, task) => {
    acc[task.priority] = (acc[task.priority] ?? 0) + 1;
    return acc;
  }, {} as Record<string, number>);
  const activityRows = [
    { label: 'Recent activity', value: 'Workspace signals', detail: 'Newest changes across this workspace', badge: `${workspaceActivitySummary.total}` },
  ];

  return (
    <section className="workspace-overview-board">
      <div className="workspace-board-row workspace-board-row-three">
        <LiveEntityStatsCard kind="merge-request" items={mrItems} title="Merge request" onOpen={onOpenMergeRequests} />
        <LiveEntityStatsCard kind="jira" items={jiraItems} title="Jira tickets" onOpen={onOpenJira} />
        <KnowledgeGraphOverviewCard density={density} edges={workspaceSummary.knowledgeEdges} nodes={workspaceSummary.knowledgeNodes} onOpen={onOpenKnowledge} />
      </div>

      <div className="workspace-board-row workspace-board-row-three">
        <article className="panel workspace-board-card stats-card">
          <PanelHeader title="Sessions" actionLabel="Open sessions" onAction={onOpenSessions} />
          <div className="kg-overview-metrics">
            <div>
              <span>Sessions</span>
              <strong>{workspaceSessions.length}</strong>
            </div>
            <div>
              <span>First active</span>
              <strong>{sessionFirstActive}</strong>
            </div>
            <div>
              <span>Last active</span>
              <strong>{sessionLastActive}</strong>
            </div>
          </div>
          <div className="kg-density-track">
            <div className="kg-density-fill" style={{ width: `${Math.min(100, workspaceSessions.length * 18)}%` }} />
          </div>
        </article>

        <article className="panel workspace-board-card stats-card">
          <PanelHeader title="Task" actionLabel="Open tasks" onAction={onOpenTasks} />
          <div className="task-summary-grid">
            <div className="task-summary-group">
              <div className="task-summary-label">Status</div>
              <div className="task-summary-chips">
                {['backlog', 'ready', 'in-progress', 'review', 'done'].map((status) => (
                  <div key={status} className={`task-summary-chip task-summary-${status}`}>
                    <span>{status}</span>
                    <strong>{taskStatusCounts[status] ?? 0}</strong>
                  </div>
                ))}
              </div>
            </div>
            <div className="task-summary-group">
              <div className="task-summary-label">Priority</div>
              <div className="task-summary-chips">
                {['critical', 'high', 'medium', 'low'].map((priority) => (
                  <div key={priority} className={`task-summary-chip task-summary-${priority}`}>
                    <span>{priority}</span>
                    <strong>{taskPriorityCounts[priority] ?? 0}</strong>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </article>

        <article className="panel workspace-board-card stats-card">
          <PanelHeader title="Notes" actionLabel="Open notes" onAction={onOpenNotes} />
          <div className="kg-overview-metrics">
            <div>
              <span>Notes</span>
              <strong>{workspaceNotes.length}</strong>
            </div>
            <div>
              <span>First added</span>
              <strong>{noteFirstAdded}</strong>
            </div>
            <div>
              <span>Last added</span>
              <strong>{noteLastAdded}</strong>
            </div>
          </div>
          <div className="kg-density-track">
            <div className="kg-density-fill" style={{ width: `${Math.min(100, workspaceNotes.length * 18)}%` }} />
          </div>
        </article>
      </div>

      <div className="workspace-board-row workspace-board-row-two">
        <article className="panel workspace-board-card">
          <PanelHeader title="Activity" actionLabel="Open activity" onAction={onOpenActivity} />
          <div className="workspace-list">
            {activityRows.map((item) => (
              <div key={item.label} className="workspace-list-item workspace-list-item-stack">
                <div className="workspace-list-main">
                  <div className="row-title">{item.label}</div>
                  <div className="row-meta">{item.value}</div>
                  <div className="row-detail">{item.detail}</div>
                </div>
                <div className="workspace-card-badge">{item.badge}</div>
              </div>
            ))}
          </div>
        </article>
        <ArtifactCard artifacts={artifactRows} onOpen={onOpenArtifacts} />
      </div>
    </section>
  );
}

function KnowledgeGraphOverviewCard({ density, edges, nodes, onOpen }: { density: string; edges: number; nodes: number; onOpen: () => void }) {
  const densityTone = density === 'dense' ? 'red' : density === 'linear' ? 'yellow' : 'green';

  return (
    <article className="panel workspace-board-card kg-overview-card">
      <PanelHeader title="Knowledge graph" actionLabel="Open knowledge graph" onAction={onOpen} />
      <div className="kg-overview-metrics">
        <div>
          <span>Nodes</span>
          <strong>{nodes}</strong>
        </div>
        <div>
          <span>Edges</span>
          <strong>{edges}</strong>
        </div>
        <div>
          <span>Density</span>
          <strong className={`kg-density-pill kg-density-${densityTone}`}>{density}</strong>
        </div>
      </div>
      <div className={`kg-density-track kg-density-${densityTone}`}>
        <div className="kg-density-fill" style={{ width: `${Math.min(100, (edges / Math.max(1, nodes)) * 100)}%` }} />
      </div>
    </article>
  );
}

function ArtifactCard({ artifacts, onOpen }: { artifacts: Artifact[]; onOpen: () => void }) {
  return (
    <article className="panel workspace-board-card">
      <PanelHeader title="Artifacts" actionLabel="Open artifacts" onAction={onOpen} />
      <div className="workspace-list">
        {artifacts.length > 0 ? artifacts.map((artifact) => (
          <div key={artifact.id} className="workspace-list-item">
            <div>
              <div className="row-title">{artifact.title}</div>
              <div className="row-meta">session file</div>
            </div>
            <div className="workspace-list-mini">{artifact.updated}</div>
          </div>
        )) : null}
      </div>
    </article>
  );
}

function LiveEntityStatsCard({
  items,
  kind,
  title,
  onOpen,
}: {
  items: { title: string; status: string; mrId?: string; ticketKey?: string }[];
  kind: 'merge-request' | 'jira';
  title: string;
  onOpen: () => void;
}) {
  const total = items.length;
  const openCount = items.filter((item) => item.status === 'open').length;
  const closedCount = items.filter((item) => item.status === 'closed').length;
  const mergedCount = items.filter((item) => item.status === 'merged').length;
  const densityTone = total > 8 ? 'red' : total > 3 ? 'yellow' : 'green';
  return (
    <article className="panel workspace-board-card stats-card">
      <PanelHeader title={title} actionLabel={`Open ${title.toLowerCase()}`} onAction={onOpen} />
      <div className="kg-overview-metrics">
        <div>
          <span>Total</span>
          <strong>{total}</strong>
        </div>
        <div>
          <span>Open</span>
          <strong>{openCount}</strong>
        </div>
        <div>
          <span>Status</span>
          <strong className={`kg-density-pill kg-density-${densityTone}`}>{kind === 'merge-request' ? `${mergedCount} merged` : `${closedCount} closed`}</strong>
        </div>
      </div>
    </article>
  );
}

function getEarliestTimestamp(values: Array<string | undefined>) {
  return values
    .filter((value): value is string => Boolean(value && !Number.isNaN(Date.parse(value))))
    .sort((a, b) => Date.parse(a) - Date.parse(b))[0];
}

function getLatestTimestamp(values: Array<string | undefined>) {
  return values
    .filter((value): value is string => Boolean(value && !Number.isNaN(Date.parse(value))))
    .sort((a, b) => Date.parse(b) - Date.parse(a))[0];
}

function formatDateLabel(value?: string) {
  if (!value) return 'Unknown';
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) return 'Unknown';
  return new Date(timestamp).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}
