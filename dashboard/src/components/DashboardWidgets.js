import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useEffect, useCallback } from 'react';
import { GripVertical } from 'lucide-react';
const STORAGE_KEY = 'flowlink_widget_layout';
function getStoredOrder() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        return raw ? JSON.parse(raw) : null;
    }
    catch {
        return null;
    }
}
function saveOrder(order) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(order));
}
export function DashboardWidgets({ widgets }) {
    const [customizing, setCustomizing] = useState(false);
    const [order, setOrder] = useState(() => {
        const stored = getStoredOrder();
        if (stored) {
            // Merge with any new widgets not in stored order
            const ids = widgets.map(w => w.id);
            const merged = [...stored];
            ids.forEach(id => { if (!merged.includes(id))
                merged.push(id); });
            return merged.filter(id => ids.includes(id));
        }
        return widgets.map(w => w.id);
    });
    const [dragId, setDragId] = useState(null);
    const [overId, setOverId] = useState(null);
    const widgetMap = Object.fromEntries(widgets.map(w => [w.id, w]));
    const resetLayout = useCallback(() => {
        const defaultOrder = widgets.map(w => w.id);
        setOrder(defaultOrder);
        localStorage.removeItem(STORAGE_KEY);
    }, [widgets]);
    useEffect(() => {
        if (!customizing)
            return;
        const handler = () => {
            setDragId(null);
            setOverId(null);
            saveOrder(order);
        };
        window.addEventListener('dragend', handler);
        return () => window.removeEventListener('dragend', handler);
    }, [customizing, order]);
    const handleDragStart = (e, id) => {
        setDragId(id);
        e.dataTransfer.effectAllowed = 'move';
        e.dataTransfer.setData('text/plain', id);
    };
    const handleDragOver = (e, id) => {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'move';
        if (dragId && dragId !== id)
            setOverId(id);
    };
    const handleDrop = (e, targetId) => {
        e.preventDefault();
        if (!dragId || dragId === targetId)
            return;
        const newOrder = [...order];
        const fromIdx = newOrder.indexOf(dragId);
        const toIdx = newOrder.indexOf(targetId);
        newOrder.splice(fromIdx, 1);
        newOrder.splice(toIdx, 0, dragId);
        setOrder(newOrder);
        setDragId(null);
        setOverId(null);
        saveOrder(newOrder);
    };
    return (_jsxs("div", { children: [_jsxs("div", { className: "flex items-center gap-3 mb-4", children: [_jsxs("button", { onClick: () => { if (customizing)
                            saveOrder(order); setCustomizing(!customizing); }, className: `flex items-center gap-2 rounded-xl px-4 py-2 text-sm font-medium transition-all ${customizing
                            ? 'bg-[var(--color-accent)] text-white'
                            : 'border border-[var(--color-border)] hover:bg-[var(--color-surface2)]'}`, children: [_jsx(GripVertical, { size: 16 }), customizing ? 'Done' : 'Customize'] }), customizing && (_jsx("button", { onClick: resetLayout, className: "text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors", children: "Reset Layout" }))] }), _jsx("div", { className: "grid gap-4", style: { gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))' }, children: order.map(id => {
                    const widget = widgetMap[id];
                    if (!widget)
                        return null;
                    const isDragging = dragId === id;
                    const isOver = overId === id;
                    return (_jsxs("div", { draggable: customizing, onDragStart: e => handleDragStart(e, id), onDragOver: e => handleDragOver(e, id), onDrop: e => handleDrop(e, id), onDragEnd: () => { setDragId(null); setOverId(null); }, className: `rounded-xl border transition-all duration-200 ${widget.colSpan === 2 ? 'col-span-2' : ''} ${customizing
                            ? `cursor-grab active:cursor-grabbing ${isDragging ? 'opacity-40 scale-95' : isOver ? 'border-[var(--color-accent)] ring-2 ring-[var(--color-accent)]/30' : 'border-[var(--color-border)] bg-[var(--color-surface)]'}`
                            : 'border-[var(--color-border)] bg-[var(--color-surface)]'}`, style: widget.colSpan === 2 ? { gridColumn: 'span 2' } : undefined, children: [customizing && (_jsxs("div", { className: "flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-2.5", children: [_jsx(GripVertical, { size: 14, className: "text-[var(--color-dim)]" }), _jsx("span", { className: "text-xs font-medium text-[var(--color-dim)]", children: widget.title })] })), _jsx("div", { className: customizing ? 'opacity-70 pointer-events-none p-5' : '', children: widget.render() })] }, id));
                }) })] }));
}
