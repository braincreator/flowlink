import { useState } from 'react';
import { Agent } from '../types';
import AgentCard from '../components/AgentCard';
import BottomSheet from '../components/BottomSheet';

const mockAgents: Agent[] = [
  { id: 'a1', hostname: 'prod-web-02', os: 'Ubuntu 22.04', status: 'online', lastSeen: new Date().toISOString(), ip: '10.0.1.5', version: '1.2.0' },
  { id: 'a2', hostname: 'prod-k8s-01', os: 'Ubuntu 24.04', status: 'online', lastSeen: new Date().toISOString(), ip: '10.0.1.10', version: '1.2.0' },
  { id: 'a3', hostname: 'prod-db-01', os: 'Debian 12', status: 'online', lastSeen: new Date(Date.now() - 60000).toISOString(), ip: '10.0.2.5', version: '1.1.9' },
  { id: 'a4', hostname: 'staging-01', os: 'Darwin 23.4', status: 'offline', lastSeen: new Date(Date.now() - 7200000).toISOString(), ip: '10.0.3.2', version: '1.2.0' },
  { id: 'a5', hostname: 'dev-laptop', os: 'Darwin 24.3', status: 'online', lastSeen: new Date().toISOString(), ip: '10.0.5.20', version: '1.2.0' },
];

const recentCommands = [
  { cmd: 'kubectl get pods', status: 'ok' as const },
  { cmd: 'docker ps', status: 'ok' as const },
  { cmd: 'systemctl status nginx', status: 'ok' as const },
  { cmd: 'apt update', status: 'blocked' as const },
  { cmd: 'df -h', status: 'ok' as const },
];

export default function Agents() {
  const [selected, setSelected] = useState<Agent | null>(null);

  const online = mockAgents.filter(a => a.status === 'online').length;

  return (
    <div className="px-4 pt-4">
      <h1 className="font-bold text-lg mb-1">Agents</h1>
      <p className="text-xs text-tg-hint mb-4">{online}/{mockAgents.length} online</p>

      {mockAgents.map(agent => (
        <AgentCard key={agent.id} agent={agent} onTap={setSelected} />
      ))}

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

            {/* Recent Commands */}
            <div>
              <span className="text-xs text-tg-hint font-semibold">Recent Commands</span>
              <div className="mt-2 space-y-1">
                {recentCommands.map((c, i) => (
                  <div key={i} className="flex items-center justify-between py-1.5 text-sm">
                    <code className="font-mono text-xs truncate flex-1">{c.cmd}</code>
                    <span className={`text-[10px] px-1.5 py-0.5 rounded ${c.status === 'ok' ? 'bg-tg-success-bg text-tg-success' : 'bg-tg-danger-bg text-tg-danger'}`}>
                      {c.status}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}
      </BottomSheet>
    </div>
  );
}
