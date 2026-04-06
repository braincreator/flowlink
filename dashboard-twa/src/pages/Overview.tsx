import { useState, useCallback } from 'react';
import { DashboardStats, AuditEvent, Alert, Agent } from '../types';
import StatCard from '../components/StatCard';
import EventItem from '../components/EventItem';

// Mock data
const mockStats: DashboardStats = { agentsOnline: 3, agentsTotal: 5, activeAlerts: 2, commandsToday: 47, shieldStatus: 'active' };
const mockEvents: AuditEvent[] = [
  { id: '1', type: 'command', message: 'kubectl get pods executed', timestamp: new Date().toISOString(), user: 'admin', agent: 'prod-k8s-01' },
  { id: '2', type: 'alert', message: 'High-risk command: rm -rf /tmp/*', timestamp: new Date(Date.now() - 300000).toISOString(), user: 'deploy', agent: 'prod-web-02' },
  { id: '3', type: 'approved', message: 'sudo apt update approved', timestamp: new Date(Date.now() - 600000).toISOString(), user: 'ops', agent: 'prod-db-01' },
  { id: '4', type: 'denied', message: 'curl suspicious-url.com blocked', timestamp: new Date(Date.now() - 900000).toISOString(), user: 'guest', agent: 'staging-01' },
  { id: '5', type: 'agent_join', message: 'prod-k8s-01 connected', timestamp: new Date(Date.now() - 1200000).toISOString(), agent: 'prod-k8s-01' },
];

export default function Overview() {
  const [stats] = useState(mockStats);
  const [events] = useState(mockEvents);
  const [refreshing, setRefreshing] = useState(false);

  const shieldColor = stats.shieldStatus === 'active' ? 'var(--tg-success)' : stats.shieldStatus === 'degraded' ? 'var(--tg-warning)' : 'var(--tg-danger)';

  const onRefresh = useCallback(() => {
    setRefreshing(true);
    setTimeout(() => setRefreshing(false), 1000);
  }, []);

  return (
    <div className="px-4 pt-4">
      {/* Shield Status */}
      <div className="flex items-center justify-center gap-2 mb-5 py-3">
        <div className="w-3 h-3 rounded-full animate-pulse" style={{ background: shieldColor }} />
        <span className="text-sm font-semibold" style={{ color: shieldColor }}>
          Shield {stats.shieldStatus.charAt(0).toUpperCase() + stats.shieldStatus.slice(1)}
        </span>
      </div>

      {/* Stats */}
      <div className="flex gap-3 mb-6">
        <StatCard label="Agents Online" value={stats.agentsOnline} icon="🤖" color="var(--tg-success)" />
        <StatCard label="Active Alerts" value={stats.activeAlerts} icon="🚨" color="var(--tg-danger)" />
        <StatCard label="Commands" value={stats.commandsToday} icon="⚡" color="var(--tg-button)" />
      </div>

      {/* Activity Feed */}
      <div className="mb-4">
        <div className="flex items-center justify-between mb-3">
          <h2 className="font-semibold text-sm">Recent Activity</h2>
          {refreshing && <span className="text-xs text-tg-hint animate-pulse">Refreshing...</span>}
        </div>
        <button onClick={onRefresh} className="text-xs text-tg-button mb-2">↓ Pull to refresh</button>
        <div className="divide-y divide-white/5">
          {events.map(e => <EventItem key={e.id} event={e} />)}
        </div>
      </div>
    </div>
  );
}
