import { useState } from 'react';
import { UserPlus, Key, Shield, Edit } from 'lucide-react';
import { Badge, DataTable, SlidePanel, Modal, LoadingSkeleton, EmptyState } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';

export default function RBAC() {
  const [addOpen, setAddOpen] = useState(false);
  const [editUser, setEditUser] = useState<any>(null);
  const [tokenUser, setTokenUser] = useState<any>(null);

  const { data, loading, error, refresh } = useApi(
    () => api.getRbacUsers(),
    { pollMs: 30000 }
  );

  const users = data || [];

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
        <div className="text-sm text-[var(--color-dim)]">{users.length} users</div>
        <button onClick={() => setAddOpen(true)} className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]">
          <UserPlus size={16} /> Add User
        </button>
      </div>

      {users.length === 0 && !error ? (
        <EmptyState icon={<Shield size={48} />} title="No users configured" description="Add users to manage access control" />
      ) : (
        <DataTable
          columns={[
            { key: 'username', label: 'User', render: (r: any) => (
              <div className="flex items-center gap-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 text-xs font-bold text-white">
                  {r.username[0].toUpperCase()}
                </div>
                <span className="font-medium">{r.username}</span>
              </div>
            )},
            { key: 'roles', label: 'Roles', render: (r: any) => (
              <div className="flex gap-1">{(r.roles || []).map((role: string) => <Badge key={role} variant={role === 'admin' ? 'red' : role === 'operator' ? 'amber' : 'blue'}>{role}</Badge>)}</div>
            )},
            { key: 'allowed_paths', label: 'Paths', render: (r: any) => (
              <div className="flex flex-wrap gap-1">{(r.allowed_paths || []).slice(0, 2).map((p: string) => <span key={p} className="rounded bg-[var(--color-surface3)] px-1.5 py-0.5 text-[10px] font-mono text-[var(--color-dim)]">{p}</span>)}
                {(r.allowed_paths || []).length > 2 && <span className="text-[10px] text-[var(--color-dim)]">+{(r.allowed_paths || []).length - 2}</span>}
              </div>
            )},
            { key: 'status', label: 'Status', render: (r: any) => <Badge variant={r.status === 'active' ? 'green' : 'red'}>{r.status}</Badge> },
            { key: 'created_at', label: 'Created', render: (r: any) => <span className="text-xs text-[var(--color-dim)]">{new Date(r.created_at).toLocaleDateString()}</span> },
            { key: 'actions', label: '', render: (r: any) => (
              <div className="flex gap-1">
                <button onClick={(e) => { e.stopPropagation(); setEditUser(r); }} className="rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors"><Edit size={14} /></button>
                <button onClick={(e) => { e.stopPropagation(); setTokenUser(r); }} className="rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors"><Key size={14} /></button>
              </div>
            )},
          ]}
          data={users} searchPlaceholder="Search users..."
        />
      )}

      <SlidePanel open={!!editUser} onClose={() => setEditUser(null)} title={`Edit ${editUser?.username || ''}`}>
        {editUser && (
          <div className="space-y-6">
            <div>
              <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Roles</label>
              <div className="flex flex-wrap gap-2">
                {['admin', 'operator', 'viewer'].map(role => (
                  <label key={role} className={`flex items-center gap-2 rounded-lg border px-3 py-2 text-sm cursor-pointer transition-colors ${(editUser.roles || []).includes(role) ? 'border-[var(--color-accent)] bg-[var(--color-accent)]/10 text-[var(--color-accent-light)]' : 'border-[var(--color-border)] text-[var(--color-dim)]'}`}>
                    <input type="checkbox" defaultChecked={(editUser.roles || []).includes(role)} className="sr-only" />
                    <Shield size={14} /> {role}
                  </label>
                ))}
              </div>
            </div>
            <div>
              <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Allowed Paths</label>
              <textarea defaultValue={(editUser.allowed_paths || []).join('\n')} rows={4}
                className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none resize-none" />
            </div>
            <button onClick={() => { setEditUser(null); refresh(); }} className="w-full rounded-xl bg-[var(--color-accent)] py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]">Save Changes</button>
          </div>
        )}
      </SlidePanel>

      <Modal open={addOpen} onClose={() => setAddOpen(false)} title="Add User" actions={
        <>
          <button onClick={() => setAddOpen(false)} className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm">Cancel</button>
          <button onClick={() => { setAddOpen(false); refresh(); }} className="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white">Create User</button>
        </>
      }>
        <div className="space-y-3">
          <div><label className="mb-1 block text-sm text-[var(--color-dim)]">Username</label>
            <input type="text" placeholder="e.g. newuser" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" /></div>
          <div><label className="mb-1 block text-sm text-[var(--color-dim)]">Role</label>
            <select className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
              <option>admin</option><option>operator</option><option>viewer</option>
            </select></div>
          <div><label className="mb-1 block text-sm text-[var(--color-dim)]">Allowed Paths</label>
            <textarea placeholder="/opt/app\n/var/log" rows={3} className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none resize-none" /></div>
        </div>
      </Modal>

      <Modal open={!!tokenUser} onClose={() => setTokenUser(null)} title={`API Tokens — ${tokenUser?.username || ''}`}>
        <div className="space-y-3">
          <div className="rounded-lg bg-[var(--color-bg)] p-3 font-mono text-xs">
            <div className="flex items-center justify-between mb-2">
              <span className="text-[var(--color-dim)]">Active Tokens</span>
              <button className="text-[var(--color-accent-light)] hover:underline">+ Issue Token</button>
            </div>
            <div className="flex items-center justify-between rounded-lg bg-[var(--color-surface)] p-2">
              <span className="text-[var(--color-dim)]">fl_token_a1b2...x9y8</span>
              <span className="text-xs text-[var(--color-dim)]">30d left</span>
            </div>
          </div>
        </div>
      </Modal>
    </div>
  );
}
