import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { Activity, Cpu, MemoryStick, CheckCircle, XCircle } from 'lucide-react';
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import { Badge } from '../components/Layout';
import { mockSystemInfo, mockAgents } from '../api/client';
const cpuData = Array.from({ length: 30 }, (_, i) => ({ t: i, cpu: Math.round(Math.random() * 40 + 15) }));
const memData = Array.from({ length: 30 }, (_, i) => ({ t: i, mem: Math.round(Math.random() * 20 + 35) }));
const promLines = `# HELP flowlink_agents_total Total registered agents
# TYPE flowlink_agents_total gauge
flowlink_agents_total{status="online"} 6
flowlink_agents_total{status="offline"} 2

# HELP flowlink_commands_total Total commands executed
# TYPE flowlink_commands_total counter
flowlink_commands_total 1247

# HELP flowlink_shield_interceptions_total Total shield interceptions
# TYPE flowlink_shield_interceptions_total counter
flowlink_shield_interceptions_total{level="L3"} 4
flowlink_shield_interceptions_total{level="L2"} 12
flowlink_shield_interceptions_total{level="L1"} 23

# HELP flowlink_sessions_active Active sessions
# TYPE flowlink_sessions_active gauge
flowlink_sessions_active 3

# HELP flowlink_uptime_seconds Relay uptime
# TYPE flowlink_uptime_seconds counter
flowlink_uptime_seconds 1234567`;
export default function Metrics() {
    const info = mockSystemInfo;
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [_jsx("div", { className: "grid grid-cols-1 gap-4 sm:grid-cols-3", children: [
                    { name: 'Relay', status: 'healthy', detail: 'v0.9.2 · 14d uptime' },
                    { name: 'Database', status: 'healthy', detail: 'SQLite · 12 MB' },
                    { name: 'Agents', status: 'degraded', detail: '6/8 online' },
                ].map(h => (_jsxs("div", { className: "flex items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4", children: [h.status === 'healthy' ? _jsx(CheckCircle, { size: 20, className: "text-emerald-400" }) : _jsx(XCircle, { size: 20, className: "text-amber-400" }), _jsxs("div", { children: [_jsx("div", { className: "font-medium", children: h.name }), _jsx("div", { className: "text-xs text-[var(--color-dim)]", children: h.detail })] }), _jsx(Badge, { variant: h.status === 'healthy' ? 'green' : 'amber', className: "ml-auto", children: h.status })] }, h.name))) }), _jsxs("div", { className: "grid grid-cols-1 gap-6 xl:grid-cols-2", children: [_jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsxs("div", { className: "flex items-center gap-2 mb-4", children: [_jsx(Cpu, { size: 16, className: "text-[var(--color-accent)]" }), _jsx("h3", { className: "text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "CPU Usage" }), _jsxs("span", { className: "ml-auto text-lg font-bold", children: [info.cpu_usage, "%"] })] }), _jsx(ResponsiveContainer, { width: "100%", height: 160, children: _jsxs(LineChart, { data: cpuData, children: [_jsx(XAxis, { dataKey: "t", tick: false, axisLine: false }), _jsx(YAxis, { tick: { fontSize: 11, fill: '#8b8fa3' }, axisLine: false, domain: [0, 100], tickFormatter: (v) => `${v}%` }), _jsx(Tooltip, { contentStyle: { background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' } }), _jsx(Line, { type: "monotone", dataKey: "cpu", stroke: "#6366f1", strokeWidth: 2, dot: false })] }) })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsxs("div", { className: "flex items-center gap-2 mb-4", children: [_jsx(MemoryStick, { size: 16, className: "text-emerald-400" }), _jsx("h3", { className: "text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Memory Usage" }), _jsxs("span", { className: "ml-auto text-lg font-bold", children: [info.memory_usage, "%"] })] }), _jsx(ResponsiveContainer, { width: "100%", height: 160, children: _jsxs(LineChart, { data: memData, children: [_jsx(XAxis, { dataKey: "t", tick: false, axisLine: false }), _jsx(YAxis, { tick: { fontSize: 11, fill: '#8b8fa3' }, axisLine: false, domain: [0, 100], tickFormatter: (v) => `${v}%` }), _jsx(Tooltip, { contentStyle: { background: '#1e2235', border: '1px solid #2e3142', borderRadius: '8px', fontSize: '12px' } }), _jsx(Line, { type: "monotone", dataKey: "mem", stroke: "#10b981", strokeWidth: 2, dot: false })] }) })] })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsx("h3", { className: "mb-4 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Agent Resources" }), _jsx("div", { className: "grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4", children: mockAgents.filter(a => a.status === 'online').map(a => (_jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] p-4", children: [_jsxs("div", { className: "flex items-center gap-2 mb-3", children: [_jsx("div", { className: "h-2 w-2 rounded-full bg-emerald-400 pulse-dot" }), _jsx("span", { className: "text-sm font-medium", children: a.hostname })] }), [
                                    { label: 'CPU', val: a.cpu, color: a.cpu > 80 ? '#f43f5e' : '#10b981' },
                                    { label: 'RAM', val: a.ram, color: a.ram > 80 ? '#f43f5e' : '#10b981' },
                                    { label: 'Disk', val: a.disk, color: a.disk > 80 ? '#f59e0b' : '#10b981' },
                                ].map(m => (_jsxs("div", { className: "mb-2", children: [_jsxs("div", { className: "flex justify-between text-xs mb-1", children: [_jsx("span", { className: "text-[var(--color-dim)]", children: m.label }), _jsxs("span", { className: "font-mono", style: { color: m.color }, children: [m.val, "%"] })] }), _jsx("div", { className: "h-1.5 rounded-full bg-[var(--color-surface3)]", children: _jsx("div", { className: "h-full rounded-full transition-all", style: { width: `${m.val}%`, backgroundColor: m.color } }) })] }, m.label)))] }, a.id))) })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsxs("div", { className: "flex items-center gap-2 mb-4", children: [_jsx(Activity, { size: 16, className: "text-[var(--color-accent)]" }), _jsx("h3", { className: "text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Prometheus Metrics" })] }), _jsx("pre", { className: "rounded-xl bg-[#0d0e14] p-4 font-mono text-xs leading-relaxed text-[var(--color-dim)] overflow-x-auto max-h-96 overflow-y-auto", children: promLines })] })] }));
}
