import type { Workspace } from '../data';

type ServerWorkspace = Omit<Partial<Workspace>, 'sessions'> & {
  workspace_id?: string;
  workspaceId?: string;
  session_count?: number;
  sessions?: number | unknown[];
  task_count?: number;
  reminder_count?: number;
  note_count?: number;
  file_count?: number;
};

function numericCount(value: unknown, fallback = 0): number {
  if (Array.isArray(value)) return value.length;
  const count = Number(value);
  return Number.isFinite(count) ? count : fallback;
}

export function normalizeServerWorkspace(raw: ServerWorkspace): Workspace | null {
  const id = String(raw.id ?? raw.workspace_id ?? raw.workspaceId ?? '').trim();
  if (!id) return null;

  const sessions = numericCount(raw.session_count ?? raw.sessions);
  return {
    ...raw,
    id,
    name: String(raw.name || id),
    status: raw.status ?? 'open',
    sessions,
    counts: raw.counts ? { ...raw.counts, total: numericCount(raw.counts.total, sessions) } : { total: sessions },
    tasks: numericCount(raw.tasks ?? raw.task_count),
    notes: numericCount(raw.notes ?? raw.note_count),
    reminders: numericCount(raw.reminders ?? raw.reminder_count),
    summary: String(raw.summary ?? raw.description ?? ''),
    files: numericCount(raw.files ?? raw.file_count),
    knowledgeNodes: numericCount(raw.knowledgeNodes ?? raw.kg_stats?.total_nodes),
    knowledgeEdges: numericCount(raw.knowledgeEdges ?? raw.kg_stats?.total_edges),
    nodeDensity: numericCount(raw.nodeDensity, numericCount(raw.knowledgeEdges ?? raw.kg_stats?.total_edges) / Math.max(1, numericCount(raw.knowledgeNodes ?? raw.kg_stats?.total_nodes))),
    aiIntelligence: raw.aiIntelligence ?? { providers: [] },
  };
}
