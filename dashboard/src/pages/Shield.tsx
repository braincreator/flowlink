import { useState } from 'react';
import { ShieldAlert as ShieldAlertIcon, ShieldCheck, Bird, FileCode } from 'lucide-react';
import { AreaChart, Area, BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, RiskGauge, LoadingSkeleton, EmptyState } from '../components/Layout';
import { useApi, useSSE } from '../hooks/useApi';
import { api } from '../api/client';

export default function Shield() {
  const [tab, setTab] = useState<'alerts' | 'canaries' | 'policies'>('alerts');

  const { connected: sseConnected } = useSSE();

  const { data: alerts, loading: alertsLoading, error: alertsError, refresh: refreshAlerts } = useApi(
    () => api.getAlerts(),
    { pollMs: 5000 }
  );

  const { data: stats, loading: statsLoading } = useApi(
    () => api.getShieldStats(),
    { pollMs: 15000 }
  );

  const { data: canaries, loading: canariesLoading } = useApi(
    () => api.getCanaries(),
    { pollMs: 30000 }
  );

  const { data: policies, loading: policiesLoading } = useApi(
    () => api.getPolicies(),
    { pollMs: 30000 }
  );

  const loading = alertsLoading || statsLoading;
  const alertList = alerts || [];
  const canaryList = canaries || [];
  const policyList = policies || [];
  const s = (stats as any) || {};
  const pending = alertList.filter((a: any) => !a.resolved);

  const resolve = async (alertId: string, approved: boolean) => {
    try {
      if (approved) await api.approveAlert(alertId);
      else await api.rejectAlert(alertId);
    } catch { /* keep optimistic state */ }
    refreshAlerts();
  };

  const levelColor = (l: string) => l === 'L3' ? 'red' : l === 'L2' ? 'amber' : 'blue';
  const levelBg = (l: string) => l === 'L3' ? 'border-rose-500/30 bg-rose-500/5' : l === 'L2' ? 'border-amber-500/30 bg-amber-500/5' : 'border-blue-500/30 bg-blue-500/5';

  if (loading && !alerts && !stats) return <LoadingSkeleton lines={8} />;

  return (
    <div className="space-y-6 fade-in">
      {alertsError && !alerts && (
        <div className="flex flex-col items-center py-16 text-center">
          <div className="text-4xl mb-4 opacity-40">⚠️</div>
          <h3 className="text-lg font-semibold text-[var(--color-dim)]">Unable to connect to relay</h3>
          <p className="mt-2 text-sm text-[var(--color-dim)] opacity-70">{alertsError}</p>
          <button onClick={refreshAlerts} className="mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]">Retry</button>
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-xl border border-rose-500/20 bg-gradient-to-br from-rose-500/10 to-transparent p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Pending</div>
          <div className="mt-1 text-2xl font-bold text-rose-400">{pending.length}</div>
        </div>
        <div className="rounded-xl border border-emerald-500/20 bg-gradient-to-br from-emerald-500/10 to-transparent p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Approved</div>
          <div className="mt-1 text-2xl font-bold text-emerald-400">{alertList.filter((a: any) => a.approved === true).length}</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Total Intercepts</div>
          <div className="mt-1 text-2xl font-bold">{alertList.length}</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">SSE</div>
          <div className="mt-1 text-2xl font-bold">{sseConnected ? <span className="text-emerald-400">● Live</span> : <span className="text-[var(--color-dim)]">○ Off</span>}</div>
        </div>
      </div>

      <div className="flex gap-1 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-1">
        {[
          { key: 'alerts' as const, label: 'Alerts', icon: ShieldAlertIcon, count: pending.length },
          { key: 'canaries' as const, label: 'Canaries', icon: Bird, count: canaryList.length },
          { key: 'policies' as const, label: 'Policy Rules', icon: FileCode, count: policyList.length },
        ].map(t => (
          <button key={t.key} onClick={() => setTab(t.key)}
            className={`flex flex-1 items-center justify-center gap-2 rounded-lg py-2.5 text-sm font-medium transition-colors ${tab === t.key ? 'bg-[var(--color-accent)] text-white' : 'text-[var(--color-dim)] hover:text-[var(--color-text)]'}`}>
            <t.icon size={16} /> {t.label} {t.count > 0 && <span className={`rounded-full px-1.5 py-0.5 text-[10px] ${tab === t.key ? 'bg-white/20' : 'bg-[var(--color-surface3)]'}`}>{t.count}</span>}
          </button>
        ))}
      </div>

      {tab === 'alerts' && (
        <div className="space-y-3">
          {pending.length === 0 && (
            <div className="flex flex-col items-center py-12 text-[var(--color-dim)]">
              <ShieldCheck size={48} className="mb-3 text-emerald-500/50" />
              <div className="text-lg font-medium">All clear!</div>
              <div className="text-sm">No pending alerts</div>
            </div>
          )}
          {pending.map((alert: any) => (
            <div key={alert.alert_id} className={`rounded-xl border p-4 ${levelBg(alert.threat_level)}`}>
              <div className="flex items-start justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-2">
                    <Badge variant={levelColor(alert.threat_level) as any}>{alert.threat_level}</Badge>
                    <Badge variant="purple">{alert.rule_name}</Badge>
                    <span className="text-xs text-[var(--color-dim)]">{alert.username}@{alert.agent_id}</span>
                  </div>
                  <div className="rounded-lg bg-[var(--color-bg)] p-3 font-mono text-sm">{alert.command}</div>
                  <div className="mt-2 flex items-center gap-4 text-xs text-[var(--color-dim)]">
                    <span>PID: {alert.pid}</span>
                    <span>UID: {alert.uid}</span>
                    <span>{new Date(alert.timestamp).toLocaleString()}</span>
                  </div>
                </div>
                <div className="flex flex-col items-center gap-2">
                  <RiskGauge score={alert.risk_score} size={72} />
                  <div className="flex gap-2">
                    <button onClick={() => resolve(alert.alert_id, true)} className="rounded-lg bg-emerald-500/20 px-3 py-1.5 text-xs font-medium text-emerald-400 transition-colors hover:bg-emerald-500/30">Approve</button>
                    <button onClick={() => resolve(alert.alert_id, false)} className="rounded-lg bg-rose-500/20 px-3 py-1.5 text-xs font-medium text-rose-400 transition-colors hover:bg-rose-500/30">Reject</button>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {tab === 'canaries' && (
        canaryList.length === 0 ? (
          <EmptyState icon={<Bird size={48} />} title="No canaries configured" description="Add canary tokens to detect file access" />
        ) : (
          <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] overflow-hidden">
            <table className="w-full text-sm">
              <thead><tr className="border-b border-[var(--color-border)]">
                <th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--color-dim)]">Path</th>
                <th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--color-dim)]">Agent</th>
                <th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--color-dim)]">Triggers</th>
                <th className="px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--color-dim)]">Last Triggered</th>
              </tr></thead>
              <tbody>
                {canaryList.map((c: any, i: number) => (
                  <tr key={i} className="border-b border-[var(--color-border)] hover:bg-[var(--color-surface2)]">
                    <td className="px-4 py-3 font-mono text-xs">{c.path}</td>
                    <td className="px-4 py-3 text-[var(--color-dim)]">{c.agent_id}</td>
                    <td className="px-4 py-3"><Badge variant={(c.triggers_count || 0) > 0 ? 'amber' : 'default'}>{c.triggers_count}</Badge></td>
                    <td className="px-4 py-3 text-xs text-[var(--color-dim)]">{c.last_triggered ? new Date(c.last_triggered).toLocaleString() : 'Never'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )
      )}

      {tab === 'policies' && (
        policyList.length === 0 ? (
          <EmptyState icon={<FileCode size={48} />} title="No policies configured" description="Add policy rules to control command execution" />
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {policyList.map((p: any, i: number) => (
              <div key={i} className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
                <div className="flex items-center justify-between mb-2">
                  <div className="font-medium">{p.name}</div>
                  <Badge variant={p.action === 'deny' ? 'red' : p.action === 'intercept' ? 'amber' : p.action === 'allow' ? 'green' : 'blue'}>{p.action}</Badge>
                </div>
                <div className="text-xs text-[var(--color-dim)] mb-2">Priority: {p.priority}</div>
                <div className="rounded-lg bg-[var(--color-bg)] p-3 font-mono text-xs text-[var(--color-dim)]">
                  {Object.entries(p.conditions || {}).map(([k, v]) => <div key={k}><span className="text-[var(--color-accent-light)]">{k}</span>: {String(v)}</div>)}
                </div>
              </div>
            ))}
          </div>
        )
      )}

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Interceptions Over Time</h3>
          <ResponsiveContainer width="100%" height={200}>
            <AreaChart data={[]}>
              <defs><linearGradient id="intGrad" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stopColor="#f43f5e" stopOpacity={0.3} /><stop offset="100%" stopColor="#f43f5e" stopOpacity={0} /></linearGradient></defs>
              <XAxis dataKey="date" tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
              <YAxis tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
              <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
              <Area type="monotone" dataKey="interceptions" stroke="#f43f5e" fill="url(#intGrad)" strokeWidth={2} />
            </AreaChart>
          </ResponsiveContainer>
          <div className="flex items-center justify-center py-4 text-sm text-[var(--color-dim)] opacity-60">No time-series data available yet</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Top Dangerous Commands</h3>
          <ResponsiveContainer width="100%" height={200}>
            <BarChart data={[]} layout="vertical">
              <XAxis type="number" tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
              <YAxis dataKey="command" type="category" width={120} tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
              <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
              <Bar dataKey="count" fill="#f59e0b" radius={[0, 4, 4, 0]} />
            </BarChart>
          </ResponsiveContainer>
          <div className="flex items-center justify-center py-4 text-sm text-[var(--color-dim)] opacity-60">No time-series data available yet</div>
        </div>
      </div>
    </div>
  );
}
