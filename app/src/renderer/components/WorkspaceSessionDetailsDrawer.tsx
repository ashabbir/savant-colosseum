import { useEffect, useMemo, useState } from 'react';
import { FileText } from 'lucide-react';
import type { Artifact, Note, Session } from '../data';
import { SessionConversation } from './SessionConversation';
import type { SessionConversationMessage, SessionFileGroup } from '../services/sessionAdapters';

type SessionStat = {
  label: string;
  value: string | number;
};

type WorkspaceSessionDetailsDrawerProps = {
  open: boolean;
  workspaceName: string;
  session: Session | null;
  stats: SessionStat[];
  files?: SessionFileGroup;
  conversation: SessionConversationMessage[];
  notes: Note[];
  artifacts: Artifact[];
  onSelectNote: (note: Note) => void;
  onBack: () => void;
  onClose: () => void;
};

export function WorkspaceSessionDetailsDrawer({ open, workspaceName, session, stats, files, conversation, notes, artifacts, onSelectNote, onBack, onClose }: WorkspaceSessionDetailsDrawerProps) {
  const [activeTab, setActiveTab] = useState<'overview' | 'chat' | 'files' | 'notes' | 'artifacts'>('overview');
  const selectedArtifactKinds = useMemo(() => ({
    file: artifacts.filter((artifact) => artifact.kind === 'file'),
    jira: artifacts.filter((artifact) => artifact.kind === 'jira'),
    mr: artifacts.filter((artifact) => artifact.kind === 'merge-request'),
  }), [artifacts]);

  useEffect(() => {
    if (!open) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, open]);

  useEffect(() => {
    if (open) setActiveTab('overview');
  }, [open, session?.id]);

  const conversationCounts = conversation.reduce((acc, message) => {
    acc[message.kind] = (acc[message.kind] ?? 0) + 1;
    return acc;
  }, {} as Record<SessionConversationMessage['kind'], number>);
  const totalConversation = conversation.length || 1;
  const chartItems = [
    { label: 'User', value: conversationCounts.user ?? 0, tone: 'yellow' },
    { label: 'Assistant', value: conversationCounts.assistant ?? 0, tone: 'green' },
    { label: 'Tool', value: conversationCounts.tool ?? 0, tone: 'cyan' },
    { label: 'System', value: conversationCounts.system ?? 0, tone: 'grey' },
  ] as const;
  if (!open || !session) return null;

  return (
    <div className="activity-drawer-backdrop" onClick={onClose}>
      <aside className="knowledge-drawer session-details-drawer" onClick={(event) => event.stopPropagation()} aria-label="Workspace session details">
        <div className="workspace-drawer-head">
          <div className="flex items-center gap-3 min-w-0">
            <button
              type="button"
              className="ghost-btn icon-only"
              aria-label="Back to sessions"
              title="Back to sessions"
              onClick={onBack}
            >
              <span aria-hidden="true">←</span>
            </button>
            <div className="min-w-0">
              <h2 className="brand-title !text-base !tracking-wider truncate">{session.title}</h2>
              <div className="workspace-drawer-subtitle truncate">{workspaceName} · {session.provider} · {session.agentType || session.model}</div>
            </div>
          </div>
          <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={onClose}>
            <svg aria-hidden="true" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M18 6 6 18" />
              <path d="m6 6 12 12" />
            </svg>
          </button>
        </div>

        <div className="session-tabbar">
          {[
            ['overview', 'Overview'],
            ['chat', 'Chat'],
            ['files', 'Files'],
            ['notes', 'Notes'],
            ['artifacts', 'Artifacts'],
          ].map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`session-tab ${activeTab === id ? 'is-active' : ''}`}
              onClick={() => setActiveTab(id as typeof activeTab)}
            >
              {label}
            </button>
          ))}
        </div>

        <div className={`workspace-drawer-body session-details-drawer-body session-details-tab-${activeTab}`}>
          {activeTab === 'overview' ? (
            <div className="session-overview-layout">
              <section className="session-detail-card">
                <div className="eyebrow">Model and provider</div>
                <div className="session-overview-hero">
                  <div>
                    <div className="session-detail-title">{session.title}</div>
                    <div className="session-detail-subtitle">{session.provider} · {session.model}</div>
                  </div>
                  <div className="session-overview-badges">
                    <span className="workspace-header-pill workspace-header-pill-high">{session.provider}</span>
                    <span className="workspace-header-pill workspace-header-pill-open">{session.model}</span>
                  </div>
                </div>
                <div className="eyebrow">Session stats</div>
                <div className="session-detail-grid session-details-stats-grid">
                  {stats.map((stat) => (
                    <div key={stat.label}>
                      <span>{stat.label}</span>
                      <strong>{stat.value}</strong>
                    </div>
                  ))}
                </div>
              </section>

              <section className="session-detail-card">
                <div className="eyebrow">Conversation mix</div>
                <div className="session-chart-stack">
                  {chartItems.map((item) => {
                    const width = `${Math.max(8, Math.round((item.value / totalConversation) * 100))}%`;
                    return (
                      <div key={item.label} className="session-chart-row">
                        <div className="session-chart-label">
                          <span>{item.label}</span>
                          <strong>{item.value}</strong>
                        </div>
                        <div className={`session-chart-track session-chart-${item.tone}`}>
                          <div className="session-chart-fill" style={{ width }} />
                        </div>
                      </div>
                    );
                  })}
                </div>
              </section>

              <section className="session-detail-card">
                <div className="eyebrow">Session map</div>
                <div className="session-tree-view">
                  <div className="session-tree-node is-root">{workspaceName}</div>
                  <div className="session-tree-connector" />
                  <div className="session-tree-node">{session.provider}</div>
                  <div className="session-tree-connector" />
                  <div className="session-tree-node">{session.model}</div>
                  <div className="session-tree-connector" />
                  <div className="session-tree-node">{session.tree}</div>
                </div>
              </section>

              <section className="session-detail-card">
                <div className="eyebrow">Session info</div>
                <div className="session-detail-grid">
                  <div><span>ID</span><strong>{session.id}</strong></div>
                  <div><span>Workspace</span><strong>{session.workspaceId}</strong></div>
                  <div><span>Updated</span><strong>{session.updated || session.updatedAt || 'server'}</strong></div>
                  <div><span>Created</span><strong>{session.createdAt || 'server'}</strong></div>
                  <div><span>Provider</span><strong>{session.provider}</strong></div>
                  <div><span>Agent type</span><strong>{session.agentType || 'not reported'}</strong></div>
                  <div><span>Mode</span><strong>{stats.find((stat) => stat.label === 'Mode')?.value ?? 'unknown'}</strong></div>
                  <div><span>Usage</span><strong>{stats.find((stat) => stat.label === 'Usage')?.value ?? '0%'}</strong></div>
                </div>
              </section>
            </div>
          ) : null}

          {activeTab === 'chat' ? (
            <section className="session-detail-card session-scroll-card">
              <div className="eyebrow">Full chat</div>
              {conversation.length > 0 ? (
                <SessionConversation messages={conversation} />
              ) : (
                <div className="activity-empty">No conversation history available for this session.</div>
              )}
            </section>
          ) : null}

          {activeTab === 'files' ? (
            <section className="session-detail-card session-scroll-card">
              <div className="eyebrow">Files</div>
              {files?.files?.length ? (
                <div className="session-file-list">
                  {files.files.map((file) => (
                    <div key={`${file.path ?? file.name}`} className="session-file-row">
                      <span className="session-file-path">{file.path ?? file.name}</span>
                      <span className="session-file-meta">{file.category ?? 'file'}{typeof file.size === 'number' ? ` · ${file.size}b` : ''}</span>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="activity-empty">No session files were returned by the server.</div>
              )}
            </section>
          ) : null}

          {activeTab === 'notes' ? (
            <section className="session-detail-card session-scroll-card">
              <div className="workspace-drawer-head session-tab-head">
                <div className="flex items-center gap-3">
                  <FileText size={18} className="text-cyan-400" />
                  <div>
                    <div className="eyebrow">Notes</div>
                    <div className="workspace-drawer-subtitle">{notes.length} notes · session scope</div>
                  </div>
                </div>
              </div>
              {notes.length > 0 ? (
                <div className="notes-drawer-grid">
                  {notes.map((note) => (
                    <button key={note.id} type="button" className="note-drawer-card" onClick={() => onSelectNote(note)}>
                      <div className="note-drawer-title">{note.title}</div>
                      <div className="note-drawer-meta">{note.createdAt || 'server'}</div>
                      <p>{note.body}</p>
                    </button>
                  ))}
                </div>
              ) : (
                <div className="activity-empty">No notes found for this session.</div>
              )}
            </section>
          ) : null}

          {activeTab === 'artifacts' ? (
            <section className="session-detail-card session-scroll-card">
              <div className="eyebrow">Artifacts</div>
              <div className="session-detail-grid">
                <div><span>Total</span><strong>{artifacts.length}</strong></div>
                <div><span>Files</span><strong>{selectedArtifactKinds.file.length}</strong></div>
                <div><span>Jira</span><strong>{selectedArtifactKinds.jira.length}</strong></div>
                <div><span>Merge requests</span><strong>{selectedArtifactKinds.mr.length}</strong></div>
              </div>
              {artifacts.length > 0 ? (
                <div className="session-detail-stack">
                  {artifacts.map((artifact) => (
                    <div key={artifact.id} className="management-drawer-row">
                      <div className="flex flex-col min-w-0 flex-1">
                        <strong className="truncate">{artifact.title}</strong>
                        <span className="text-xs opacity-60 truncate">{artifact.kind}</span>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="activity-empty">No artifacts linked to this session.</div>
              )}
            </section>
          ) : null}
        </div>
      </aside>
    </div>
  );
}
