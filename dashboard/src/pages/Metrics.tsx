import { useState } from 'react';
import { Activity, Cpu, MemoryStick, CheckCircle, XCircle } from 'lucide-react';
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, LoadingSkeleton } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import { mockSystemInfo, mockAgents, mockPromMetrics } from '../api/client';

const cpuData = Array.from({ length: 30 }, (_, i) => ({ t: i, cpu: Math.round(Math.random() * 40 + 15) }));
const memData = Array.from({ length: 30 }, (_, i) => ({ t: i, mem: Math.round(Math.random() * 20 + 35) }));

export default function Metrics() {
  const { data: agents, loading: agentsLoading, isLive: agentsLive } = useApi(
    () => api.getAgents(),
    mockAgents,
    { pollMs: 15000 }
  );

  const { data: promText, loading: metricsLoading, isLive: metricsLive } = useApi(
    () => api.getMetrics(),
    mockPromMetrics,
    { pollMs: 10000 }
  );

  const { data: health, isLive: healthLive } = useApi(
    () => api.getHealth(),
    { status: 'ok' },
    { pollMs: 10000 }
  );

  const { data: systemInfo } = useApi(
    async () => mockSystemInfo, // System info from relay config (TODO: dedicated endpoint)
    mockSystemInfo,
  );

  const loading = agentsLoading || metricsLoading;
  const info = { ...mockSystemInfo, ...systemInfo };

  if (loading) return <LoadingSkeleton lines={8} />;

  return (
    <div className="space-y-6 fade-in">
      {!healthLive && (
        <div className="rounded-xl border border-amber-500/30 bg-amber-500/5 px-4 py-3 text-sm text-amber-400">
          ⚠️ Connected to mock data. Start relay for live data.
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <div className="flex items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          {healthLive ? <CheckCircle size={20} className="text-emerald-400" /> : <XCircle size={20} className="text-amber-400" />}
          <div>
            <div className="font-medium">Relay</div>
            <div className="text-xs text-[var(--color-dim)]">{healthLive ? `${info.version} · Connected` : 'Offline'}</div>
          </div>
          <Badge variant={healthLive ? 'green' : 'amber'} className="ml-auto">{healthLive ? 'healthy' : 'offline'}</Badge>
        </div>
        <div className="flex items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <CheckCircle size={20} className="text-emerald-400" />
          <div>
            <div className="font-medium">SSE</div>
            <div className="text-xs text-[var(--color-dim)]">Real-time events</div>
          </div>
          <Badge variant="green" className="ml-auto">ready</Badge>
        </div>
        <div className="flex items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          {agents.filter((a: any) => a.status === 'online').length === agents.length ? <CheckCircle size={20} className="text-emerald-400" /> : <XCircle size={20} className="text-amber-400" />}
          <div>
            <div className="font-medium">Agents</div>
            <div className="text-xs text-[var(--color-dim)]">{agents.filter((a: any) => a.status === 'online').length}/{agents.length} online</div>
          </div>
          <Badge variant={agents.filter((a: any) => a.status === 'online').length === agents.length ? 'green' : 'amber'} className="ml-auto">
            {agents.filter((a: any) => a.status === 'online').length === agents.length ? 'healthy' : 'degraded'}
          </Badge>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <div className="flex items-center gap-2 mb-4">
            <Cpu size={16} className="text-[var(--color-accent)]" />
            <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">CPU Usage</h3>
            <span className="ml-auto text-lg font-bold">{info.cpu_usage}%</span>
          </div>
          <ResponsiveContainer width="100%" height={160}>
            <LineChart data={cpuData}>
              <XAxis dataKey="t" tick={false} axisLine={false} />
              <YAxis tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} domain={[0, 100]} tickFormatter={(v: number) => `${v}%`} />
              <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
              <Line type="monotone" dataKey="cpu" stroke="#6366f1" strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <div className="flex items-center gap-2 mb-4">
            <MemoryStick size={16} className="text-emerald-400" />
            <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Memory Usage</h3>
            <span className="ml-auto text-lg font-bold">{info.memory_usage}%</span>
          </div>
          <ResponsiveContainer width="100%" height={160}>
            <LineChart data={memData}>
              <XAxis dataKey="t" tick={false} axisLine={false} />
              <YAxis tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} domain={[0, 100]} tickFormatter={(v: number) => `${v}%`} />
              <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
              <Line type="monotone" dataKey="mem" stroke="#10b981" strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Agent Resources</h3>
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
          {agents.filter((a: any) => a.status === 'online').map((a: any) => (
            <div key={a.id} className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
              <div className="flex items-center gap-2 mb-3">
                <div className="h-2 w-2 rounded-full bg-emerald-400 pulse-dot" />
                <span className="text-sm font-medium">{a.hostname}</span>
              </div>
              {[
                { label: 'CPU', val: a.cpu ?? 0, color: a.cpu > 80 ? '#f43f5e' : '#10b981' },
                { label: 'RAM', val: a.ram ?? 0, color: a.ram > 80 ? '#f43f5e' : '#10b981' },
                { label: 'Disk', val: a.disk ?? 0, color: a.disk > 80 ? '#f59e0b' : '#10b981' },
              ].map((m: any) => (
                <div key={m.label} className="mb-2">
                  <div className="flex justify-between text-xs mb-1">
                    <span className="text-[var(--color-dim)]">{m.label}</span>
                    <span className="font-mono" style={{ color: m.color }}>{m.val}%</span>
                  </div>
                  <div className="h-1.5 rounded-full bg-[var(--color-surface3)]">
                    <div className="h-full rounded-full transition-all" style={{ width: `${m.val}%`, backgroundColor: m.color }} />
                  </div>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Activity size={16} className="text-[var(--color-accent)]" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Prometheus Metrics {metricsLive && <span className="ml-2 text-emerald-400 text-xs">● Live</span>}</h3>
        </div>
        <pre className="rounded-xl bg-[#0d0e14] p-4 font-mono text-xs leading-relaxed text-[var(--color-dim)] overflow-x-auto max-h-96 overflow-y-auto">
          {promText}
        </pre>
      </div>
    </div>
  );
}
