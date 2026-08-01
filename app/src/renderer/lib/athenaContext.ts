import { buildSavantHeaders } from '../services/httpClient';

export type AthenaKnowledgeNode = {
  node_id?: string;
  id?: string;
  title?: string;
  node_type?: string;
  status?: string;
  content?: string;
  metadata?: Record<string, any>;
};

export type AthenaKnowledgeEdge = {
  source_id?: string;
  target_id?: string;
  source?: string | { id?: string; node_id?: string };
  target?: string | { id?: string; node_id?: string };
  edge_type?: string;
  label?: string;
};

export const ATHENA_SYSTEM_DIRECTIVE = [
  'You are ATHENA inside Savant Sanctum.',
  'Ground every answer in the current workspace knowledge graph, linked sessions, tasks, notes, merge requests, Jira tickets, and artifacts provided in the prompt.',
  'Always treat the full persisted chat history and task context as part of the working conversation state.',
  'Treat the workspace context as authoritative for this conversation.',
  'If the context is incomplete, say exactly what is missing and answer from the available evidence.',
  'Keep responses extremely concise, short, direct, and under 2-3 sentences. Avoid long or verbose explanations.',
  'You have context for all tasks across all workspaces. You have access to all MCPs (Model Context Protocol servers) available to the underlying ACP gateway, and you know how to get different tasks through these different MCPs.',
  'You have the ability to edit the task, update the task, change task status, and add links using the workspace MCP tools. Use them to make modifications when requested by the user.',
].join(' ');

export const TASK_ATHENA_SYSTEM_DIRECTIVE = [
  'You are ATHENA inside Savant Sanctum, acting as a Task Grooming Assistant.',
  'PRIMARY GOAL: Groom the ticket with the operator, refine scope and acceptance criteria, and when instructed to "lock down requirements" (or when the user agrees/requests), IMMEDIATELY update the ticket title and description via the savant-workspace update_task MCP tool.',
  'OPERATIONAL DIRECTIVES:',
  '1. MAIN OBJECTIVE - LOCK DOWN REQUIREMENTS: When the operator says "lock down the requirements", "lock it down", "update the ticket", or agrees to finalized scope, YOUR PRIMARY TASK IS TO CALL update_task via savant-workspace MCP to persist the refined Title and detailed Markdown Description.',
  '2. FAST & DIRECT: Do NOT loop, wander, or make extra unnecessary tool calls. Be direct and fast.',
  '3. REPOSITORY AWARENESS: Read the Linked Target Repository from the task context and use it to ground all domain and technical recommendations.',
  '4. RESTRICTED TOOLS: You are ONLY allowed to call Savant MCP tools (savant-context, savant-knowledge, savant-workspace). NEVER call filesystem tools, shell commands, or perform local file read/write operations.',
  '5. NO LOCAL FILE OPERATIONS: Do NOT touch files on disk. Operate strictly in-memory with ticket metadata and Savant MCP tools.',
  '6. CONFIRMATION GATE: When grooming reaches clarity, ask the user: "Would you like me to lock down these requirements and update the ticket title and description?" Once confirmed, execute update_task immediately.',
].join(' ');

export function buildAthenaPromptSections(sections: Array<[string, string]>, customSystemDirective?: string) {
  return [customSystemDirective || ATHENA_SYSTEM_DIRECTIVE, ...sections.map(([title, body]) => `[${title}]\n${body}`)].join('\n\n');
}

export async function fetchWorkspaceKnowledgeGraph(baseUrl: string, apiKey: string, workspaceId: string) {
  if (!workspaceId) return { nodes: [] as AthenaKnowledgeNode[], edges: [] as AthenaKnowledgeEdge[] };
  const normalizedBase = baseUrl.replace(/\/+$/, '');
  const params = new URLSearchParams({
    workspace_id: workspaceId,
    limit: '200',
    slim: 'false',
    include_staged: 'true',
    _: Date.now().toString(),
  });
  const headers = buildSavantHeaders(apiKey);
  const res = await fetch(`${normalizedBase}/api/knowledge/graph?${params.toString()}`, { headers });
  if (!res.ok) throw new Error(`Knowledge graph fetch failed: ${res.status}`);
  const data = await res.json();
  return {
    nodes: Array.isArray(data?.nodes) ? data.nodes : [],
    edges: Array.isArray(data?.edges) ? data.edges : [],
  };
}

export function formatWorkspaceKnowledgeGraph(nodes: AthenaKnowledgeNode[], edges: AthenaKnowledgeEdge[]) {
  const nodeLines = nodes.slice(0, 80).map((node, index) => {
    const id = node.node_id || node.id || `node-${index + 1}`;
    const metadata = node.metadata || {};
    const repo = metadata.repo ? ` repo=${metadata.repo}` : '';
    const status = node.status ? ` status=${node.status}` : '';
    const content = (node.content || '').replace(/\s+/g, ' ').trim().slice(0, 320);
    return `- ${id} [${node.node_type || 'unknown'}${status}${repo}] ${node.title || 'Untitled'}${content ? ` :: ${content}` : ''}`;
  });

  const edgeLines = edges.slice(0, 120).map((edge) => {
    const source = edge.source_id || formatEdgeEndpoint(edge.source);
    const target = edge.target_id || formatEdgeEndpoint(edge.target);
    const label = edge.label || edge.edge_type || 'relates_to';
    return `- ${source || 'unknown'} --${label}--> ${target || 'unknown'}`;
  });

  return [
    `Nodes: ${nodes.length}`,
    `Edges: ${edges.length}`,
    '',
    '[NODES]',
    nodeLines.length ? nodeLines.join('\n') : 'No workspace knowledge nodes returned.',
    '',
    '[EDGES]',
    edgeLines.length ? edgeLines.join('\n') : 'No workspace knowledge edges returned.',
  ].join('\n');
}

function formatEdgeEndpoint(endpoint: AthenaKnowledgeEdge['source']) {
  if (!endpoint) return '';
  if (typeof endpoint === 'string') return endpoint;
  return endpoint.node_id || endpoint.id || '';
}

export async function fetchGatewayMCPs(gatewayUrl: string): Promise<any> {
  const cleanUrl = gatewayUrl.replace(/\/+$/, '');
  const endpoints = ['/mcp', '/mcp/servers', '/mcp/config', '/api/mcp', '/v1/mcp', '/mcp-servers'];
  for (const ep of endpoints) {
    try {
      const res = await fetch(`${cleanUrl}${ep}`);
      if (res.ok) {
        const data = await res.json();
        if (data) return data;
      }
    } catch (_e) {
      // ignore
    }
  }
  return null;
}

export function filterSavantMcpData(mcpData: any): any {
  if (!mcpData) return null;
  if (Array.isArray(mcpData)) {
    return mcpData.filter((item: any) => {
      const name = String(item?.name || item?.id || item || '').toLowerCase();
      return name.includes('savant');
    });
  }
  if (typeof mcpData === 'object') {
    const filtered: Record<string, any> = {};
    for (const [key, val] of Object.entries(mcpData)) {
      if (key.toLowerCase().includes('savant')) {
        filtered[key] = val;
      }
    }
    return Object.keys(filtered).length ? filtered : mcpData;
  }
  return mcpData;
}

export function formatGatewayMCPs(mcpData: any): string {
  if (!mcpData) return 'No external MCP configuration detected on the gateway.';
  try {
    return typeof mcpData === 'object' ? JSON.stringify(mcpData, null, 2) : String(mcpData);
  } catch (_e) {
    return String(mcpData);
  }
}
