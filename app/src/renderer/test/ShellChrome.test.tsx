import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ShellChrome } from '../components/ShellChrome';
import type { Workspace } from '../data';

const workspace: Workspace = {
  id: 'ws-olympus',
  name: 'Olympus',
  sessions: 3,
  tasks: 1,
  notes: 2,
  reminders: 0,
  status: 'open',
  summary: '',
  files: 0,
  knowledgeNodes: 4,
  knowledgeEdges: 5,
  nodeDensity: 1.25,
  aiIntelligence: { providers: [] },
};

describe('ShellChrome workspace tree', () => {
  it('collapses metrics into session, task, and KG-node count pills by default', () => {
    const html = renderToStaticMarkup(
      <ShellChrome
        sanctumMark="S"
        navigation={[]}
        sectionIcons={{}}
        activeSection="workspace"
        activityCount={0}
        toasts={[]}
        workspaceList={[workspace]}
        workspaceSearch=""
        onWorkspaceSearchChange={() => {}}
        workspaceIndex={0}
        onWorkspaceSelect={() => {}}
        isWorkspacePaneOpen
        toggleWorkspacePane={() => {}}
        workspaceSessions={[]}
        onAlerts={() => {}}
        onHelp={() => {}}
        onLogout={() => {}}
        onWorkspaceCreate={() => {}}
        onWorkspaceSelected={() => {}}
        onSettings={() => {}}
        rightRailItems={[]}
        footerItems={[]}
      >
        <div />
      </ShellChrome>,
    );

    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain('workspace-title-counts');
    expect(html).toContain('workspace-count-sessions');
    expect(html).toContain('workspace-count-tasks');
    expect(html).toContain('workspace-count-notes');
    expect(html).toContain('workspace-count-nodes');
    expect(html).toContain('>3</span>');
    expect(html).toContain('>1</span>');
    expect(html).toContain('>4</span>');
    expect(html).not.toContain('(3)');
    expect(html).not.toContain('workspace-tree-metrics');
  });
});
