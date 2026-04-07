import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useTranslation } from 'react-i18next';
import { Brain, RefreshCw, Globe, Server } from 'lucide-react';
import { StatCard, Badge, LoadingSkeleton, EmptyState, DataTable } from '../components/Layout';
export default function LLM() {
    const { t } = useTranslation();
    const loading = false;
    const backends = [];
    const healthy = true;
    if (loading) {
        return _jsx(LoadingSkeleton, { lines: 6 });
    }
    return (_jsxs("div", { className: "space-y-6", children: [_jsxs("div", { className: "grid grid-cols-1 gap-4 sm:grid-cols-3", children: [_jsx(StatCard, { label: t("common.backends"), value: "0", color: "accent", icon: _jsx(Brain, { size: 24 }) }), _jsx(StatCard, { label: t('metrics.health'), value: healthy ? 'OK' : 'Degraded', color: healthy ? 'green' : 'red', icon: _jsx(Server, { size: 24 }) }), _jsx(StatCard, { label: t("common.models"), value: "\u2014", color: "blue", icon: _jsx(Globe, { size: 24 }) })] }), _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]", children: [_jsxs("div", { className: "flex items-center justify-between border-b border-[var(--color-border)] px-6 py-4", children: [_jsx("h3", { className: "font-semibold", children: `${t("common.backends")} LLM` }), _jsxs("button", { className: "flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs text-[var(--color-dim)] hover:bg-[var(--color-surface2)] transition-colors", children: [_jsx(RefreshCw, { size: 12 }), " ", t('common.refresh')] })] }), backends.length === 0 ? (_jsx(EmptyState, { icon: _jsx(Brain, { size: 40 }), title: t("common.no_backends"), description: t("common.no_backends_desc") })) : (_jsx(DataTable, { columns: [
                            { key: 'name', label: 'Name' },
                            { key: 'url', label: 'URL' },
                            { key: 'model', label: 'Model' },
                            { key: 'status', label: 'Status', render: (row) => (_jsx(Badge, { variant: row.status === 'healthy' ? 'green' : 'red', children: row.status })) },
                        ], data: backends, searchPlaceholder: t("common.search_backends") }))] })] }));
}
