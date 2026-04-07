import { useState, useRef, useEffect, useCallback, type DragEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Maximize2, X, Clock, TerminalSquare } from 'lucide-react';
import TerminalFeed from './TerminalFeedCard';
import type { FeedState } from '../hooks/useTerminalStream';

/* ── Individual Feed Card ── */
interface FeedCardProps {
  feed: FeedState;
  index: number;
  onExpand: (agentId: string) => void;
  onDetach: (agentId: string) => void;
  onDragStart: (index: number) => void;
  onDrop: (index: number) => void;
}

function FeedCard({ feed, index, onExpand, onDetach, onDragStart, onDrop }: FeedCardProps) {
  const { t } = useTranslation();
  const [dragOver, setDragOver] = useState(false);

  const statusColor = feed.status === 'online' ? '#34d399' : feed.status === 'idle' ? '#fbbf24' : '#f43f5e';
  const duration = feed.connectedAt ? Math.floor((Date.now() - feed.connectedAt) / 60000) : 0;
  const durationStr = duration >= 60 ? `${Math.floor(duration / 60)}h ${duration % 60}m` : `${duration}m`;

  const handleMount = useCallback((term: any) => {
    if (feed.lastOutput) term.write(feed.lastOutput);
  }, [feed.lastOutput]);

  return (
    <div
      draggable
      onDragStart={(e) => { e.dataTransfer.effectAllowed = 'move'; onDragStart(index); }}
      onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; setDragOver(true); }}
      onDrop={(e) => { e.preventDefault(); setDragOver(false); onDrop(index); }}
      onDragLeave={() => setDragOver(false)}
      className={`
        relative flex flex-col rounded-xl overflow-hidden border transition-all duration-300
        ${dragOver ? 'border-indigo-500/60 scale-[1.02] z-10' : ''}
        ${feed.status === 'disconnected' ? 'border-rose-500/30' : 'border-white/[0.06]'}
        ${feed.alertCount > 0 ? 'animate-alert-flash' : ''}
        bg-[#060a14]/80 backdrop-blur-sm
      `}
      style={{
        boxShadow: '0 2px 12px rgba(0, 0, 0, 0.4)',
        animation: 'feedFadeIn 0.3s ease-out',
      }}
      data-feed-id={feed.agentId}
    >
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 bg-white/[0.03] border-b border-white/[0.04]">
        <div className="relative">
          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: statusColor }} />
          {feed.status === 'online' && (
            <div className="absolute inset-0 w-2 h-2 rounded-full animate-ping" style={{ backgroundColor: statusColor, opacity: 0.5 }} />
          )}
        </div>
        <span className="text-xs font-semibold text-white truncate">{feed.hostname}</span>
        {feed.tags[0] && (
          <span className="px-1.5 py-0.5 text-[10px] font-medium rounded bg-indigo-500/20 text-indigo-300">
            {feed.tags[0]}
          </span>
        )}
        <div className="ml-auto flex items-center gap-1">
          <button onClick={() => onExpand(feed.agentId)} className="p-1 rounded text-white/40 hover:text-white hover:bg-white/10 transition-colors" title={t('terminal_soc.expand')}>
            <Maximize2 size={13} />
          </button>
          <button onClick={() => onDetach(feed.agentId)} className="p-1 rounded text-white/40 hover:text-rose-400 hover:bg-white/10 transition-colors" title={t('terminal_soc.detach')}>
            <X size={13} />
          </button>
        </div>
      </div>

      {/* Terminal */}
      <div className="flex-1 min-h-0 relative">
        <TerminalFeed agentId={feed.agentId} interactive={false} onMount={handleMount} />
        {feed.status === 'disconnected' && (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-[#060a14]/80 backdrop-blur-sm fade-in">
            <div className="text-rose-400 text-2xl mb-2">⊘</div>
            <span className="text-xs font-medium text-rose-400">{t('terminal_soc.disconnected')}</span>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="flex items-center gap-3 px-3 py-1.5 bg-white/[0.02] border-t border-white/[0.04] text-[10px] text-white/30">
        <div className="flex items-center gap-1"><Clock size={10} /><span>{durationStr}</span></div>
        <div className="flex items-center gap-1"><TerminalSquare size={10} /><span>{feed.commandCount} cmds</span></div>
        {feed.alertCount > 0 && (
          <span className="ml-auto px-1.5 py-0.5 rounded-full bg-rose-500/20 text-rose-400 font-medium animate-pulse">
            {feed.alertCount} ⚡
          </span>
        )}
      </div>

      <style>{`
        @keyframes feedFadeIn { from { opacity: 0; transform: scale(0.95); } to { opacity: 1; transform: scale(1); } }
        @keyframes alertFlash { 0%, 100% { box-shadow: 0 0 0 rgba(244, 63, 94, 0); } 50% { box-shadow: 0 0 16px rgba(244, 63, 94, 0.4); } }
        .animate-alert-flash { animation: alertFlash 1.5s ease-in-out 3; }
      `}</style>
    </div>
  );
}

/* ── Grid Component ── */
export type GridLayout = '1x1' | '2x2' | '3x2' | '3x3';
export const GRID_COLS: Record<GridLayout, number> = { '1x1': 1, '2x2': 2, '3x2': 3, '3x3': 3 };
export const GRID_MAX: Record<GridLayout, number> = { '1x1': 1, '2x2': 4, '3x2': 6, '3x3': 9 };

interface GridProps {
  feeds: FeedState[];
  layout: GridLayout;
  onExpand: (agentId: string) => void;
  onDetach: (agentId: string) => void;
}

export default function TerminalGrid({ feeds, layout, onExpand, onDetach }: GridProps) {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [order, setOrder] = useState<number[]>(feeds.map((_, i) => i));

  useEffect(() => { setOrder(feeds.map((_, i) => i)); }, [feeds.length]);

  const handleDrop = useCallback((targetIndex: number) => {
    if (dragIndex === null || dragIndex === targetIndex) return;
    setOrder(prev => {
      const next = [...prev];
      [next[dragIndex], next[targetIndex]] = [next[targetIndex], next[dragIndex]];
      return next;
    });
    setDragIndex(null);
  }, [dragIndex]);

  const orderedFeeds = order.map(i => feeds[i]).filter(Boolean);

  return (
    <div
      className="grid gap-3 h-full"
      style={{ gridTemplateColumns: `repeat(${GRID_COLS[layout]}, 1fr)` }}
    >
      {orderedFeeds.map((feed, i) => (
        <FeedCard
          key={feed.agentId}
          feed={feed}
          index={i}
          onExpand={onExpand}
          onDetach={onDetach}
          onDragStart={setDragIndex}
          onDrop={handleDrop}
        />
      ))}
    </div>
  );
}
