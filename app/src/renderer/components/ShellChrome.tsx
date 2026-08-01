import { useState, type ReactNode } from 'react';
import { Bell, ChevronDown, ChevronLeft, ChevronRight, Folder, Keyboard, ListChecks, LogOut, Plus, Settings } from 'lucide-react';
import type { Session, Workspace } from '../data';
import { BottomBar } from './BottomBar';

type NavItem = {
  id: string;
  label: string;
  icon: ReactNode;
};

type ToastLike = {
  id: string;
  title: string;
  detail: string;
  tone: 'good' | 'warning' | 'muted';
};

type ActivityItem = {
  id: string;
  label: string;
  detail: string;
  section: string;
  surfaceMode: string;
  workspaceIndex: number;
  sessionIndex: number;
};

type ShellChromeProps = {
  sanctumMark: string;
  navigation: NavItem[];
  sectionIcons: Record<string, ReactNode>;
  activeSection: string;
  activityCount: number;
  toasts: ToastLike[];
  workspaceList: Workspace[];
  isWorkspaceLoading?: boolean;
  workspaceSearch: string;
  onWorkspaceSearchChange: (value: string) => void;
  workspaceIndex: number;
  onWorkspaceSelect: (index: number) => void;
  isWorkspacePaneOpen: boolean;
  toggleWorkspacePane: () => void;
  workspaceSessions: Session[];
  athenaProvider?: string;
  athenaModel?: string;
  onAlerts: () => void;
  onHelp: () => void;
  onLogout: () => void;
  onWorkspaceCreate: () => void;
  onWorkspaceSelected: () => void;
  onSettings: () => void;
  rightRailItems: { id: string; label: string; icon: ReactNode; active: boolean; action: () => void }[];
  children: ReactNode;
  footerItems: { label: string; status: string; detail: string }[];
};

export function ShellChrome({
  sanctumMark,
  navigation,
  sectionIcons,
  activeSection,
  activityCount,
  toasts,
  workspaceList,
  isWorkspaceLoading = false,
  workspaceSearch,
  onWorkspaceSearchChange,
  workspaceIndex,
  onWorkspaceSelect,
  isWorkspacePaneOpen,
  toggleWorkspacePane,
  workspaceSessions,
  athenaProvider,
  athenaModel,
  onAlerts,
  onHelp,
  onLogout,
  onWorkspaceCreate,
  onWorkspaceSelected,
  onSettings,
  rightRailItems,
  children,
  footerItems,
}: ShellChromeProps) {
  const [expandedWorkspaceIds, setExpandedWorkspaceIds] = useState<Set<string>>(() => new Set());
  const filteredWorkspaces = workspaceList.filter((ws) => ws.name.toLowerCase().includes(workspaceSearch.toLowerCase()));
  const showWorkspacePane = isWorkspacePaneOpen && activeSection !== 'tasks';

  return (
    <div className="app-shell">
      <header className="topbar-shell">
        <div style={{ background: 'var(--cp-bg-1)', borderBottom: '1px solid var(--cp-border)', boxShadow: '0 1px 0 rgba(0,229,255,0.08)' }} className="flex items-center justify-between px-3 h-10 w-full shrink-0">
          <div className="w-24" />
          <div className="flex items-center gap-2">
            <div style={{ background: 'linear-gradient(135deg, var(--cp-cyan), var(--cp-purple))', boxShadow: 'var(--cp-glow-cyan)' }} className="w-6 h-6 flex items-center justify-center overflow-hidden">
              <img src={sanctumMark} alt="Savant" className="w-full h-full object-contain" />
            </div>
            <span style={{ fontFamily: "'Orbitron', monospace", color: 'var(--cp-cyan)', textShadow: 'var(--cp-glow-cyan)', letterSpacing: '0.15em' }} className="text-sm font-bold uppercase tracking-widest">sanctum</span>
          </div>
          <div className="flex items-center gap-2 w-24 justify-end"><span className="text-[10px] opacity-40 font-mono">OP_CON</span></div>
        </div>
      </header>

      <div className="toast-stack" aria-live="polite" aria-atomic="true">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast toast-${toast.tone}`}>
            <strong>{toast.title}</strong>
            <span>{toast.detail}</span>
          </div>
        ))}
      </div>

      <div className={`shell-body ${showWorkspacePane ? 'left-pane-open' : 'left-pane-collapsed'} ${activeSection === 'tasks' ? 'task-mode' : ''}`}>
        <aside className="rail rail-left">
          <div className="rail-group">
            <button className={`nav-item ${activeSection === 'workspace' ? 'is-active' : ''}`} onClick={toggleWorkspacePane} title="Workspaces" aria-label="Workspaces">
              <span className="nav-icon" aria-hidden="true">{sectionIcons.workspace || <Folder size={14} />}</span>
            </button>
            <button className={`nav-item ${activeSection === 'tasks' ? 'is-active' : ''}`} onClick={() => onWorkspaceSelected()} title="Task manager" aria-label="Task manager">
              <span className="nav-icon" aria-hidden="true"><ListChecks size={14} /></span>
            </button>
          </div>
          <div className="rail-spacer" />
          <div className="rail-group rail-group-bottom">
            <button className="nav-item" onClick={onAlerts} title="Alerts" aria-label="Alerts"><span className="nav-icon" aria-hidden="true"><Bell size={14} /></span><span className="nav-count">{Math.max(activityCount, toasts.length)}</span></button>
            <button className="nav-item" onClick={onHelp} title="Shortcuts" aria-label="Shortcuts"><span className="nav-icon" aria-hidden="true"><Keyboard size={14} /></span></button>
            <button className={`nav-item ${activeSection === 'settings' ? 'is-active' : ''}`} onClick={onSettings} title="Preferences" aria-label="Preferences"><span className="nav-icon" aria-hidden="true"><Settings size={14} /></span></button>
            <button className="nav-item" onClick={onLogout} title="Logout" aria-label="Logout"><span className="nav-icon" aria-hidden="true"><LogOut size={14} /></span></button>
          </div>
        </aside>

        {activeSection !== 'tasks' && (
        <aside className={`workspace-pane ${isWorkspacePaneOpen ? 'is-open' : 'is-collapsed'}`}>
          {!isWorkspacePaneOpen ? (
            <>
              <button
                type="button"
                className="workspace-pane-top-toggle"
                onClick={toggleWorkspacePane}
                title="Expand workspaces panel"
                aria-label="Expand workspaces panel"
              >
                <ChevronRight size={14} />
              </button>
              <button
                type="button"
                className="workspace-pane-bar"
                onClick={toggleWorkspacePane}
                title="Expand workspaces panel"
                aria-label="Expand workspaces panel"
              >
                Workspaces
              </button>
            </>
          ) : (
            <>
              <div className="workspace-pane-head">
                <span>Workspaces</span>
                <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                  <button
                    type="button"
                    onClick={toggleWorkspacePane}
                    title="Collapse workspaces panel"
                    aria-label="Collapse workspaces panel"
                    style={{
                      background: 'none',
                      border: 'none',
                      color: 'var(--cyan)',
                      cursor: 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      padding: '0'
                    }}
                  >
                    <ChevronLeft size={14} />
                  </button>
                </div>
              </div>
              <div className="workspace-search-container">
                <input type="text" placeholder="Search workspaces..." className="workspace-search-input" value={workspaceSearch} onChange={(e) => onWorkspaceSearchChange(e.target.value)} />
              </div>
              <div className="workspace-pane-list">
                {filteredWorkspaces.length > 0 ? filteredWorkspaces.map((workspace) => {
                  const index = workspaceList.findIndex((ws) => ws.id === workspace.id);
                  const density = Number(workspace.nodeDensity ?? (workspace.knowledgeEdges / Math.max(1, workspace.knowledgeNodes)));
                  const isExpanded = expandedWorkspaceIds.has(workspace.id);
                  return (
                    <div key={workspace.id} className={`workspace-pane-item ${index === workspaceIndex ? 'is-active' : ''}`}>
                      <div className="workspace-tree-node">
                        <button
                          type="button"
                          className="workspace-tree-toggle"
                          onClick={() => setExpandedWorkspaceIds((current) => {
                            const next = new Set(current);
                            if (next.has(workspace.id)) next.delete(workspace.id);
                            else next.add(workspace.id);
                            return next;
                          })}
                          aria-expanded={isExpanded}
                          aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${workspace.name} workspace statistics`}
                        >
                          {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                        </button>
                        <Folder size={14} className="tree-icon" />
                        <button type="button" className="workspace-tree-select" onClick={() => onWorkspaceSelect(index)}>
                          <span className="workspace-pane-name">{workspace.name}</span>
                          {!isExpanded && (
                            <span className="workspace-title-counts" aria-label={`${workspace.sessions} sessions, ${workspace.tasks} tasks, ${workspace.notes ?? 0} notes, ${workspace.knowledgeNodes} knowledge graph nodes`}>
                              <span className="workspace-count-sessions" title="Sessions">{workspace.sessions}</span>
                              <span className="workspace-count-tasks" title="Tasks">{workspace.tasks}</span>
                              <span className="workspace-count-notes" title="Notes">{workspace.notes ?? 0}</span>
                              <span className="workspace-count-nodes" title="KG nodes">{workspace.knowledgeNodes}</span>
                            </span>
                          )}
                        </button>
                      </div>
                      {isExpanded && <div className="workspace-tree-metrics" aria-label={`${workspace.sessions} sessions, ${workspace.tasks} tasks, ${workspace.notes ?? 0} notes, ${workspace.knowledgeNodes} knowledge graph nodes, ${density.toFixed(2)} node density`}>
                        <span><b>{workspace.sessions}</b> sessions</span>
                        <span><b>{workspace.tasks}</b> tasks</span>
                        <span><b>{workspace.notes ?? 0}</b> notes</span>
                        <span><b>{workspace.knowledgeNodes}</b> KG nodes</span>
                        <span><b>{density.toFixed(2)}</b> density</span>
                      </div>}
                    </div>
                  );
                }) : (
                  <div className="workspace-pane-empty">
                    <div className="eyebrow">Server data</div>
                    <div className="text-sm opacity-70">
                      {isWorkspaceLoading ? 'Loading workspaces from server...' : workspaceSearch ? 'No workspaces match this search.' : 'No workspaces returned from the server.'}
                    </div>
                  </div>
                )}
              </div>
            </>
          )}
        </aside>
        )}

        {children}

        <aside className="rail rail-right">
          <div className="rail-group">{rightRailItems.map((item) => (<button key={item.id} type="button" className={`nav-item ${item.id === 'global-task-create' ? 'nav-item-add' : ''} ${item.active ? 'is-active' : ''}`} onClick={item.action} title={item.label} aria-label={item.label}><span className="nav-icon">{item.icon}</span></button>))}</div>
          <div className="rail-spacer" />
        </aside>
      </div>

      <BottomBar
        workspaces={workspaceList}
        sessions={workspaceSessions}
        activeModel={athenaModel}
        activeProvider={athenaProvider}
      />
    </div>
  );
}
