import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { HardDrive, RotateCcw } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, DataTable, Modal, LoadingSkeleton } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
export default function Backups() {
    const { t } = useTranslation();
    const [restoreTarget, setRestoreTarget] = useState(null);
    const { data, loading, error, refresh } = useApi(() => api.getBackups(), { pollMs: 30000 });
    const backups = data || [];
    const formatSize = (bytes) => bytes > 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${(bytes / 1e6).toFixed(0)} MB`;
    if (loading && !data)
        return _jsx(LoadingSkeleton, { lines: 6 });
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [error && !data && (_jsxs("div", { className: "flex flex-col items-center py-16 text-center", children: [_jsx("div", { className: "text-4xl mb-4 opacity-40", children: "\u26A0\uFE0F" }), _jsx("h3", { className: "text-lg font-semibold text-[var(--color-dim)]", children: t('common.unable_connect') }), _jsx("p", { className: "mt-2 text-sm text-[var(--color-dim)] opacity-70", children: error }), _jsx("button", { onClick: refresh, className: "mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]", children: t('common.retry') })] })), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: t('backups.storage_usage') }), _jsx(ResponsiveContainer, { width: "100%", height: 180, children: _jsxs(BarChart, { data: [], children: [_jsx(XAxis, { dataKey: "agent", tick: { fontSize: 11, fill: '#8b8fa3' }, axisLine: false }), _jsx(YAxis, { tick: { fontSize: 11, fill: '#8b8fa3' }, axisLine: false, tickFormatter: (v) => `${v} GB` }), _jsx(Tooltip, { contentStyle: { background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' } }), _jsx(Bar, { dataKey: "used", fill: "#6366f1", radius: [4, 4, 0, 0] })] }) }), _jsx("div", { className: "flex items-center justify-center py-4 text-sm text-[var(--color-dim)] opacity-60", children: t('common.no_time_series') })] }), _jsx(DataTable, { columns: [
                    { key: 'id', label: t('backups.id'), render: (r) => _jsx("span", { className: "font-mono text-xs", children: r.id }) },
                    { key: 'hostname', label: t('backups.agent'), render: (r) => (_jsxs("div", { className: "flex items-center gap-2", children: [_jsx(HardDrive, { size: 14, className: "text-[var(--color-accent)]" }), r.hostname || r.agent_id || '—'] })) },
                    { key: 'files', label: t('backups.files'), render: (r) => r.files ? (_jsx("div", { className: "flex flex-wrap gap-1", children: r.files.map((f) => _jsx("span", { className: "rounded bg-[var(--color-surface3)] px-1.5 py-0.5 text-[10px] font-mono text-[var(--color-dim)]", children: f.split('/').pop() }, f)) })) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'size_bytes', label: t('backups.size'), render: (r) => r.size_bytes ? _jsx("span", { className: "font-mono text-xs", children: formatSize(r.size_bytes) }) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'timestamp', label: t('backups.time'), render: (r) => _jsx("span", { className: "text-xs text-[var(--color-dim)]", children: new Date(r.timestamp || r.timestamp_iso).toLocaleString() }) },
                    { key: 'status', label: 'Status', render: (r) => {
                            const status = r.status || 'completed';
                            const v = status === 'completed' ? 'green' : status === 'failed' ? 'red' : 'amber';
                            return _jsx(Badge, { variant: v, children: status });
                        } },
                    { key: 'restore', label: '', render: (r) => (r.status || 'completed') === 'completed' ? (_jsxs("button", { onClick: (e) => { e.stopPropagation(); setRestoreTarget(r); }, className: "flex items-center gap-1 rounded-lg border border-[var(--color-border)] px-2.5 py-1 text-xs hover:bg-[var(--color-surface2)] transition-colors", children: [_jsx(RotateCcw, { size: 12 }), " ", t('backups.restore')] })) : null },
                ], data: backups, searchPlaceholder: t("backups.search_backups") }), _jsx(Modal, { open: !!restoreTarget, onClose: () => setRestoreTarget(null), title: t('backups.confirm_restore'), actions: _jsxs(_Fragment, { children: [_jsx("button", { onClick: () => setRestoreTarget(null), className: "rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm", children: t('common.cancel') }), _jsx("button", { onClick: () => setRestoreTarget(null), className: "rounded-lg bg-amber-500 px-4 py-2 text-sm font-medium text-black hover:bg-amber-400", children: t('backups.restore') })] }), children: restoreTarget && (_jsxs("div", { className: "text-sm", children: [_jsxs("p", { className: "text-[var(--color-dim)]", children: [t('backups.restore_from'), " ", _jsx("span", { className: "text-[var(--color-text)]", children: new Date(restoreTarget.timestamp).toLocaleString() }), "."] }), restoreTarget.files && (_jsxs("div", { className: "mt-3 rounded-lg bg-[var(--color-bg)] p-3", children: [_jsx("div", { className: "text-xs text-[var(--color-dim)] mb-1", children: t("backups.restore_files") }), restoreTarget.files.map((f) => _jsx("div", { className: "font-mono text-xs", children: f }, f))] })), _jsx("p", { className: "mt-3 text-amber-400 text-xs", children: t("backups.restore_warning") })] })) })] }));
}
