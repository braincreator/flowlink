import { jsxs as _jsxs, jsx as _jsx, Fragment as _Fragment } from "react/jsx-runtime";
import { useState } from 'react';
import { Smartphone, Laptop, QrCode, Trash2, Plus } from 'lucide-react';
import { Badge, DataTable, Modal } from '../components/Layout';
import { mockDevices } from '../api/client';
const typeIcons = { ios: Smartphone, macos: Laptop, android: Smartphone };
export default function Devices() {
    const [devices] = useState(mockDevices);
    const [pairOpen, setPairOpen] = useState(false);
    const [removeTarget, setRemoveTarget] = useState(null);
    const [pairCode] = useState(String(Math.floor(100000 + Math.random() * 900000)));
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [_jsxs("div", { className: "flex justify-between items-center", children: [_jsxs("div", { className: "text-sm text-[var(--color-dim)]", children: [devices.length, " paired devices"] }), _jsxs("button", { onClick: () => setPairOpen(true), className: "flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]", children: [_jsx(Plus, { size: 16 }), " Pair Device"] })] }), _jsx(DataTable, { columns: [
                    { key: 'name', label: 'Device', render: (r) => {
                            const Icon = typeIcons[r.type] || Smartphone;
                            return (_jsxs("div", { className: "flex items-center gap-3", children: [_jsx("div", { className: "flex h-10 w-10 items-center justify-center rounded-xl bg-[var(--color-surface3)]", children: _jsx(Icon, { size: 18, className: "text-[var(--color-accent-light)]" }) }), _jsxs("div", { children: [_jsx("div", { className: "font-medium", children: r.name }), _jsx("div", { className: "text-xs text-[var(--color-dim)]", children: r.type })] })] }));
                        } },
                    { key: 'last_active', label: 'Last Active', render: (r) => _jsx("span", { className: "text-sm text-[var(--color-dim)]", children: new Date(r.last_active).toLocaleString() }) },
                    { key: 'status', label: 'Status', render: (r) => _jsx(Badge, { variant: r.status === 'active' ? 'green' : 'default', children: r.status }) },
                    { key: 'remove', label: '', render: (r) => (_jsx("button", { onClick: (e) => { e.stopPropagation(); setRemoveTarget(r); }, className: "rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-rose-500/10 hover:text-rose-400 transition-colors", children: _jsx(Trash2, { size: 14 }) })) },
                ], data: devices }), _jsx(Modal, { open: pairOpen, onClose: () => setPairOpen(false), title: "Pair New Device", children: _jsxs("div", { className: "flex flex-col items-center py-6", children: [_jsx(QrCode, { size: 120, className: "mb-4 text-[var(--color-accent-light)]" }), _jsx("div", { className: "text-lg font-bold font-mono tracking-[0.3em] text-[var(--color-accent-light)]", children: pairCode }), _jsx("p", { className: "mt-3 text-sm text-[var(--color-dim)] text-center max-w-xs", children: "Enter this code on your device or scan the QR code to pair." })] }) }), _jsx(Modal, { open: !!removeTarget, onClose: () => setRemoveTarget(null), title: "Remove Device", actions: _jsxs(_Fragment, { children: [_jsx("button", { onClick: () => setRemoveTarget(null), className: "rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm", children: "Cancel" }), _jsx("button", { onClick: () => setRemoveTarget(null), className: "rounded-lg bg-rose-500 px-4 py-2 text-sm text-white hover:bg-rose-400", children: "Remove" })] }), children: _jsxs("p", { className: "text-sm text-[var(--color-dim)]", children: ["Remove ", _jsx("span", { className: "font-medium text-[var(--color-text)]", children: removeTarget?.name }), "? The device will need to be re-paired to reconnect."] }) })] }));
}
