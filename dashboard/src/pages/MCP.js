import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from 'react';
import { Wrench, Play, Server, Terminal } from 'lucide-react';
import { StatCard, Badge, LoadingSkeleton, EmptyState, DataTable } from '../components/Layout';
export default function MCP() {
    const [toolInput, setToolInput] = useState('');
    const [result, setResult] = useState(null);
    const servers = [];
    const loading = false;
    if (loading) {
        return _jsx(LoadingSkeleton, { lines: 6 });
    }
    return (_jsxs("div", { className: "space-y-6", children: [_jsxs("div", { className: "grid grid-cols-1 gap-4 sm:grid-cols-3", children: [_jsx(StatCard, { label: "Servers", value: "0", color: "accent", icon: _jsx(Server, { size: 24 }) }), _jsx(StatCard, { label: "Tools", value: "0", color: "green", icon: _jsx(Wrench, { size: 24 }) }), _jsx(StatCard, { label: "Calls Today", value: "\u2014", color: "blue", icon: _jsx(Terminal, { size: 24 }) })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6", children: [_jsx("h3", { className: "mb-4 font-semibold", children: "Tool Execution" }), _jsxs("div", { className: "flex gap-3", children: [_jsx("input", { type: "text", placeholder: '{"tool": "name", "args": {}}}', value: toolInput, onChange: e => setToolInput(e.target.value), className: "flex-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm font-mono placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none transition-colors" }), _jsxs("button", { className: "flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90 transition-opacity", children: [_jsx(Play, { size: 14 }), " Execute"] })] }), result && (_jsx("pre", { className: "mt-4 rounded-lg bg-[var(--color-bg)] border border-[var(--color-border)] p-4 text-xs font-mono text-[var(--color-dim)] overflow-x-auto max-h-64", children: result }))] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]", children: [_jsx("div", { className: "border-b border-[var(--color-border)] px-6 py-4", children: _jsx("h3", { className: "font-semibold", children: "Connected MCP Servers" }) }), servers.length === 0 ? (_jsx(EmptyState, { icon: _jsx(Wrench, { size: 40 }), title: "No MCP servers connected", description: "Configure MCP servers via the API to manage tools here." })) : (_jsx(DataTable, { columns: [
                            { key: 'name', label: 'Server' },
                            { key: 'url', label: 'URL' },
                            { key: 'tools', label: 'Tools' },
                            { key: 'status', label: 'Status', render: (row) => (_jsx(Badge, { variant: row.status === 'connected' ? 'green' : 'red', children: row.status })) },
                        ], data: servers, searchPlaceholder: "Search servers\u2026" }))] })] }));
}
