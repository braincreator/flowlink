import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { LineChart, Line, AreaChart, Area, PieChart, Pie, Cell, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Shield, Bot, AlertTriangle, Activity, Clock, Zap } from 'lucide-react';
import { StatCard, Badge } from '../components/Layout';
import { mockDashboardStats, mockCommandsOver24h, mockAlerts, mockAuditEvents, mockInterceptionsOverTime } from '../api/client';

const PIE_COLORS = ['#f43f5e', '#f59e0b', '#6366f1'];

export default function Dashboard() {
  const navigate = useNavigate();
  const s = mockDashboardStats;
  const pendingAlerts = mockAlerts.filter(a => !a.resolved);
  const recentEvents = mockAuditEvents.slice(0, 20);
  const pieData = [
    { name: 'L3 Critical', value: s.l3_count },
    { name: 'L2 Medium', value: s.l2_count },
    { name: 'L1 Low', value: s.l1_count },
  ];

  return (
    <div className="space-y-6 fade-in">
      {/* Stats */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard label="Total Agents" value={`${s.online_agents}/${s.total_agents}`} trend={{ value: 12, label: 'this week' }} icon={<Bot size={24} />} color="accent" />
        <StatCard label="Active Alerts" value={s.active_alerts} trend={{ value: -8, label: 'vs yesterday' }} icon={<AlertTriangle size={24} />} color="red" />
        <StatCard label="Commands Today" value={s.commands_today.toLocaleString()} trend={{ value: 5, label: 'vs yesterday' }} icon={<Activity size={24} />} color="green" />
        <StatCard label="Uptime" value={`${s.uptime_pct}%`} icon={<Clock size={24} />} color="blue" />
      </div>

      {/* Quick Actions */}
      <div className="flex flex-wrap gap-3">
        <button onClick={() => navigate('/shield')} className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] hover:shadow-lg hover:shadow-indigo-500/20">
          <Shield size={16} /> Approve Pending ({pendingAlerts.length})
        </button>
        <button onClick={() => navigate('/shield')} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
          <AlertTriangle size={16} /> View Alerts
        </button>
        <button onClick={() => navigate('/agents')} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
          <Bot size={16} /> Deploy Agent
        </button>
      </div>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
        {/* Commands chart */}
        <div className="col-span-1 xl:col-span-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Commands — Last 24h</h3>
          <ResponsiveContainer width="100%" height={240}>
            <AreaChart data={mockCommandsOver24h}>
              <defs>
                <linearGradient id="cmdGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#6366f1" stopOpacity={0.3} />
                  <stop offset="100%" stopColor="#6366f1" stopOpacity={0} />
                </linearGradient>
              </defs>
              <XAxis dataKey="hour" tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} tickLine={false} />
              <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
              <Area type="monotone" dataKey="commands" stroke="#6366f1" fill="url(#cmdGrad)" strokeWidth={2} />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Shield Status */}
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Shield Status</h3>
          <div className="flex items-center justify-center">
            <ResponsiveContainer width="100%" height={180}>
              <PieChart>
                <Pie data={pieData} cx="50%" cy="50%" innerRadius={50} outerRadius={75} paddingAngle={4} dataKey="value">
                  {pieData.map((_, i) => <Cell key={i} fill={PIE_COLORS[i]} />)}
                </Pie>
                <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
              </PieChart>
            </ResponsiveContainer>
          </div>
          <div className="mt-2 flex justify-center gap-4 text-xs">
            <span className="flex items-center gap-1"><span className="h-2 w-2 rounded-full bg-rose-500" /> L3: {s.l3_count}</span>
            <span className="flex items-center gap-1"><span className="h-2 w-2 rounded-full bg-amber-500" /> L2: {s.l2_count}</span>
            <span className="flex items-center gap-1"><span className="h-2 w-2 rounded-full bg-indigo-500" /> L1: {s.l1_count}</span>
          </div>
        </div>
      </div>

      {/* Activity Feed */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Recent Activity</h3>
        <div className="space-y-2 max-h-[360px] overflow-y-auto">
          {recentEvents.map(ev => {
            const icons: Record<string, string> = {
              command_executed: '✓', command_intercepted: '🛡', session_started: '→',
              session_ended: '←', canary_triggered: '🐦', policy_violation: '⚠', agent_heartbeat: '💓',
            };
            const badgeMap: Record<string, 'green' | 'red' | 'amber' | 'blue' | 'default'> = {
              command_executed: 'green', command_intercepted: 'red', session_started: 'blue',
              canary_triggered: 'amber', policy_violation: 'red', agent_heartbeat: 'default',
            };
            return (
              <div key={ev.id} className="flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors hover:bg-[var(--color-surface2)]">
                <span className="text-base">{icons[ev.event_type] || '•'}</span>
                <Badge variant={badgeMap[ev.event_type] || 'default'}>{ev.event_type.replace(/_/g, ' ')}</Badge>
                <span className="flex-1 truncate text-sm">
                  {ev.command || `${ev.user || 'system'} — ${ev.event_type.replace(/_/g, ' ')}`}
                </span>
                {ev.risk_score && <span className="text-xs font-mono text-rose-400">risk:{ev.risk_score}</span>}
                <span className="text-xs text-[var(--color-dim)]">{new Date(ev.timestamp_iso).toLocaleTimeString()}</span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
