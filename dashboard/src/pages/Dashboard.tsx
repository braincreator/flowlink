import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { AreaChart, Area, PieChart, Pie, Cell, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Shield, Bot, AlertTriangle, Activity, Clock, TerminalSquare } from 'lucide-react';
import { StatCard, Badge, LoadingSkeleton } from '../components/Layout';
import { DashboardWidgets, type WidgetDef } from '../components/DashboardWidgets';
import { useApi, useSSE } from '../hooks/useApi';
import { api } from '../api/client';

const PIE_COLORS = ['#f43f5e', '#f59e0b', '#6366f1'];

export default function Dashboard() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data: agents, loading: agentsLoading, error: agentsError, refresh: refreshAgents } = useApi(
    () => api.getAgents(),
    { pollMs: 15000 }
  );
  const { data: shieldStats, loading: shieldLoading, error: shieldError, refresh: refreshShield } = useApi(
    () => api.getShieldStats(),
    { pollMs: 15000 }
  );
  const { data: auditStats, loading: auditLoading } = useApi(
    () => api.getAuditStats(),
    { pollMs: 30000 }
  );
  const { data: alerts, loading: alertsLoading, error: alertsError, refresh: refreshAlerts } = useApi(
    () => api.getAlerts(),
    { pollMs: 10000 }
  );
  const { data: auditEvents, loading: eventsLoading } = useApi(
    () => api.getAuditEvents({ limit: 20 }),
    { pollMs: 15000 }
  );
  const { events: sseEvents, connected: sseConnected } = useSSE();

  const loading = agentsLoading || shieldLoading || auditLoading;
  const error = agentsError || shieldError || alertsError;

  const agentList = agents || [];
  const onlineAgents = agentList.filter((a: any) => a.status === 'online').length;
  const totalAgents = agentList.length;
  const s = { ...(shieldStats as any), ...(auditStats as any) };
  const alertList = alerts || [];
  const pendingAlerts = alertList.filter((a: any) => !a.resolved);
  const eventList = auditEvents || [];
  const recentEvents = sseConnected && sseEvents.length > 0 ? sseEvents.slice(0, 20) : eventList.slice(0, 20);
  const pieData = [
    { name: 'L3 Critical', value: (s as any).l3_count || 0 },
    { name: 'L2 Medium', value: (s as any).l2_count || 0 },
    { name: 'L1 Low', value: (s as any).l1_count || 0 },
  ];

  if (loading && !agents && !shieldStats) return <LoadingSkeleton lines={8} />;

  const widgets: WidgetDef[] = [
    {
      id: 'stat-cards', title: t('dashboard.statistics'), defaultOrder: 0,
      render: () => (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <StatCard label={t('dashboard.agents_online')} value={`${onlineAgents}/${totalAgents}`} trend={{ value: 12, label: t('dashboard.this_week') }} icon={<Bot size={24} />} color="accent" />
          <StatCard label={t('dashboard.active_alerts')} value={pendingAlerts.length} trend={{ value: -8, label: t('dashboard.vs_yesterday') }} icon={<AlertTriangle size={24} />} color="red" />
          <StatCard label={t('dashboard.commands_today')} value={((s as any).commands_today || 0).toLocaleString()} trend={{ value: 5, label: 'vs yesterday' }} icon={<Activity size={24} />} color="green" />
          <StatCard label={t('dashboard.uptime')} value={`${(s as any).uptime_pct || 0}%`} icon={<Clock size={24} />} color="blue" />
        </div>
      ),
    },
    {
      id: 'quick-actions', title: t('dashboard.quick_actions'), defaultOrder: 1,
      render: () => (
        <div>
          <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('dashboard.quick_actions')}</h3>
          <div className="flex flex-wrap gap-3">
            <button onClick={() => navigate('/shield')} className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] hover:shadow-lg hover:shadow-indigo-500/20">
              <Shield size={16} /> {t('dashboard.approve_pending')} ({pendingAlerts.length})
            </button>
            <button onClick={() => navigate('/shield')} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
              <AlertTriangle size={16} /> {t('dashboard.view_alerts')}
            </button>
            <button onClick={() => navigate('/agents')} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
              <Bot size={16} /> {t('dashboard.deploy_agent')}
            </button>
            <button onClick={() => navigate('/terminal')} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
              <TerminalSquare size={16} /> {t('nav.terminal')}
            </button>
          </div>
        </div>
      ),
    },
    {
      id: 'commands-chart', title: t('dashboard.commands_24h'), defaultOrder: 2, colSpan: 2,
      render: () => (
        <div>
          <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('dashboard.last_24h')}</h3>
          <ResponsiveContainer width="100%" height={240}>
            <AreaChart data={[]}>
              <defs><linearGradient id="cmdGrad" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stopColor="#6366f1" stopOpacity={0.3} /><stop offset="100%" stopColor="#6366f1" stopOpacity={0} /></linearGradient></defs>
              <XAxis dataKey="hour" tick={{ fontSize: 11, fill: 'var(--color-dim)' }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fontSize: 11, fill: 'var(--color-dim)' }} axisLine={false} tickLine={false} />
              <Tooltip contentStyle={{ background: 'var(--color-surface2)', border: '1px solid var(--color-border)', borderRadius: '8px', fontSize: '12px' }} />
              <Area type="monotone" dataKey="commands" stroke="#6366f1" fill="url(#cmdGrad)" strokeWidth={2} />
            </AreaChart>
          </ResponsiveContainer>
          <div className="flex items-center justify-center py-8 text-sm text-[var(--color-dim)] opacity-60">{t('common.no_time_series')}</div>
        </div>
      ),
    },
    {
      id: 'risk-pie', title: t('dashboard.shield_status'), defaultOrder: 3,
      render: () => (
        <div>
          <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('dashboard.shield_status')}</h3>
          <div className="flex items-center justify-center">
            <ResponsiveContainer width="100%" height={180}>
              <PieChart>
                <Pie data={pieData} cx="50%" cy="50%" innerRadius={50} outerRadius={75} paddingAngle={4} dataKey="value">
                  {pieData.map((_, i) => <Cell key={i} fill={PIE_COLORS[i]} />)}
                </Pie>
                <Tooltip contentStyle={{ background: 'var(--color-surface2)', border: '1px solid var(--color-border)', borderRadius: '8px', fontSize: '12px' }} />
              </PieChart>
            </ResponsiveContainer>
          </div>
          <div className="mt-2 flex justify-center gap-4 text-xs">
            <span className="flex items-center gap-1"><span className="h-2 w-2 rounded-full bg-rose-500" /> L3: {(s as any).l3_count || 0}</span>
            <span className="flex items-center gap-1"><span className="h-2 w-2 rounded-full bg-amber-500" /> L2: {(s as any).l2_count || 0}</span>
            <span className="flex items-center gap-1"><span className="h-2 w-2 rounded-full bg-indigo-500" /> L1: {(s as any).l1_count || 0}</span>
          </div>
        </div>
      ),
    },
    {
      id: 'activity-feed', title: t('dashboard.activity_feed'), defaultOrder: 4,
      render: () => (
        <div>
          <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('dashboard.activity_feed')} {sseConnected && <span className="ml-2 text-emerald-400">● Live</span>}</h3>
          {recentEvents.length === 0 ? (
            <div className="flex items-center justify-center py-12 text-sm text-[var(--color-dim)] opacity-60">{t('common.no_recent_activity')}</div>
          ) : (
            <div className="space-y-2 max-h-[360px] overflow-y-auto">
              {recentEvents.map((ev: any, i: number) => {
                const icons: Record<string, string> = {
                  command_executed: '✓', command_intercepted: '🛡', session_started: '→',
                  session_ended: '←', canary_triggered: '🐦', policy_violation: '⚠', agent_heartbeat: '💓',
                };
                const badgeMap: Record<string, 'green' | 'red' | 'amber' | 'blue' | 'default'> = {
                  command_executed: 'green', command_intercepted: 'red', session_started: 'blue',
                  canary_triggered: 'amber', policy_violation: 'red', agent_heartbeat: 'default',
                };
                const eventType = ev.event_type || ev.type || 'unknown';
                const ts = ev.timestamp_iso || ev.timestamp || ev.time;
                return (
                  <div key={ev.id || i} className="flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors hover:bg-[var(--color-surface2)]">
                    <span className="text-base">{icons[eventType] || '•'}</span>
                    <Badge variant={badgeMap[eventType] || 'default'}>{eventType.replace(/_/g, ' ')}</Badge>
                    <span className="flex-1 truncate text-sm">
                      {ev.command || `${ev.user || ev.username || 'system'} — ${eventType.replace(/_/g, ' ')}`}
                    </span>
                    {ev.risk_score && <span className="text-xs font-mono text-rose-400">risk:{ev.risk_score}</span>}
                    <span className="text-xs text-[var(--color-dim)]">{ts ? new Date(ts).toLocaleTimeString() : ''}</span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      ),
    },
  ];

  return (
    <div className="space-y-6 fade-in">
      {error && !agents && !shieldStats && (
        <div className="flex flex-col items-center py-16 text-center">
          <div className="text-4xl mb-4 opacity-40">⚠️</div>
          <h3 className="text-lg font-semibold text-[var(--color-dim)]">{t('common.unable_connect')}</h3>
          <p className="mt-2 text-sm text-[var(--color-dim)] opacity-70">{error}</p>
          <button onClick={() => { refreshAgents(); refreshShield(); refreshAlerts(); }} className="mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]">{t('common.retry')}</button>
        </div>
      )}

      <DashboardWidgets widgets={widgets} />
    </div>
  );
}
