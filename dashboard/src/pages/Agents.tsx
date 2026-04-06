import { useState } from 'react';
import { Bot, Cpu, MemoryStick, HardDrive, Terminal, X } from 'lucide-react';
import { DataTable, Badge, SlidePanel, Modal, RiskGauge } from '../components/Layout';
import { mockAgents } from '../api/client';
import type { Agent } from '../types';

export default function Agents() {
  const [selected, setSelected] = useState<Agent | null>(null);
  const [execOpen, setExecOpen] = useState(false);
  const [cmd, setCmd] = useState('');
  const [filterStatus, setFilterStatus] = useState<string>('all');

  const filtered = filterStatus === 'all' ? mockAgents : mockAgents.filter(a => a.status === filterStatus);

  return (
    <div className="space-y-6 fade-in">
      {/* Filters */}
      <div className="flex flex-wrap items-center gap-3">
        {['all', 'online', 'offline'].map(s => (
          <button key={s} onClick={() => setFilterStatus(s)}
            className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${filterStatus === s ? 'bg-[var(--color-accent)] text-white' : 'border border-[var(--color-border)] text-[var(--color-dim)] hover:text-[var(--color-text)]'}`}>
            {s === 'all' ? 'All' : s === 'online' ? '🟢 Online' : '🔴 Offline'}
          </button>
        ))}
      </div>

      <DataTable
        columns={[
          { key: 'hostname', label: 'Hostname', render: (r: Agent) => (
            <div className="flex items-center gap-2">
              <Bot size={16} className="text-[var(--color-accent)]" />
              <div><div className="font-medium">{r.hostname}</div><div className="text-xs text-[var(--color-dim)]">{r.id}</div></div>
            </div>
          )},
          { key: 'os', label: 'OS', render: (r: Agent) => <span className="text-xs font-mono">{r.os}</span> },
          { key: 'version', label: 'Version' },
          { key: 'status', label: 'Status', render: (r: Agent) => (
            <Badge variant={r.status === 'online' ? 'green' : 'red'}>
              <span className={`inline-block h-1.5 w-1.5 rounded-full ${r.status === 'online' ? 'bg-emerald-400 pulse-dot' : 'bg-rose-400'}`} />
              {r.status}
            </Badge>
          )},
          { key: 'last_heartbeat', label: 'Last Heartbeat', render: (r: Agent) => <span className="text-xs text-[var(--color-dim)]">{new Date(r.last_heartbeat).toLocaleTimeString()}</span> },
          { key: 'tags', label: 'Tags', render: (r: Agent) => <div className="flex gap-1">{r.tags.map(t => <span key={t} className="rounded-md bg-[var(--color-surface3)] px-1.5 py-0.5 text-[10px] text-[var(--color-dim)]">{t}</span>)}</div> },
        ]}
        data={filtered} onRowClick={setSelected} searchPlaceholder="Search agents..."
      />

      {/* Detail Panel */}
      <SlidePanel open={!!selected} onClose={() => setSelected(null)} title={selected?.hostname || ''}>
        {selected && (
          <div className="space-y-6">
            <div className="flex items-center gap-3">
              <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-purple-600 text-xl text-white"><Bot /></div>
              <div>
                <div className="font-semibold">{selected.hostname}</div>
                <div className="text-sm text-[var(--color-dim)]">{selected.id} · {selected.os}</div>
              </div>
            </div>

            <div className="grid grid-cols-3 gap-3">
              {[
                { label: 'CPU', value: `${selected.cpu}%`, icon: <Cpu size={16} />, color: selected.cpu! > 80 ? 'text-rose-400' : 'text-emerald-400' },
                { label: 'RAM', value: `${selected.ram}%`, icon: <MemoryStick size={16} />, color: selected.ram! > 80 ? 'text-rose-400' : 'text-emerald-400' },
                { label: 'Disk', value: `${selected.disk}%`, icon: <HardDrive size={16} />, color: selected.disk! > 80 ? 'text-amber-400' : 'text-emerald-400' },
              ].map(s => (
                <div key={s.label} className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] p-4 text-center">
                  <div className={`mb-1 flex justify-center ${s.color}`}>{s.icon}</div>
                  <div className="text-xl font-bold">{s.value}</div>
                  <div className="text-xs text-[var(--color-dim)]">{s.label}</div>
                </div>
              ))}
            </div>

            <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
              <div className="text-xs text-[var(--color-dim)] uppercase tracking-wider mb-2">System Info</div>
              <div className="grid grid-cols-2 gap-2 text-sm">
                <div><span className="text-[var(--color-dim)]">IP:</span> {selected.ip}</div>
                <div><span className="text-[var(--color-dim)]">Sessions:</span> {selected.sessions_count}</div>
                <div><span className="text-[var(--color-dim)]">Version:</span> {selected.version}</div>
                <div><span className="text-[var(--color-dim)]">Tags:</span> {selected.tags.join(', ')}</div>
              </div>
            </div>

            <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] p-4">
              <div className="text-xs text-[var(--color-dim)] uppercase tracking-wider mb-3">Recent Commands</div>
              <div className="space-y-1.5 max-h-48 overflow-y-auto">
                {['systemctl status nginx', 'docker ps -a', 'tail -f /var/log/app.log', 'df -h', 'free -m'].map((c, i) => (
                  <div key={i} className="rounded-lg bg-[var(--color-surface)] px-3 py-2 font-mono text-xs text-[var(--color-dim)]">
                    <span className="text-[var(--color-accent-light)]">$</span> {c}
                  </div>
                ))}
              </div>
            </div>

            <div className="flex gap-3">
              <button onClick={() => setExecOpen(true)} className="flex-1 flex items-center justify-center gap-2 rounded-xl bg-[var(--color-accent)] py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]">
                <Terminal size={16} /> Execute Command
              </button>
              <button className="flex-1 rounded-xl border border-rose-500/30 bg-rose-500/10 py-2.5 text-sm font-medium text-rose-400 transition-colors hover:bg-rose-500/20">
                Disconnect
              </button>
            </div>
          </div>
        )}
      </SlidePanel>

      {/* Exec Modal */}
      <Modal open={execOpen} onClose={() => setExecOpen(false)} title="Execute Command" actions={
        <>
          <button onClick={() => setExecOpen(false)} className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm">Cancel</button>
          <button onClick={() => setExecOpen(false)} className="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white">Execute</button>
        </>
      }>
        <div>
          <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Command</label>
          <input type="text" value={cmd} onChange={e => setCmd(e.target.value)} placeholder="e.g. systemctl restart nginx"
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
        </div>
        <div>
          <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Shell</label>
          <select className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
            <option>/bin/bash</option><option>/bin/sh</option><option>/bin/zsh</option>
          </select>
        </div>
      </Modal>
    </div>
  );
}
