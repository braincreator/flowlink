import { Agent } from '../types';

const osIcons: Record<string, string> = { linux: '🐧', darwin: '🍎', windows: '🪟' };

interface Props {
  agent: Agent;
  onTap: (agent: Agent) => void;
}

export default function AgentCard({ agent, onTap }: Props) {
  const icon = osIcons[agent.os.toLowerCase()] || '💻';
  const lastSeen = new Date(agent.lastSeen).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

  return (
    <div onClick={() => onTap(agent)}
      className="bg-tg-surface rounded-xl p-4 mb-3 cursor-pointer active:opacity-80 transition-opacity">
      <div className="flex items-center gap-3">
        <span className="text-2xl">{icon}</span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full shrink-0" style={{
              background: agent.status === 'online' ? 'var(--tg-success)' : 'var(--tg-hint)'
            }} />
            <span className="font-semibold text-sm truncate">{agent.hostname}</span>
          </div>
          <div className="text-xs text-tg-hint mt-1">
            {agent.os} · {agent.ip} · {agent.status === 'online' ? `Last seen ${lastSeen}` : 'Offline'}
          </div>
        </div>
        <span className="text-tg-hint text-lg">›</span>
      </div>
    </div>
  );
}
