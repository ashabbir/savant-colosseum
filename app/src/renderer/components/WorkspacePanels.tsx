import type { Artifact, Session, Task, Workspace } from '../data';
import { SessionConversation } from './SessionConversation';

type SessionStats = { label: string; value: string | number }[];
type SessionFileGroup = {
  session_id?: string;
  id?: string;
  provider?: string;
  summary?: string;
  file_count?: number;
  files?: { path?: string; name?: string; category?: string; size?: number }[];
};

export function WorkspaceSessionPanel({
  surfaceTitle,
  activeModeLabel,
  activeSection,
  activeSession,
  workspaceList,
  workspaceIndex,
  setWorkspaceIndex,
  setSessionIndex,
  workspaceSessions,
  sessionStats,
  sessionCheckpoints,
  sessionTimeline,
  sessionConversation,
  sessionNotes,
  sessionArtifacts,
  sessionFileGroup,
  setActiveSection,
  pushToast,
  taskFlags,
}: {
  surfaceTitle: string;
  activeModeLabel: string;
  activeSection: string;
  activeSession: Session;
  workspaceList: Workspace[];
  workspaceIndex: number;
  setWorkspaceIndex: (value: number) => void;
  setSessionIndex: (value: number) => void;
  workspaceSessions: Session[];
  sessionStats: SessionStats;
  sessionCheckpoints: string[];
  sessionTimeline: { id: string; time: string; title: string; detail: string; kind: string }[];
  sessionConversation: { id: string; time: string; title: string; detail: string; kind: 'user' | 'assistant' | 'tool' | 'system'; role: string; provider: string }[];
  sessionNotes: { id: string; title: string; body: string }[];
  sessionArtifacts: Artifact[];
  sessionFileGroup?: SessionFileGroup;
  setActiveSection: (section: string) => void;
  pushToast: (title: string, detail: string, tone?: 'good' | 'warning' | 'muted') => void;
  taskFlags: Record<string, { done?: boolean }>;
}) {
  return (
    <section className="panel panel-spanning">
      <PanelHeader title={surfaceTitle} action={activeModeLabel} />
      <div className="two-column-list">
        <div className="stack">
          <div className="eyebrow">Workspaces</div>
          {workspaceList.map((workspace, index) => (
            <button key={workspace.id} className={`row row-button ${index === workspaceIndex ? 'is-active' : ''}`} onClick={() => { setWorkspaceIndex(index); setSessionIndex(0); }}>
              <div><div className="row-title">{workspace.name}</div><div className="row-meta">{workspace.summary}</div></div>
              <div className="row-detail">{workspace.sessions} sessions</div>
            </button>
          ))}
        </div>
        <div className="stack">
          <div className="eyebrow">Sessions</div>
          {workspaceSessions.map((session, index) => (
            <button key={session.id} className={`row row-button ${session.id === activeSession.id ? 'is-active' : ''}`} onClick={() => { setSessionIndex(index); setActiveSection('session'); }}>
              <div className="min-w-0"><div className="row-title">{session.title}</div><div className="row-meta">{`${session.provider} · ${session.model} · ${session.updated}`}</div></div>
              <div className="row-detail">{`${session.files} files · ${session.notes} notes · ${session.jira} jira · ${session.mergeRequests} mrs`}</div>
            </button>
          ))}
        </div>
      </div>
      {activeSection === 'session' && (
        <div className="session-surface">
          <div className="session-cockpit">
            <div className="session-detail-card">
              <div className="eyebrow">Session</div>
              <div className="session-detail-title">{activeSession.title}</div>
              <div className="session-detail-subtitle">{activeSession.provider} · {activeSession.model} · {activeSession.updated}</div>
              <div className="session-detail-grid">
                <div><span>ID</span><strong>{activeSession.id}</strong></div>
                <div><span>Workspace</span><strong>{activeSession.workspaceId}</strong></div>
                <div><span>Created</span><strong>{activeSession.createdAt || 'server'}</strong></div>
                <div><span>Updated</span><strong>{activeSession.updatedAt || 'server'}</strong></div>
                <div><span>Files</span><strong>{activeSession.files}</strong></div>
                <div><span>Linked</span><strong>{activeSession.linked}</strong></div>
                <div><span>Notes</span><strong>{activeSession.notes}</strong></div>
                <div><span>Tree</span><strong>{activeSession.tree}</strong></div>
              </div>
            </div>
            <div className="cockpit-grid">
              {sessionStats.map((stat) => <article key={stat.label} className="cockpit-stat"><div className="eyebrow">{stat.label}</div><strong>{stat.value}</strong></article>)}
            </div>
            <div className="cockpit-tree"><div className="eyebrow">Tree</div><div className="cockpit-tree-value">{activeSession.tree}</div></div>
            <div className="cockpit-checkpoints"><div className="eyebrow">Checkpoints</div><div className="checkpoint-list">{sessionCheckpoints.map((checkpoint) => <div key={checkpoint} className="checkpoint-item">{checkpoint}</div>)}</div></div>
          </div>
          <div className="timeline-panel">
            <div className="eyebrow">Timeline</div>
            <div className="timeline-list">
              {sessionTimeline.map((event) => (
                <article key={event.id} className="timeline-item">
                  <div className="timeline-meta"><span className={`timeline-kind timeline-kind-${event.kind}`}>{event.kind}</span><span>{event.time}</span></div>
                  <div className="row-title">{event.title}</div>
                  <p>{event.detail}</p>
                </article>
              ))}
            </div>
          </div>
          <div className="chat-panel">
            <div className="eyebrow">Chats</div>
            <SessionConversation messages={sessionConversation} />
          </div>
          <div className="attachment-grid">
            {[
              ['Notes', sessionNotes.length, sessionNotes.map((note) => note.title).slice(0, 3).join(' · ') || 'No notes recorded.'],
              ['Files', sessionArtifacts.filter((artifact) => artifact.kind === 'file').length, sessionFileGroup?.files?.slice(0, 4).map((file) => file.path ?? file.name).filter(Boolean).join(' · ') || 'No files surfaced from the server.'],
              ['Jira', sessionArtifacts.filter((artifact) => artifact.kind === 'jira').length, sessionArtifacts.filter((artifact) => artifact.kind === 'jira').slice(0, 3).map((artifact) => artifact.title).join(' · ') || 'No Jira links.'],
              ['Merge requests', sessionArtifacts.filter((artifact) => artifact.kind === 'merge-request').length, sessionArtifacts.filter((artifact) => artifact.kind === 'merge-request').slice(0, 3).map((artifact) => artifact.title).join(' · ') || 'No merge requests.'],
            ].map(([title, count, text]) => <article key={String(title)} className="attachment-card"><div className="eyebrow">{title}</div><div className="attachment-stat">{count as number}</div><p>{text as string}</p></article>)}
          </div>
          {sessionFileGroup?.files?.length ? (
            <div className="session-file-panel">
              <div className="eyebrow">Session files</div>
              <div className="session-file-list">
                {sessionFileGroup.files.map((file) => (
                  <div key={`${file.path ?? file.name}`} className="session-file-row">
                    <span className="session-file-path">{file.path ?? file.name}</span>
                    <span className="session-file-meta">{file.category ?? 'file'}{typeof file.size === 'number' ? ` · ${file.size}b` : ''}</span>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      )}
    </section>
  );
}

export function WorkspaceManagePanel({
  manageSummary,
}: {
  manageSummary: { todayTasks: number; todayReminders: number; openTasks: number; pendingReminders: number };
}) {
  return (
    <section className="workspace-hub">
      <div className="hub-col">
        <div className="eyebrow mb-2">Today's work</div>
        <div className="h-24 w-full bg-white/[0.02] border border-white/[0.05] p-3 flex flex-col justify-between">
          <div className="flex justify-between items-end">
            <div className="flex flex-col">
              <span className="text-bg bg-cyan-500 text-[9px] font-bold px-1 py-0.5 rounded-sm w-fit mb-1">FOCUS</span>
              <span className="text-[14px] font-bold">What is active now</span>
            </div>
            <span className="text-[18px] font-mono opacity-80">{manageSummary.todayTasks + manageSummary.todayReminders}</span>
          </div>
        </div>
      </div>
    </section>
  );
}

function PanelHeader({ title, action }: { title: string; action: string }) {
  return <div className="panel-head panel-head-small"><h3>{title}</h3><button type="button" className="text-btn icon-only" aria-label={action} title={action}><span aria-hidden="true">↗</span></button></div>;
}
