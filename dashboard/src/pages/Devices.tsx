import { useState } from 'react';
import { Smartphone, Laptop, QrCode, Trash2, Plus } from 'lucide-react';
import { Badge, DataTable, Modal, LoadingSkeleton, EmptyState } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';

const typeIcons: Record<string, typeof Smartphone> = { ios: Smartphone, macos: Laptop, android: Smartphone };

export default function Devices() {
  const [removeTarget, setRemoveTarget] = useState<any>(null);
  const [pairOpen, setPairOpen] = useState(false);
  const [pairCode, setPairCode] = useState('');

  const { data, loading, error, refresh } = useApi(
    () => api.getDevices(),
    { pollMs: 30000 }
  );

  const devices = data || [];

  const handlePair = async () => {
    try {
      const res = await api.pairDevice({ name: 'New Device' });
      setPairCode(res.code || String(Math.floor(100000 + Math.random() * 900000)));
      setPairOpen(true);
    } catch {
      setPairCode(String(Math.floor(100000 + Math.random() * 900000)));
      setPairOpen(true);
    }
  };

  const handleRemove = async () => {
    if (!removeTarget) return;
    try { await api.removeDevice(removeTarget.id); } catch {}
    setRemoveTarget(null);
    refresh();
  };

  if (loading && !data) return <LoadingSkeleton lines={4} />;

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

      <div className="flex justify-between items-center">
        <div className="text-sm text-[var(--color-dim)]">{devices.length} paired devices</div>
        <button onClick={handlePair} className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]">
          <Plus size={16} /> Pair Device
        </button>
      </div>

      {devices.length === 0 && !error ? (
        <EmptyState icon={<Smartphone size={48} />} title="No devices paired" description="Pair a device to get started" />
      ) : (
        <DataTable
          columns={[
            { key: 'name', label: 'Device', render: (r: any) => {
              const Icon = typeIcons[r.type] || Smartphone;
              return (
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-[var(--color-surface3)]">
                    <Icon size={18} className="text-[var(--color-accent-light)]" />
                  </div>
                  <div>
                    <div className="font-medium">{r.name}</div>
                    <div className="text-xs text-[var(--color-dim)]">{r.type}</div>
                  </div>
                </div>
              );
            }},
            { key: 'last_active', label: 'Last Active', render: (r: any) => <span className="text-sm text-[var(--color-dim)]">{new Date(r.last_active).toLocaleString()}</span> },
            { key: 'status', label: 'Status', render: (r: any) => <Badge variant={r.status === 'active' ? 'green' : 'default'}>{r.status}</Badge> },
            { key: 'remove', label: '', render: (r: any) => (
              <button onClick={(e) => { e.stopPropagation(); setRemoveTarget(r); }}
                className="rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-rose-500/10 hover:text-rose-400 transition-colors">
                <Trash2 size={14} />
              </button>
            )},
          ]}
          data={devices}
        />
      )}

      <Modal open={pairOpen} onClose={() => setPairOpen(false)} title="Pair New Device">
        <div className="flex flex-col items-center py-6">
          <QrCode size={120} className="mb-4 text-[var(--color-accent-light)]" />
          <div className="text-lg font-bold font-mono tracking-[0.3em] text-[var(--color-accent-light)]">{pairCode}</div>
          <p className="mt-3 text-sm text-[var(--color-dim)] text-center max-w-xs">
            Enter this code on your device or scan the QR code to pair.
          </p>
        </div>
      </Modal>

      <Modal open={!!removeTarget} onClose={() => setRemoveTarget(null)} title="Remove Device" actions={
        <>
          <button onClick={() => setRemoveTarget(null)} className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm">Cancel</button>
          <button onClick={handleRemove} className="rounded-lg bg-rose-500 px-4 py-2 text-sm text-white hover:bg-rose-400">Remove</button>
        </>
      }>
        <p className="text-sm text-[var(--color-dim)]">
          Remove <span className="font-medium text-[var(--color-text)]">{removeTarget?.name}</span>? The device will need to be re-paired to reconnect.
        </p>
      </Modal>
    </div>
  );
}
