import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LayoutGrid, Plus, Search, Radio, Monitor, Settings } from 'lucide-react';
import TerminalGrid, { type GridLayout, GRID_MAX } from '../components/TerminalGrid';
import TerminalFeed from '../components/TerminalFeedCard';
import TerminalMinimap from '../components/TerminalMinimap';
import TerminalSettingsPanel from '../components/terminal/TerminalSettings';
import { useLiveRecorder } from '../hooks/useLiveRecorder';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { FeedState } from '../hooks/useTerminalStream';

const layoutOptions: { key: GridLayout; icon: string; tKey: string }[] = [
  { key: '1x1', icon: '⊕', tKey: 'focus_mode' },
  { key: '2x2', icon: '⊞', tKey: 'grid_2x2' },
  { key: '3x2', icon: '⊞⊞', tKey: 'grid_3x2' },
  { key: '3x3', icon: '⊞⊞⊞', tKey: 'grid_3x3' },
];

export default function TerminalSOC() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data: agents } = useApi(() => api.getAgents(), { pollMs: 10000 });
  const agentList = ((agents || []) as any[]).filter((a: any) => a.status === 'online');

  const [layout, setLayout] = useState<GridLayout>(() =>
    (localStorage.getItem('flowlink_soc_layout') as GridLayout) || '2x2'
  );
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [feedIds, setFeedIds] = useState<string[]>(() => {
    try { return JSON.parse(localStorage.getItem('flowlink_soc_feeds') || '[]'); } catch { return []; }
  });
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [showAddModal, setShowAddModal] = useState(false);
  const [search, setSearch] = useState('');
  const feedContainerRef = useRef<HTMLDivElement>(null);
  const { startRecording: startFeedRec, stopRecording: stopFeedRec, recording: feedRecActive, duration: feedRecDur } = useLiveRecorder(feedContainerRef);

  useEffect(() => { localStorage.setItem('flowlink_soc_layout', layout); }, [layout]);
  useEffect(() => { localStorage.setItem('flowlink_soc_feeds', JSON.stringify(feedIds)); }, [feedIds]);

  const feeds: FeedState[] = useMemo(() =>
    feedIds.map(id => {
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
    }),
    [feedIds, agentList],
  );

  const handleAddFeed = useCallback((agentId: string) => {
    if (feedIds.length >= GRID_MAX[layout]) return;
    setFeedIds(prev => prev.includes(agentId) ? prev : [...prev, agentId]);
    setShowAddModal(false);
  }, [feedIds, layout]);

  const handleDetach = useCallback((agentId: string) => {
    setFeedIds(prev => prev.filter(id => id !== agentId));
    if (expandedId === agentId) setExpandedId(null);
  }, [expandedId]);

  const filteredAgents = agentList.filter(a =>
    a.hostname.toLowerCase().includes(search.toLowerCase()) ||
    (a.tags || []).some((tag: string) => tag.toLowerCase().includes(search.toLowerCase()))
  );

  const activeCount = feeds.filter(f => f.status === 'online').length;
  const disconnectedCount = feeds.filter(f => f.status === 'disconnected').length;
  const expandedFeed = feeds.find(f => f.agentId === expandedId);

  return (
    <div className="flex flex-col h-[calc(100vh-7rem)] -m-6 bg-[#060a14]">
      <TerminalSettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
      {/* Top bar */}
      <div className="flex items-center gap-3 border-b border-white/[0.06] bg-white/[0.02] px-4 py-3">
        <Radio size={18} className="text-indigo-400" />
        <h2 className="text-sm font-semibold text-white">{t('terminal_soc.title')}</h2>

        <div className="flex items-center gap-1 ml-4 rounded-lg bg-white/[0.04] p-1">
          {layoutOptions.map(opt => (
            <button
              key={opt.key}
              onClick={() => setLayout(opt.key)}
              className={`px-2.5 py-1 rounded-md text-xs font-medium transition-all
                ${layout === opt.key
                  ? 'bg-indigo-500/20 text-indigo-300 shadow-sm'
                  : 'text-white/40 hover:text-white/70 hover:bg-white/[0.04]'
                }`}
              title={t(`terminal_soc.${opt.tKey}`)}
            >
              {opt.icon}
            </button>
          ))}
        </div>

        <div className="ml-auto flex items-center gap-2">
          <button onClick={() => setSettingsOpen(true)} className="p-1.5 rounded-md hover:bg-white/10 transition-colors" title="Terminal Settings">
            <Settings size={14} className="text-white/50" />
          </button>
          <span className="text-[10px] text-white/30">
            🟢 {activeCount} · 🔴 {disconnectedCount} · Σ {feeds.length}
          </span>
          <button
            onClick={() => setShowAddModal(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-indigo-500/20 text-indigo-300 text-xs font-medium hover:bg-indigo-500/30 transition-colors"
          >
            <Plus size={14} />
            {t('terminal_soc.add_feed')}
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden p-3">
        {feeds.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-4 fade-in">
            <div className="w-16 h-16 rounded-2xl bg-white/[0.03] flex items-center justify-center">
              <LayoutGrid size={28} className="text-white/20" />
            </div>
            <div className="text-center">
              <p className="text-sm font-medium text-white/40">{t('terminal_soc.no_feeds')}</p>
              <p className="text-xs text-white/20 mt-1">{t('terminal_soc.no_feeds_desc')}</p>
            </div>
            <button
              onClick={() => setShowAddModal(true)}
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-indigo-500/20 text-indigo-300 text-sm font-medium hover:bg-indigo-500/30 transition-colors"
            >
              <Plus size={16} />
              {t('terminal_soc.add_to_grid')}
            </button>
          </div>
        ) : expandedFeed ? (
          <div className="relative h-full flex flex-col fade-in">
            <div className="flex items-center gap-3 px-4 py-2 bg-white/[0.03] border-b border-white/[0.04]">
              <Monitor size={14} className="text-indigo-400" />
              <span className="text-xs font-semibold text-white">{expandedFeed.hostname}</span>
              <span className="text-[10px] text-white/30">{expandedFeed.os}</span>
              <button
                onClick={() => setExpandedId(null)}
                className="ml-auto px-2 py-1 rounded text-[10px] text-white/40 hover:text-white hover:bg-white/10 transition-colors"
              >
                ✕ {t('common.close')}
              </button>
              {feedRecActive ? (
                <button onClick={() => stopFeedRec()} className="flex items-center gap-1.5 rounded-lg bg-rose-500/15 px-2.5 py-1 text-xs text-rose-400">
                  <span className="h-2 w-2 rounded-full bg-rose-500 animate-pulse" />
                  {Math.floor(feedRecDur / 60)}:{(feedRecDur % 60).toString().padStart(2, '0')} ⏹
                </button>
              ) : (
                <button onClick={() => startFeedRec()} className="px-2 py-1 rounded text-[10px] text-rose-400 hover:bg-rose-500/10 transition-colors">
                  ⏺ {t('sessions.record_feed')}
                </button>
              )}
            </div>
            <div className="flex-1 min-h-0" ref={feedContainerRef}>
              <TerminalFeed agentId={expandedFeed.agentId} interactive />
            </div>
            <TerminalMinimap feeds={feeds} activeId={expandedId} onClick={setExpandedId} />
          </div>
        ) : (
          <TerminalGrid feeds={feeds} layout={layout} onExpand={setExpandedId} onDetach={handleDetach} />
        )}
      </div>

      {/* Add Modal */}
      {showAddModal && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm fade-in" onClick={() => setShowAddModal(false)}>
          <div className="w-full max-w-md mx-4 rounded-2xl bg-[#0c1120] border border-white/[0.08] shadow-2xl p-5" onClick={e => e.stopPropagation()}>
            <div className="flex items-center gap-2 mb-4">
              <Search size={16} className="text-indigo-400" />
              <h3 className="text-sm font-semibold text-white">{t('terminal_soc.search_agents')}</h3>
            </div>
            <input
              autoFocus
              value={search}
              onChange={e => setSearch(e.target.value)}
              placeholder={t('agents.search_agents')}
              className="w-full rounded-lg bg-white/[0.04] border border-white/[0.06] px-3 py-2 text-sm text-white placeholder-white/30 focus:border-indigo-500/50 focus:outline-none mb-3"
            />
            <div className="max-h-64 overflow-y-auto space-y-1">
              {filteredAgents.length === 0 && (
                <p className="text-xs text-white/30 py-4 text-center">{t('common.no_data')}</p>
              )}
              {filteredAgents.map(a => {
                const inGrid = feedIds.includes(a.id);
                return (
                  <button
                    key={a.id}
                    onClick={() => !inGrid && handleAddFeed(a.id)}
                    disabled={inGrid}
                    className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left transition-all
                      ${inGrid ? 'opacity-40 cursor-not-allowed' : 'hover:bg-white/[0.04] cursor-pointer'}`}
                  >
                    <div className="w-2 h-2 rounded-full bg-emerald-400 flex-shrink-0" />
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium text-white truncate">{a.hostname}</p>
                      <p className="text-[10px] text-white/30">{a.os} · {(a.tags || []).join(', ')}</p>
                    </div>
                    {inGrid && <span className="text-[10px] text-white/30">✓</span>}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
