import { useState } from 'react';
import { HardDrive, RotateCcw } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, DataTable, Modal, LoadingSkeleton, EmptyState } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';

export default function Backups() {
  const [restoreTarget, setRestoreTarget] = useState<any>(null);

  const { data, loading, error, refresh } = useApi<any[]>(
    () => api.getBackups(),
    { pollMs: 30000 }
  );

  const backups = data || [];

  const formatSize = (bytes: number) => bytes > 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${(bytes / 1e6).toFixed(0)} MB`;

  if (loading && !data) return <LoadingSkeleton lines={6} />;

  return (
    <div className="space-y-6 fade-in">
      {error && !data && (
        <div className="flex flex-col items-center py-16 text-center">
          <div className="text-4xl mb-4 opacity-40">⚠️</div>
          <h3 className="text-lg font-semibold text-[var(--color-dim)]">Unable to connect to relay</h3>
          <p className="mt-2 text-sm text-[var(--color-dim)] opacity-70">{error}</p>
          <button onClick={refresh} className="mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]">Retry</button>
        </div>
      )}

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Storage Usage by Agent</h3>
        <ResponsiveContainer width="100%" height={180}>
          <BarChart data={[]}>
            <XAxis dataKey="agent" tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
            <YAxis tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} tickFormatter={(v: number) => `${v} GB`} />
            <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
            <Bar dataKey="used" fill="#6366f1" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
        <div className="flex items-center justify-center py-4 text-sm text-[var(--color-dim)] opacity-60">No time-series data available yet</div>
      </div>

      <DataTable
        columns={[
          { key: 'id', label: 'ID', render: (r: any) => <span className="font-mono text-xs">{r.id}</span> },
          { key: 'hostname', label: 'Agent', render: (r: any) => (
            <div className="flex items-center gap-2"><HardDrive size={14} className="text-[var(--color-accent)]" />{r.hostname || r.agent_id || '—'}</div>
          )},
          { key: 'files', label: 'Files', render: (r: any) => r.files ? (
            <div className="flex flex-wrap gap-1">{(r.files as string[]).map((f: string) => <span key={f} className="rounded bg-[var(--color-surface3)] px-1.5 py-0.5 text-[10px] font-mono text-[var(--color-dim)]">{f.split('/').pop()}</span>)}</div>
          ) : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'size_bytes', label: 'Size', render: (r: any) => r.size_bytes ? <span className="font-mono text-xs">{formatSize(r.size_bytes)}</span> : <span className="text-[var(--color-dim)]">—</span> },
          { key: 'timestamp', label: 'Time', render: (r: any) => <span className="text-xs text-[var(--color-dim)]">{new Date(r.timestamp || r.timestamp_iso).toLocaleString()}</span> },
          { key: 'status', label: 'Status', render: (r: any) => {
            const status = r.status || 'completed';
            const v = status === 'completed' ? 'green' : status === 'failed' ? 'red' : 'amber';
            return <Badge variant={v}>{status}</Badge>;
          }},
          { key: 'restore', label: '', render: (r: any) => (r.status || 'completed') === 'completed' ? (
            <button onClick={(e) => { e.stopPropagation(); setRestoreTarget(r); }}
              className="flex items-center gap-1 rounded-lg border border-[var(--color-border)] px-2.5 py-1 text-xs hover:bg-[var(--color-surface2)] transition-colors">
              <RotateCcw size={12} /> Restore
            </button>
          ) : null },
        ]}
        data={backups} searchPlaceholder="Search backups..."
      />

      <Modal open={!!restoreTarget} onClose={() => setRestoreTarget(null)} title="Restore Backup" actions={
        <>
          <button onClick={() => setRestoreTarget(null)} className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm">Cancel</button>
          <button onClick={() => setRestoreTarget(null)} className="rounded-lg bg-amber-500 px-4 py-2 text-sm font-medium text-black hover:bg-amber-400">Restore</button>
        </>
      }>
        {restoreTarget && (
          <div className="text-sm">
            <p className="text-[var(--color-dim)]">Restore from backup created on <span className="text-[var(--color-text)]">{new Date(restoreTarget.timestamp).toLocaleString()}</span>.</p>
            {restoreTarget.files && (
              <div className="mt-3 rounded-lg bg-[var(--color-bg)] p-3">
                <div className="text-xs text-[var(--color-dim)] mb-1">Files:</div>
                {restoreTarget.files.map((f: string) => <div key={f} className="font-mono text-xs">{f}</div>)}
              </div>
            )}
            <p className="mt-3 text-amber-400 text-xs">⚠ This will overwrite current files on the agent.</p>
          </div>
        )}
      </Modal>
    </div>
  );
}
