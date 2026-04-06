import { useState } from 'react';
import { Search, Download, Filter } from 'lucide-react';
import { Badge, DataTable } from '../components/Layout';
import { mockAuditEvents } from '../api/client';
import type { AuditEvent } from '../types';

const eventTypeIcons: Record<string, string> = {
  command_executed: '✓', command_intercepted: '🛡', command_approved: '✅',
  command_rejected: '❌', canary_triggered: '🐦', session_started: '→',
  session_ended: '←', agent_heartbeat: '💓', policy_violation: '⚠',
};

const actionBadge: Record<string, 'green' | 'red' | 'amber' | 'blue' | 'default'> = {
  allowed: 'green', denied: 'red', intercept: 'amber', alert: 'blue',
};

export default function Audit() {
  const [search, setSearch] = useState('');
  const [filterType, setFilterType] = useState('all');

  const types = [...new Set(mockAuditEvents.map(e => e.event_type))];
  const filtered = mockAuditEvents.filter(e => {
    if (filterType !== 'all' && e.event_type !== filterType) return false;
    if (search && !`${e.command} ${e.user} ${e.agent_id}`.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  const denied = mockAuditEvents.filter(e => e.result === 'denied').length;
  const allowed = mockAuditEvents.filter(e => e.result === 'allowed').length;

  return (
    <div className="space-y-6 fade-in">
      {/* Stats */}
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Total Events</div>
          <div className="mt-1 text-2xl font-bold">{mockAuditEvents.length}</div>
        </div>
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Allowed</div>
          <div className="mt-1 text-2xl font-bold text-emerald-400">{allowed}</div>
        </div>
        <div className="rounded-xl border border-rose-500/20 bg-rose-500/5 p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Denied</div>
          <div className="mt-1 text-2xl font-bold text-rose-400">{denied}</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Deny Rate</div>
          <div className="mt-1 text-2xl font-bold">{((denied / (allowed + denied)) * 100).toFixed(1)}%</div>
        </div>
      </div>

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative flex-1 max-w-sm">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-dim)]" />
          <input type="text" placeholder="Search commands, users..." value={search} onChange={e => setSearch(e.target.value)}
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] py-2.5 pl-9 pr-3 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none" />
        </div>
        <select value={filterType} onChange={e => setFilterType(e.target.value)}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
          <option value="all">All Events</option>
          {types.map(t => <option key={t} value={t}>{t.replace(/_/g, ' ')}</option>)}
        </select>
        <div className="flex gap-2">
          <button className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
            <Download size={14} /> JSON
          </button>
          <button className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
            <Download size={14} /> CEF
          </button>
          <button className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
            <Download size={14} /> LEEF
          </button>
        </div>
      </div>

      {/* Table */}
      <DataTable
        columns={[
          { key: 'timestamp_iso', label: 'Time', render: (r: AuditEvent) => <span className="text-xs text-[var(--color-dim)] whitespace-nowrap">{new Date(r.timestamp_iso).toLocaleString()}</span> },
          { key: 'event_type', label: 'Event', render: (r: AuditEvent) => (
            <span className="flex items-center gap-1.5">
              <span>{eventTypeIcons[r.event_type] || '•'}</span>
              <Badge variant={r.event_type === 'command_intercepted' ? 'red' : r.event_type === 'canary_triggered' ? 'amber' : 'default'}>
                {r.event_type.replace(/_/g, ' ')}
              </Badge>
            </span>
          )},
          { key: 'agent_id', label: 'Agent', render: (r: AuditEvent) => <span className="text-xs font-mono">{r.agent_id}</span> },
          { key: 'user', label: 'User' },
          { key: 'command', label: 'Command', render: (r: AuditEvent) => r.command ? <code className="rounded bg-[var(--color-surface3)] px-1.5 py-0.5 text-xs">{r.command}</code> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'risk_score', label: 'Risk', render: (r: AuditEvent) => r.risk_score != null ? <span className={`font-mono text-xs font-bold ${r.risk_score >= 70 ? 'text-rose-400' : r.risk_score >= 40 ? 'text-amber-400' : 'text-emerald-400'}`}>{r.risk_score}</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'result', label: 'Result', render: (r: AuditEvent) => <Badge variant={actionBadge[r.result || ''] || 'default'}>{r.result || '—'}</Badge> },
        ]}
        data={filtered} emptyText="No matching audit events"
      />
    </div>
  );
}
