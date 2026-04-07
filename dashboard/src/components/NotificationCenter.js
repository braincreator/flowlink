import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Bell, CheckCheck, Trash2, AlertTriangle, Bot, Shield, AlertCircle, Info } from 'lucide-react';
import { useNotifications } from '../hooks/useNotifications';
import { useSSE } from '../hooks/useApi';
const typeIcons = {
    alert: AlertTriangle,
    approval: Shield,
    agent_online: Bot,
    agent_offline: Bot,
    error: AlertCircle,
    info: Info,
};
const typeColors = {
    alert: 'text-amber-400',
    approval: 'text-indigo-400',
    agent_online: 'text-emerald-400',
    agent_offline: 'text-rose-400',
    error: 'text-rose-400',
    info: 'text-blue-400',
};
export default function NotificationCenter() {
    const { t } = useTranslation();
    const { notifications, unread, addNotification, markAllRead, markRead, clearAll } = useNotifications();
    const [open, setOpen] = useState(false);
    const dropdownRef = useRef(null);
    const navigate = useNavigate();
    const { events: sseEvents } = useSSE();
    // Convert SSE events to notifications
    useEffect(() => {
        for (const ev of sseEvents.slice(0, 5)) {
            const eventType = ev.event_type || ev.type || '';
            let type = 'info';
            if (eventType === 'command_intercepted' || eventType === 'policy_violation')
                type = 'alert';
            else if (eventType === 'canary_triggered')
                type = 'error';
            else if (eventType === 'agent_heartbeat') {
                type = ev.status === 'online' ? 'agent_online' : 'agent_offline';
            }
            addNotification({
                type,
                title: eventType.replace(/_/g, ' '),
                body: ev.command || ev.message || '',
                link: eventType.includes('command') ? '/shield' : eventType.includes('agent') ? '/agents' : undefined,
                risk_score: ev.risk_score,
                level: ev.threat_level,
                agent: ev.hostname || ev.agent_id,
            });
        }
    }, [sseEvents]);
    // Close dropdown on click outside
    useEffect(() => {
        const handler = (e) => {
            if (dropdownRef.current && !dropdownRef.current.contains(e.target))
                setOpen(false);
        };
        document.addEventListener('mousedown', handler);
        return () => document.removeEventListener('mousedown', handler);
    }, []);
    const handleClick = (n) => {
        markRead(n.id);
        setOpen(false);
        if (n.link)
            navigate(n.link);
    };
    const recent = notifications.slice(0, 20);
    return (_jsxs("div", { className: "relative", ref: dropdownRef, children: [_jsxs("button", { onClick: () => setOpen(!open), className: "relative flex h-9 w-9 items-center justify-center rounded-xl border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: [_jsx(Bell, { size: 16 }), unread > 0 && (_jsx("span", { className: "absolute -top-1 -right-1 flex h-5 min-w-[20px] items-center justify-center rounded-full bg-rose-500 px-1 text-[10px] font-bold text-white", children: unread > 99 ? '99+' : unread }))] }), open && (_jsxs("div", { className: "absolute right-0 top-full mt-2 w-80 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] shadow-2xl z-50 fade-in", children: [_jsxs("div", { className: "flex items-center justify-between border-b border-[var(--color-border)] px-4 py-3", children: [_jsx("h3", { className: "text-sm font-semibold", children: t('settings.notifications') }), _jsxs("div", { className: "flex items-center gap-1", children: [unread > 0 && (_jsx("button", { onClick: markAllRead, className: "rounded-md p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-emerald-400 transition-colors", title: t('common.mark_all_read'), children: _jsx(CheckCheck, { size: 14 }) })), _jsx("button", { onClick: clearAll, className: "rounded-md p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-rose-400 transition-colors", title: t('common.clear_all'), children: _jsx(Trash2, { size: 14 }) })] })] }), _jsx("div", { className: "max-h-96 overflow-y-auto", children: recent.length === 0 ? (_jsxs("div", { className: "flex flex-col items-center py-8 text-center", children: [_jsx(Bell, { size: 24, className: "mb-2 text-[var(--color-dim)] opacity-40" }), _jsx("p", { className: "text-sm text-[var(--color-dim)]", children: t('common.no_notifications') })] })) : recent.map(n => {
                            const Icon = typeIcons[n.type] || Info;
                            const color = typeColors[n.type] || 'text-[var(--color-dim)]';
                            const timeAgo = getTimeAgo(n.timestamp);
                            return (_jsx("button", { onClick: () => handleClick(n), className: `w-full text-left px-4 py-3 border-b border-[var(--color-border)] hover:bg-[var(--color-surface2)] transition-colors ${!n.read ? 'bg-[var(--color-accent)]/5' : ''}`, children: _jsxs("div", { className: "flex items-start gap-3", children: [_jsx(Icon, { size: 14, className: `mt-0.5 flex-shrink-0 ${color}` }), _jsxs("div", { className: "min-w-0 flex-1", children: [_jsxs("div", { className: "flex items-center gap-2", children: [_jsx("span", { className: "text-sm font-medium truncate", children: n.title }), !n.read && _jsx("span", { className: "h-2 w-2 flex-shrink-0 rounded-full bg-[var(--color-accent)]" })] }), _jsx("p", { className: "text-xs text-[var(--color-dim)] truncate mt-0.5", children: n.body }), _jsxs("div", { className: "flex items-center gap-2 mt-1", children: [_jsx("span", { className: "text-[10px] text-[var(--color-dim)] opacity-60", children: timeAgo }), n.risk_score && (_jsxs("span", { className: `text-[10px] font-mono ${n.risk_score >= 70 ? 'text-rose-400' : 'text-amber-400'}`, children: ["risk:", n.risk_score] }))] })] })] }) }, n.id));
                        }) })] }))] }));
}
function getTimeAgo(ts) {
    const diff = Date.now() - ts;
    if (diff < 60000)
        return 'just now';
    if (diff < 3600000)
        return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000)
        return `${Math.floor(diff / 3600000)}h ago`;
    return `${Math.floor(diff / 86400000)}d ago`;
}
