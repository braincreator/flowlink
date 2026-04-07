import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Play } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, DataTable, Modal, LoadingSkeleton, EmptyState } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import SessionPlayer from '../components/SessionPlayer';
import SessionRecorder from '../components/SessionRecorder';

export default function Sessions() {
  const { t } = useTranslation();
  const [replaySession, setReplaySession] = useState<any>(null);
  const [recordSession, setRecordSession] = useState<any>(null);

  const { data, loading, error, refresh } = useApi<any[]>(
    () => api.getSessions(),
    { pollMs: 15000 }
  );

  const sessions = data || [];
  const activeSessions = sessions.filter((s: any) => s.status === 'active');
  const durationData = sessions.map((s: any) => ({
    id: (s.id || '').slice(0, 8),
    duration: Math.round((s.duration_ms || 0) / 60000),
  }));

  if (loading && !data) return <LoadingSkeleton lines={6} />;

  return (
    <div className="space-y-6 fade-in">
      {error && !data && (
        <div className="flex flex-col items-center py-16 text-center">
          <div className="text-4xl mb-4 opacity-40">⚠️</div>
          <h3 className="text-lg font-semibold text-[var(--color-dim)]">{t('common.unable_connect')}</h3>
          <p className="mt-2 text-sm text-[var(--color-dim)] opacity-70">{error}</p>
          <button onClick={refresh} className="mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]">{t('common.retry')}</button>
        </div>
      )}

      <div className="grid grid-cols-3 gap-4">
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">{t('sessions.active')}</div>
          <div className="mt-1 text-2xl font-bold text-emerald-400">{activeSessions.length}</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">{t('sessions.total_today')}</div>
          <div className="mt-1 text-2xl font-bold">{sessions.length}</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">{t('sessions.avg_commands')}</div>
          <div className="mt-1 text-2xl font-bold">{sessions.length > 0 ? Math.round(sessions.reduce((a: number, s: any) => a + (s.commands_count || 0), 0) / sessions.length) : 0}</div>
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('sessions.duration')}</h3>
        {durationData.length === 0 ? (
          <div className="flex items-center justify-center py-8 text-sm text-[var(--color-dim)] opacity-60">{t('sessions.no_sessions')}</div>
        ) : (
          <ResponsiveContainer width="100%" height={160}>
            <BarChart data={durationData}>
              <XAxis dataKey="id" tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
              <YAxis tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
              <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
              <Bar dataKey="duration" fill="#6366f1" radius={[4, 4, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        )}
      </div>

      <DataTable
        columns={[
          { key: 'id', label: t('sessions.session'), render: (r: any) => <span className="font-mono text-xs">{r.id}</span> },
          { key: 'user', label: t('sessions.user'), render: (r: any) => r.user || '—' },
          { key: 'agent_id', label: t('audit.agent'), render: (r: any) => <span className="font-mono text-xs text-[var(--color-dim)]">{r.agent_id}</span> },
          { key: 'origin', label: t('sessions.origin'), render: (r: any) => r.origin ? <span className="font-mono text-xs">{r.origin}</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'terminal', label: t('sessions.terminal'), render: (r: any) => r.terminal ? <span className="text-xs text-[var(--color-dim)]">{r.terminal}</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'commands_count', label: t('sessions.commands_count'), render: (r: any) => r.commands_count ?? '—' },
          { key: 'duration_ms', label: t('sessions.duration'), render: (r: any) => r.duration_ms ? <span className="text-xs">{Math.round(r.duration_ms / 60000)}m</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'status', label: t('agents.status'), render: (r: any) => (
            <Badge variant={r.status === 'active' ? 'green' : 'default'}>
              <span className={`inline-block h-1.5 w-1.5 rounded-full ${r.status === 'active' ? 'bg-emerald-400 pulse-dot' : ''}`} />
              {r.status}
            </Badge>
          )},
          { key: 'actions', label: '', render: (r: any) => r.status === 'ended' ? (
            <div className="flex items-center gap-1.5">
              <button onClick={(e) => { e.stopPropagation(); setReplaySession(r); }} className="flex items-center gap-1 rounded-lg bg-[var(--color-accent)]/15 px-2.5 py-1 text-xs font-medium text-[var(--color-accent-light)] hover:bg-[var(--color-accent)]/25 transition-colors">
                <Play size={12} /> {t('sessions.replay')}
              </button>
              <button onClick={(e) => { e.stopPropagation(); setRecordSession(r); }} className="flex items-center gap-1 rounded-lg bg-rose-500/15 px-2.5 py-1 text-xs font-medium text-rose-400 hover:bg-rose-500/25 transition-colors">
                ⏺ {t('sessions.record_session')}
              </button>
            </div>
          ) : null },
        ]}
        data={sessions} searchPlaceholder={t('sessions.search_sessions')}
      />

      <Modal open={!!replaySession} onClose={() => setReplaySession(null)} title={`${t('sessions.session_replay')} — ${replaySession?.id?.slice(0, 8)}`}>
        {replaySession?.castData ? (
          <SessionPlayer castData={replaySession.castData} autoPlay />
        ) : (
          <div className="rounded-xl bg-[#0d0e14] p-4 font-mono text-sm min-h-[300px]">
            <div className="text-[var(--color-dim)]">{t('sessions.replay_placeholder')}</div>
            <div className="mt-2 text-xs text-[var(--color-dim)]">{t('sessions.asciinema_integration')}</div>
          </div>
        )}
      </Modal>

      <SessionRecorder
        open={!!recordSession}
        onClose={() => setRecordSession(null)}
        session={recordSession}
      />
    </div>
  );
}
