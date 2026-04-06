import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from 'react';
import { Search, Download } from 'lucide-react';
import { Badge, DataTable } from '../components/Layout';
import { mockAuditEvents } from '../api/client';
const eventTypeIcons = {
    command_executed: '✓', command_intercepted: '🛡', command_approved: '✅',
    command_rejected: '❌', canary_triggered: '🐦', session_started: '→',
    session_ended: '←', agent_heartbeat: '💓', policy_violation: '⚠',
};
const actionBadge = {
    allowed: 'green', denied: 'red', intercept: 'amber', alert: 'blue',
};
export default function Audit() {
    const [search, setSearch] = useState('');
    const [filterType, setFilterType] = useState('all');
    const types = [...new Set(mockAuditEvents.map(e => e.event_type))];
    const filtered = mockAuditEvents.filter(e => {
        if (filterType !== 'all' && e.event_type !== filterType)
            return false;
        if (search && !`${e.command} ${e.user} ${e.agent_id}`.toLowerCase().includes(search.toLowerCase()))
            return false;
        return true;
    });
    const denied = mockAuditEvents.filter(e => e.result === 'denied').length;
    const allowed = mockAuditEvents.filter(e => e.result === 'allowed').length;
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [_jsxs("div", { className: "grid grid-cols-2 gap-4 xl:grid-cols-4", children: [_jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4", children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-[var(--color-dim)]", children: "Total Events" }), _jsx("div", { className: "mt-1 text-2xl font-bold", children: mockAuditEvents.length })] }), _jsxs("div", { className: "rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4", children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-[var(--color-dim)]", children: "Allowed" }), _jsx("div", { className: "mt-1 text-2xl font-bold text-emerald-400", children: allowed })] }), _jsxs("div", { className: "rounded-xl border border-rose-500/20 bg-rose-500/5 p-4", children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-[var(--color-dim)]", children: "Denied" }), _jsx("div", { className: "mt-1 text-2xl font-bold text-rose-400", children: denied })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4", children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-[var(--color-dim)]", children: "Deny Rate" }), _jsxs("div", { className: "mt-1 text-2xl font-bold", children: [((denied / (allowed + denied)) * 100).toFixed(1), "%"] })] })] }), _jsxs("div", { className: "flex flex-wrap items-center gap-3", children: [_jsxs("div", { className: "relative flex-1 max-w-sm", children: [_jsx(Search, { size: 16, className: "absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-dim)]" }), _jsx("input", { type: "text", placeholder: "Search commands, users...", value: search, onChange: e => setSearch(e.target.value), className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] py-2.5 pl-9 pr-3 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none" })] }), _jsxs("select", { value: filterType, onChange: e => setFilterType(e.target.value), className: "rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none", children: [_jsx("option", { value: "all", children: "All Events" }), types.map(t => _jsx("option", { value: t, children: t.replace(/_/g, ' ') }, t))] }), _jsxs("div", { className: "flex gap-2", children: [_jsxs("button", { className: "flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors", children: [_jsx(Download, { size: 14 }), " JSON"] }), _jsxs("button", { className: "flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors", children: [_jsx(Download, { size: 14 }), " CEF"] }), _jsxs("button", { className: "flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors", children: [_jsx(Download, { size: 14 }), " LEEF"] })] })] }), _jsx(DataTable, { columns: [
                    { key: 'timestamp_iso', label: 'Time', render: (r) => _jsx("span", { className: "text-xs text-[var(--color-dim)] whitespace-nowrap", children: new Date(r.timestamp_iso).toLocaleString() }) },
                    { key: 'event_type', label: 'Event', render: (r) => (_jsxs("span", { className: "flex items-center gap-1.5", children: [_jsx("span", { children: eventTypeIcons[r.event_type] || '•' }), _jsx(Badge, { variant: r.event_type === 'command_intercepted' ? 'red' : r.event_type === 'canary_triggered' ? 'amber' : 'default', children: r.event_type.replace(/_/g, ' ') })] })) },
                    { key: 'agent_id', label: 'Agent', render: (r) => _jsx("span", { className: "text-xs font-mono", children: r.agent_id }) },
                    { key: 'user', label: 'User' },
                    { key: 'command', label: 'Command', render: (r) => r.command ? _jsx("code", { className: "rounded bg-[var(--color-surface3)] px-1.5 py-0.5 text-xs", children: r.command }) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'risk_score', label: 'Risk', render: (r) => r.risk_score != null ? _jsx("span", { className: `font-mono text-xs font-bold ${r.risk_score >= 70 ? 'text-rose-400' : r.risk_score >= 40 ? 'text-amber-400' : 'text-emerald-400'}`, children: r.risk_score }) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'result', label: 'Result', render: (r) => _jsx(Badge, { variant: actionBadge[r.result || ''] || 'default', children: r.result || '—' }) },
                ], data: filtered, emptyText: "No matching audit events" })] }));
}
