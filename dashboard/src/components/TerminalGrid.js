import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Maximize2, X, Clock, TerminalSquare } from 'lucide-react';
import TerminalFeed from './TerminalFeedCard';
function FeedCard({ feed, index, onExpand, onDetach, onDragStart, onDrop }) {
    const { t } = useTranslation();
    const [dragOver, setDragOver] = useState(false);
    const statusColor = feed.status === 'online' ? '#34d399' : feed.status === 'idle' ? '#fbbf24' : '#f43f5e';
    const duration = feed.connectedAt ? Math.floor((Date.now() - feed.connectedAt) / 60000) : 0;
    const durationStr = duration >= 60 ? `${Math.floor(duration / 60)}h ${duration % 60}m` : `${duration}m`;
    const handleMount = useCallback((term) => {
        if (feed.lastOutput)
            term.write(feed.lastOutput);
    }, [feed.lastOutput]);
    return (_jsxs("div", { draggable: true, onDragStart: (e) => { e.dataTransfer.effectAllowed = 'move'; onDragStart(index); }, onDragOver: (e) => { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; setDragOver(true); }, onDrop: (e) => { e.preventDefault(); setDragOver(false); onDrop(index); }, onDragLeave: () => setDragOver(false), className: `
        relative flex flex-col rounded-xl overflow-hidden border transition-all duration-300
        ${dragOver ? 'border-indigo-500/60 scale-[1.02] z-10' : ''}
        ${feed.status === 'disconnected' ? 'border-rose-500/30' : 'border-white/[0.06]'}
        ${feed.alertCount > 0 ? 'animate-alert-flash' : ''}
        bg-[#060a14]/80 backdrop-blur-sm
      `, style: {
            boxShadow: '0 2px 12px rgba(0, 0, 0, 0.4)',
            animation: 'feedFadeIn 0.3s ease-out',
        }, "data-feed-id": feed.agentId, children: [_jsxs("div", { className: "flex items-center gap-2 px-3 py-2 bg-white/[0.03] border-b border-white/[0.04]", children: [_jsxs("div", { className: "relative", children: [_jsx("div", { className: "w-2 h-2 rounded-full", style: { backgroundColor: statusColor } }), feed.status === 'online' && (_jsx("div", { className: "absolute inset-0 w-2 h-2 rounded-full animate-ping", style: { backgroundColor: statusColor, opacity: 0.5 } }))] }), _jsx("span", { className: "text-xs font-semibold text-white truncate", children: feed.hostname }), feed.tags[0] && (_jsx("span", { className: "px-1.5 py-0.5 text-[10px] font-medium rounded bg-indigo-500/20 text-indigo-300", children: feed.tags[0] })), _jsxs("div", { className: "ml-auto flex items-center gap-1", children: [_jsx("button", { onClick: () => onExpand(feed.agentId), className: "p-1 rounded text-white/40 hover:text-white hover:bg-white/10 transition-colors", title: t('terminal_soc.expand'), children: _jsx(Maximize2, { size: 13 }) }), _jsx("button", { onClick: () => onDetach(feed.agentId), className: "p-1 rounded text-white/40 hover:text-rose-400 hover:bg-white/10 transition-colors", title: t('terminal_soc.detach'), children: _jsx(X, { size: 13 }) })] })] }), _jsxs("div", { className: "flex-1 min-h-0 relative", children: [_jsx(TerminalFeed, { agentId: feed.agentId, interactive: false, onMount: handleMount }), feed.status === 'disconnected' && (_jsxs("div", { className: "absolute inset-0 flex flex-col items-center justify-center bg-[#060a14]/80 backdrop-blur-sm fade-in", children: [_jsx("div", { className: "text-rose-400 text-2xl mb-2", children: "\u2298" }), _jsx("span", { className: "text-xs font-medium text-rose-400", children: t('terminal_soc.disconnected') })] }))] }), _jsxs("div", { className: "flex items-center gap-3 px-3 py-1.5 bg-white/[0.02] border-t border-white/[0.04] text-[10px] text-white/30", children: [_jsxs("div", { className: "flex items-center gap-1", children: [_jsx(Clock, { size: 10 }), _jsx("span", { children: durationStr })] }), _jsxs("div", { className: "flex items-center gap-1", children: [_jsx(TerminalSquare, { size: 10 }), _jsxs("span", { children: [feed.commandCount, " cmds"] })] }), feed.alertCount > 0 && (_jsxs("span", { className: "ml-auto px-1.5 py-0.5 rounded-full bg-rose-500/20 text-rose-400 font-medium animate-pulse", children: [feed.alertCount, " \u26A1"] }))] }), _jsx("style", { children: `
        @keyframes feedFadeIn { from { opacity: 0; transform: scale(0.95); } to { opacity: 1; transform: scale(1); } }
        @keyframes alertFlash { 0%, 100% { box-shadow: 0 0 0 rgba(244, 63, 94, 0); } 50% { box-shadow: 0 0 16px rgba(244, 63, 94, 0.4); } }
        .animate-alert-flash { animation: alertFlash 1.5s ease-in-out 3; }
      ` })] }));
}
export const GRID_COLS = { '1x1': 1, '2x2': 2, '3x2': 3, '3x3': 3 };
export const GRID_MAX = { '1x1': 1, '2x2': 4, '3x2': 6, '3x3': 9 };
export default function TerminalGrid({ feeds, layout, onExpand, onDetach }) {
    const [dragIndex, setDragIndex] = useState(null);
    const [order, setOrder] = useState(feeds.map((_, i) => i));
    useEffect(() => { setOrder(feeds.map((_, i) => i)); }, [feeds.length]);
    const handleDrop = useCallback((targetIndex) => {
        if (dragIndex === null || dragIndex === targetIndex)
            return;
        setOrder(prev => {
            const next = [...prev];
            [next[dragIndex], next[targetIndex]] = [next[targetIndex], next[dragIndex]];
            return next;
        });
        setDragIndex(null);
    }, [dragIndex]);
    const orderedFeeds = order.map(i => feeds[i]).filter(Boolean);
    return (_jsx("div", { className: "grid gap-3 h-full", style: { gridTemplateColumns: `repeat(${GRID_COLS[layout]}, 1fr)` }, children: orderedFeeds.map((feed, i) => (_jsx(FeedCard, { feed: feed, index: i, onExpand: onExpand, onDetach: onDetach, onDragStart: setDragIndex, onDrop: handleDrop }, feed.agentId))) }));
}
