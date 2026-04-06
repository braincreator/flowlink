import { useState } from 'react';
import { HardDrive, RotateCcw, Clock, Trash2 } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, DataTable, Modal } from '../components/Layout';
import { mockBackups, mockStorageByAgent } from '../api/client';
import type { Backup } from '../types';

export default function Backups() {
  const [restoreTarget, setRestoreTarget] = useState<Backup | null>(null);

  const formatSize = (bytes: number) => bytes > 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${(bytes / 1e6).toFixed(0)} MB`;

  return (
    <div className="space-y-6 fade-in">
      {/* Storage chart */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <h3 className="mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Storage Usage by Agent</h3>
        <ResponsiveContainer width="100%" height={180}>
          <BarChart data={mockStorageByAgent}>
            <XAxis dataKey="agent" tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} />
            <YAxis tick={{ fontSize: 11, fill: '#8b8fa3' }} axisLine={false} tickFormatter={(v: number) => `${v} GB`} />
            <Tooltip contentStyle={{ background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' }} />
            <Bar dataKey="used" fill="#6366f1" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>

      <DataTable
        columns={[
          { key: 'id', label: 'ID', render: (r: Backup) => <span className="font-mono text-xs">{r.id}</span> },
          { key: 'hostname', label: 'Agent', render: (r: Backup) => (
            <div className="flex items-center gap-2"><HardDrive size={14} className="text-[var(--color-accent)]" />{r.hostname}</div>
          )},
          { key: 'files', label: 'Files', render: (r: Backup) => (
            <div className="flex flex-wrap gap-1">{r.files.map(f => <span key={f} className="rounded bg-[var(--color-surface3)] px-1.5 py-0.5 text-[10px] font-mono text-[var(--color-dim)]">{f.split('/').pop()}</span>)}</div>
          )},
          { key: 'size_bytes', label: 'Size', render: (r: Backup) => <span className="font-mono text-xs">{formatSize(r.size_bytes)}</span> },
          { key: 'timestamp', label: 'Time', render: (r: Backup) => <span className="text-xs text-[var(--color-dim)]">{new Date(r.timestamp).toLocaleString()}</span> },
          { key: 'status', label: 'Status', render: (r: Backup) => {
            const v = r.status === 'completed' ? 'green' : r.status === 'failed' ? 'red' : 'amber';
            return <Badge variant={v}>{r.status}</Badge>;
          }},
          { key: 'restore', label: '', render: (r: Backup) => r.status === 'completed' ? (
            <button onClick={(e) => { e.stopPropagation(); setRestoreTarget(r); }}
              className="flex items-center gap-1 rounded-lg border border-[var(--color-border)] px-2.5 py-1 text-xs hover:bg-[var(--color-surface2)] transition-colors">
              <RotateCcw size={12} /> Restore
            </button>
          ) : null },
        ]}
        data={mockBackups} searchPlaceholder="Search backups..."
      />

      {/* Retention */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <h3 className="mb-3 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Retention Policy</h3>
        <div className="grid grid-cols-3 gap-4">
          <div>
            <label className="mb-1 block text-xs text-[var(--color-dim)]">Keep daily backups</label>
            <input type="number" defaultValue={7} className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1 block text-xs text-[var(--color-dim)]">Keep weekly backups</label>
            <input type="number" defaultValue={4} className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1 block text-xs text-[var(--color-dim)]">Max storage per agent</label>
            <input type="text" defaultValue="10 GB" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
        </div>
      </div>

      {/* Restore modal */}
      <Modal open={!!restoreTarget} onClose={() => setRestoreTarget(null)} title="Restore Backup" actions={
        <>
          <button onClick={() => setRestoreTarget(null)} className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm">Cancel</button>
          <button onClick={() => setRestoreTarget(null)} className="rounded-lg bg-amber-500 px-4 py-2 text-sm font-medium text-black hover:bg-amber-400">Restore</button>
        </>
      }>
        {restoreTarget && (
          <div className="text-sm">
            <p className="text-[var(--color-dim)]">This will restore files from <span className="font-mono text-[var(--color-text)]">{restoreTarget.hostname}</span> backup created on <span className="text-[var(--color-text)]">{new Date(restoreTarget.timestamp).toLocaleString()}</span>.</p>
            <div className="mt-3 rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)] mb-1">Files to restore:</div>
              {restoreTarget.files.map(f => <div key={f} className="font-mono text-xs">{f}</div>)}
            </div>
            <p className="mt-3 text-amber-400 text-xs">⚠ This will overwrite current files on the agent.</p>
          </div>
        )}
      </Modal>
    </div>
  );
}
