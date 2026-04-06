import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
export function StatCard({ label, value, trend, sparkline, icon, color = 'accent' }) {
    const colors = {
        accent: 'from-indigo-500/20 to-indigo-600/5 border-indigo-500/30',
        green: 'from-emerald-500/20 to-emerald-600/5 border-emerald-500/30',
        red: 'from-rose-500/20 to-rose-600/5 border-rose-500/30',
        amber: 'from-amber-500/20 to-amber-600/5 border-amber-500/30',
        blue: 'from-blue-500/20 to-blue-600/5 border-blue-500/30',
    };
    return (_jsxs("div", { className: `relative overflow-hidden rounded-xl border bg-gradient-to-br p-5 transition-all duration-200 hover:scale-[1.01] ${colors[color] || colors.accent}`, children: [icon && _jsx("div", { className: "absolute top-4 right-4 opacity-40", children: icon }), _jsx("div", { className: "text-xs font-medium uppercase tracking-wider text-[var(--color-dim)] mb-2", children: label }), _jsx("div", { className: "text-3xl font-bold tracking-tight", children: value }), trend && (_jsxs("div", { className: `mt-2 text-xs font-medium ${trend.value >= 0 ? 'text-emerald-400' : 'text-rose-400'}`, children: [trend.value >= 0 ? '↑' : '↓', " ", Math.abs(trend.value), "% ", trend.label] })), sparkline && _jsx("div", { className: "mt-3 h-8", children: sparkline })] }));
}
export function Badge({ children, variant = 'default', className = '' }) {
    const styles = {
        default: 'bg-surface3 text-dim',
        green: 'bg-emerald-500/15 text-emerald-400',
        red: 'bg-rose-500/15 text-rose-400',
        amber: 'bg-amber-500/15 text-amber-400',
        blue: 'bg-blue-500/15 text-blue-400',
        purple: 'bg-indigo-500/15 text-indigo-400',
    };
    return _jsx("span", { className: `inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-semibold ${styles[variant]} ${className}`, children: children });
}
export function DataTable({ columns, data, onRowClick, emptyText = 'No data', searchPlaceholder }) {
    const [search, setSearch] = useState('');
    const [sortKey, setSortKey] = useState(null);
    const [sortDir, setSortDir] = useState('asc');
    const [page, setPage] = useState(0);
    const pageSize = 10;
    const filtered = data.filter(row => !search || Object.values(row).some(v => String(v).toLowerCase().includes(search.toLowerCase())));
    const sorted = [...filtered].sort((a, b) => {
        if (!sortKey)
            return 0;
        const av = a[sortKey], bv = b[sortKey];
        const cmp = String(av).localeCompare(String(bv));
        return sortDir === 'asc' ? cmp : -cmp;
    });
    const paged = sorted.slice(page * pageSize, (page + 1) * pageSize);
    const totalPages = Math.ceil(sorted.length / pageSize);
    const toggleSort = (key) => {
        if (sortKey === key)
            setSortDir(d => d === 'asc' ? 'desc' : 'asc');
        else {
            setSortKey(key);
            setSortDir('asc');
        }
        setPage(0);
    };
    return (_jsxs("div", { children: [searchPlaceholder && (_jsx("div", { className: "mb-4", children: _jsx("input", { type: "text", placeholder: searchPlaceholder, value: search, onChange: e => { setSearch(e.target.value); setPage(0); }, className: "w-full max-w-xs rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none transition-colors" }) })), _jsx("div", { className: "overflow-x-auto rounded-xl border border-[var(--color-border)]", children: _jsxs("table", { className: "w-full text-sm", children: [_jsx("thead", { children: _jsx("tr", { className: "border-b border-[var(--color-border)] bg-[var(--color-surface)]", children: columns.map(col => (_jsxs("th", { onClick: () => toggleSort(col.key), className: `cursor-pointer px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors ${col.className || ''}`, children: [col.label, " ", sortKey === col.key && (sortDir === 'asc' ? '↑' : '↓')] }, col.key))) }) }), _jsx("tbody", { children: paged.length === 0 ? (_jsx("tr", { children: _jsx("td", { colSpan: columns.length, className: "px-4 py-12 text-center text-[var(--color-dim)]", children: emptyText }) })) : paged.map((row, i) => (_jsx("tr", { onClick: () => onRowClick?.(row), className: `border-b border-[var(--color-border)] transition-colors hover:bg-[var(--color-surface2)] ${onRowClick ? 'cursor-pointer' : ''}`, children: columns.map(col => (_jsx("td", { className: `px-4 py-3 ${col.className || ''}`, children: col.render ? col.render(row) : String(row[col.key] ?? '') }, col.key))) }, i))) })] }) }), totalPages > 1 && (_jsxs("div", { className: "mt-4 flex items-center justify-between text-sm text-[var(--color-dim)]", children: [_jsxs("span", { children: [filtered.length, " results"] }), _jsxs("div", { className: "flex gap-2", children: [_jsx("button", { onClick: () => setPage(p => Math.max(0, p - 1)), disabled: page === 0, className: "rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs hover:bg-[var(--color-surface2)] disabled:opacity-30 transition-colors", children: "Prev" }), _jsxs("span", { className: "flex items-center px-2", children: [page + 1, " / ", totalPages] }), _jsx("button", { onClick: () => setPage(p => Math.min(totalPages - 1, p + 1)), disabled: page >= totalPages - 1, className: "rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs hover:bg-[var(--color-surface2)] disabled:opacity-30 transition-colors", children: "Next" })] })] }))] }));
}
import { useState } from 'react';
export function Modal({ open, onClose, title, children, actions }) {
    if (!open)
        return null;
    return (_jsxs("div", { className: "fixed inset-0 z-50 flex items-center justify-center", onClick: onClose, children: [_jsx("div", { className: "absolute inset-0 bg-black/60 backdrop-blur-sm" }), _jsxs("div", { className: "relative w-full max-w-lg rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6 shadow-2xl fade-in", onClick: e => e.stopPropagation(), children: [_jsxs("div", { className: "mb-4 flex items-center justify-between", children: [_jsx("h3", { className: "text-lg font-semibold", children: title }), _jsx("button", { onClick: onClose, className: "rounded-lg p-1 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: "\u2715" })] }), _jsx("div", { className: "space-y-4", children: children }), actions && _jsx("div", { className: "mt-6 flex justify-end gap-3", children: actions })] })] }));
}
export function SlidePanel({ open, onClose, title, children, width = 'w-[480px]' }) {
    if (!open)
        return null;
    return (_jsxs("div", { className: "fixed inset-0 z-50", onClick: onClose, children: [_jsx("div", { className: "absolute inset-0 bg-black/40 backdrop-blur-sm" }), _jsxs("div", { className: `absolute right-0 top-0 bottom-0 ${width} overflow-y-auto border-l border-[var(--color-border)] bg-[var(--color-surface)] shadow-2xl slide-in-right`, onClick: e => e.stopPropagation(), children: [_jsxs("div", { className: "sticky top-0 z-10 flex items-center justify-between border-b border-[var(--color-border)] bg-[var(--color-surface)] px-6 py-4", children: [_jsx("h3", { className: "text-lg font-semibold", children: title }), _jsx("button", { onClick: onClose, className: "rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: "\u2715" })] }), _jsx("div", { className: "p-6", children: children })] })] }));
}
export function RiskGauge({ score, size = 100 }) {
    const r = (size - 12) / 2;
    const circ = 2 * Math.PI * r;
    const offset = circ * (1 - score / 100);
    const color = score >= 70 ? 'var(--color-red)' : score >= 40 ? 'var(--color-amber)' : 'var(--color-green)';
    return (_jsxs("div", { className: "inline-flex flex-col items-center gap-1", children: [_jsxs("svg", { width: size, height: size, className: "-rotate-90", children: [_jsx("circle", { cx: size / 2, cy: size / 2, r: r, fill: "none", stroke: "var(--color-surface3)", strokeWidth: "6" }), _jsx("circle", { cx: size / 2, cy: size / 2, r: r, fill: "none", stroke: color, strokeWidth: "6", strokeDasharray: circ, strokeDashoffset: offset, strokeLinecap: "round", className: "transition-all duration-700" })] }), _jsx("span", { className: "text-lg font-bold", style: { color }, children: score })] }));
}
export function TerminalOutput({ text }) {
    const [copied, setCopied] = useState(false);
    const copy = () => { navigator.clipboard.writeText(text); setCopied(true); setTimeout(() => setCopied(false), 2000); };
    return (_jsxs("div", { className: "relative rounded-xl border border-[var(--color-border)] bg-[#0d0e14] p-4 font-mono text-sm", children: [_jsx("button", { onClick: copy, className: "absolute top-2 right-2 rounded-md bg-[var(--color-surface2)] px-2 py-1 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors", children: copied ? '✓ Copied' : 'Copy' }), _jsx("pre", { className: "whitespace-pre-wrap break-all text-[var(--color-text)] max-h-80 overflow-auto", children: text })] }));
}
export function EmptyState({ icon, title, description }) {
    return (_jsxs("div", { className: "flex flex-col items-center justify-center py-16 text-center", children: [_jsx("div", { className: "mb-4 text-4xl opacity-40", children: icon }), _jsx("h3", { className: "text-lg font-semibold text-[var(--color-dim)]", children: title }), description && _jsx("p", { className: "mt-2 max-w-sm text-sm text-[var(--color-dim)] opacity-70", children: description })] }));
}
export function LoadingSkeleton({ lines = 3 }) {
    return (_jsx("div", { className: "space-y-3", children: Array.from({ length: lines }, (_, i) => (_jsx("div", { className: "shimmer h-4 rounded", style: { width: `${60 + Math.random() * 40}%` } }, i))) }));
}
export function Toast({ toasts, onRemove }) {
    const icons = { success: '✓', error: '✕', info: 'ℹ', warning: '⚠' };
    const colors = {
        success: 'border-emerald-500/40 bg-emerald-500/10',
        error: 'border-rose-500/40 bg-rose-500/10',
        info: 'border-blue-500/40 bg-blue-500/10',
        warning: 'border-amber-500/40 bg-amber-500/10',
    };
    return (_jsx("div", { className: "fixed bottom-6 right-6 z-[100] flex flex-col gap-2", children: toasts.map(t => (_jsxs("div", { className: `flex items-center gap-3 rounded-xl border px-4 py-3 shadow-lg fade-in ${colors[t.type]}`, children: [_jsx("span", { className: "text-lg", children: icons[t.type] }), _jsxs("div", { children: [_jsx("div", { className: "text-sm font-medium", children: t.title }), t.message && _jsx("div", { className: "text-xs text-[var(--color-dim)]", children: t.message })] }), _jsx("button", { onClick: () => onRemove(t.id), className: "ml-2 text-[var(--color-dim)] hover:text-[var(--color-text)]", children: "\u2715" })] }, t.id))) }));
}
export function YamlEditor({ value, onChange, readOnly = false }) {
    const [val, setVal] = useState(value);
    const update = (v) => { setVal(v); onChange?.(v); };
    return (_jsx("textarea", { value: val, onChange: e => update(e.target.value), readOnly: readOnly, spellCheck: false, className: "h-96 w-full rounded-xl border border-[var(--color-border)] bg-[#0d0e14] p-4 font-mono text-sm leading-relaxed text-[var(--color-text)] focus:border-[var(--color-accent)] focus:outline-none resize-none" }));
}
