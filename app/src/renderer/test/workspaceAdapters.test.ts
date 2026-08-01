import { describe, expect, it } from 'vitest';
import { normalizeServerWorkspace } from '../services/workspaceAdapters';

describe('normalizeServerWorkspace', () => {
  it('converts the live attached-session array into a render-safe count', () => {
    const workspace = normalizeServerWorkspace({
      workspace_id: '17840847469787888397441',
      name: 'Savant Forge Planning Workflow',
      status: 'open',
      sessions: [
        { attached_at: '2026-07-16', provider: 'session', session_id: 'one', workspace_id: '17840847469787888397441' },
        { attached_at: '2026-07-15', provider: 'session', session_id: 'two', workspace_id: '17840847469787888397441' },
      ],
      session_count: 2,
      task_count: 4,
      note_count: 3,
      kg_stats: { total_nodes: 8, total_edges: 12, staged_count: 0, nodes_by_type: {} },
    });

    expect(workspace?.id).toBe('17840847469787888397441');
    expect(workspace?.sessions).toBe(2);
    expect(workspace?.counts?.total).toBe(2);
    expect(workspace?.tasks).toBe(4);
    expect(workspace?.notes).toBe(3);
    expect(workspace?.knowledgeNodes).toBe(8);
    expect(workspace?.nodeDensity).toBe(1.5);
  });
});
