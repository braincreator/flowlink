import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useNavigate } from 'react-router-dom';
import { AreaChart, Area, PieChart, Pie, Cell, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Shield, Bot, AlertTriangle, Activity, Clock, Terminal } from 'lucide-react';
import { StatCard, Badge, LoadingSkeleton } from '../components/Layout';
import { DashboardWidgets } from '../components/DashboardWidgets';
import { useApi, useSSE } from '../hooks/useApi';
import { api } from '../api/client';
const PIE_COLORS = ['#f43f5e', '#f59e0b', '#6366f1'];
export default function Dashboard() {
    const navigate = useNavigate();
    const { data: agents, loading: agentsLoading, error: agentsError, refresh: refreshAgents } = useApi(() => api.getAgents(), { pollMs: 15000 });
    const { data: shieldStats, loading: shieldLoading, error: shieldError, refresh: refreshShield } = useApi(() => api.getShieldStats(), { pollMs: 15000 });
    const { data: auditStats, loading: auditLoading } = useApi(() => api.getAuditStats(), { pollMs: 30000 });
    const { data: alerts, loading: alertsLoading, error: alertsError, refresh: refreshAlerts } = useApi(() => api.getAlerts(), { pollMs: 10000 });
    const { data: auditEvents, loading: eventsLoading } = useApi(() => api.getAuditEvents({ limit: 20 }), { pollMs: 15000 });
    const { events: sseEvents, connected: sseConnected } = useSSE();
    const loading = agentsLoading || shieldLoading || auditLoading;
    const error = agentsError || shieldError || alertsError;
    const agentList = agents || [];
    const onlineAgents = agentList.filter((a) => a.status === 'online').length;
    const totalAgents = agentList.length;
    const s = { ...shieldStats, ...auditStats };
    const alertList = alerts || [];
    const pendingAlerts = alertList.filter((a) => !a.resolved);
    const eventList = auditEvents || [];
    const recentEvents = sseConnected && sseEvents.length > 0 ? sseEvents.slice(0, 20) : eventList.slice(0, 20);
    const pieData = [
        { name: 'L3 Critical', value: s.l3_count || 0 },
        { name: 'L2 Medium', value: s.l2_count || 0 },
        { name: 'L1 Low', value: s.l1_count || 0 },
    ];
    if (loading && !agents && !shieldStats)
        return _jsx(LoadingSkeleton, { lines: 8 });
    const widgets = [
        {
            id: 'stat-cards', title: 'Statistics', defaultOrder: 0,
            render: () => (_jsxs("div", { className: "grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4", children: [_jsx(StatCard, { label: "Total Agents", value: `${onlineAgents}/${totalAgents}`, trend: { value: 12, label: 'this week' }, icon: _jsx(Bot, { size: 24 }), color: "accent" }), _jsx(StatCard, { label: "Active Alerts", value: pendingAlerts.length, trend: { value: -8, label: 'vs yesterday' }, icon: _jsx(AlertTriangle, { size: 24 }), color: "red" }), _jsx(StatCard, { label: "Commands Today", value: (s.commands_today || 0).toLocaleString(), trend: { value: 5, label: 'vs yesterday' }, icon: _jsx(Activity, { size: 24 }), color: "green" }), _jsx(StatCard, { label: "Uptime", value: `${s.uptime_pct || 0}%`, icon: _jsx(Clock, { size: 24 }), color: "blue" })] })),
        },
        {
            id: 'quick-actions', title: 'Quick Actions', defaultOrder: 1,
            render: () => (_jsxs("div", { children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Quick Actions" }), _jsxs("div", { className: "flex flex-wrap gap-3", children: [_jsxs("button", { onClick: () => navigate('/shield'), className: "flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] hover:shadow-lg hover:shadow-indigo-500/20", children: [_jsx(Shield, { size: 16 }), " Approve Pending (", pendingAlerts.length, ")"] }), _jsxs("button", { onClick: () => navigate('/shield'), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(AlertTriangle, { size: 16 }), " View Alerts"] }), _jsxs("button", { onClick: () => navigate('/agents'), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Bot, { size: 16 }), " Deploy Agent"] }), _jsxs("button", { onClick: () => navigate('/terminal'), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Terminal, { size: 16 }), " Open Terminal"] })] })] })),
        },
        {
            id: 'commands-chart', title: 'Commands (24h)', defaultOrder: 2, colSpan: 2,
            render: () => (_jsxs("div", { children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Commands \u2014 Last 24h" }), _jsx(ResponsiveContainer, { width: "100%", height: 240, children: _jsxs(AreaChart, { data: [], children: [_jsx("defs", { children: _jsxs("linearGradient", { id: "cmdGrad", x1: "0", y1: "0", x2: "0", y2: "1", children: [_jsx("stop", { offset: "0%", stopColor: "#6366f1", stopOpacity: 0.3 }), _jsx("stop", { offset: "100%", stopColor: "#6366f1", stopOpacity: 0 })] }) }), _jsx(XAxis, { dataKey: "hour", tick: { fontSize: 11, fill: 'var(--color-dim)' }, axisLine: false, tickLine: false }), _jsx(YAxis, { tick: { fontSize: 11, fill: 'var(--color-dim)' }, axisLine: false, tickLine: false }), _jsx(Tooltip, { contentStyle: { background: 'var(--color-surface2)', border: '1px solid var(--color-border)', borderRadius: '8px', fontSize: '12px' } }), _jsx(Area, { type: "monotone", dataKey: "commands", stroke: "#6366f1", fill: "url(#cmdGrad)", strokeWidth: 2 })] }) }), _jsx("div", { className: "flex items-center justify-center py-8 text-sm text-[var(--color-dim)] opacity-60", children: "No time-series data available yet" })] })),
        },
        {
            id: 'risk-pie', title: 'Shield Status', defaultOrder: 3,
            render: () => (_jsxs("div", { children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Shield Status" }), _jsx("div", { className: "flex items-center justify-center", children: _jsx(ResponsiveContainer, { width: "100%", height: 180, children: _jsxs(PieChart, { children: [_jsx(Pie, { data: pieData, cx: "50%", cy: "50%", innerRadius: 50, outerRadius: 75, paddingAngle: 4, dataKey: "value", children: pieData.map((_, i) => _jsx(Cell, { fill: PIE_COLORS[i] }, i)) }), _jsx(Tooltip, { contentStyle: { background: 'var(--color-surface2)', border: '1px solid var(--color-border)', borderRadius: '8px', fontSize: '12px' } })] }) }) }), _jsxs("div", { className: "mt-2 flex justify-center gap-4 text-xs", children: [_jsxs("span", { className: "flex items-center gap-1", children: [_jsx("span", { className: "h-2 w-2 rounded-full bg-rose-500" }), " L3: ", s.l3_count || 0] }), _jsxs("span", { className: "flex items-center gap-1", children: [_jsx("span", { className: "h-2 w-2 rounded-full bg-amber-500" }), " L2: ", s.l2_count || 0] }), _jsxs("span", { className: "flex items-center gap-1", children: [_jsx("span", { className: "h-2 w-2 rounded-full bg-indigo-500" }), " L1: ", s.l1_count || 0] })] })] })),
        },
        {
            id: 'activity-feed', title: 'Recent Activity', defaultOrder: 4,
            render: () => (_jsxs("div", { children: [_jsxs("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: ["Recent Activity ", sseConnected && _jsx("span", { className: "ml-2 text-emerald-400", children: "\u25CF Live" })] }), recentEvents.length === 0 ? (_jsx("div", { className: "flex items-center justify-center py-12 text-sm text-[var(--color-dim)] opacity-60", children: "No recent activity" })) : (_jsx("div", { className: "space-y-2 max-h-[360px] overflow-y-auto", children: recentEvents.map((ev, i) => {
                            const icons = {
                                command_executed: '✓', command_intercepted: '🛡', session_started: '→',
                                session_ended: '←', canary_triggered: '🐦', policy_violation: '⚠', agent_heartbeat: '💓',
                            };
                            const badgeMap = {
                                command_executed: 'green', command_intercepted: 'red', session_started: 'blue',
                                canary_triggered: 'amber', policy_violation: 'red', agent_heartbeat: 'default',
                            };
                            const eventType = ev.event_type || ev.type || 'unknown';
                            const ts = ev.timestamp_iso || ev.timestamp || ev.time;
                            return (_jsxs("div", { className: "flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx("span", { className: "text-base", children: icons[eventType] || '•' }), _jsx(Badge, { variant: badgeMap[eventType] || 'default', children: eventType.replace(/_/g, ' ') }), _jsx("span", { className: "flex-1 truncate text-sm", children: ev.command || `${ev.user || ev.username || 'system'} — ${eventType.replace(/_/g, ' ')}` }), ev.risk_score && _jsxs("span", { className: "text-xs font-mono text-rose-400", children: ["risk:", ev.risk_score] }), _jsx("span", { className: "text-xs text-[var(--color-dim)]", children: ts ? new Date(ts).toLocaleTimeString() : '' })] }, ev.id || i));
                        }) }))] })),
        },
    ];
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [error && !agents && !shieldStats && (_jsxs("div", { className: "flex flex-col items-center py-16 text-center", children: [_jsx("div", { className: "text-4xl mb-4 opacity-40", children: "\u26A0\uFE0F" }), _jsx("h3", { className: "text-lg font-semibold text-[var(--color-dim)]", children: "Unable to connect to relay" }), _jsx("p", { className: "mt-2 text-sm text-[var(--color-dim)] opacity-70", children: error }), _jsx("button", { onClick: () => { refreshAgents(); refreshShield(); refreshAlerts(); }, className: "mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]", children: "Retry" })] })), _jsx(DashboardWidgets, { widgets: widgets })] }));
}
