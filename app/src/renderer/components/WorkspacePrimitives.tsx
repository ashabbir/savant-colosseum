import { Cpu } from 'lucide-react';
import { useMemo, type ReactNode } from 'react';
import type { Session, Workspace } from '../data';

export function Metric({ label, value }: { label: string; value: string | number }) {
  const numValue = typeof value === 'number' ? value : 0;
  const max = label === 'Sessions' ? 50 : label === 'Tasks' ? 30 : 200;
  const percent = Math.min(100, (numValue / max) * 100);
  return (
    <div className="metric">
      <div className="metric-head">
        <span className="metric-label">{label}</span>
        <strong className="metric-value">{value}</strong>
        <div className="w-1 h-1 rounded-full bg-cyan-400 shadow-[0_0_8px_rgba(0,229,255,0.8)]" />
      </div>
      <div className="metric-bar">
        <div className="metric-bar-fill" style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}

export function StatusOrb({ status }: { status: string }) {
  const isOnline = status === 'online' || status === 'active';
  return <div className="relative w-4 h-4 mr-2"><div className={`absolute inset-0 rounded-full ${isOnline ? 'bg-emerald-500' : 'bg-amber-500'} opacity-20 animate-ping`} /><div className={`absolute inset-0 rounded-full ${isOnline ? 'bg-emerald-500' : 'bg-amber-500'} shadow-[0_0_12px_rgba(16,185,129,0.8)]`} /></div>;
}

export function IntelligencePulse({ sessions, workspaceProviders }: { sessions: Session[]; workspaceProviders: Workspace['aiIntelligence']['providers'] }) {
  const usageMap = useMemo(() => {
    const map: Record<string, { total: number; models: Record<string, number> }> = {};
    sessions.forEach((s) => {
      if (!map[s.provider]) map[s.provider] = { total: 0, models: {} };
      map[s.provider].total++;
      map[s.provider].models[s.model] = (map[s.provider].models[s.model] ?? 0) + 1;
    });
    return map;
  }, [sessions]);
  return <div className="relative min-h-24 w-full bg-white/[0.02] border border-white/[0.05] flex flex-col p-3 overflow-hidden"><div className="absolute inset-0 opacity-5 pointer-events-none"><svg width="100%" height="100%" viewBox="0 0 400 100" preserveAspectRatio="none"><path d="M 0 80 Q 50 20 100 80 T 200 80 T 300 80 T 400 80" fill="none" stroke="var(--cp-cyan)" strokeWidth="1" className="animate-[pulse_3s_infinite]" /></svg></div><div className="flex flex-wrap gap-4 items-center justify-around z-10">{workspaceProviders.map((p) => { const usage = usageMap[p.name] || { total: 0, models: {} }; return <div key={p.name} className="flex flex-col items-center group"><div className="relative"><div className={`w-10 h-10 rounded-full border border-white/10 flex items-center justify-center transition-all ${usage.total > 0 ? 'bg-cyan-500/10 border-cyan-500/30 shadow-[0_0_15px_rgba(0,229,255,0.2)]' : 'opacity-40 grayscale'}`}><span className="text-xs font-bold">{p.name[0]}</span></div>{usage.total > 0 && <div className="absolute -top-1 -right-1 bg-cyan-500 text-bg text-[9px] font-bold px-1.5 py-0.5 rounded-full min-w-[18px] text-center shadow-[0_0_10px_rgba(0,229,255,0.5)]">{usage.total}</div>}</div><span className="text-[10px] uppercase tracking-tighter mt-1.5 opacity-60 font-medium">{p.name}</span><div className="flex flex-col items-center mt-1">{Object.entries(usage.models).map(([model, count]) => <span key={model} className="text-[8px] opacity-40 leading-tight">{model} ({count})</span>)}</div></div>; })}</div></div>;
}

export function KnowledgeNetwork({ nodes, edges }: { nodes: number; edges: number }) {
  return <div className="h-24 w-full bg-white/[0.02] border border-white/[0.05] relative p-3 overflow-hidden"><div className="flex justify-between items-center mb-2"><div className="flex flex-col"><span className="text-[20px] font-bold leading-none">{nodes}</span><span className="text-[8px] uppercase opacity-40">Nodes</span></div><div className="h-6 w-[1px] bg-white/10" /><div className="flex flex-col items-end"><span className="text-[20px] font-bold leading-none">{edges}</span><span className="text-[8px] uppercase opacity-40">Edges</span></div></div><svg width="100%" height="40" className="opacity-30"><circle cx="20%" cy="50%" r="2" fill="var(--cp-cyan)" /><circle cx="50%" cy="20%" r="3" fill="var(--cp-cyan)" /><circle cx="80%" cy="60%" r="2" fill="var(--cp-cyan)" /><circle cx="40%" cy="80%" r="1.5" fill="var(--cp-cyan)" /><line x1="20%" y1="50%" x2="50%" y2="20%" stroke="var(--cp-cyan)" strokeWidth="0.5" /><line x1="50%" y1="20%" x2="80%" y2="60%" stroke="var(--cp-cyan)" strokeWidth="0.5" /><line x1="80%" y1="60%" x2="40%" y2="80%" stroke="var(--cp-cyan)" strokeWidth="0.5" /><line x1="40%" y1="80%" x2="20%" y2="50%" stroke="var(--cp-cyan)" strokeWidth="0.5" /></svg></div>;
}

export function PanelHeader({ title, actionLabel, onAction }: { title: string; actionLabel: string; onAction?: () => void }) {
  return (
    <div className="panel-head panel-head-small">
      <h3>{title}</h3>
      <button type="button" className="text-btn icon-only" aria-label={actionLabel} title={actionLabel} onClick={onAction}>
        <span aria-hidden="true">↗</span>
      </button>
    </div>
  );
}

export function Row({ title, meta, detail, active = false, onClick }: { title: string; meta: string; detail: string; active?: boolean; onClick?: () => void }) {
  return <div className={`row ${active ? 'is-active' : ''}`} onClick={onClick} style={{ cursor: onClick ? 'pointer' : 'default' }}><div><div className="row-title">{title}</div><div className="row-meta">{meta}</div></div><div className="row-detail">{detail}</div></div>;
}

export function DetailBlock({ eyebrow, title, children }: { eyebrow: string; title: string; children: ReactNode }) {
  return <section className="detail-panel"><div className="eyebrow">{eyebrow}</div><h2>{title}</h2><div className="detail-stack">{children}</div></section>;
}

export function DetailRow({ label, value, tone = 'muted' }: { label: string; value: string; tone?: 'good' | 'muted' | 'warning' }) {
  return <div className="detail-row"><span>{label}</span><strong className={`tone-${tone}`}>{value}</strong></div>;
}
