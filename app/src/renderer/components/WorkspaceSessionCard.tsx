import type { MouseEvent } from 'react';
import { Trash2 } from 'lucide-react';
import type { Session } from '../data';
import type { SessionFileGroup } from '../services/sessionAdapters';

type WorkspaceSessionCardProps = {
  session: Session;
  active: boolean;
  files?: SessionFileGroup;
  onSelect: (sessionId: string) => void;
  onDelete?: (sessionId: string) => void;
};

function statusTone(provider: string) {
  const key = provider.trim().toLowerCase();
  if (key === 'codex') return 'green';
  if (key === 'claude') return 'yellow';
  if (key === 'copilot') return 'cyan';
  if (key === 'gemini') return 'purple';
  if (key === 'agy' || key === 'agt') return 'orange';
  if (key === 'hermes') return 'magenta';
  return 'grey';
}

function getResumeCommand(session: Session, files?: SessionFileGroup) {
  const firstPath = files?.filePath ?? files?.file_path ?? files?.files?.find((file) => file.path || file.name)?.path ?? files?.files?.find((file) => file.path || file.name)?.name ?? '';
  const sessionDir = firstPath ? firstPath.split('/').slice(0, -1).join('/') : '';
  const provider = session.provider.trim().toLowerCase() || 'codex';
  const sessionTarget = session.id.trim();
  if (provider === 'agy' || provider === 'agt') {
    return `cd ${sessionDir || '.'} && agy --conversation ${sessionTarget} --prompt-interactive --dangerously-skip-permissions`;
  }
  if (provider === 'hermes') return `cd ${sessionDir || '.'} && hermes --session ${sessionTarget}`;
  return `cd ${sessionDir || '.'} && ${provider} start --session ${sessionTarget} --yolo`;
}

export function WorkspaceSessionCard({ session, active, files, onSelect, onDelete }: WorkspaceSessionCardProps) {
  const resumeCommand = getResumeCommand(session, files);

  const handleResume = async (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    try {
      await navigator.clipboard.writeText(resumeCommand);
    } catch {
      // Clipboard copy can fail in restricted environments.
    }
  };

  return (
    <div
      className={`note-drawer-card session-drawer-card ${active ? 'is-active' : ''}`}
      role="button"
      tabIndex={0}
      onClick={() => onSelect(session.id)}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          onSelect(session.id);
        }
      }}
    >
      <div className="session-drawer-card-head">
        <div className="note-drawer-title truncate">{session.title}</div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexShrink: 0 }}>
          <button
            type="button"
            className="ghost-btn icon-only session-resume-btn"
            aria-label="Copy resume command"
            title={resumeCommand}
            onClick={handleResume}
          >
            <span aria-hidden="true">↻</span>
          </button>
          {onDelete && (
            <button
              type="button"
              className="ghost-btn icon-only"
              style={{ color: '#ff2244' }}
              aria-label="Delete session"
              title="Delete session"
              onClick={(e) => {
                e.stopPropagation();
                onDelete(session.id);
              }}
            >
              <Trash2 size={12} />
            </button>
          )}
        </div>
      </div>
      <div className="note-drawer-meta truncate">{session.provider} · {session.agentType || session.model}</div>
      <p className="session-drawer-summary">{session.tree}</p>
      <div className="workspace-list-row session-drawer-row">
        <span className={`workspace-card-badge workspace-card-badge-${statusTone(session.provider)}`}>{session.updated}</span>
        <span className="workspace-list-chip">{session.files} files</span>
        <span className="workspace-list-chip">{session.notes} notes</span>
      </div>
    </div>
  );
}
