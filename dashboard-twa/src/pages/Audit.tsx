import { useState, useMemo } from 'react';
import { AuditEvent } from '../types';
import EventItem from '../components/EventItem';
import BottomSheet from '../components/BottomSheet';
import { api } from '../api/client';
import { useApi } from '../hooks/useApi';

const filters = ['all', 'denied', 'approved', 'alerts'] as const;
type Filter = typeof filters[number];

export default function Audit() {
  const { data, loading, error, refresh } = useApi(() => api.getAuditEvents(), { pollMs: 15000 });
  const [filter, setFilter] = useState<Filter>('all');
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<AuditEvent | null>(null);

  const events: AuditEvent[] = useMemo(() => (data || []).map((e: any, i: number) => ({
    id: e.id || String(i),
    type: e.type || 'command',
    message: e.message || e.command || e.action || 'Unknown event',
    timestamp: e.timestamp || e.created_at || new Date().toISOString(),
    user: e.user || e.username,
    agent: e.agent || e.hostname || e.agent_id,
    details: e.details,
  })), [data]);

  const filtered = useMemo(() => {
    let evts = events;
    if (filter === 'denied') evts = evts.filter(e => e.type === 'denied');
    else if (filter === 'approved') evts = evts.filter(e => e.type === 'approved');
    else if (filter === 'alerts') evts = evts.filter(e => e.type === 'alert');
    if (search) {
      const q = search.toLowerCase();
      evts = evts.filter(e => e.message.toLowerCase().includes(q) || (e.user?.toLowerCase().includes(q)) || (e.agent?.toLowerCase().includes(q)));
    }
    return evts;
  }, [events, filter, search]);

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-tg-button border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-tg-hint mt-3">Loading audit log...</p>
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
      <h1 className="font-bold text-lg mb-3">Audit Log</h1>

      {/* Search */}
      <input type="text" placeholder="Search events..." value={search} onChange={e => setSearch(e.target.value)}
        className="w-full px-4 py-3 rounded-xl bg-tg-surface text-sm text-tg-text placeholder:text-tg-hint outline-none mb-3 min-h-[44px]" />

      {/* Filter Chips */}
      <div className="flex gap-2 mb-4 overflow-x-auto pb-1 -mx-1 px-1">
        {filters.map(f => (
          <button key={f} onClick={() => setFilter(f)}
            className="px-3 py-1.5 rounded-full text-xs font-medium whitespace-nowrap min-h-[36px] transition-colors"
            style={{
              background: filter === f ? 'var(--tg-button)' : 'var(--tg-surface)',
              color: filter === f ? 'var(--tg-button-text)' : 'var(--tg-hint)',
            }}>
            {f.charAt(0).toUpperCase() + f.slice(1)}
          </button>
        ))}
      </div>

      {/* Events */}
      <div className="divide-y divide-white/5">
        {filtered.length === 0 ? (
          <p className="text-center text-tg-hint text-sm py-8">No events found</p>
        ) : (
          filtered.map(e => <EventItem key={e.id} event={e} onTap={setSelected} />)
        )}
      </div>

      <BottomSheet open={!!selected} onClose={() => setSelected(null)} title="Event Details">
        {selected && (
          <div className="space-y-3">
            <div>
              <span className="text-xs text-tg-hint">Message</span>
              <p className="text-sm mt-1">{selected.message}</p>
            </div>
            <div className="grid grid-cols-2 gap-3 text-sm">
              <div><span className="text-xs text-tg-hint">Type</span><p className="capitalize">{selected.type.replace('_', ' ')}</p></div>
              <div><span className="text-xs text-tg-hint">Time</span><p>{new Date(selected.timestamp).toLocaleString()}</p></div>
              {selected.user && <div><span className="text-xs text-tg-hint">User</span><p>{selected.user}</p></div>}
              {selected.agent && <div><span className="text-xs text-tg-hint">Agent</span><p>{selected.agent}</p></div>}
            </div>
            {selected.details && (
              <div>
                <span className="text-xs text-tg-hint">Details</span>
                <p className="text-sm mt-1 text-tg-hint">{selected.details}</p>
              </div>
            )}
          </div>
        )}
      </BottomSheet>
    </div>
  );
}
