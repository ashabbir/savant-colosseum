/// <reference types="vite/client" />

interface Window {
  sanctum: {
    version: string;
    runAgent: (payload: {
      prompt: string;
      chain?: Array<{ provider: string; model?: string }>;
      cwd?: string;
      sessionId?: string;
    }) => Promise<string>;
    pickRepository: (defaultPath?: string) => Promise<string | null>;
    listColosseumProviders: () => Promise<Array<{ id: string; label: string; command: string }>>;
  };
  system: {
    getSettings: () => Promise<Record<string, any>>;
    saveSetting: (key: string, value: any) => Promise<boolean>;
    saveAthenaThread: (payload: { threadKey: string; messages: Array<{ id: string; sender: 'user' | 'assistant'; text: string; timestamp: string }>; input: string }) => Promise<boolean>;
    loadAthenaThreads: () => Promise<Record<string, { messages: Array<{ id: string; sender: 'user' | 'assistant'; text: string; timestamp: string }>; input: string; updatedAt?: string }>>;
    getUser: () => Promise<string>;
    listProviders: (gatewayUrl?: string) => Promise<any>;
    getDbStatus: () => Promise<string>;
    getLocalConversation?: (provider: string, sessionId: string) => Promise<Array<{
      id: string;
      time: string;
      kind: 'user' | 'assistant' | 'tool' | 'system';
      role: string;
      title: string;
      detail: string;
      provider: string;
    }>>;
    getLocalSessionMetadata?: (sessionId: string, provider?: string) => Promise<{
      provider?: string;
      title?: string;
      agentType?: string;
      files?: Array<{ path: string; name: string; category: string; size?: number }>;
    }>;
  };
}
