import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Search, Download } from 'lucide-react';
import { Badge, DataTable, LoadingSkeleton } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';

const eventTypeIcons: Record<string, string> = {
  command_executed: '✓', command_intercepted: '🛡', command_approved: '✅',
  command_rejected: '❌', canary_triggered: '🐦', session_started: '→',
  session_ended: '←', agent_heartbeat: '💓', policy_violation: '⚠',
};

const actionBadge: Record<string, 'green' | 'red' | 'amber' | 'blue' | 'default'> = {
  allowed: 'green', denied: 'red', intercept: 'amber', alert: 'blue',
};

export default function Audit() {
  const { t } = useTranslation();
  const [search, setSearch] = useState('');
  const [filterType, setFilterType] = useState('all');

  const fetchEvents = useCallback(() => api.getAuditEvents({
    event_type: filterType !== 'all' ? filterType : undefined,
    limit: 100,
  }), [filterType]);

  const { data: events, loading, error, refresh } = useApi(fetchEvents, { pollMs: 15000 });

  const { data: auditStats } = useApi(
    () => api.getAuditStats(),
    { pollMs: 30000 }
  );

  const eventList = events || [];
  const types = [...new Set(eventList.map((e: any) => e.event_type))];
  const filtered = eventList.filter((e: any) => {
    if (search && !`${e.command} ${e.user} ${e.agent_id}`.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  const denied = eventList.filter((e: any) => e.result === 'denied').length;
  const allowed = eventList.filter((e: any) => e.result === 'allowed').length;

  const handleExport = async (format: string) => {
    try {
      const text = await api.exportAudit(format);
      const blob = new Blob([text], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `flowlink-audit.${format === 'json' ? 'json' : format === 'cef' ? 'cef' : 'leef'}`;
      a.click();
      URL.revokeObjectURL(url);
    } catch {}
  };

  if (loading && !events) return <LoadingSkeleton lines={8} />;

  return (
    <div className="space-y-6 fade-in">
      {error && !events && (
        <div className="flex flex-col items-center py-16 text-center">
          <div className="text-4xl mb-4 opacity-40">⚠️</div>
          <h3 className="text-lg font-semibold text-[var(--color-dim)]">{t('common.unable_connect')}</h3>
          <p className="mt-2 text-sm text-[var(--color-dim)] opacity-70">{error}</p>
          <button onClick={refresh} className="mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]">{t('common.retry')}</button>
        </div>
      )}

      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">{t('audit.total_events')}</div>
          <div className="mt-1 text-2xl font-bold">{(auditStats as any)?.total || eventList.length}</div>
        </div>
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">{t('audit.approved')}</div>
          <div className="mt-1 text-2xl font-bold text-emerald-400">{(auditStats as any)?.allowed || allowed}</div>
        </div>
        <div className="rounded-xl border border-rose-500/20 bg-rose-500/5 p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">{t('audit.denied')}</div>
          <div className="mt-1 text-2xl font-bold text-rose-400">{(auditStats as any)?.denied || denied}</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">{t('audit.denied_ratio')}</div>
          <div className="mt-1 text-2xl font-bold">{allowed + denied > 0 ? ((denied / (allowed + denied)) * 100).toFixed(1) : '0'}%</div>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <div className="relative flex-1 max-w-sm">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-dim)]" />
          <input type="text" placeholder={t('audit.search')} value={search} onChange={e => setSearch(e.target.value)}
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] py-2.5 pl-9 pr-3 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none" />
        </div>
        <select value={filterType} onChange={e => { setFilterType(e.target.value); setTimeout(refresh, 0); }}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
          <option value="all">{t('audit.all')}</option>
          {types.map(t => <option key={t} value={t}>{t.replace(/_/g, ' ')}</option>)}
        </select>
        <div className="flex gap-2">
          <button onClick={() => handleExport('json')} className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
            <Download size={14} /> JSON
          </button>
          <button onClick={() => handleExport('cef')} className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
            <Download size={14} /> CEF
          </button>
          <button onClick={() => handleExport('leef')} className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
            <Download size={14} /> LEEF
          </button>
        </div>
      </div>

      <DataTable
        columns={[
          { key: 'timestamp_iso', label: t('audit.timestamp'), render: (r: any) => <span className="text-xs text-[var(--color-dim)] whitespace-nowrap">{new Date(r.timestamp_iso).toLocaleString()}</span> },
          { key: 'event_type', label: t('audit.event'), render: (r: any) => (
            <span className="flex items-center gap-1.5">
              <span>{eventTypeIcons[r.event_type] || '•'}</span>
              <Badge variant={r.event_type === 'command_intercepted' ? 'red' : r.event_type === 'canary_triggered' ? 'amber' : 'default'}>
                {r.event_type.replace(/_/g, ' ')}
              </Badge>
            </span>
          )},
          { key: 'agent_id', label: t('audit.agent'), render: (r: any) => <span className="text-xs font-mono">{r.agent_id}</span> },
          { key: 'user', label: t('audit.user') },
          { key: 'command', label: t('audit.command'), render: (r: any) => r.command ? <code className="rounded bg-[var(--color-surface3)] px-1.5 py-0.5 text-xs">{r.command}</code> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'risk_score', label: t('audit.risk'), render: (r: any) => r.risk_score != null ? <span className={`font-mono text-xs font-bold ${r.risk_score >= 70 ? 'text-rose-400' : r.risk_score >= 40 ? 'text-amber-400' : 'text-emerald-400'}`}>{r.risk_score}</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'result', label: t('audit.result'), render: (r: any) => <Badge variant={actionBadge[r.result || ''] || 'default'}>{r.result || '—'}</Badge> },
        ]}
        data={filtered} emptyText={t('audit.no_events')}
      />
    </div>
  );
}
