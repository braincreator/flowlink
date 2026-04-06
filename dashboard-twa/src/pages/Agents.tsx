import { useState, useMemo } from 'react';
import { Agent } from '../types';
import AgentCard from '../components/AgentCard';
import BottomSheet from '../components/BottomSheet';
import { api } from '../api/client';
import { useApi } from '../hooks/useApi';

export default function Agents() {
  const { data, loading, error, refresh } = useApi(() => api.getAgents(), { pollMs: 10000 });
  const [selected, setSelected] = useState<Agent | null>(null);

  const agents: Agent[] = useMemo(() => (data || []).map((a: any) => ({
    id: a.id || a.agent_id || a.hostname,
    hostname: a.hostname || a.name || 'unknown',
    os: a.os || a.platform || 'Unknown',
    status: a.status === 'online' ? 'online' as const : 'offline' as const,
    lastSeen: a.last_seen || a.lastSeen || a.last_heartbeat || new Date().toISOString(),
    ip: a.ip || a.address || '—',
    version: a.version || '—',
  })), [data]);

  const online = agents.filter(a => a.status === 'online').length;

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-tg-button border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-tg-hint mt-3">Loading agents...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <span className="text-3xl block mb-3">⚠️</span>
        <p className="text-sm text-tg-danger mb-1">{error}</p>
        <button onClick={refresh} className="mt-2 px-4 py-2 rounded-xl bg-tg-button text-tg-button-text text-sm font-medium">
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="px-4 pt-4">
      <h1 className="font-bold text-lg mb-1">Agents</h1>
      <p className="text-xs text-tg-hint mb-4">{online}/{agents.length} online</p>

      {agents.length === 0 ? (
        <p className="text-center text-tg-hint text-sm py-8">No agents connected</p>
      ) : (
        agents.map(agent => (
          <AgentCard key={agent.id} agent={agent} onTap={setSelected} />
        ))
      )}

      <BottomSheet open={!!selected} onClose={() => setSelected(null)} title={selected?.hostname}>
        {selected && (
          <div className="space-y-4">
            <div className="flex items-center gap-2">
              <span className={`w-3 h-3 rounded-full`} style={{ background: selected.status === 'online' ? 'var(--tg-success)' : 'var(--tg-hint)' }} />
              <span className="text-sm font-semibold capitalize">{selected.status}</span>
            </div>
            <div className="grid grid-cols-2 gap-3 text-sm">
              <div><span className="text-xs text-tg-hint">OS</span><p className="truncate">{selected.os}</p></div>
              <div><span className="text-xs text-tg-hint">IP</span><p>{selected.ip}</p></div>
              <div><span className="text-xs text-tg-hint">Version</span><p>{selected.version}</p></div>
              <div><span className="text-xs text-tg-hint">Last Seen</span><p>{new Date(selected.lastSeen).toLocaleString()}</p></div>
            </div>

            {/* Quick Actions */}
            <div className="space-y-2 pt-2">
              <button className="w-full py-3 rounded-xl bg-tg-button text-tg-button-text font-semibold text-sm min-h-[44px]">
                ⚡ Execute Command
              </button>
              <div className="flex gap-2">
                <button className="flex-1 py-3 rounded-xl bg-tg-surface text-sm min-h-[44px]">🔄 Restart</button>
                <button className="flex-1 py-3 rounded-xl bg-tg-danger-bg text-tg-danger text-sm min-h-[44px]">🔌 Disconnect</button>
              </div>
            </div>
          </div>
        )}
      </BottomSheet>
    </div>
  );
}
