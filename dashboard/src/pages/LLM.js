import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { Brain, RefreshCw, Globe, Server } from 'lucide-react';
import { StatCard, Badge, LoadingSkeleton, EmptyState, DataTable } from '../components/Layout';
export default function LLM() {
    // No API endpoints yet — show placeholder
    const loading = false;
    const backends = [];
    const healthy = true;
    if (loading) {
        return _jsx(LoadingSkeleton, { lines: 6 });
    }
    return (_jsxs("div", { className: "space-y-6", children: [_jsxs("div", { className: "grid grid-cols-1 gap-4 sm:grid-cols-3", children: [_jsx(StatCard, { label: "Backends", value: "0", color: "accent", icon: _jsx(Brain, { size: 24 }) }), _jsx(StatCard, { label: "Health", value: healthy ? 'OK' : 'Degraded', color: healthy ? 'green' : 'red', icon: _jsx(Server, { size: 24 }) }), _jsx(StatCard, { label: "Models", value: "\u2014", color: "blue", icon: _jsx(Globe, { size: 24 }) })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]", children: [_jsxs("div", { className: "flex items-center justify-between border-b border-[var(--color-border)] px-6 py-4", children: [_jsx("h3", { className: "font-semibold", children: "LLM Backends" }), _jsxs("button", { className: "flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs text-[var(--color-dim)] hover:bg-[var(--color-surface2)] transition-colors", children: [_jsx(RefreshCw, { size: 12 }), " Refresh"] })] }), backends.length === 0 ? (_jsx(EmptyState, { icon: _jsx(Brain, { size: 40 }), title: "No LLM backends configured", description: "Add an LLM backend via the API to see it here." })) : (_jsx(DataTable, { columns: [
                            { key: 'name', label: 'Name' },
                            { key: 'url', label: 'URL' },
                            { key: 'model', label: 'Model' },
                            { key: 'status', label: 'Status', render: (row) => (_jsx(Badge, { variant: row.status === 'healthy' ? 'green' : 'red', children: row.status })) },
                        ], data: backends, searchPlaceholder: "Search backends\u2026" }))] })] }));
}
