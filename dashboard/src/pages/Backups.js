import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useState } from 'react';
import { HardDrive, RotateCcw } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, DataTable, Modal, LoadingSkeleton } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import { mockBackups, mockStorageByAgent } from '../api/client';
export default function Backups() {
    const [restoreTarget, setRestoreTarget] = useState(null);
    const { data: backups, loading, isLive } = useApi(() => api.getAuditEvents({ event_type: 'backup', limit: 100 }), mockBackups, { pollMs: 30000 });
    const formatSize = (bytes) => bytes > 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${(bytes / 1e6).toFixed(0)} MB`;
    if (loading)
        return _jsx(LoadingSkeleton, { lines: 6 });
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [!isLive && (_jsx("div", { className: "rounded-xl border border-amber-500/30 bg-amber-500/5 px-4 py-3 text-sm text-amber-400", children: "\u26A0\uFE0F Connected to mock data. Start relay for live data." })), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Storage Usage by Agent" }), _jsx(ResponsiveContainer, { width: "100%", height: 180, children: _jsxs(BarChart, { data: mockStorageByAgent, children: [_jsx(XAxis, { dataKey: "agent", tick: { fontSize: 11, fill: '#8b8fa3' }, axisLine: false }), _jsx(YAxis, { tick: { fontSize: 11, fill: '#8b8fa3' }, axisLine: false, tickFormatter: (v) => `${v} GB` }), _jsx(Tooltip, { contentStyle: { background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' } }), _jsx(Bar, { dataKey: "used", fill: "#6366f1", radius: [4, 4, 0, 0] })] }) })] }), _jsx(DataTable, { columns: [
                    { key: 'id', label: 'ID', render: (r) => _jsx("span", { className: "font-mono text-xs", children: r.id }) },
                    { key: 'hostname', label: 'Agent', render: (r) => (_jsxs("div", { className: "flex items-center gap-2", children: [_jsx(HardDrive, { size: 14, className: "text-[var(--color-accent)]" }), r.hostname || r.agent_id || '—'] })) },
                    { key: 'files', label: 'Files', render: (r) => r.files ? (_jsx("div", { className: "flex flex-wrap gap-1", children: r.files.map((f) => _jsx("span", { className: "rounded bg-[var(--color-surface3)] px-1.5 py-0.5 text-[10px] font-mono text-[var(--color-dim)]", children: f.split('/').pop() }, f)) })) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'size_bytes', label: 'Size', render: (r) => r.size_bytes ? _jsx("span", { className: "font-mono text-xs", children: formatSize(r.size_bytes) }) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'timestamp', label: 'Time', render: (r) => _jsx("span", { className: "text-xs text-[var(--color-dim)]", children: new Date(r.timestamp || r.timestamp_iso).toLocaleString() }) },
                    { key: 'status', label: 'Status', render: (r) => {
                            const status = r.status || 'completed';
                            const v = status === 'completed' ? 'green' : status === 'failed' ? 'red' : 'amber';
                            return _jsx(Badge, { variant: v, children: status });
                        } },
                    { key: 'restore', label: '', render: (r) => (r.status || 'completed') === 'completed' ? (_jsxs("button", { onClick: (e) => { e.stopPropagation(); setRestoreTarget(r); }, className: "flex items-center gap-1 rounded-lg border border-[var(--color-border)] px-2.5 py-1 text-xs hover:bg-[var(--color-surface2)] transition-colors", children: [_jsx(RotateCcw, { size: 12 }), " Restore"] })) : null },
                ], data: backups, searchPlaceholder: "Search backups..." }), _jsx(Modal, { open: !!restoreTarget, onClose: () => setRestoreTarget(null), title: "Restore Backup", actions: _jsxs(_Fragment, { children: [_jsx("button", { onClick: () => setRestoreTarget(null), className: "rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm", children: "Cancel" }), _jsx("button", { onClick: () => setRestoreTarget(null), className: "rounded-lg bg-amber-500 px-4 py-2 text-sm font-medium text-black hover:bg-amber-400", children: "Restore" })] }), children: restoreTarget && (_jsxs("div", { className: "text-sm", children: [_jsxs("p", { className: "text-[var(--color-dim)]", children: ["Restore from backup created on ", _jsx("span", { className: "text-[var(--color-text)]", children: new Date(restoreTarget.timestamp).toLocaleString() }), "."] }), restoreTarget.files && (_jsxs("div", { className: "mt-3 rounded-lg bg-[var(--color-bg)] p-3", children: [_jsx("div", { className: "text-xs text-[var(--color-dim)] mb-1", children: "Files:" }), restoreTarget.files.map((f) => _jsx("div", { className: "font-mono text-xs", children: f }, f))] })), _jsx("p", { className: "mt-3 text-amber-400 text-xs", children: "\u26A0 This will overwrite current files on the agent." })] })) })] }));
}
