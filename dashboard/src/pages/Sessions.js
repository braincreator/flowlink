import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from 'react';
import { Play } from 'lucide-react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge, DataTable, Modal, LoadingSkeleton } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import { mockSessions } from '../api/client';
export default function Sessions() {
    const [replaySession, setReplaySession] = useState(null);
    const { data: sessions, loading, isLive } = useApi(() => api.getAuditEvents({ event_type: 'session_started', limit: 50 }), mockSessions, { pollMs: 15000 });
    const activeSessions = sessions.filter((s) => s.status === 'active');
    const durationData = sessions.map((s) => ({
        id: (s.id || '').slice(0, 8),
        duration: Math.round((s.duration_ms || 0) / 60000),
    }));
    if (loading)
        return _jsx(LoadingSkeleton, { lines: 6 });
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [!isLive && (_jsx("div", { className: "rounded-xl border border-amber-500/30 bg-amber-500/5 px-4 py-3 text-sm text-amber-400", children: "\u26A0\uFE0F Connected to mock data. Start relay for live data." })), _jsxs("div", { className: "grid grid-cols-3 gap-4", children: [_jsxs("div", { className: "rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4", children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-[var(--color-dim)]", children: "Active Now" }), _jsx("div", { className: "mt-1 text-2xl font-bold text-emerald-400", children: activeSessions.length })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4", children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-[var(--color-dim)]", children: "Total Today" }), _jsx("div", { className: "mt-1 text-2xl font-bold", children: sessions.length })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4", children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-[var(--color-dim)]", children: "Avg Commands" }), _jsx("div", { className: "mt-1 text-2xl font-bold", children: sessions.length > 0 ? Math.round(sessions.reduce((a, s) => a + (s.commands_count || 0), 0) / sessions.length) : 0 })] })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Session Duration (min)" }), _jsx(ResponsiveContainer, { width: "100%", height: 160, children: _jsxs(BarChart, { data: durationData, children: [_jsx(XAxis, { dataKey: "id", tick: { fontSize: 11, fill: '#8b8fa3' }, axisLine: false }), _jsx(YAxis, { tick: { fontSize: 11, fill: '#8b8fa3' }, axisLine: false }), _jsx(Tooltip, { contentStyle: { background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' } }), _jsx(Bar, { dataKey: "duration", fill: "#6366f1", radius: [4, 4, 0, 0] })] }) })] }), _jsx(DataTable, { columns: [
                    { key: 'id', label: 'Session', render: (r) => _jsx("span", { className: "font-mono text-xs", children: r.id }) },
                    { key: 'user', label: 'User', render: (r) => r.user || '—' },
                    { key: 'agent_id', label: 'Agent', render: (r) => _jsx("span", { className: "font-mono text-xs text-[var(--color-dim)]", children: r.agent_id }) },
                    { key: 'origin', label: 'Origin', render: (r) => r.origin ? _jsx("span", { className: "font-mono text-xs", children: r.origin }) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'terminal', label: 'Terminal', render: (r) => r.terminal ? _jsx("span", { className: "text-xs text-[var(--color-dim)]", children: r.terminal }) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'commands_count', label: 'Commands', render: (r) => r.commands_count ?? '—' },
                    { key: 'duration_ms', label: 'Duration', render: (r) => r.duration_ms ? _jsxs("span", { className: "text-xs", children: [Math.round(r.duration_ms / 60000), "m"] }) : _jsx("span", { className: "text-[var(--color-dim)]", children: "\u2014" }) },
                    { key: 'status', label: 'Status', render: (r) => (_jsxs(Badge, { variant: r.status === 'active' ? 'green' : 'default', children: [_jsx("span", { className: `inline-block h-1.5 w-1.5 rounded-full ${r.status === 'active' ? 'bg-emerald-400 pulse-dot' : ''}` }), r.status] })) },
                    { key: 'replay', label: '', render: (r) => r.status === 'ended' ? (_jsxs("button", { onClick: (e) => { e.stopPropagation(); setReplaySession(r); }, className: "flex items-center gap-1 rounded-lg bg-[var(--color-accent)]/15 px-2.5 py-1 text-xs font-medium text-[var(--color-accent-light)] hover:bg-[var(--color-accent)]/25 transition-colors", children: [_jsx(Play, { size: 12 }), " Replay"] })) : null },
                ], data: sessions, searchPlaceholder: "Search sessions..." }), _jsx(Modal, { open: !!replaySession, onClose: () => setReplaySession(null), title: `Session Replay — ${replaySession?.id}`, children: _jsxs("div", { className: "rounded-xl bg-[#0d0e14] p-4 font-mono text-sm min-h-[300px]", children: [_jsx("div", { className: "text-[var(--color-dim)]", children: "Session replay placeholder" }), _jsx("div", { className: "mt-2 text-xs text-[var(--color-dim)]", children: "asciinema-player integration point" })] }) })] }));
}
