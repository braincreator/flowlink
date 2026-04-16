import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { AreaChart, Area, PieChart, Pie, Cell, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Shield, Bot, AlertTriangle, Activity, Clock, TerminalSquare, Download, RefreshCw } from 'lucide-react';
import { StatCard, Badge, LoadingSkeleton } from '../components/Layout';
import { DashboardWidgets } from '../components/DashboardWidgets';
import { useApi, useSSE } from '../hooks/useApi';
import { useSettings } from '../hooks/useSettings';
import { api } from '../api/client';
import { exportChartImage } from '../utils/chartExport';
const PIE_COLORS = ['#f43f5e', '#f59e0b', '#6366f1'];
// Mock data for charts until we implement proper time series API
const generateMockHourlyData = () => {
    const hours = [];
    const now = new Date();
    for (let i = 23; i >= 0; i--) {
        const hour = new Date(now.getTime() - i * 60 * 60 * 1000);
        hours.push({
            hour: hour.getHours() + ':00',
            commands: Math.floor(Math.random() * 100) + 20,
        });
    }
    return hours;
};
export default function Dashboard() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const { data: shieldStats, loading: shieldLoading, error: shieldError, refresh: refreshShield } = useApi(() => api.getShieldStats(), { pollMs: 15000 });
    const { data: auditStats, loading: auditLoading } = useApi(() => api.getAuditStats(), { pollMs: 30000 });
    const { data: alerts, loading: alertsLoading, error: alertsError, refresh: refreshAlerts } = useApi(() => api.getAlerts(), { pollMs: 10000 });
    const { data: auditEvents, loading: eventsLoading } = useApi(() => api.getAuditEvents({ limit: 20 }), { pollMs: 15000 });
    const { events: sseEvents, connected: sseConnected } = useSSE();
    const { billingInfo, servers, usage } = useSettings();
    const [hourlyData, setHourlyData] = useState([]);
    const loading = shieldLoading || auditLoading;
    const error = shieldError || alertsError;
    // Initialize mock data
    useEffect(() => {
        setHourlyData(generateMockHourlyData());
    }, []);
    const agentList = servers || [];
    const onlineAgents = agentList.filter((s) => s.status === 'online').length;
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
    if (loading && !shieldStats)
        return _jsx(LoadingSkeleton, { lines: 8 });
    const widgets = [
        {
            id: 'stat-cards', title: t('dashboard.statistics'), defaultOrder: 0,
            render: () => (_jsxs("div", { className: "grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4", children: [_jsx(StatCard, { label: t('dashboard.agents_online'), value: `${onlineAgents}/${totalAgents}`, trend: { value: 12, label: t('dashboard.this_week') }, icon: _jsx(Bot, { size: 24 }), color: "accent" }), _jsx(StatCard, { label: t('dashboard.active_alerts'), value: pendingAlerts.length, trend: { value: -8, label: t('dashboard.vs_yesterday') }, icon: _jsx(AlertTriangle, { size: 24 }), color: "red" }), _jsx(StatCard, { label: t('dashboard.commands_today'), value: (usage?.tracker?.daily_requests || 0).toLocaleString(), trend: { value: 5, label: 'vs yesterday' }, icon: _jsx(Activity, { size: 24 }), color: "green" }), _jsx(StatCard, { label: t('dashboard.uptime'), value: `${s.uptime_pct || 0}%`, icon: _jsx(Clock, { size: 24 }), color: "blue" })] })),
        },
        {
            id: 'quick-actions', title: t('dashboard.quick_actions'), defaultOrder: 1,
            render: () => (_jsxs("div", { children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: t('dashboard.quick_actions') }), _jsxs("div", { className: "flex flex-wrap gap-3", children: [_jsxs("button", { onClick: () => navigate('/shield'), className: "flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] hover:shadow-lg hover:shadow-indigo-500/20", children: [_jsx(Shield, { size: 16 }), " ", t('dashboard.approve_pending'), " (", pendingAlerts.length, ")"] }), _jsxs("button", { onClick: () => navigate('/shield'), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(AlertTriangle, { size: 16 }), " ", t('dashboard.view_alerts')] }), _jsxs("button", { onClick: () => navigate('/servers'), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Bot, { size: 16 }), " ", t('dashboard.manage_servers')] }), _jsxs("button", { onClick: () => navigate('/terminal'), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(TerminalSquare, { size: 16 }), " ", t('nav.terminal')] })] })] })),
        },
        {
            id: 'billing-overview', title: t('dashboard.billing_overview'), defaultOrder: 2,
            render: () => (_jsxs("div", { children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: t('dashboard.billing_overview') }), _jsxs("div", { className: "grid grid-cols-3 gap-4", children: [_jsxs("div", { className: "rounded-lg bg-[var(--color-bg)] p-4 text-center", children: [_jsx("div", { className: "text-xs text-[var(--color-dim)] mb-1", children: t('dashboard.current_plan') }), _jsx("div", { className: "text-lg font-semibold", children: billingInfo?.plan_name || '—' }), _jsx("div", { className: "text-xs text-[var(--color-dim)] mt-1", children: billingInfo?.balance_rub || '—' })] }), _jsxs("div", { className: "rounded-lg bg-[var(--color-bg)] p-4 text-center", children: [_jsx("div", { className: "text-xs text-[var(--color-dim)] mb-1", children: t('dashboard.active_agents') }), _jsx("div", { className: "text-lg font-semibold", children: usage?.tracker?.active_agents || 0 }), _jsx("div", { className: "text-xs text-[var(--color-dim)] mt-1", children: "servers" })] }), _jsxs("div", { className: "rounded-lg bg-[var(--color-bg)] p-4 text-center", children: [_jsx("div", { className: "text-xs text-[var(--color-dim)] mb-1", children: t('daily_usage') }), _jsx("div", { className: "text-lg font-semibold", children: (usage?.tracker?.daily_tokens || 0).toLocaleString() }), _jsx("div", { className: "text-xs text-[var(--color-dim)] mt-1", children: "tokens" })] })] })] })),
        },
        {
            id: 'commands-chart', title: t('dashboard.commands_24h'), defaultOrder: 3, colSpan: 2,
            render: () => (_jsxs("div", { children: [_jsxs("div", { className: "flex items-center justify-between mb-4", children: [_jsx("h3", { className: "text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: t('dashboard.last_24h') }), _jsxs("button", { onClick: () => {
                                    setHourlyData(generateMockHourlyData());
                                }, className: "flex items-center gap-1.5 text-xs text-indigo-400 hover:text-indigo-300 transition-colors", children: [_jsx(RefreshCw, { size: 12 }), t('common.refresh')] })] }), _jsx("div", { className: "flex items-center justify-end mb-2", children: _jsx("button", { onClick: () => exportChartImage('chart-commands', 'flowlink-commands-24h'), className: "rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", "aria-label": "Export chart as image", title: "Export as PNG", children: _jsx(Download, { size: 14 }) }) }), _jsx(ResponsiveContainer, { width: "100%", height: 240, children: _jsxs(AreaChart, { data: hourlyData, children: [_jsx("defs", { children: _jsxs("linearGradient", { id: "cmdGrad", x1: "0", y1: "0", x2: "0", y2: "1", children: [_jsx("stop", { offset: "0%", stopColor: "#6366f1", stopOpacity: 0.3 }), _jsx("stop", { offset: "100%", stopColor: "#6366f1", stopOpacity: 0 })] }) }), _jsx(XAxis, { dataKey: "hour", tick: { fontSize: 11, fill: 'var(--color-dim)' }, axisLine: false, tickLine: false }), _jsx(YAxis, { tick: { fontSize: 11, fill: 'var(--color-dim)' }, axisLine: false, tickLine: false }), _jsx(Tooltip, { contentStyle: { background: 'var(--color-surface2)', border: '1px solid var(--color-border)', borderRadius: '8px', fontSize: '12px' } }), _jsx(Area, { type: "monotone", dataKey: "commands", stroke: "#6366f1", fill: "url(#cmdGrad)", strokeWidth: 2 })] }) })] })),
        },
        {
            id: 'risk-pie', title: t('dashboard.shield_status'), defaultOrder: 4,
            render: () => (_jsxs("div", { children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: t('dashboard.shield_status') }), _jsx("div", { className: "flex items-center justify-end mb-2", children: _jsx("button", { onClick: () => exportChartImage('chart-risk', 'flowlink-risk-distribution'), className: "rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", "aria-label": "Export chart as image", title: "Export as PNG", children: _jsx(Download, { size: 14 }) }) }), _jsx("div", { className: "flex items-center justify-center", children: _jsx(ResponsiveContainer, { width: "100%", height: 180, children: _jsxs(PieChart, { children: [_jsx(Pie, { data: pieData, cx: "50%", cy: "50%", innerRadius: 50, outerRadius: 75, paddingAngle: 4, dataKey: "value", children: pieData.map((_, i) => _jsx(Cell, { fill: PIE_COLORS[i] }, i)) }), _jsx(Tooltip, { contentStyle: { background: 'var(--color-surface2)', border: '1px solid var(--color-border)', borderRadius: '8px', fontSize: '12px' } })] }) }) }), _jsxs("div", { className: "mt-2 flex justify-center gap-4 text-xs", children: [_jsxs("span", { className: "flex items-center gap-1", children: [_jsx("span", { className: "h-2 w-2 rounded-full bg-rose-500" }), " L3: ", s.l3_count || 0] }), _jsxs("span", { className: "flex items-center gap-1", children: [_jsx("span", { className: "h-2 w-2 rounded-full bg-amber-500" }), " L2: ", s.l2_count || 0] }), _jsxs("span", { className: "flex items-center gap-1", children: [_jsx("span", { className: "h-2 w-2 rounded-full bg-indigo-500" }), " L1: ", s.l1_count || 0] })] })] })),
        },
        {
            id: 'server-status', title: t('dashboard.server_status'), defaultOrder: 5,
            render: () => (_jsxs("div", { children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: t('dashboard.server_status') }), _jsx("div", { className: "space-y-3", children: servers.length > 0 ? (servers.slice(0, 5).map((server) => (_jsxs("div", { className: "flex items-center justify-between rounded-lg bg-[var(--color-bg)] p-3", children: [_jsxs("div", { className: "flex items-center gap-3", children: [_jsx("div", { className: `h-2 w-2 rounded-full ${server.status === 'online' ? 'bg-green-400' : 'bg-red-400'}` }), _jsx("span", { className: "font-medium", children: server.name })] }), _jsx("div", { className: "text-xs text-[var(--color-dim)]", children: server.status === 'online' ? 'Online' : 'Offline' })] }, server.id)))) : (_jsxs("div", { className: "text-center py-8 text-[var(--color-dim)]", children: [_jsx(Bot, { size: 24, className: "mx-auto mb-2 opacity-50" }), _jsx("p", { children: t('dashboard.no_servers') })] })) })] })),
        },
        {
            id: 'activity-feed', title: t('dashboard.activity_feed'), defaultOrder: 6,
            render: () => (_jsxs("div", { children: [_jsxs("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: [t('dashboard.activity_feed'), sseConnected && _jsx("span", { className: "ml-2 text-emerald-400", children: "\u25CF Live" })] }), recentEvents.length === 0 ? (_jsx("div", { className: "flex items-center justify-center py-12 text-sm text-[var(--color-dim)] opacity-60", children: t('common.no_recent_activity') })) : (_jsx("div", { className: "space-y-2 max-h-[360px] overflow-y-auto", children: recentEvents.map((ev, i) => {
                            const icons = {
                                command_executed: '✓', command_intercepted: '🛡', session_started: '→',
                                session_ended: '←', canary_triggered: '🐦', policy_violation: '⚠', agent_heartbeat: '💓',
                                payment_success: '💰', payment_failed: '❌', plan_changed: '🔄', server_started: '🚀',
                                server_stopped: '🛑', backup_created: '💾', error_occurred: '⚠',
                            };
                            const badgeMap = {
                                command_executed: 'green', command_intercepted: 'red', session_started: 'blue',
                                canary_triggered: 'amber', policy_violation: 'red', agent_heartbeat: 'default',
                                payment_success: 'green', payment_failed: 'red', plan_changed: 'blue', server_started: 'green',
                                server_stopped: 'amber', backup_created: 'blue', error_occurred: 'red',
                            };
                            const eventType = ev.event_type || ev.type || 'unknown';
                            const ts = ev.timestamp_iso || ev.timestamp || ev.time;
                            return (_jsxs("div", { className: "flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx("span", { className: "text-base", children: icons[eventType] || '•' }), _jsx(Badge, { variant: badgeMap[eventType] || 'default', children: eventType.replace(/_/g, ' ') }), _jsx("span", { className: "flex-1 truncate text-sm", children: ev.command || `${ev.user || ev.username || 'system'} — ${eventType.replace(/_/g, ' ')}` }), ev.risk_score && _jsxs("span", { className: "text-xs font-mono text-rose-400", children: ["risk:", ev.risk_score] }), _jsx("span", { className: "text-xs text-[var(--color-dim)]", children: ts ? new Date(ts).toLocaleTimeString() : '' })] }, ev.id || i));
                        }) }))] })),
        },
    ];
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [error && !shieldStats && (_jsxs("div", { className: "flex flex-col items-center py-16 text-center", children: [_jsx(AlertTriangle, { size: 40, className: "mb-4 text-[var(--color-dim)] opacity-40" }), _jsx("h3", { className: "text-lg font-semibold text-[var(--color-dim)]", children: t('common.unable_connect') }), _jsx("p", { className: "mt-2 text-sm text-[var(--color-dim)] opacity-70", children: error }), _jsx("button", { onClick: () => { refreshShield(); refreshAlerts(); }, className: "mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]", children: t('common.retry') })] })), _jsx(DashboardWidgets, { widgets: widgets })] }));
}
