import { useState } from 'react';
import { Play } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, DataTable, Modal, LoadingSkeleton } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import { mockSessions } from '../api/client';
import type { Session } from '../types';

export default function Sessions() {
  const [replaySession, setReplaySession] = useState<Session | null>(null);

  const { data: sessions, loading, isLive } = useApi<any[]>(
    () => api.getAuditEvents({ event_type: 'session_started', limit: 50 }),
    mockSessions,
    { pollMs: 15000 }
  );

  const activeSessions = sessions.filter((s: any) => s.status === 'active');
  const durationData = sessions.map((s: any) => ({
    id: (s.id || '').slice(0, 8),
    duration: Math.round((s.duration_ms || 0) / 60000),
  }));

  if (loading) return <LoadingSkeleton lines={6} />;

  return (
    <div className="space-y-6 fade-in">
      {!isLive && (
        <div className="rounded-xl border border-amber-500/30 bg-amber-500/5 px-4 py-3 text-sm text-amber-400">
          ⚠️ Connected to mock data. Start relay for live data.
        </div>
      )}

      <div className="grid grid-cols-3 gap-4">
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Active Now</div>
          <div className="mt-1 text-2xl font-bold text-emerald-400">{activeSessions.length}</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Total Today</div>
          <div className="mt-1 text-2xl font-bold">{sessions.length}</div>
        </div>
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <div className="text-xs uppercase tracking-wider text-[var(--color-dim)]">Avg Commands</div>
          <div className="mt-1 text-2xl font-bold">{sessions.length > 0 ? Math.round(sessions.reduce((a: number, s: any) => a + (s.commands_count || 0), 0) / sessions.length) : 0}</div>
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Session Duration (min)</h3>
        <ResponsiveContainer width="100%" height={160}>
          <BarChart data={durationData}>
            <XAxis dataKey="id" tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
            <YAxis tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
            <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
            <Bar dataKey="duration" fill="#6366f1" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>

      <DataTable
        columns={[
          { key: 'id', label: 'Session', render: (r: Session) => <span className="font-mono text-xs">{r.id}</span> },
          { key: 'user', label: 'User', render: (r: any) => r.user || '—' },
          { key: 'agent_id', label: 'Agent', render: (r: Session) => <span className="font-mono text-xs text-[var(--color-dim)]">{r.agent_id}</span> },
          { key: 'origin', label: 'Origin', render: (r: any) => r.origin ? <span className="font-mono text-xs">{r.origin}</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'terminal', label: 'Terminal', render: (r: any) => r.terminal ? <span className="text-xs text-[var(--color-dim)]">{r.terminal}</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'commands_count', label: 'Commands', render: (r: any) => r.commands_count ?? '—' },
          { key: 'duration_ms', label: 'Duration', render: (r: any) => r.duration_ms ? <span className="text-xs">{Math.round(r.duration_ms / 60000)}m</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'status', label: 'Status', render: (r: Session) => (
            <Badge variant={r.status === 'active' ? 'green' : 'default'}>
              <span className={`inline-block h-1.5 w-1.5 rounded-full ${r.status === 'active' ? 'bg-emerald-400 pulse-dot' : ''}`} />
              {r.status}
            </Badge>
          )},
          { key: 'replay', label: '', render: (r: Session) => r.status === 'ended' ? (
            <button onClick={(e) => { e.stopPropagation(); setReplaySession(r); }} className="flex items-center gap-1 rounded-lg bg-[var(--color-accent)]/15 px-2.5 py-1 text-xs font-medium text-[var(--color-accent-light)] hover:bg-[var(--color-accent)]/25 transition-colors">
              <Play size={12} /> Replay
            </button>
          ) : null },
        ]}
        data={sessions} searchPlaceholder="Search sessions..."
      />

      <Modal open={!!replaySession} onClose={() => setReplaySession(null)} title={`Session Replay — ${replaySession?.id}`}>
        <div className="rounded-xl bg-[#0d0e14] p-4 font-mono text-sm min-h-[300px]">
          <div className="text-[var(--color-dim)]">Session replay placeholder</div>
          <div className="mt-2 text-xs text-[var(--color-dim)]">asciinema-player integration point</div>
        </div>
      </Modal>
    </div>
  );
}
