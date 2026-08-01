import { contextBridge, ipcRenderer } from 'electron';

type RunAgentPayload = { prompt: string; chain?: Array<{ provider: string; model?: string }>; cwd?: string; sessionId?: string };

async function getGatewayBaseUrl() {
  try {
    const settings = await ipcRenderer.invoke('get-settings');
    const config = settings?.['gateway:config'];
    const url = typeof config === 'string' ? config : config?.url;
    return String(url || 'http://127.0.0.1:3100').replace(/\/+$/, '');
  } catch {
    return 'http://127.0.0.1:3100';
  }
}

async function runAgentViaGateway(payload: RunAgentPayload) {
  const baseUrl = await getGatewayBaseUrl();
  const response = await fetch(`${baseUrl}/runs`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      prompt: payload.prompt,
      chain: payload.chain,
      cwd: payload.cwd,
      session_id: payload.sessionId,
    }),
  });

  if (!response.ok) {
    throw new Error(`Gateway run failed: ${response.status} ${await response.text()}`);
  }

  const run = await response.json();
  const runId = String(run?.id || '');
  if (!runId) throw new Error('Gateway did not return a run id');

  const startedAt = Date.now();
  while (Date.now() - startedAt < 120_000) {
    await new Promise((resolve) => setTimeout(resolve, 700));
    const statusResponse = await fetch(`${baseUrl}/runs/${encodeURIComponent(runId)}`);
    if (!statusResponse.ok) {
      throw new Error(`Gateway status failed: ${statusResponse.status} ${await statusResponse.text()}`);
    }
    const status = await statusResponse.json();
    if (status.status === 'complete') return String(status.result?.response || '');
    if (status.status === 'error' || status.status === 'killed') {
      throw new Error(status.error || `Gateway run ${status.status}`);
    }
  }

  throw new Error('Gateway run timed out');
}

async function invokeRunAgent(payload: RunAgentPayload) {
  try {
    return await ipcRenderer.invoke('run-agent', payload);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error ?? '');
    if (message.includes("No handler registered for 'run-agent'")) {
      return runAgentViaGateway(payload);
    }
    throw error;
  }
}

contextBridge.exposeInMainWorld('system', {
  getSettings: () => ipcRenderer.invoke('get-settings'),
  saveSetting: (key: string, value: any) => ipcRenderer.invoke('save-setting', { key, value }),
  saveAthenaThread: (payload: { threadKey: string; messages: any[]; input: string }) => ipcRenderer.invoke('save-athena-thread', payload),
  loadAthenaThreads: () => ipcRenderer.invoke('load-athena-threads'),
  getUser: () => ipcRenderer.invoke('get-user'),
  listProviders: (gatewayUrl?: string) => ipcRenderer.invoke('list-providers', gatewayUrl),
  getDbStatus: () => ipcRenderer.invoke('get-db-status'),
  getLocalConversation: (provider: string, sessionId: string) => ipcRenderer.invoke('get-local-conversation', provider, sessionId),
  getLocalSessionMetadata: (sessionId: string, provider?: string) => ipcRenderer.invoke('get-local-session-metadata', sessionId, provider),
});

contextBridge.exposeInMainWorld('sanctum', {
  version: '1.0.0',
  runAgent: (payload: RunAgentPayload) => invokeRunAgent(payload),
  pickRepository: (defaultPath?: string) => ipcRenderer.invoke('pick-repository', defaultPath),
  listColosseumProviders: () => ipcRenderer.invoke('list-colosseum-providers'),
});
