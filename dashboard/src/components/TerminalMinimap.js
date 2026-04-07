import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useTranslation } from 'react-i18next';
export default function TerminalMinimap({ feeds, activeId, onClick }) {
    const { t } = useTranslation();
    if (feeds.length === 0)
        return null;
    return (_jsxs("div", { className: "absolute bottom-4 right-4 flex flex-col gap-1.5 p-2 rounded-xl bg-[#0a0e1a]/90 backdrop-blur-md border border-white/[0.06] shadow-xl", children: [_jsx("span", { className: "text-[9px] font-medium text-white/30 uppercase tracking-wider px-1 mb-0.5", children: t('terminal_soc.other_feeds') }), feeds
                .filter(f => f.agentId !== activeId)
                .slice(0, 6)
                .map(feed => (_jsxs("button", { onClick: () => onClick(feed.agentId), className: `
              flex items-center gap-1.5 px-2 py-1 rounded-lg text-left transition-all
              ${feed.agentId === activeId
                    ? 'bg-indigo-500/20 border border-indigo-500/30'
                    : 'hover:bg-white/5 border border-transparent'}
            `, children: [_jsx("div", { className: "w-1.5 h-1.5 rounded-full flex-shrink-0", style: { backgroundColor: feed.status === 'online' ? '#34d399' : feed.status === 'idle' ? '#fbbf24' : '#f43f5e' } }), _jsx("span", { className: "text-[10px] text-white/50 truncate max-w-[60px]", children: feed.hostname })] }, feed.agentId)))] }));
}
