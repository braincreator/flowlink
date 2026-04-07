import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LayoutGrid, Plus, Search, Radio, Monitor, Settings } from 'lucide-react';
import TerminalGrid, { GRID_MAX } from '../components/TerminalGrid';
import TerminalFeed from '../components/TerminalFeedCard';
import TerminalMinimap from '../components/TerminalMinimap';
import TerminalSettingsPanel from '../components/terminal/TerminalSettings';
import { useLiveRecorder } from '../hooks/useLiveRecorder';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
const layoutOptions = [
    { key: '1x1', icon: '⊕', tKey: 'focus_mode' },
    { key: '2x2', icon: '⊞', tKey: 'grid_2x2' },
    { key: '3x2', icon: '⊞⊞', tKey: 'grid_3x2' },
    { key: '3x3', icon: '⊞⊞⊞', tKey: 'grid_3x3' },
];
export default function TerminalSOC() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const { data: agents } = useApi(() => api.getAgents(), { pollMs: 10000 });
    const agentList = (agents || []).filter((a) => a.status === 'online');
    const [layout, setLayout] = useState(() => localStorage.getItem('flowlink_soc_layout') || '2x2');
    const [settingsOpen, setSettingsOpen] = useState(false);
    const [feedIds, setFeedIds] = useState(() => {
        try {
            return JSON.parse(localStorage.getItem('flowlink_soc_feeds') || '[]');
        }
        catch {
            return [];
        }
    });
    const [expandedId, setExpandedId] = useState(null);
    const [showAddModal, setShowAddModal] = useState(false);
    const [search, setSearch] = useState('');
    const feedContainerRef = useRef(null);
    const { startRecording: startFeedRec, stopRecording: stopFeedRec, recording: feedRecActive, duration: feedRecDur } = useLiveRecorder(feedContainerRef);
    useEffect(() => { localStorage.setItem('flowlink_soc_layout', layout); }, [layout]);
    useEffect(() => { localStorage.setItem('flowlink_soc_feeds', JSON.stringify(feedIds)); }, [feedIds]);
    const feeds = useMemo(() => feedIds.map(id => {
        const a = agentList.find(ag => ag.id === id);
        return {
            agentId: id,
            hostname: a?.hostname || 'Unknown',
            status: a ? (a.status === 'online' ? 'online' : 'disconnected') : 'disconnected',
            tags: a?.tags || [],
            os: a?.os || 'linux',
            uptime: a?.uptime || 0,
            commandCount: a?.command_count || 0,
            alertCount: a?.alert_count || 0,
            lastOutput: '',
            connectedAt: a?.connected_at ? new Date(a.connected_at).getTime() : Date.now(),
        };
    }), [feedIds, agentList]);
    const handleAddFeed = useCallback((agentId) => {
        if (feedIds.length >= GRID_MAX[layout])
            return;
        setFeedIds(prev => prev.includes(agentId) ? prev : [...prev, agentId]);
        setShowAddModal(false);
    }, [feedIds, layout]);
    const handleDetach = useCallback((agentId) => {
        setFeedIds(prev => prev.filter(id => id !== agentId));
        if (expandedId === agentId)
            setExpandedId(null);
    }, [expandedId]);
    const filteredAgents = agentList.filter(a => a.hostname.toLowerCase().includes(search.toLowerCase()) ||
        (a.tags || []).some((tag) => tag.toLowerCase().includes(search.toLowerCase())));
    const activeCount = feeds.filter(f => f.status === 'online').length;
    const disconnectedCount = feeds.filter(f => f.status === 'disconnected').length;
    const expandedFeed = feeds.find(f => f.agentId === expandedId);
    return (_jsxs("div", { className: "flex flex-col h-[calc(100vh-7rem)] -m-6 bg-[#060a14]", children: [_jsx(TerminalSettingsPanel, { open: settingsOpen, onClose: () => setSettingsOpen(false) }), _jsxs("div", { className: "flex items-center gap-3 border-b border-white/[0.06] bg-white/[0.02] px-4 py-3", children: [_jsx(Radio, { size: 18, className: "text-indigo-400" }), _jsx("h2", { className: "text-sm font-semibold text-white", children: t('terminal_soc.title') }), _jsx("div", { className: "flex items-center gap-1 ml-4 rounded-lg bg-white/[0.04] p-1", children: layoutOptions.map(opt => (_jsx("button", { onClick: () => setLayout(opt.key), className: `px-2.5 py-1 rounded-md text-xs font-medium transition-all
                ${layout === opt.key
                                ? 'bg-indigo-500/20 text-indigo-300 shadow-sm'
                                : 'text-white/40 hover:text-white/70 hover:bg-white/[0.04]'}`, title: t(`terminal_soc.${opt.tKey}`), children: opt.icon }, opt.key))) }), _jsxs("div", { className: "ml-auto flex items-center gap-2", children: [_jsx("button", { onClick: () => setSettingsOpen(true), className: "p-1.5 rounded-md hover:bg-white/10 transition-colors", title: "Terminal Settings", children: _jsx(Settings, { size: 14, className: "text-white/50" }) }), _jsxs("span", { className: "text-[10px] text-white/30", children: ["\uD83D\uDFE2 ", activeCount, " \u00B7 \uD83D\uDD34 ", disconnectedCount, " \u00B7 \u03A3 ", feeds.length] }), _jsxs("button", { onClick: () => setShowAddModal(true), className: "flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-indigo-500/20 text-indigo-300 text-xs font-medium hover:bg-indigo-500/30 transition-colors", children: [_jsx(Plus, { size: 14 }), t('terminal_soc.add_feed')] })] })] }), _jsx("div", { className: "flex-1 overflow-hidden p-3", children: feeds.length === 0 ? (_jsxs("div", { className: "flex flex-col items-center justify-center h-full gap-4 fade-in", children: [_jsx("div", { className: "w-16 h-16 rounded-2xl bg-white/[0.03] flex items-center justify-center", children: _jsx(LayoutGrid, { size: 28, className: "text-white/20" }) }), _jsxs("div", { className: "text-center", children: [_jsx("p", { className: "text-sm font-medium text-white/40", children: t('terminal_soc.no_feeds') }), _jsx("p", { className: "text-xs text-white/20 mt-1", children: t('terminal_soc.no_feeds_desc') })] }), _jsxs("button", { onClick: () => setShowAddModal(true), className: "flex items-center gap-2 px-4 py-2 rounded-xl bg-indigo-500/20 text-indigo-300 text-sm font-medium hover:bg-indigo-500/30 transition-colors", children: [_jsx(Plus, { size: 16 }), t('terminal_soc.add_to_grid')] })] })) : expandedFeed ? (_jsxs("div", { className: "relative h-full flex flex-col fade-in", children: [_jsxs("div", { className: "flex items-center gap-3 px-4 py-2 bg-white/[0.03] border-b border-white/[0.04]", children: [_jsx(Monitor, { size: 14, className: "text-indigo-400" }), _jsx("span", { className: "text-xs font-semibold text-white", children: expandedFeed.hostname }), _jsx("span", { className: "text-[10px] text-white/30", children: expandedFeed.os }), _jsxs("button", { onClick: () => setExpandedId(null), className: "ml-auto px-2 py-1 rounded text-[10px] text-white/40 hover:text-white hover:bg-white/10 transition-colors", children: ["\u2715 ", t('common.close')] }), feedRecActive ? (_jsxs("button", { onClick: () => stopFeedRec(), className: "flex items-center gap-1.5 rounded-lg bg-rose-500/15 px-2.5 py-1 text-xs text-rose-400", children: [_jsx("span", { className: "h-2 w-2 rounded-full bg-rose-500 animate-pulse" }), Math.floor(feedRecDur / 60), ":", (feedRecDur % 60).toString().padStart(2, '0'), " \u23F9"] })) : (_jsxs("button", { onClick: () => startFeedRec(), className: "px-2 py-1 rounded text-[10px] text-rose-400 hover:bg-rose-500/10 transition-colors", children: ["\u23FA ", t('sessions.record_feed')] }))] }), _jsx("div", { className: "flex-1 min-h-0", ref: feedContainerRef, children: _jsx(TerminalFeed, { agentId: expandedFeed.agentId, interactive: true }) }), _jsx(TerminalMinimap, { feeds: feeds, activeId: expandedId, onClick: setExpandedId })] })) : (_jsx(TerminalGrid, { feeds: feeds, layout: layout, onExpand: setExpandedId, onDetach: handleDetach })) }), showAddModal && (_jsx("div", { className: "absolute inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm fade-in", onClick: () => setShowAddModal(false), children: _jsxs("div", { className: "w-full max-w-md mx-4 rounded-2xl bg-[#0c1120] border border-white/[0.08] shadow-2xl p-5", onClick: e => e.stopPropagation(), children: [_jsxs("div", { className: "flex items-center gap-2 mb-4", children: [_jsx(Search, { size: 16, className: "text-indigo-400" }), _jsx("h3", { className: "text-sm font-semibold text-white", children: t('terminal_soc.search_agents') })] }), _jsx("input", { autoFocus: true, value: search, onChange: e => setSearch(e.target.value), placeholder: t('agents.search_agents'), className: "w-full rounded-lg bg-white/[0.04] border border-white/[0.06] px-3 py-2 text-sm text-white placeholder-white/30 focus:border-indigo-500/50 focus:outline-none mb-3" }), _jsxs("div", { className: "max-h-64 overflow-y-auto space-y-1", children: [filteredAgents.length === 0 && (_jsx("p", { className: "text-xs text-white/30 py-4 text-center", children: t('common.no_data') })), filteredAgents.map(a => {
                                    const inGrid = feedIds.includes(a.id);
                                    return (_jsxs("button", { onClick: () => !inGrid && handleAddFeed(a.id), disabled: inGrid, className: `w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left transition-all
                      ${inGrid ? 'opacity-40 cursor-not-allowed' : 'hover:bg-white/[0.04] cursor-pointer'}`, children: [_jsx("div", { className: "w-2 h-2 rounded-full bg-emerald-400 flex-shrink-0" }), _jsxs("div", { className: "flex-1 min-w-0", children: [_jsx("p", { className: "text-sm font-medium text-white truncate", children: a.hostname }), _jsxs("p", { className: "text-[10px] text-white/30", children: [a.os, " \u00B7 ", (a.tags || []).join(', ')] })] }), inGrid && _jsx("span", { className: "text-[10px] text-white/30", children: "\u2713" })] }, a.id));
                                })] })] }) }))] }));
}
