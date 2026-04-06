import { useCallback } from 'react';
import { AuditEvent, Agent } from '../types';
import StatCard from '../components/StatCard';
import EventItem from '../components/EventItem';
import { api } from '../api/client';
import { useApi } from '../hooks/useApi';

export default function Overview() {
  const { data: agents, loading: agentsLoading, error: agentsError, refresh: refreshAgents } = useApi(() => api.getAgents(), { pollMs: 15000 });
  const { data: events, loading: eventsLoading, error: eventsError, refresh: refreshEvents } = useApi(() => api.getAuditEvents(), { pollMs: 15000 });

  const loading = agentsLoading || eventsLoading;
  const error = agentsError || eventsError;

  const agentsOnline = (agents || []).filter((a: any) => a.status === 'online').length;
  const agentsTotal = (agents || []).length;

  const mappedEvents: AuditEvent[] = (events || []).map((e: any, i: number) => ({
    id: e.id || String(i),
    type: e.type || 'command',
    message: e.message || e.command || e.action || 'Unknown event',
    timestamp: e.timestamp || e.created_at || new Date().toISOString(),
    user: e.user || e.username,
    agent: e.agent || e.hostname || e.agent_id,
    details: e.details,
  }));

  const shieldColor = loading ? 'var(--tg-hint)' : error ? 'var(--tg-danger)' : 'var(--tg-success)';

  const onRetry = useCallback(() => {
    refreshAgents();
    refreshEvents();
  }, [refreshAgents, refreshEvents]);

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-tg-button border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-tg-hint mt-3">Loading dashboard...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <span className="text-3xl block mb-3">⚠️</span>
        <p className="text-sm text-tg-danger mb-1">{error}</p>
        <button onClick={onRetry} className="mt-2 px-4 py-2 rounded-xl bg-tg-button text-tg-button-text text-sm font-medium">
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="px-4 pt-4">
      {/* Shield Status */}
      <div className="flex items-center justify-center gap-2 mb-5 py-3">
        <div className="w-3 h-3 rounded-full animate-pulse" style={{ background: shieldColor }} />
        <span className="text-sm font-semibold" style={{ color: shieldColor }}>
          Shield Active
        </span>
      </div>

      {/* Stats */}
      <div className="flex gap-3 mb-6">
        <StatCard label="Agents Online" value={agentsOnline} icon="🤖" color="var(--tg-success)" />
        <StatCard label="Total Agents" value={agentsTotal} icon="💻" color="var(--tg-button)" />
        <StatCard label="Events" value={mappedEvents.length} icon="⚡" color="var(--tg-button)" />
      </div>

      {/* Activity Feed */}
      <div className="mb-4">
        <div className="flex items-center justify-between mb-3">
          <h2 className="font-semibold text-sm">Recent Activity</h2>
        </div>
        {mappedEvents.length === 0 ? (
          <p className="text-center text-tg-hint text-sm py-8">No activity yet</p>
        ) : (
          <div className="divide-y divide-white/5">
            {mappedEvents.slice(0, 20).map(e => <EventItem key={e.id} event={e} />)}
          </div>
        )}
      </div>
    </div>
  );
}
