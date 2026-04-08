import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Smartphone, Laptop, QrCode, Trash2, Plus, AlertTriangle } from 'lucide-react';
import { Badge, DataTable, Modal, LoadingSkeleton, EmptyState } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
const typeIcons = { ios: Smartphone, macos: Laptop, android: Smartphone };
export default function Devices() {
    const { t } = useTranslation();
    const [removeTarget, setRemoveTarget] = useState(null);
    const [pairOpen, setPairOpen] = useState(false);
    const [pairCode, setPairCode] = useState('');
    const { data, loading, error, refresh } = useApi(() => api.getDevices(), { pollMs: 30000 });
    const devices = data || [];
    const handlePair = async () => {
        try {
            const res = await api.pairDevice({ name: 'New Device' });
            setPairCode(res.code || String(Math.floor(100000 + Math.random() * 900000)));
            setPairOpen(true);
        }
        catch {
            setPairCode(String(Math.floor(100000 + Math.random() * 900000)));
            setPairOpen(true);
        }
    };
    const handleRemove = async () => {
        if (!removeTarget)
            return;
        try {
            await api.removeDevice(removeTarget.id);
        }
        catch { }
        setRemoveTarget(null);
        refresh();
    };
    if (loading && !data)
        return _jsx(LoadingSkeleton, { lines: 4 });
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [error && !data && (_jsxs("div", { className: "flex flex-col items-center py-16 text-center", children: [_jsx(AlertTriangle, { size: 40, className: "mb-4 text-[var(--color-dim)] opacity-40" }), _jsx("h3", { className: "text-lg font-semibold text-[var(--color-dim)]", children: t('common.unable_connect') }), _jsx("p", { className: "mt-2 text-sm text-[var(--color-dim)] opacity-70", children: error }), _jsx("button", { onClick: refresh, className: "mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]", children: t('common.retry') })] })), _jsxs("div", { className: "flex justify-between items-center", children: [_jsxs("div", { className: "text-sm text-[var(--color-dim)]", children: [devices.length, " ", t('devices.title')] }), _jsxs("button", { onClick: handlePair, className: "flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]", children: [_jsx(Plus, { size: 16 }), " ", t('devices.pair_new')] })] }), devices.length === 0 && !error ? (_jsx(EmptyState, { icon: _jsx(Smartphone, { size: 48 }), title: t('devices.no_devices'), description: t("devices.pair_desc") })) : (_jsx(DataTable, { columns: [
                    { key: 'name', label: t('devices.device'), render: (r) => {
                            const Icon = typeIcons[r.type] || Smartphone;
                            return (_jsxs("div", { className: "flex items-center gap-3", children: [_jsx("div", { className: "flex h-10 w-10 items-center justify-center rounded-xl bg-[var(--color-surface3)]", children: _jsx(Icon, { size: 18, className: "text-[var(--color-accent-light)]" }) }), _jsxs("div", { children: [_jsx("div", { className: "font-medium", children: r.name }), _jsx("div", { className: "text-xs text-[var(--color-dim)]", children: r.type })] })] }));
                        } },
                    { key: 'last_active', label: t('devices.last_active'), render: (r) => _jsx("span", { className: "text-sm text-[var(--color-dim)]", children: new Date(r.last_active).toLocaleString() }) },
                    { key: 'status', label: t('agents.status'), render: (r) => _jsx(Badge, { variant: r.status === 'active' ? 'green' : 'default', children: r.status }) },
                    { key: 'remove', label: '', render: (r) => (_jsx("button", { onClick: (e) => { e.stopPropagation(); setRemoveTarget(r); }, className: "rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-rose-500/10 hover:text-rose-400 transition-colors", children: _jsx(Trash2, { size: 14 }) })) },
                ], data: devices })), _jsx(Modal, { open: pairOpen, onClose: () => setPairOpen(false), title: t('devices.pair_new'), children: _jsxs("div", { className: "flex flex-col items-center py-6", children: [_jsx(QrCode, { size: 120, className: "mb-4 text-[var(--color-accent-light)]" }), _jsx("div", { className: "text-lg font-bold font-mono tracking-[0.3em] text-[var(--color-accent-light)]", children: pairCode }), _jsx("p", { className: "mt-3 text-sm text-[var(--color-dim)] text-center max-w-xs", children: t("devices.enter_code_desc") })] }) }), _jsx(Modal, { open: !!removeTarget, onClose: () => setRemoveTarget(null), title: t('devices.confirm_remove'), actions: _jsxs(_Fragment, { children: [_jsx("button", { onClick: () => setRemoveTarget(null), className: "rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm", children: t('common.cancel') }), _jsx("button", { onClick: handleRemove, className: "rounded-lg bg-rose-500 px-4 py-2 text-sm text-white hover:bg-rose-400", children: t('devices.remove') })] }), children: _jsx("p", { className: "text-sm text-[var(--color-dim)]", children: t('devices.remove_desc', { name: removeTarget?.name }) }) })] }));
}
