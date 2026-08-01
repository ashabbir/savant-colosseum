import React, { useEffect, useRef, useState } from 'react';
import * as d3 from 'd3';
import { 
  ZoomIn, ZoomOut, Maximize, Search, Loader2, Info, Sparkles, Send, 
  Copy, Trash2, GitFork, Network, Layers, RefreshCw, Plus, Check,
  ArrowRight, ArrowLeft, Box, X
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { buildSavantHeaders } from './services/httpClient';
import * as Tooltip from '@radix-ui/react-tooltip';

interface KnowledgeGraphProps {
  workspaceId: string;
  baseUrl?: string;
  apiKey?: string;
  onClose?: () => void;
  selectedProvider?: string;
  selectedModel?: string;
}

interface SessionModalState {
  node: any;
  editTitle: string;
  editContent: string;
  saving: boolean;
}

interface Node extends d3.SimulationNodeDatum {
  id: string;
  node_id: string;
  title?: string;
  node_type: string;
  content?: string;
  status?: string;
  created_at?: string;
  metadata?: {
    repo?: string;
    files?: string[] | string;
    workspaces?: string[];
    workspace_id?: string;
    source?: string;
  };
  x?: number;
  y?: number;
  fx?: number | null;
  fy?: number | null;
  z?: number;
  vz?: number;
  px?: number;
  py?: number;
  pScale?: number;
}

interface Edge extends d3.SimulationLinkDatum<Node> {
  source: string | Node;
  target: string | Node;
  edge_type?: string;
  edge_id?: string;
  weight?: number;
}

interface ChatMessage {
  id: string;
  sender: 'user' | 'assistant';
  text: string;
  timestamp: string;
}

const KNOWLEDGE_NODE_TYPES = [
  'insight', 'client', 'domain', 'service', 'library', 
  'technology', 'project', 'concept', 'repo', 'session', 'issue', 'person'
];

const NODE_TYPE_COLORS: Record<string, string> = {
  domain: '#facc15',
  service: '#22d3ee',
  library: '#d946ef',
  technology: '#4ade80',
  concept: '#a78bfa',
  session: '#6b7280',
  person: '#f97316',
  insight: '#00ff88',
  client: '#ff00aa',
  project: '#38bdf8',
  repo: '#94a3b8',
  issue: '#ff4444',
};

const DOMAIN_HULL_COLORS = [
  'rgba(34,211,238,0.38)',
  'rgba(167,139,250,0.38)',
  'rgba(74,222,128,0.38)',
  'rgba(244,63,94,0.38)',
  'rgba(251,146,60,0.38)',
];

const TYPE_CLUSTER_ORDER = ['domain', 'service', 'library', 'technology', 'concept', 'session', 'person'];

export const KnowledgeGraph: React.FC<KnowledgeGraphProps> = ({
  workspaceId,
  baseUrl = 'http://localhost:3000',
  apiKey = '',
  onClose,
  selectedProvider,
  selectedModel
}) => {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [rawNodes, setRawNodes] = useState<any[]>([]);
  const [rawEdges, setRawEdges] = useState<any[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  
  const [selectedNode, setSelectedNode] = useState<Node | null>(null);
  const [selectedNodes, setSelectedNodes] = useState<Map<string, Node>>(new Map());
  const [activeLayer, setActiveLayer] = useState('all');
  const [is3DMode, setIs3DMode] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [isGraphMutating, setIsGraphMutating] = useState(false);
  const [isInspectorOpen, setIsInspectorOpen] = useState(true);
  
  const [searchQuery, setSearchQuery] = useState('');
  const [drawerTab, setDrawerTab] = useState<'info' | 'ai'>('info');
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [chatInput, setChatInput] = useState('');
  const [isAiLoading, setIsAiLoading] = useState(false);
  const chatEndRef = useRef<HTMLDivElement>(null);
  
  const [exploreDepth, setExploreDepth] = useState(2);
  const [isExploreActive, setIsExploreActive] = useState(false);
  const [focalNodeId, setFocalNodeId] = useState<string | null>(null);
  const [mergeNodeType, setMergeNodeType] = useState('insight');
  const [sessionModal, setSessionModal] = useState<SessionModalState | null>(null);

  useEffect(() => {
    if (chatEndRef.current) {
      chatEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [chatMessages, isAiLoading, drawerTab]);

  const loadGraph = async () => {
    setIsLoading(true);
    setLoadError(null);
    setSelectedNode(null);
    setIsExploreActive(false);
    setFocalNodeId(null);
    
    try {
      const normalizedBase = baseUrl.replace(/\/+$/, '');
      const headers = buildSavantHeaders(apiKey);
      const workspaceUrl = `${normalizedBase}/api/knowledge/graph?workspace_id=${encodeURIComponent(workspaceId)}&slim=true&include_staged=true&_=${Date.now()}`;
      const fullUrl = `${normalizedBase}/api/knowledge/graph?slim=true&include_staged=true&_=${Date.now()}`;

      const workspaceRes = await fetch(workspaceUrl, { headers }).catch(() => null);
      let raw: { nodes?: any[]; edges?: any[] } = { nodes: [], edges: [] };

      if (workspaceRes && workspaceRes.ok) {
        raw = await workspaceRes.json();
      }

      const workspaceNodes = raw.nodes || [];
      const workspaceEdges = raw.edges || [];

      if (workspaceNodes.length === 0) {
        const fullRes = await fetch(fullUrl, { headers }).catch(() => null);
        if (fullRes && fullRes.ok) {
          const fullRaw = await fullRes.json();
          const allNodes = fullRaw.nodes || [];
          const allEdges = fullRaw.edges || [];
          const filteredNodes = allNodes.filter((node: any) => {
            const workspaces = node?.metadata?.workspaces ?? [];
            return !workspaceId || workspaces.includes(workspaceId);
          });
          const filteredIds = new Set(filteredNodes.map((node: any) => node.node_id));
          const filteredEdges = allEdges.filter((edge: any) => filteredIds.has(edge.source_id) && filteredIds.has(edge.target_id));
          setRawNodes(filteredNodes);
          setRawEdges(filteredEdges);
          processGraphData(filteredNodes, filteredEdges);
          return;
        }
      }

      setRawNodes(workspaceNodes);
      setRawEdges(workspaceEdges);
      processGraphData(workspaceNodes, workspaceEdges);
    } catch (err) {
      console.error(err);
      setLoadError('Unable to load workspace knowledge graph.');
    } finally {
      setIsLoading(false);
    }
  };

  const processGraphData = (rawNodes: any[], rawEdges: any[]) => {
    let filteredNodes = rawNodes || [];
    let filteredEdges = rawEdges || [];

    if (activeLayer !== 'all') {
      filteredNodes = filteredNodes.filter((n: any) => n.node_type === activeLayer);
      const keptIds = new Set(filteredNodes.map((n: any) => n.node_id));
      filteredEdges = filteredEdges.filter(
        (e: any) => keptIds.has(e.source_id) && keptIds.has(e.target_id)
      );
    }

    const d3Nodes: Node[] = filteredNodes.map((n: any, index: number) => ({
      ...n,
      id: n.node_id,
      z: n.z ?? (index % 2 === 0 ? 1 : -1) * (20 + (index * 25) % 150),
      vz: 0,
    }));

    const d3Edges: Edge[] = filteredEdges.map((e: any) => ({
      ...e,
      source: e.source_id,
      target: e.target_id
    }));

    setNodes(d3Nodes);
    setEdges(d3Edges);
    setSelectedNodes((current) => {
      const kept = new Map<string, Node>();
      d3Nodes.forEach((node) => {
        if (current.has(node.node_id)) kept.set(node.node_id, node);
      });
      return kept;
    });
  };

  useEffect(() => {
    loadGraph();
  }, [workspaceId, baseUrl, activeLayer]);

  // Explore Mode BFS
  const handleExploreNode = (nodeId: string) => {
    setIsExploreActive(true);
    setFocalNodeId(nodeId);
    
    // Build adjacency list
    const adj: Record<string, string[]> = {};
    rawEdges.forEach(e => {
      const s = e.source_id;
      const t = e.target_id;
      if (!adj[s]) adj[s] = [];
      if (!adj[t]) adj[t] = [];
      adj[s].push(t);
      adj[t].push(s);
    });

    const distances = new Map<string, number>();
    const queue: string[] = [nodeId];
    distances.set(nodeId, 0);
    
    let i = 0;
    while (i < queue.length) {
      const cur = queue[i++];
      const d = distances.get(cur)!;
      if (d >= exploreDepth) continue;
      for (const nb of adj[cur] || []) {
        if (!distances.has(nb)) {
          distances.set(nb, d + 1);
          queue.push(nb);
        }
      }
    }

    const keptIds = new Set(distances.keys());
    const filteredNodes = rawNodes.filter(n => keptIds.has(n.node_id));
    const filteredEdges = rawEdges.filter(e => keptIds.has(e.source_id) && keptIds.has(e.target_id));

    processGraphData(filteredNodes, filteredEdges);
  };

  useEffect(() => {
    if (isExploreActive && focalNodeId) {
      handleExploreNode(focalNodeId);
    }
  }, [exploreDepth]);

  const clearExploreMode = () => {
    setIsExploreActive(false);
    setFocalNodeId(null);
    processGraphData(rawNodes, rawEdges);
  };

  const mergeSelectedNodes = async () => {
    const nodeIds = Array.from(selectedNodes.keys());
    if (nodeIds.length < 2 || isGraphMutating) return;
    setIsGraphMutating(true);
    try {
      const normalizedBase = baseUrl.replace(/\/+$/, '');
      const headers = buildSavantHeaders(apiKey, true);
      const response = await fetch(`${normalizedBase}/api/knowledge/nodes/merge`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ node_ids: nodeIds, node_type: mergeNodeType }),
      });
      if (!response.ok) throw new Error(`Merge failed: ${response.status}`);
      setSelectedNodes(new Map());
      setSelectedNode(null);
      await loadGraph();
    } catch (err) {
      console.error(err);
      setLoadError('Unable to merge selected knowledge nodes.');
    } finally {
      setIsGraphMutating(false);
    }
  };

  const deleteSelectedNodes = async () => {
    const nodeIds = Array.from(selectedNodes.keys());
    if (nodeIds.length === 0 || isGraphMutating) return;
    if (!window.confirm(`Delete ${nodeIds.length} selected knowledge node${nodeIds.length === 1 ? '' : 's'}?`)) return;
    setIsGraphMutating(true);
    try {
      const normalizedBase = baseUrl.replace(/\/+$/, '');
      const headers = buildSavantHeaders(apiKey, true);
      const response = await fetch(`${normalizedBase}/api/knowledge/nodes/bulk-delete`, {
        method: 'POST',
        headers,
        body: JSON.stringify({ node_ids: nodeIds }),
      });
      if (!response.ok) throw new Error(`Delete failed: ${response.status}`);
      setSelectedNodes(new Map());
      setSelectedNode(null);
      await loadGraph();
    } catch (err) {
      console.error(err);
      setLoadError('Unable to delete selected knowledge nodes.');
    } finally {
      setIsGraphMutating(false);
    }
  };

  const handleSendChatMessage = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!chatInput.trim() || !selectedNode || isAiLoading) return;

    const userMsg: ChatMessage = {
      id: Date.now().toString(),
      sender: 'user',
      text: chatInput,
      timestamp: new Date().toISOString(),
    };

    setChatMessages(prev => [...prev, userMsg]);
    setChatInput('');
    setIsAiLoading(true);

    try {
      const sanctum = (window as any).sanctum;
      if (!sanctum?.runAgent) {
        throw new Error('Sanctum IPC bridge not available. Make sure the Electron preload is loaded.');
      }
      const response = await sanctum.runAgent({
        prompt: `Entity: ${selectedNode.title} (${selectedNode.node_type})\nContext: ${selectedNode.content || ''}\nQuery: ${chatInput}`,
        chain: selectedProvider && selectedModel ? [{ provider: selectedProvider, model: selectedModel }] : undefined,
        sessionId: workspaceId,
      });

      const aiMsg: ChatMessage = {
        id: (Date.now() + 1).toString(),
        sender: 'assistant',
        text: typeof response === 'string' ? response : JSON.stringify(response),
        timestamp: new Date().toISOString(),
      };
      setChatMessages(prev => [...prev, aiMsg]);
    } catch (err: any) {
      console.error('Agent error:', err);
      setChatMessages(prev => [...prev, {
        id: (Date.now() + 1).toString(),
        sender: 'assistant',
        text: `Error: ${err?.message || String(err)}`,
        timestamp: new Date().toISOString(),
      }]);
    } finally {
      setIsAiLoading(false);
    }
  };

  // D3 Rendering
  const zoomBehaviorRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);

  useEffect(() => {
    if (!svgRef.current || !containerRef.current || nodes.length === 0) return;

    const d3Svg = d3.select(svgRef.current);
    d3Svg.selectAll('*').remove();

    const width = containerRef.current.clientWidth;
    const height = containerRef.current.clientHeight;

    d3Svg.attr('width', width).attr('height', height);

    const g = d3Svg.append('g');

    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.1, 5])
      .on('zoom', (event) => {
        g.attr('transform', event.transform);
      });
    
    if (!is3DMode) d3Svg.call(zoom);
    zoomBehaviorRef.current = zoom;
    if (!is3DMode) d3Svg.call(zoom.transform, d3.zoomIdentity.translate(width / 2, height / 2).scale(0.8));

    const nodeMap = new Map(nodes.map((node) => [node.id, node]));
    const resolvedEdges = edges.filter((edge) => {
      const source = typeof edge.source === 'string' ? edge.source : edge.source.id;
      const target = typeof edge.target === 'string' ? edge.target : edge.target.id;
      return nodeMap.has(source) && nodeMap.has(target);
    });

    const clusterCenters: Record<string, { x: number; y: number }> = {};
    TYPE_CLUSTER_ORDER.forEach((type, index) => {
      const angle = (index / TYPE_CLUSTER_ORDER.length) * 2 * Math.PI - Math.PI / 2;
      const radius = Math.min(width, height) * 0.28;
      clusterCenters[type] = { x: radius * Math.cos(angle), y: radius * Math.sin(angle) };
    });

    const forceCluster = (alpha: number) => {
      nodes.forEach((node) => {
        const center = clusterCenters[node.node_type];
        if (!center) return;
        node.vx = (node.vx || 0) + (center.x - (node.x || 0)) * 0.035 * alpha;
        node.vy = (node.vy || 0) + (center.y - (node.y || 0)) * 0.035 * alpha;
      });
    };

    const simulation = d3.forceSimulation<Node>(nodes)
      .force('link', d3.forceLink<Node, Edge>(resolvedEdges).id(d => d.id).distance(110).strength(0.4))
      .force('charge', d3.forceManyBody().strength(-340))
      .force('center', d3.forceCenter(0, 0))
      .force('collision', d3.forceCollide<Node>().radius(40))
      .force('cluster', forceCluster)
      .alphaDecay(0.04);

    const domainNodes = nodes.filter((node) => node.node_type === 'domain');
    const domainAreas = domainNodes.map((domainNode, index) => {
      const memberIds = new Set([domainNode.node_id]);
      resolvedEdges.forEach((edge: any) => {
        const sourceId = typeof edge.source === 'string' ? edge.source : edge.source.id;
        const targetId = typeof edge.target === 'string' ? edge.target : edge.target.id;
        if (sourceId === domainNode.node_id) memberIds.add(targetId);
        if (targetId === domainNode.node_id) memberIds.add(sourceId);
      });
      return { domain: domainNode, memberIds, color: DOMAIN_HULL_COLORS[index % DOMAIN_HULL_COLORS.length] };
    });

    if (domainAreas.length > 0) {
      simulation.force('domainGravity', (alpha: number) => {
        domainAreas.forEach((area) => {
          const domainNode = nodeMap.get(area.domain.node_id);
          if (!domainNode || domainNode.x == null || domainNode.y == null) return;
          nodes.forEach((node) => {
            if (!area.memberIds.has(node.node_id) || node.node_id === domainNode.node_id) return;
            node.vx = (node.vx || 0) + (domainNode.x - (node.x || 0)) * 0.012 * alpha;
            node.vy = (node.vy || 0) + (domainNode.y - (node.y || 0)) * 0.012 * alpha;
          });
        });
      });
    }

    const domainHullGroup = g.append('g').attr('class', 'domain-hulls');
    const hullLine = d3.line<[number, number]>().curve(d3.curveCardinalClosed.tension(0.65));
    const domainHullPath = (memberNodes: Node[], padding: number) => {
      if (!memberNodes.length) return null;
      const coord = (node: Node): [number, number] => is3DMode ? [node.px || 0, node.py || 0] : [node.x || 0, node.y || 0];
      if (memberNodes.length === 1) {
        const [x, y] = coord(memberNodes[0]);
        const radius = padding + 18;
        return hullLine(Array.from({ length: 8 }, (_, index) => {
          const angle = (index / 8) * 2 * Math.PI;
          return [x + radius * Math.cos(angle), y + radius * Math.sin(angle)] as [number, number];
        }));
      }
      if (memberNodes.length === 2) {
        const [x1, y1] = coord(memberNodes[0]);
        const [x2, y2] = coord(memberNodes[1]);
        const dx = x2 - x1;
        const dy = y2 - y1;
        const length = Math.sqrt(dx * dx + dy * dy) || 1;
        const nx = -dy / length;
        const ny = dx / length;
        const radius = padding + 12;
        return hullLine([
          [x1 + nx * radius, y1 + ny * radius],
          [x2 + nx * radius, y2 + ny * radius],
          [x2 - nx * radius, y2 - ny * radius],
          [x1 - nx * radius, y1 - ny * radius],
        ]);
      }
      const rawHull = d3.polygonHull(memberNodes.map(coord));
      if (!rawHull || rawHull.length < 2) return null;
      const cx = rawHull.reduce((sum, point) => sum + point[0], 0) / rawHull.length;
      const cy = rawHull.reduce((sum, point) => sum + point[1], 0) / rawHull.length;
      return hullLine(rawHull.map(([x, y]) => {
        const dx = x - cx;
        const dy = y - cy;
        const length = Math.sqrt(dx * dx + dy * dy) || 1;
        return [x + (dx / length) * padding, y + (dy / length) * padding] as [number, number];
      }));
    };

    const domainElements = domainAreas.map((area) => {
      const path = domainHullGroup.append('path')
        .attr('fill', area.color)
        .attr('fill-opacity', 0.12)
        .attr('stroke', area.color)
        .attr('stroke-opacity', 0.58)
        .attr('stroke-width', 1.5)
        .attr('stroke-dasharray', '6,4')
        .attr('pointer-events', 'none');
      const label = domainHullGroup.append('text')
        .attr('text-anchor', 'middle')
        .attr('font-family', 'Share Tech Mono, monospace')
        .attr('font-size', '8px')
        .attr('font-weight', '700')
        .attr('letter-spacing', '2px')
        .attr('fill', area.color)
        .attr('opacity', 0.6)
        .attr('pointer-events', 'none')
        .text((area.domain.title || 'DOMAIN').toUpperCase().slice(0, 22));
      return { area, path, label };
    });

    // Edge lines
    const link = g.append('g')
      .attr('stroke', 'var(--cp-border)')
      .attr('stroke-opacity', 0.6)
      .selectAll('line')
      .data(resolvedEdges)
      .enter().append('line')
      .attr('stroke-width', 1.5);

    // Node groups
    const nodeGroup = g.append('g')
      .selectAll('g')
      .data(nodes)
      .enter().append('g')
      .attr('cursor', 'pointer')
      .on('click', (event, d) => {
        event.stopPropagation();
        if (event.metaKey || event.ctrlKey) {
          setSelectedNodes((current) => {
            const next = new Map(current);
            if (next.has(d.node_id)) next.delete(d.node_id);
            else next.set(d.node_id, d);
            return next;
          });
          setSelectedNode(d);
        } else if (d.node_type === 'session') {
          setSelectedNodes(new Map());
          setSelectedNode(d);
          // Lazy-fetch full content then open modal
          const base = (baseUrl || 'http://127.0.0.1:8090').replace(/\/+$/, '');
          const headers: Record<string,string> = buildSavantHeaders(apiKey);
          fetch(`${base}/api/knowledge/nodes/${d.node_id}`, { headers })
            .then(r => r.ok ? r.json() : d)
            .then(full => setSessionModal({ node: full, editTitle: full.title || '', editContent: full.content || '', saving: false }))
            .catch(() => setSessionModal({ node: d, editTitle: d.title || '', editContent: d.content || '', saving: false }));
        } else {
          setSelectedNodes(new Map());
          // Lazy-fetch full node content
          const base = (baseUrl || 'http://127.0.0.1:8090').replace(/\/+$/, '');
          const headers: Record<string,string> = buildSavantHeaders(apiKey);
          fetch(`${base}/api/knowledge/nodes/${d.node_id}`, { headers })
            .then(r => r.ok ? r.json() : d)
            .then(full => setSelectedNode(full))
            .catch(() => setSelectedNode(d));
          setSelectedNode(d);
        }
        if (event.altKey) {
            handleExploreNode(d.node_id);
        }
      })
      .call(d3.drag<SVGGElement, Node>()
        .on('start', (event, d) => {
          if (!event.active) simulation.alphaTarget(0.3).restart();
          d.fx = d.x; d.fy = d.y;
        })
        .on('drag', (event, d) => {
          d.fx = event.x; d.fy = event.y;
        })
        .on('end', (event, d) => {
          if (!event.active) simulation.alphaTarget(0);
          d.fx = null; d.fy = null;
        })
      );

    // Session nodes: rounded rect card
    nodeGroup.filter(d => d.node_type === 'session')
      .append('rect')
      .attr('x', -52).attr('y', -20)
      .attr('width', 104).attr('height', 40)
      .attr('rx', 4).attr('ry', 4)
      .attr('fill', d => selectedNodes.has(d.node_id) || d.id === selectedNode?.id ? '#1e3a4a' : '#111827')
      .attr('stroke', d => selectedNodes.has(d.node_id) ? '#00e6c8' : d.id === selectedNode?.id ? '#6b7280' : '#374151')
      .attr('stroke-width', d => selectedNodes.has(d.node_id) ? 2 : 1)
      .attr('stroke-dasharray', d => d.status === 'staged' ? '3,3' : 'none')
      .attr('opacity', d => selectedNodes.size > 0 && !selectedNodes.has(d.node_id) ? 0.32 : d.status === 'staged' ? 0.65 : 1);

    // Session: type badge text
    nodeGroup.filter(d => d.node_type === 'session')
      .append('text')
      .attr('dy', -6).attr('text-anchor', 'middle')
      .text('SESSION')
      .attr('fill', '#6b7280').attr('font-size', '7px')
      .attr('font-family', 'Share Tech Mono, monospace')
      .attr('letter-spacing', '1.5px')
      .style('pointer-events', 'none');

    // Session: title text (truncated, centered)
    nodeGroup.filter(d => d.node_type === 'session')
      .append('text')
      .attr('dy', 8).attr('text-anchor', 'middle')
      .text(d => { const t = d.title || d.node_id; return t.length > 18 ? t.slice(0, 18) + '…' : t; })
      .attr('fill', '#e2e8f0').attr('font-size', '9px')
      .attr('font-family', 'Share Tech Mono, monospace')
      .style('pointer-events', 'none');

    // Session: created_at badge
    nodeGroup.filter(d => d.node_type === 'session')
      .append('text')
      .attr('dy', 21).attr('text-anchor', 'middle')
      .text(d => d.created_at ? new Date(d.created_at).toLocaleDateString('en-US', { month: 'short', day: 'numeric' }) : '')
      .attr('fill', '#4b5563').attr('font-size', '7px')
      .attr('font-family', 'Share Tech Mono, monospace')
      .style('pointer-events', 'none');

    // Non-session node circles
    nodeGroup.filter(d => d.node_type !== 'session')
      .append('circle')
      .attr('r', d => selectedNodes.has(d.node_id) || d.id === selectedNode?.id ? 15 : 10)
      .attr('fill', d => NODE_TYPE_COLORS[d.node_type] || '#6b7280')
      .attr('stroke', d => selectedNodes.has(d.node_id) ? '#00e6c8' : d.id === selectedNode?.id ? '#fff' : 'rgba(255,255,255,0.1)')
      .attr('stroke-width', d => selectedNodes.has(d.node_id) ? 4 : 2)
      .attr('stroke-dasharray', d => d.status === 'staged' ? '3,3' : 'none')
      .attr('opacity', d => selectedNodes.size > 0 && !selectedNodes.has(d.node_id) ? 0.32 : d.status === 'staged' ? 0.65 : 1);

    // Non-session node labels (truncated)
    nodeGroup.filter(d => d.node_type !== 'session')
      .append('text')
      .attr('dy', 25)
      .attr('text-anchor', 'middle')
      .text(d => { const t = d.title || d.node_id; return t.length > 20 ? t.slice(0, 20) + '…' : t; })
      .attr('fill', 'rgba(255,255,255,0.8)')
      .attr('font-size', '10px')
      .attr('font-family', 'Share Tech Mono, monospace')
      .style('pointer-events', 'none');

    simulation.on('tick', () => {
      if (is3DMode) {
        const yaw = -0.55;
        const pitch = 0.42;
        const cameraDistance = 520;
        nodes.forEach((node) => {
          const x = node.x || 0;
          const y = node.y || 0;
          const z = node.z || 0;
          const cosYaw = Math.cos(yaw);
          const sinYaw = Math.sin(yaw);
          const cosPitch = Math.cos(pitch);
          const sinPitch = Math.sin(pitch);
          const rx = x * cosYaw - z * sinYaw;
          const rz = x * sinYaw + z * cosYaw;
          const ry = y * cosPitch - rz * sinPitch;
          const depth = y * sinPitch + rz * cosPitch + cameraDistance;
          const scale = cameraDistance / Math.max(140, depth);
          node.px = rx * scale;
          node.py = ry * scale;
          node.pScale = scale;
        });
      }

      link
        .attr('x1', (d: any) => is3DMode ? d.source.px : d.source.x)
        .attr('y1', (d: any) => is3DMode ? d.source.py : d.source.y)
        .attr('x2', (d: any) => is3DMode ? d.target.px : d.target.x)
        .attr('y2', (d: any) => is3DMode ? d.target.py : d.target.y)
        .attr('opacity', (d: any) => {
          if (selectedNodes.size === 0) return 0.65;
          return selectedNodes.has(d.source.node_id) && selectedNodes.has(d.target.node_id) ? 0.85 : 0.15;
        });

      domainElements.forEach(({ area, path, label }) => {
        const members = nodes.filter((node) => area.memberIds.has(node.node_id));
        const hullPath = domainHullPath(members, 30);
        path.attr('d', hullPath || '').attr('display', hullPath ? null : 'none');
        const domainNode = nodeMap.get(area.domain.node_id);
        const lx = is3DMode ? domainNode?.px : domainNode?.x;
        const ly = is3DMode ? domainNode?.py : domainNode?.y;
        label.attr('x', lx || 0).attr('y', (ly || 0) - 38);
      });

      nodeGroup
        .attr('transform', (d: any) => `translate(${is3DMode ? d.px : d.x},${is3DMode ? d.py : d.y}) scale(${is3DMode ? Math.max(0.65, Math.min(1.35, d.pScale || 1)) : 1})`)
        .attr('opacity', (d) => selectedNodes.size > 0 && !selectedNodes.has(d.node_id) ? 0.4 : 1);
    });

    return () => simulation.stop();
  }, [nodes, edges, selectedNode, selectedNodes, is3DMode]);

  const handleZoom = (factor: number) => {
    if (svgRef.current && zoomBehaviorRef.current) {
        d3.select(svgRef.current).transition().duration(250).call(zoomBehaviorRef.current.scaleBy, factor);
    }
  };

  const handleSaveSessionNode = async () => {
    if (!sessionModal) return;
    setSessionModal(m => m ? { ...m, saving: true } : null);
    try {
      const normalizedBase = (baseUrl || 'http://127.0.0.1:8090').replace(/\/+$/, '');
      const headers = buildSavantHeaders(apiKey, true);
      const res = await fetch(`${normalizedBase}/api/knowledge/nodes/${sessionModal.node.node_id}`, {
        method: 'PUT',
        headers,
        body: JSON.stringify({ title: sessionModal.editTitle, content: sessionModal.editContent }),
      });
      if (res.ok) {
        setSessionModal(null);
        await loadGraph();
      } else {
        alert('Failed to save session node');
      }
    } catch (err) {
      alert('Error saving: ' + String(err));
    } finally {
      setSessionModal(m => m ? { ...m, saving: false } : null);
    }
  };

  const handleResetZoom = () => {
    if (svgRef.current && zoomBehaviorRef.current && containerRef.current) {
        const w = containerRef.current.clientWidth;
        const h = containerRef.current.clientHeight;
        d3.select(svgRef.current).transition().duration(250).call(
            zoomBehaviorRef.current.transform, 
            d3.zoomIdentity.translate(w / 2, h / 2).scale(0.8)
        );
    }
  };

  return (
    <Tooltip.Provider>
    <div className="flex flex-col h-full bg-[#080b12] text-white font-mono overflow-hidden">
      {/* Top Controls Bar */}
      <div className="h-12 flex items-center justify-between px-4 border-b border-white/5 bg-[var(--cp-bg-1)] shrink-0">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2 text-white/40 text-[10px] uppercase tracking-widest border-r border-white/5 pr-4 h-6">
            <Network size={14} className="text-cyan-400" />
            <span>// Knowledge Network</span>
          </div>
          
          <div className="flex items-center gap-1 bg-[var(--cp-bg-2)] border border-white/5 p-1 rounded-sm">
            {['all', ...KNOWLEDGE_NODE_TYPES.slice(0, 6)].map(layer => (
              <button
                key={layer}
                onClick={() => setActiveLayer(layer)}
                className={`px-2 py-1 text-[10px] uppercase transition-colors cursor-pointer rounded-sm ${activeLayer === layer ? 'bg-cyan-600 text-white font-bold' : 'text-white/40 hover:text-white/60'}`}
              >
                {layer}
              </button>
            ))}
          </div>
          <div className="flex items-center gap-3 text-[10px] uppercase tracking-widest text-white/35">
            <span>{workspaceId}</span>
            <span>{nodes.length} nodes</span>
            <span>{edges.length} edges</span>
          </div>
        </div>

        <div className="flex items-center gap-4">
          <button
            onClick={() => setIs3DMode((current) => !current)}
            className={`flex items-center gap-1.5 px-3 py-1 border border-white/10 transition-colors rounded-sm text-[10px] font-bold uppercase tracking-wider ${is3DMode ? 'bg-cyan-400 text-black' : 'bg-white/5 text-white/60 hover:text-white'}`}
          >
            <Box size={12} /> {is3DMode ? '3D mode' : '2D mode'}
          </button>
          <button className="flex items-center justify-center px-3 py-1 bg-cyan-600 hover:bg-cyan-500 transition-colors rounded-sm text-[10px] font-bold uppercase tracking-wider" aria-label="Add node" title="Add node">
            <Plus size={12} />
          </button>
          <div className="flex items-center bg-[var(--cp-bg-2)] border border-white/5 rounded-sm px-2 py-1 gap-2">
            <Search size={12} className="text-white/40" />
            <input
              type="text"
              placeholder="Find knowledge node..."
              className="bg-transparent border-0 outline-none text-[11px] w-48 text-white/80"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </div>
          <button onClick={loadGraph} className="p-2 hover:bg-white/5 transition-colors rounded-sm text-cyan-400" title="Refresh Graph">
            <RefreshCw size={14} className={isLoading ? 'animate-spin' : ''} />
          </button>
          {onClose && (
            <button onClick={onClose} className="p-2 hover:bg-white/5 transition-colors rounded-sm text-rose-400 hover:text-rose-300" title="Close">
                <X size={14} />
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 flex min-h-0 relative">
        {/* Canvas Area */}
        <div className="flex-1 relative" ref={containerRef}>
          <svg ref={svgRef} className="w-full h-full cursor-grab active:cursor-grabbing" />
          
          {isExploreActive && (
            <div className="absolute bottom-6 left-1/2 -translate-x-1/2 flex items-center gap-3 px-4 py-2 bg-[var(--cp-bg-1)] border border-cyan-500/50 rounded-sm shadow-2xl z-10">
              <span className="text-[10px] uppercase tracking-widest text-cyan-400 font-bold">Explore Depth</span>
              <div className="flex items-center gap-2">
                <button onClick={() => setExploreDepth(d => Math.max(1, d - 1))} className="w-6 h-6 flex items-center justify-center bg-white/5 hover:bg-white/10 rounded-sm border border-white/10">-</button>
                <span className="text-sm font-bold w-4 text-center">{exploreDepth}</span>
                <button onClick={() => setExploreDepth(d => Math.min(5, d + 1))} className="w-6 h-6 flex items-center justify-center bg-white/5 hover:bg-white/10 rounded-sm border border-white/10">+</button>
              </div>
              <div className="w-px h-4 bg-white/10 mx-1" />
              <button onClick={clearExploreMode} className="flex items-center gap-1 text-[10px] uppercase tracking-widest text-red-400 hover:text-red-300 transition-colors"><X size={12} /> Clear</button>
            </div>
          )}

          {selectedNodes.size >= 2 && (
            <div className="absolute top-5 left-5 w-[360px] border border-cyan-500/25 bg-black/70 backdrop-blur-md shadow-2xl z-20">
              <div className="flex items-center justify-between gap-3 px-4 py-3 border-b border-white/10">
                <div>
                  <div className="text-[10px] uppercase tracking-[0.18em] text-cyan-400 font-bold">// Multi edit</div>
                  <div className="text-[11px] text-white/45">{selectedNodes.size} selected · first selected node survives merge</div>
                </div>
                <button onClick={() => setSelectedNodes(new Map())} className="text-rose-400 hover:text-rose-300" title="Close"><X size={14} /></button>
              </div>
              <div className="p-4 grid gap-3">
                <div className="grid gap-2 max-h-32 overflow-auto">
                  {Array.from(selectedNodes.values()).map((node, index) => (
                    <div key={node.node_id} className="flex items-center justify-between gap-2 text-[11px] border border-white/5 bg-white/[0.03] px-2 py-1.5">
                      <span className="truncate">{index === 0 ? 'survivor · ' : ''}{node.title || node.node_id}</span>
                      <span style={{ color: NODE_TYPE_COLORS[node.node_type] || '#94a3b8' }} className="uppercase">{node.node_type}</span>
                    </div>
                  ))}
                </div>
                <label className="grid gap-1 text-[10px] uppercase tracking-[0.14em] text-white/40">
                  Merge type
                  <select value={mergeNodeType} onChange={(event) => setMergeNodeType(event.target.value)} className="bg-white/[0.04] border border-white/10 px-2 py-2 text-[11px] text-white outline-none">
                    {KNOWLEDGE_NODE_TYPES.map((type) => <option key={type} value={type}>{type}</option>)}
                  </select>
                </label>
                <div className="grid grid-cols-2 gap-2">
                  <button onClick={mergeSelectedNodes} disabled={isGraphMutating} className="flex items-center justify-center gap-2 border border-cyan-500/30 bg-cyan-500/10 px-3 py-2 text-[10px] uppercase tracking-widest text-cyan-300 hover:bg-cyan-500/20 disabled:opacity-45">
                    <GitFork size={13} /> Merge
                  </button>
                  <button onClick={deleteSelectedNodes} disabled={isGraphMutating} className="flex items-center justify-center gap-2 border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-[10px] uppercase tracking-widest text-rose-300 hover:bg-rose-500/20 disabled:opacity-45">
                    <Trash2 size={13} /> Delete
                  </button>
                </div>
              </div>
            </div>
          )}

          <div className="absolute bottom-6 right-6 flex flex-col gap-2">
            <button onClick={() => handleZoom(1.3)} className="p-2.5 bg-black/40 border border-white/10 rounded-sm hover:bg-white/5 transition-colors shadow-lg">
              <ZoomIn size={14} />
            </button>
            <button onClick={() => handleZoom(0.7)} className="p-2.5 bg-black/40 border border-white/10 rounded-sm hover:bg-white/5 transition-colors shadow-lg">
              <ZoomOut size={14} />
            </button>
            <button onClick={handleResetZoom} className="p-2.5 bg-black/40 border border-white/10 rounded-sm hover:bg-white/5 transition-colors shadow-lg">
              <Maximize size={14} />
            </button>
          </div>

          {isLoading && (
            <div className="absolute inset-0 flex items-center justify-center bg-black/20 backdrop-blur-[1px]">
               <div className="flex items-center gap-3 px-5 py-2.5 bg-black/60 border border-cyan-500/30 rounded-sm">
                  <Loader2 size={16} className="animate-spin text-cyan-400" />
                  <span className="text-[11px] uppercase tracking-[0.2em] text-cyan-400">Synchronizing_Network</span>
               </div>
            </div>
          )}

          {!isLoading && loadError && (
            <div className="absolute inset-0 flex items-center justify-center bg-black/30">
              <div className="max-w-sm rounded-sm border border-white/10 bg-[var(--cp-bg-1)] px-4 py-3 text-[11px] text-white/70">
                {loadError}
              </div>
            </div>
          )}

        </div>

        {/* Inspector Panel - Rail to Rail (covers right side) */}
        {selectedNode && selectedNodes.size < 2 && (
          <div className="w-[450px] bg-[#0a0e17] border-l border-white/5 flex flex-col shadow-2xl overflow-hidden" style={{ animation: 'slideInRight 0.2s ease-out' }}>
            <div className="h-12 flex items-center justify-between px-5 border-b border-white/5 bg-[var(--cp-bg-2)] shrink-0">
               <div className="flex items-center gap-2">
                  <span className="text-[10px] uppercase text-cyan-400 font-bold tracking-widest">// Node Inspector</span>
                  <div className="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse" />
               </div>
               <button onClick={() => setSelectedNode(null)} className="text-rose-400 hover:text-rose-300 transition-colors" title="Close">
                  <X size={14} />
               </button>
            </div>

            <div className="flex border-b border-white/5 bg-[#080b12] shrink-0">
              <button
                onClick={() => setDrawerTab('info')}
                className={`flex-1 py-3 text-[10px] uppercase tracking-[0.15em] transition-colors border-r border-white/5 ${drawerTab === 'info' ? 'text-cyan-400 bg-white/[0.02]' : 'text-white/30 hover:text-white/50'}`}
              >
                Information
              </button>
              <button
                onClick={() => setDrawerTab('ai')}
                className={`flex-1 py-3 text-[10px] uppercase tracking-[0.15em] transition-colors ${drawerTab === 'ai' ? 'text-cyan-400 bg-white/[0.02]' : 'text-white/30 hover:text-white/50'}`}
              >
                Athena Analysis
              </button>
            </div>

            <div className="flex-1 overflow-y-auto p-6">
              {drawerTab === 'info' ? (
                <div className="space-y-8">
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                       <span className="text-[9px] uppercase text-white/20 tracking-widest">Classification</span>
                       <span className="px-2 py-0.5 bg-cyan-500/10 border border-cyan-500/30 text-cyan-400 text-[10px] uppercase rounded-sm">{selectedNode.node_type}</span>
                    </div>
                    <h2 className="text-xl font-bold text-white tracking-tight">{selectedNode.title || selectedNode.node_id}</h2>
                    <div className="text-[10px] text-white/30 font-mono">UUID: {selectedNode.node_id}</div>
                  </div>

                  {selectedNode.content && (
                    <div className="space-y-3">
                      <div className="text-[9px] uppercase text-white/20 tracking-widest">// Captured Context</div>
                      <div className="p-4 bg-white/[0.02] border border-white/5 rounded-sm text-[12px] leading-relaxed text-white/70 italic font-sans whitespace-pre-wrap">
                        {selectedNode.content}
                      </div>
                    </div>
                  )}

                  {selectedNode.metadata && Object.keys(selectedNode.metadata).length > 0 && (
                    <div className="space-y-3">
                      <div className="text-[9px] uppercase text-white/20 tracking-widest">// Associated Metadata</div>
                      <div className="grid gap-2">
                        {Object.entries(selectedNode.metadata).map(([k, v]) => (
                          <div key={k} className="flex justify-between items-center py-2 border-b border-white/5 text-[11px]">
                            <span className="text-white/30 uppercase">{k}</span>
                            <span className="text-white/80 font-bold">{Array.isArray(v) ? v.join(', ') : String(v)}</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  <div className="pt-8 border-t border-white/5">
                    <button 
                       onClick={() => handleExploreNode(selectedNode.node_id)}
                       className="w-full py-3 border border-cyan-500/30 text-cyan-400 hover:bg-cyan-500/10 transition-colors uppercase text-[10px] tracking-widest font-bold flex items-center justify-center gap-2"
                    >
                       <Layers size={14} /> Explore Neighborhood
                    </button>
                  </div>
                </div>
              ) : (
                <div className="h-full flex flex-col">
                  <div className="flex-1 space-y-5 mb-6">
                    {chatMessages.length === 0 ? (
                      <div className="h-full flex flex-col items-center justify-center opacity-30 text-center px-10">
                        <Sparkles size={32} className="mb-4 text-cyan-400" />
                        <div className="text-[11px] leading-relaxed uppercase tracking-wider">
                          Consult Athena for architectural synthesis of this knowledge point.
                        </div>
                      </div>
                    ) : (
                      chatMessages.map((msg, i) => (
                        <div key={i} className={`flex flex-col ${msg.sender === 'user' ? 'items-end' : 'items-start'}`}>
                          <div className={`p-4 rounded-sm border text-[12px] leading-relaxed font-sans max-w-[95%] ${msg.sender === 'user' ? 'bg-cyan-500/10 border-cyan-500/20 text-cyan-100' : 'bg-white/[0.03] border-white/5 text-white/80'}`}>
                            {msg.sender === 'assistant' ? (
                                <div className="prose prose-invert prose-sm max-w-none">
                                    <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.text}</ReactMarkdown>
                                </div>
                            ) : msg.text}
                          </div>
                          <span className="text-[8px] text-white/20 mt-1 uppercase tracking-tighter">{msg.sender === 'user' ? 'Operator' : 'Athena'} • {new Date(msg.timestamp).toLocaleTimeString()}</span>
                        </div>
                      ))
                    )}
                    {isAiLoading && (
                      <div className="flex items-center gap-3 text-cyan-400 bg-cyan-500/5 p-3 border border-cyan-500/10 rounded-sm">
                        <Loader2 size={14} className="animate-spin" />
                        <span className="text-[10px] uppercase tracking-[0.2em]">Consulting_Core_Vectors...</span>
                      </div>
                    )}
                    <div ref={chatEndRef} />
                  </div>
                  
                  <form onSubmit={handleSendChatMessage} className="mt-auto flex gap-2 pt-4 border-t border-white/5">
                    <input
                      type="text"
                      value={chatInput}
                      onChange={(e) => setChatInput(e.target.value)}
                      placeholder="Input query for Athena..."
                      className="flex-1 bg-white/[0.03] border border-white/10 rounded-sm px-4 py-3 text-xs outline-none focus:border-cyan-500/50 text-white/90"
                    />
                    <button type="submit" className="px-5 bg-cyan-600 hover:bg-cyan-500 transition-colors rounded-sm flex items-center justify-center">
                      <Send size={16} />
                    </button>
                  </form>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>

    {/* Session Node Detail / Edit Modal */}
    {sessionModal && (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
        <div className="w-full max-w-lg bg-[#0a0e17] border border-white/10 shadow-2xl rounded-sm flex flex-col" style={{ maxHeight: '85vh' }}>
          {/* Header */}
          <div className="flex items-center justify-between px-5 py-4 border-b border-white/5 shrink-0">
            <div className="flex items-center gap-3">
              <span className="text-[9px] uppercase tracking-widest px-2 py-0.5 bg-[#6b7280]/20 border border-[#6b7280]/30 text-[#6b7280] rounded-sm">session</span>
              {sessionModal.node.status === 'staged' && (
                <span className="text-[9px] uppercase tracking-widest px-2 py-0.5 bg-yellow-500/10 border border-yellow-500/30 text-yellow-400 rounded-sm">staged</span>
              )}
            </div>
            <button onClick={() => setSessionModal(null)} className="text-white/30 hover:text-white/60 transition-colors">
              <X size={16} />
            </button>
          </div>

          {/* Body */}
          <div className="flex-1 overflow-y-auto p-5 space-y-5">
            {/* Meta row */}
            <div className="grid grid-cols-2 gap-3 text-[10px] font-mono">
              <div className="bg-white/[0.02] border border-white/5 px-3 py-2 rounded-sm">
                <div className="text-white/30 uppercase tracking-widest mb-1">Created</div>
                <div className="text-white/70">{sessionModal.node.created_at ? new Date(sessionModal.node.created_at).toLocaleString() : '—'}</div>
              </div>
              <div className="bg-white/[0.02] border border-white/5 px-3 py-2 rounded-sm">
                <div className="text-white/30 uppercase tracking-widest mb-1">Node ID</div>
                <div className="text-white/40 truncate">{sessionModal.node.node_id}</div>
              </div>
              {sessionModal.node.metadata?.workspace_id && (
                <div className="bg-white/[0.02] border border-white/5 px-3 py-2 rounded-sm col-span-2">
                  <div className="text-white/30 uppercase tracking-widest mb-1">Workspace</div>
                  <div className="text-cyan-400/80">{sessionModal.node.metadata.workspace_id}</div>
                </div>
              )}
              {sessionModal.node.metadata?.source && (
                <div className="bg-white/[0.02] border border-white/5 px-3 py-2 rounded-sm col-span-2">
                  <div className="text-white/30 uppercase tracking-widest mb-1">Source</div>
                  <div className="text-white/60 break-all">{sessionModal.node.metadata.source}</div>
                </div>
              )}
            </div>

            {/* Editable title */}
            <div className="space-y-1.5">
              <label className="text-[10px] uppercase tracking-widest text-white/30">Title</label>
              <input
                value={sessionModal.editTitle}
                onChange={e => setSessionModal(m => m ? { ...m, editTitle: e.target.value } : null)}
                className="w-full bg-white/[0.03] border border-white/10 rounded-sm px-3 py-2 text-sm text-white/90 font-mono outline-none focus:border-cyan-500/50"
              />
            </div>

            {/* Editable content */}
            <div className="space-y-1.5">
              <label className="text-[10px] uppercase tracking-widest text-white/30">Content</label>
              <textarea
                value={sessionModal.editContent}
                onChange={e => setSessionModal(m => m ? { ...m, editContent: e.target.value } : null)}
                rows={10}
                className="w-full bg-white/[0.03] border border-white/10 rounded-sm px-3 py-2 text-xs text-white/80 font-mono outline-none focus:border-cyan-500/50 resize-y leading-relaxed"
              />
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between gap-3 px-5 py-4 border-t border-white/5 shrink-0">
            <button
              onClick={() => { setSessionModal(null); handleExploreNode(sessionModal.node.node_id); }}
              className="flex items-center gap-2 px-4 py-2 border border-white/10 text-white/50 hover:text-white/70 text-[10px] uppercase tracking-widest rounded-sm transition-colors"
            >
              <Layers size={12} /> Explore
            </button>
            <div className="flex gap-2">
              <button onClick={() => setSessionModal(null)} className="px-4 py-2 border border-white/10 text-white/40 hover:text-white/60 text-[10px] uppercase tracking-widest rounded-sm transition-colors">
                Cancel
              </button>
              <button
                onClick={handleSaveSessionNode}
                disabled={sessionModal.saving}
                className="px-5 py-2 bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 text-white text-[10px] uppercase tracking-widest font-bold rounded-sm transition-colors flex items-center gap-2"
              >
                <Check size={12} /> {sessionModal.saving ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>
        </div>
      </div>
    )}
    </Tooltip.Provider>
  );
};

export default KnowledgeGraph;
