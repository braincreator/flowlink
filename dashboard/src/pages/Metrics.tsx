import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BarChart3, Cpu, MemoryStick, CheckCircle, XCircle, AlertTriangle, Download } from 'lucide-react';
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, LoadingSkeleton, EmptyState } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import { exportChartImage } from '../utils/chartExport';

export default function Metrics() {
  const { t } = useTranslation();
  const { data: agents, loading: agentsLoading, error: agentsError, refresh: refreshAgents } = useApi(
    () => api.getAgents(),
    { pollMs: 15000 }
  );

  const { data: promText, loading: metricsLoading, error: metricsError, refresh: refreshMetrics } = useApi(
    () => api.getMetrics(),
    { pollMs: 10000 }
  );

  const { data: health, error: healthError, refresh: refreshHealth } = useApi(
    () => api.getHealth(),
    { pollMs: 10000 }
  );

  const { data: systemInfo, error: sysInfoError } = useApi(
    () => api.getSystemInfo(),
    { pollMs: 30000 }
  );

  const loading = agentsLoading || metricsLoading;
  const error = agentsError || metricsError || healthError;
  const info = (systemInfo as any) || {};
  const agentList = agents || [];
  const healthStatus = (health as any)?.status === 'ok';

  if (loading && !agents && !promText) return <LoadingSkeleton lines={8} />;

  return (
    <div className="space-y-6 fade-in">
      {error && !agents && !promText && (
        <div className="flex flex-col items-center py-16 text-center">
          <AlertTriangle size={40} className="mb-4 text-[var(--color-dim)] opacity-40" />
          <h3 className="text-lg font-semibold text-[var(--color-dim)]">{t('common.unable_connect')}</h3>
          <p className="mt-2 text-sm text-[var(--color-dim)] opacity-70">{error}</p>
          <button onClick={() => { refreshAgents(); refreshMetrics(); refreshHealth(); }} className="mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]">{t('common.retry')}</button>
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <div className="flex items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          {healthStatus ? <CheckCircle size={20} className="text-emerald-400" /> : <XCircle size={20} className="text-amber-400" />}
          <div>
            <div className="font-medium">{t('metrics.relay')}</div>
            <div className="text-xs text-[var(--color-dim)]">{info.version ? `${info.version} · Connected` : t('metrics.checking')}</div>
          </div>
          <Badge variant={healthStatus ? 'green' : 'amber'} className="ml-auto">{healthStatus ? t('metrics.healthy') : t('metrics.offline')}</Badge>
        </div>
        <div className="flex items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <CheckCircle size={20} className="text-emerald-400" />
          <div>
            <div className="font-medium">{t('metrics.sse')}</div>
            <div className="text-xs text-[var(--color-dim)]">{t('metrics.real_time_events')}</div>
          </div>
          <Badge variant="green" className="ml-auto">{t('metrics.ready')}</Badge>
        </div>
        <div className="flex items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          {agentList.length > 0 && agentList.filter((a: any) => a.status === 'online').length === agentList.length ? <CheckCircle size={20} className="text-emerald-400" /> : <XCircle size={20} className="text-amber-400" />}
          <div>
            <div className="font-medium">{t('metrics.agents')}</div>
            <div className="text-xs text-[var(--color-dim)]">{agentList.filter((a: any) => a.status === 'online').length}/{agentList.length} online</div>
          </div>
          <Badge variant={agentList.length > 0 && agentList.filter((a: any) => a.status === 'online').length === agentList.length ? 'green' : 'amber'} className="ml-auto">
            {agentList.length > 0 && agentList.filter((a: any) => a.status === 'online').length === agentList.length ? t('metrics.healthy') : t('metrics.degraded')}
          </Badge>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <div id="chart-cpu" className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <div className="flex items-center gap-2 mb-4">
            <Cpu size={16} className="text-[var(--color-accent)]" />
            <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('metrics.cpu')}</h3>
            <span className="ml-auto text-lg font-bold">{info.cpu_usage ?? 0}%</span>
            <button onClick={() => exportChartImage('chart-cpu', 'flowlink-cpu')}
              className="ml-auto rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors"
              aria-label="Export chart as image"
              title="Export as PNG">
              <Download size={14} />
            </button>
          </div>
          <ResponsiveContainer width="100%" height={160}>
            <LineChart data={[]}>
              <XAxis dataKey="t" tick={false} axisLine={false} />
              <YAxis tick={{ fontSize: 11, fill: 'var(--color-dim)' }} axisLine={false} domain={[0, 100]} tickFormatter={(v: number) => `${v}%`} />
              <Tooltip contentStyle={{ background: 'var(--color-surface2)', border: '1px solid var(--color-border)', borderRadius: '8px', fontSize: '12px', color: 'var(--color-text)' }} />
              <Line type="monotone" dataKey="cpu" stroke="#6366f1" strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
          <div className="flex items-center justify-center py-4 text-sm text-[var(--color-dim)] opacity-60">{t('common.no_time_series_short')}</div>
        </div>
        <div id="chart-memory" className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <div className="flex items-center gap-2 mb-4">
            <MemoryStick size={16} className="text-emerald-400" />
            <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('metrics.memory')}</h3>
            <span className="ml-auto text-lg font-bold">{info.memory_usage ?? 0}%</span>
            <button onClick={() => exportChartImage('chart-memory', 'flowlink-memory')}
              className="ml-auto rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors"
              aria-label="Export chart as image"
              title="Export as PNG">
              <Download size={14} />
            </button>
          </div>
          <ResponsiveContainer width="100%" height={160}>
            <LineChart data={[]}>
              <XAxis dataKey="t" tick={false} axisLine={false} />
              <YAxis tick={{ fontSize: 11, fill: 'var(--color-dim)' }} axisLine={false} domain={[0, 100]} tickFormatter={(v: number) => `${v}%`} />
              <Tooltip contentStyle={{ background: 'var(--color-surface2)', border: '1px solid var(--color-border)', borderRadius: '8px', fontSize: '12px', color: 'var(--color-text)' }} />
              <Line type="monotone" dataKey="mem" stroke="#10b981" strokeWidth={2} dot={false} />
            </LineChart>
          </ResponsiveContainer>
          <div className="flex items-center justify-center py-4 text-sm text-[var(--color-dim)] opacity-60">{t('common.no_time_series_short')}</div>
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('metrics.agent_resources')}</h3>
        {agentList.filter((a: any) => a.status === 'online').length === 0 ? (
          <EmptyState icon={<Cpu size={48} />} title={t('common.no_online_agents')} description={t('metrics.agent_resource_desc')} />
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
            {agentList.filter((a: any) => a.status === 'online').map((a: any) => (
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
        )}
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <BarChart3 size={16} className="text-[var(--color-accent)]" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('metrics.prometheus')}</h3>
        </div>
        {metricsError && !promText ? (
          <div className="flex items-center justify-center py-8 text-sm text-[var(--color-dim)] opacity-60">{metricsError}</div>
        ) : (
          <pre className="rounded-xl bg-[#0d0e14] p-4 font-mono text-xs leading-relaxed text-[var(--color-dim)] overflow-x-auto max-h-96 overflow-y-auto">
            {promText || t('metrics.no_metrics')}
          </pre>
        )}
      </div>
    </div>
  );
}
