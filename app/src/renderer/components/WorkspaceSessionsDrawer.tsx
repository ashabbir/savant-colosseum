import { useEffect } from 'react';
import type { Session } from '../data';
import type { SessionFileGroup } from '../services/sessionAdapters';
import { WorkspaceSessionCard } from './WorkspaceSessionCard';

type WorkspaceSessionsDrawerProps = {
  open: boolean;
  workspaceName: string;
  sessions: Session[];
  sessionFileGroups?: Record<string, SessionFileGroup>;
  activeSessionId: string;
  onSelectSession: (sessionId: string) => void;
  onDeleteSession?: (sessionId: string) => void;
  onClose: () => void;
};

export function WorkspaceSessionsDrawer({ open, workspaceName, sessions, sessionFileGroups = {}, activeSessionId, onSelectSession, onDeleteSession, onClose }: WorkspaceSessionsDrawerProps) {
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

  if (!open) return null;

  return (
    <div className="activity-drawer-backdrop" onClick={onClose}>
      <aside className="knowledge-drawer" onClick={(event) => event.stopPropagation()} aria-label="Workspace sessions">
        <div className="workspace-drawer-head">
          <div className="flex items-center gap-3">
            <svg aria-hidden="true" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" className="text-cyan-400">
              <path d="M4 12a8 8 0 1 0 8-8" />
              <path d="M4 4v4h4" />
              <path d="M12 7v5l3 2" />
            </svg>
            <div>
              <h2 className="brand-title !text-base !tracking-wider">Sessions</h2>
              <div className="workspace-drawer-subtitle">{workspaceName} · {sessions.length} sessions</div>
            </div>
          </div>
          <button className="text-btn action-close icon-only" aria-label="Close" title="Close" onClick={onClose}>
            <svg aria-hidden="true" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M18 6 6 18" />
              <path d="m6 6 12 12" />
            </svg>
          </button>
        </div>

        <div className="workspace-drawer-body">
          <div className="notes-drawer-grid sessions-drawer-grid">
            {sessions.length > 0 ? sessions.map((session) => (
              <WorkspaceSessionCard
                key={session.id}
                session={session}
                files={sessionFileGroups[session.id]}
                active={session.id === activeSessionId}
                onSelect={onSelectSession}
                onDelete={onDeleteSession}
              />
            )) : (
              <div className="activity-empty">No sessions in this workspace.</div>
            )}
          </div>
        </div>
      </aside>
    </div>
  );
}
