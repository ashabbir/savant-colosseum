import { useEffect, useRef, useState } from 'react';
import { Check, X, RefreshCw, CheckCircle, XCircle, WifiOff, Folder } from 'lucide-react';
import { buildSavantHeaders, SAVANT_APP_HEADERS } from '../services/httpClient';

type ConnectionStatus = 'idle' | 'checking' | 'connected' | 'failed';

interface ServiceConfig {
  url: string;
  enabled: boolean;
  status: ConnectionStatus;
  version?: string;
}

type SettingsModalProps = {
  open: boolean;
  authUser: string;
  authDraft: string;
  serverDraft: string;
  gatewayDraft: string;
  selectedProvider: string;
  selectedModel: string;
  providers: Array<{ id: string; label: string; models: string[]; defaultModel?: string }>;
  hasApiKey: boolean;
  onClose: () => void;
  onAuthUserChange: (value: string) => void;
  onAuthDraftChange: (value: string) => void;
  onServerDraftChange: (value: string) => void;
  onGatewayDraftChange: (value: string) => void;
  onSelectedProviderChange: (value: string) => void;
  onSelectedModelChange: (value: string) => void;
  onSave: () => Promise<void> | void;
  onLogout: () => Promise<void> | void;
  onRefreshProviders?: () => Promise<void> | void;
  colosseumGroomingSettings?: ColosseumSettings;
  colosseumReadySettings?: ColosseumSettings;
  colosseumReviewSettings?: ColosseumSettings;
  onColosseumGroomingSettingsChange?: (val: ColosseumSettings) => void;
  onColosseumReadySettingsChange?: (val: ColosseumSettings) => void;
  onColosseumReviewSettingsChange?: (val: ColosseumSettings) => void;
};

export type ColosseumSettings = { persona: string; tags: string; provider: string; model: string };

export type ColosseumPhaseConfig = {
  persona: string;
  tags: string[];
  provider: string;
  model: string;
};

export type ColosseumPhaseConfigs = {
  grooming: ColosseumPhaseConfig;
  ready: ColosseumPhaseConfig;
  review: ColosseumPhaseConfig;
};

export const DEFAULT_COLOSSEUM_GROOMING_SETTINGS: ColosseumSettings = {
  persona: 'persona.architect', tags: 'engineering, execution, code-review, requirements, grooming', provider: '', model: '',
};

export const DEFAULT_COLOSSEUM_READY_SETTINGS: ColosseumSettings = {
  persona: 'persona.engineer', tags: 'engineering, execution, code-review', provider: '', model: '',
};

export const DEFAULT_COLOSSEUM_REVIEW_SETTINGS: ColosseumSettings = {
  persona: 'persona.reviewer', tags: 'code-review', provider: '', model: '',
};

export function withDefaultColosseumSettings(
  settings: ColosseumSettings | undefined,
  defaults: ColosseumSettings,
) {
  return settings ?? defaults;
}

function phaseConfig(settings: ColosseumSettings, fallback?: ColosseumSettings): ColosseumPhaseConfig {
  const provider = settings.provider.trim() || fallback?.provider.trim() || '';
  const configuredModel = settings.model.trim();
  const inheritedModel = provider === fallback?.provider.trim() ? fallback?.model.trim() : '';
  return {
    persona: settings.persona.trim(),
    tags: [...new Set(settings.tags.split(',').map(tag => tag.trim()).filter(Boolean))],
    provider,
    model: configuredModel || inheritedModel || '',
  };
}

export function buildColosseumPhaseConfigs(
  grooming: ColosseumSettings,
  ready: ColosseumSettings,
  review: ColosseumSettings,
): ColosseumPhaseConfigs {
  return {
    grooming: phaseConfig(grooming, ready),
    ready: phaseConfig(ready),
    review: phaseConfig(review, ready),
  };
}

const FALLBACK_PROVIDERS = [
  { id: 'claude', label: 'Claude', defaultModel: 'claude-sonnet-4-6', models: ['claude-haiku-4-5-20251001', 'claude-sonnet-4-6', 'claude-opus-4-7', 'haiku', 'sonnet', 'opus'] },
  { id: 'gemini', label: 'Gemini', defaultModel: 'gemini-2.5-flash', models: ['gemini-2.5-flash', 'gemini-2.5-pro', 'gemini-2.0-flash'] },
  { id: 'codex', label: 'Codex', defaultModel: 'o4-mini', models: ['o4-mini', 'gpt-4.1', 'gpt-5-mini', 'o3'] },
  { id: 'copilot', label: 'Copilot', defaultModel: 'claude-sonnet-4.6', models: ['claude-haiku-4.5', 'claude-sonnet-4.6', 'gpt-4.1', 'gpt-5-mini'] },
  { id: 'agy', label: 'AGY', defaultModel: 'gemini-2.5-flash', models: ['gemini-2.5-flash', 'gemini-2.5-pro', 'gemini-2.0-flash'] },
];

const TABS = [
  { id: 'system', label: 'system' },
  { id: 'gateway', label: 'gateway' },
  { id: 'server', label: 'server' },
  { id: 'colosseum', label: 'colosseum' },
] as const;
type TabId = typeof TABS[number]['id'];

function normalizeUrl(url: string) {
  return url.trim().replace(/\/+$/, '');
}

function ServicePanel({
  description,
  config,
  onChange,
  healthPath,
  apiKey,
  includeApiKey = false,
}: {
  description: string;
  config: ServiceConfig;
  onChange: (patch: Partial<ServiceConfig>) => void;
  healthPath: string;
  apiKey?: string;
  includeApiKey?: boolean;
}) {
  async function checkHealth() {
    onChange({ status: 'checking' });
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 4000);
    try {
      const res = await fetch(`${normalizeUrl(config.url)}${healthPath}`, {
        signal: controller.signal,
        headers: includeApiKey ? buildSavantHeaders(apiKey) : SAVANT_APP_HEADERS,
      });
      let payload: any = {};
      try { payload = await res.json(); } catch { /* Health bodies may be empty. */ }
      clearTimeout(timer);
      onChange({ status: res.ok ? 'connected' : 'failed', version: res.ok && typeof payload?.version === 'string' ? payload.version : undefined });
    } catch {
      clearTimeout(timer);
      onChange({ status: 'failed' });
    }
  }

  const statusColor =
    config.status === 'connected' ? '#00e5ff' :
    config.status === 'failed' ? '#ff2244' :
    'rgba(255,255,255,0.5)';

  const StatusIcon =
    config.status === 'connected' ? CheckCircle :
    config.status === 'failed' ? XCircle :
    config.status === 'checking' ? RefreshCw :
    WifiOff;

  return (
    <div className="space-y-4">
      <p className="text-sm opacity-60 font-sans">{description}</p>

      <div className="flex items-center gap-3">
        <label className="text-[10px] uppercase tracking-widest opacity-60">Status</label>
        <button
          type="button"
          onClick={() => onChange({ enabled: !config.enabled })}
          className="px-3 py-1 text-xs transition-all font-mono"
          style={{
            background: config.enabled ? 'var(--cp-cyan)' : 'var(--cp-bg-3)',
            border: '1px solid var(--cp-border)',
            color: config.enabled ? 'var(--cp-bg-0)' : 'var(--foreground)',
          }}
        >
          {config.enabled ? 'ENABLED' : 'DISABLED'}
        </button>
      </div>

      <div>
        <label className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">URL</label>
        <input
          value={config.url}
          onChange={e => onChange({ url: e.target.value })}
          placeholder="http://..."
          className="rounded-none"
        />
      </div>

      <div>
        <label className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">Health endpoint</label>
        <div
          className="px-3 py-2 text-xs opacity-50 font-mono"
          style={{ background: 'var(--cp-bg-3)', border: '1px solid var(--cp-border)', color: 'var(--foreground)' }}
        >
          {normalizeUrl(config.url)}{healthPath}
        </div>
      </div>

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={checkHealth}
          disabled={!config.enabled || config.status === 'checking'}
          className="px-3 py-1.5 text-xs flex items-center gap-1.5 hover:opacity-80 transition-opacity disabled:opacity-30 font-mono"
          style={{ background: 'var(--cp-bg-3)', border: '1px solid var(--cp-cyan)', color: 'var(--cp-cyan)' }}
        >
          <RefreshCw size={12} className={config.status === 'checking' ? 'animate-spin' : ''} />
          Check Connection
        </button>

        {config.status !== 'idle' && (
          <div className="flex items-center gap-1.5 text-xs uppercase font-mono" style={{ color: statusColor }}>
            <StatusIcon size={13} className={config.status === 'checking' ? 'animate-spin' : ''} />
            {config.status === 'checking' ? 'checking...' : config.status}
            {config.status === 'connected' && config.version ? ` · v${config.version}` : ''}
          </div>
        )}
      </div>
    </div>
  );
}

type ColosseumProvider = { id: string; label: string; models: string[]; defaultModel?: string };

function ColosseumPhaseSettingsPanel({
  title,
  accent,
  settings,
  providers,
  inheritReadyProvider = false,
  onChange,
}: {
  title: string;
  accent: string;
  settings: ColosseumSettings;
  providers: ColosseumProvider[];
  inheritReadyProvider?: boolean;
  onChange: (settings: ColosseumSettings) => void;
}) {
  return (
    <div className="p-4 rounded-none" style={{ background: 'var(--cp-bg-3)', border: '1px solid var(--cp-border)' }}>
      <h4 className="text-xs uppercase tracking-wider mb-3 font-mono" style={{ color: accent }}>{title} Status Config</h4>
      <div className="space-y-4">
        <label className="block">
          <span className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">Persona</span>
          <input
            value={settings.persona}
            onChange={event => onChange({ ...settings, persona: event.target.value })}
            className="rounded-none w-full"
          />
        </label>
        <label className="block">
          <span className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">Tags (comma separated)</span>
          <input
            value={settings.tags}
            onChange={event => onChange({ ...settings, tags: event.target.value })}
            className="rounded-none w-full"
          />
        </label>
        <div className="grid grid-cols-2 gap-4">
          <label className="block">
            <span className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">Provider</span>
            <select
              value={settings.provider}
              onChange={event => {
                const provider = event.target.value;
                const selected = providers.find(item => item.id === provider);
                onChange({ ...settings, provider, model: selected?.defaultModel || selected?.models[0] || '' });
              }}
              className="w-full rounded-none"
            >
              <option value="">{inheritReadyProvider ? 'Use Ready provider' : 'Select provider...'}</option>
              {providers.map(provider => (
                <option key={provider.id} value={provider.id} style={{ background: '#080b12', color: '#fff' }}>{provider.label}</option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">Model</span>
            <select
              value={settings.model}
              onChange={event => onChange({ ...settings, model: event.target.value })}
              className="w-full rounded-none"
            >
              <option value="">Use provider default</option>
              {(providers.find(provider => provider.id === settings.provider)?.models || []).map(model => (
                <option key={model} value={model} style={{ background: '#080b12', color: '#fff' }}>{model}</option>
              ))}
            </select>
          </label>
        </div>
      </div>
    </div>
  );
}

export function SettingsModal({
  open,
  authUser,
  authDraft,
  serverDraft,
  gatewayDraft,
  selectedProvider,
  selectedModel,
  providers,
  hasApiKey,
  onClose,
  onAuthUserChange,
  onAuthDraftChange,
  onServerDraftChange,
  onGatewayDraftChange,
  onSelectedProviderChange,
  onSelectedModelChange,
  onSave,
  onLogout,
  onRefreshProviders,
  colosseumGroomingSettings: groomingSettings,
  colosseumReadySettings: readySettings,
  colosseumReviewSettings: reviewSettings,
  onColosseumGroomingSettingsChange = () => {},
  onColosseumReadySettingsChange = () => {},
  onColosseumReviewSettingsChange = () => {},
}: SettingsModalProps) {
  const colosseumGroomingSettings = withDefaultColosseumSettings(groomingSettings, DEFAULT_COLOSSEUM_GROOMING_SETTINGS);
  const colosseumReadySettings = withDefaultColosseumSettings(readySettings, DEFAULT_COLOSSEUM_READY_SETTINGS);
  const colosseumReviewSettings = withDefaultColosseumSettings(reviewSettings, DEFAULT_COLOSSEUM_REVIEW_SETTINGS);
  const [saving, setSaving] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [activeTab, setActiveTab] = useState<TabId>('system');
  const [gateway, setGateway] = useState<ServiceConfig>({ url: gatewayDraft || 'http://127.0.0.1:3100', enabled: true, status: 'idle' });
  const [server, setServer] = useState<ServiceConfig>({ url: serverDraft || 'http://127.0.0.1:8090', enabled: true, status: 'idle' });
  const [localProviders, setLocalProviders] = useState<Array<{ id: string; label: string; models: string[]; defaultModel: string }>>([]);
  const directoryInputRef = useRef<HTMLInputElement>(null);

  // Sync props → local service state when modal opens, then auto-fetch providers
  useEffect(() => {
    if (open) {
      setGateway(prev => ({ ...prev, url: gatewayDraft || prev.url, status: 'idle' }));
      setServer(prev => ({ ...prev, url: serverDraft || prev.url, status: 'idle' }));
      // Auto-fetch providers from gateway on open
      const gUrl = normalizeUrl(gatewayDraft || 'http://127.0.0.1:3100');
      fetch(`${gUrl}/health`, { signal: AbortSignal.timeout(4000), headers: SAVANT_APP_HEADERS })
        .then(r => r.ok ? r.json() : null)
        .then(data => {
          if (!data) return;
          const raw: any[] = data?.providerDetails ?? data?.providers ?? [];
          const parsed = raw.map((p: any) => {
            if (typeof p === 'string') return { id: p, label: p, models: [], defaultModel: '' };
            const id = String(p.id || p.name || '').trim();
            if (!id) return null;
            const models = Array.isArray(p.models)
              ? p.models.map((m: any) => String(m.id || m.name || m)).filter(Boolean) : [];
            return { id, label: String(p.label || p.name || id), defaultModel: p.defaultModel || models[0] || '', models };
          }).filter(Boolean) as any[];
          if (parsed.length > 0) setLocalProviders(parsed);
        })
        .catch(() => {});
    }
  }, [open]);

  // Propagate local URL changes up
  useEffect(() => { onGatewayDraftChange(gateway.url); }, [gateway.url]);
  useEffect(() => { onServerDraftChange(server.url); }, [server.url]);

  if (!open) return null;

  const allProviders = localProviders.length > 0 ? localProviders : (providers && providers.length > 0 ? providers : FALLBACK_PROVIDERS);
  const colosseumProviders = allProviders.filter(provider => ['hermes', 'codex', 'claude', 'copilot', 'agy'].includes(provider.id));
  const currentProvider = allProviders.find(p => p.id === selectedProvider) || allProviders[0];
  const currentModels = currentProvider?.models || [];

  const handleProviderChange = (provId: string) => {
    onSelectedProviderChange(provId);
    const prov = allProviders.find(p => p.id === provId);
    if (prov) onSelectedModelChange(prov.defaultModel || prov.models[0] || selectedModel);
  };

  const handleRefreshProviders = async () => {
    setRefreshing(true);
    try {
      const baseUrl = normalizeUrl(gateway.url || 'http://127.0.0.1:3100');
      const res = await fetch(`${baseUrl}/health`, { signal: AbortSignal.timeout(5000), headers: SAVANT_APP_HEADERS });
      if (!res.ok) throw new Error(`Gateway returned ${res.status}`);
      const data = await res.json();
      const raw: any[] = data?.providerDetails ?? data?.providers ?? [];
      const parsed = raw
        .map((p: any) => {
          if (typeof p === 'string') return { id: p, label: p, models: [], defaultModel: '' };
          const id = String(p.id || p.name || '').trim();
          if (!id) return null;
          const models = Array.isArray(p.models)
            ? p.models.map((m: any) => String(m.id || m.name || m)).filter(Boolean)
            : [];
          return { id, label: String(p.label || p.name || id), defaultModel: p.defaultModel || models[0] || '', models };
        })
        .filter(Boolean) as Array<{ id: string; label: string; models: string[]; defaultModel: string }>;

      if (parsed.length > 0 && onRefreshProviders) {
        await onRefreshProviders();
      }
      // Update local display immediately even before parent re-renders
      if (parsed.length > 0) {
        const first = parsed.find(p => p.id === selectedProvider) || parsed[0];
        onSelectedProviderChange(first.id);
        onSelectedModelChange(first.defaultModel || first.models[0] || '');
        // Store parsed so we can display them right away
        setLocalProviders(parsed);
      }
    } catch (err: any) {
      console.error('Failed to fetch providers from gateway:', err);
      alert(`Could not reach gateway at ${gateway.url}: ${err.message}`);
    } finally {
      setRefreshing(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try { await onSave(); onClose(); } finally { setSaving(false); }
  };

  const handleLogout = async () => {
    setSaving(true);
    try { await onLogout(); onClose(); } finally { setSaving(false); }
  };

  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        onClick={e => e.stopPropagation()}
        style={{
          background: 'var(--cp-bg-2)',
          border: '1px solid var(--cp-border)',
          boxShadow: '0 0 20px rgba(0,229,255,0.15)',
          width: '90vw',
          maxWidth: '680px',
          maxHeight: '82vh',
          display: 'flex',
          flexDirection: 'column',
          position: 'fixed',
          top: '50%',
          left: '50%',
          transform: 'translate(-50%, -50%)',
          zIndex: 200,
        }}
      >
        {/* Header */}
        <div style={{ borderBottom: '1px solid var(--cp-border)' }} className="flex items-center justify-between px-6 py-5 shrink-0">
          <div>
            <div className="eyebrow">User preferences</div>
            <h2 id="settings-title" className="brand-title !text-base !tracking-wider">Settings</h2>
            <p className="hero-copy mt-1">Configure system, gateway and AI provider preferences.</p>
          </div>
          <button onClick={onClose} style={{ color: 'var(--cp-cyan)' }} className="opacity-60 hover:opacity-100 transition-opacity">
            <X size={18} />
          </button>
        </div>

        {/* Tabs */}
        <div style={{ borderBottom: '1px solid var(--cp-border)' }} className="flex gap-1 px-6 shrink-0">
          {TABS.map(tab => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className="px-3 py-2 text-xs uppercase tracking-wide hover:opacity-100 transition-opacity font-mono"
              style={{
                color: activeTab === tab.id ? 'var(--cp-cyan)' : 'var(--foreground)',
                borderBottom: activeTab === tab.id ? '2px solid var(--cp-cyan)' : '2px solid transparent',
                opacity: activeTab === tab.id ? 1 : 0.5,
              }}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-6 space-y-5" style={{ scrollbarWidth: 'thin', scrollbarColor: 'rgba(0,229,255,0.1) transparent' }}>

          {/* ── SYSTEM ── */}
          {activeTab === 'system' && (
            <div className="space-y-5">
              <p className="text-sm opacity-60 font-sans">System configuration and preferences.</p>

              <label>
                <span className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">User name</span>
                <input value={authUser} onChange={e => onAuthUserChange(e.target.value)} className="rounded-none" />
              </label>

              <label>
                <span className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">API key</span>
                <input type="password" value={authDraft} onChange={e => onAuthDraftChange(e.target.value)} className="rounded-none" placeholder="sk-..." />
              </label>

              <div>
                <div className="flex items-center justify-between mb-1.5">
                  <span className="text-[10px] uppercase tracking-widest opacity-60">Athena Provider</span>
                  <button
                    type="button"
                    onClick={handleRefreshProviders}
                    disabled={refreshing}
                    className="flex items-center gap-1 text-[10px] uppercase tracking-widest opacity-50 hover:opacity-100 transition-opacity disabled:opacity-30 font-mono"
                  >
                    <RefreshCw size={10} className={refreshing ? 'animate-spin' : ''} />
                    {refreshing ? 'Fetching…' : 'Refresh from gateway'}
                  </button>
                </div>
                <select value={selectedProvider} onChange={e => handleProviderChange(e.target.value)} className="w-full rounded-none">
                  {allProviders.map(p => (
                    <option key={p.id} value={p.id} style={{ background: '#080b12', color: '#fff' }}>{p.label}</option>
                  ))}
                </select>
                {(!providers || providers.length === 0) && (
                  <div className="mt-1 text-[10px] font-mono opacity-40">Using built-in fallback providers. Open Gateway tab to connect to your gateway.</div>
                )}
              </div>

              <label>
                <span className="block text-[10px] uppercase tracking-widest opacity-60 mb-1.5">Athena Model</span>
                <select value={selectedModel} onChange={e => onSelectedModelChange(e.target.value)} className="w-full rounded-none">
                  {currentModels.map(m => (
                    <option key={m} value={m} style={{ background: '#080b12', color: '#fff' }}>{m}</option>
                  ))}
                </select>
              </label>
            </div>
          )}

          {/* ── GATEWAY ── */}
          {activeTab === 'gateway' && (
            <ServicePanel
              description="API gateway routing and agent execution settings."
              config={gateway}
              onChange={patch => setGateway(prev => ({ ...prev, ...patch }))}
              healthPath="/health"
              apiKey={authDraft}
            />
          )}

          {/* ── SERVER ── */}
          {activeTab === 'server' && (
            <ServicePanel
              description="Savant server backend — workspace, knowledge graph, tasks."
              config={server}
              onChange={patch => setServer(prev => ({ ...prev, ...patch }))}
              healthPath="/health/ready"
              apiKey={authDraft}
              includeApiKey
            />
          )}

          {/* ── COLOSSEUM ── */}
          {activeTab === 'colosseum' && (
            <div className="space-y-6">
              <p className="text-sm opacity-60 font-sans">Configure agent execution parameters for different board statuses.</p>

              <ColosseumPhaseSettingsPanel
                title="Grooming"
                accent="#b58cff"
                settings={colosseumGroomingSettings}
                providers={colosseumProviders}
                inheritReadyProvider
                onChange={onColosseumGroomingSettingsChange}
              />
              <ColosseumPhaseSettingsPanel
                title="Ready"
                accent="var(--cp-cyan)"
                settings={colosseumReadySettings}
                providers={colosseumProviders}
                onChange={onColosseumReadySettingsChange}
              />
              <ColosseumPhaseSettingsPanel
                title="Review"
                accent="var(--cp-magenta)"
                settings={colosseumReviewSettings}
                providers={colosseumProviders}
                inheritReadyProvider
                onChange={onColosseumReviewSettingsChange}
              />
            </div>
          )}
        </div>

        {/* Footer */}
        <div style={{ borderTop: '1px solid var(--cp-border)' }} className="px-6 py-4 shrink-0 space-y-3">
          <div className="flex items-center justify-between text-[10px] uppercase tracking-widest opacity-50 font-mono">
            <span>Session is managed in app</span>
            <span>{hasApiKey ? 'Authenticated' : 'Logged out'}</span>
          </div>
          <div className="flex items-center justify-between gap-2">
            <button className="ghost-btn" onClick={handleLogout} disabled={saving}>Log out</button>
            <div className="flex gap-2">
              <button className="ghost-btn action-close icon-only" aria-label="Close" onClick={onClose} disabled={saving}><X size={14} /></button>
              <button className="ghost-btn action-save icon-only" aria-label="Save" onClick={handleSave} disabled={saving}><Check size={14} /></button>
            </div>
          </div>
        </div>
      </div>
      <input ref={directoryInputRef} type="file" className="hidden" {...({ webkitdirectory: '', directory: '' } as any)} />
    </div>
  );
}
