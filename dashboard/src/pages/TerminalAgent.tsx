import { useState, useRef, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Monitor, Wifi, WifiOff, ArrowLeft } from 'lucide-react';
import TerminalComponent from '../components/Terminal';
import { useWebSocket } from '../hooks/useWebSocket';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';

export default function TerminalAgent() {
  const { id } = useParams<{ id: string }>();
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data: agents } = useApi(() => api.getAgents(), { pollMs: 15000 });
  const agentList = (agents || []) as any[];
  const agent = agentList.find((a: any) => a.id === id);
  const hostname = agent?.hostname || id || 'Unknown';

  const wsUrl = id ? `${api.getApiBase().replace(/^http/, 'ws')}/api/agents/${id}/shell` : null;

  const { connected, reconnecting, send } = useWebSocket({
    url: wsUrl,
    onMessage: (data) => {
      const text = new TextDecoder().decode(data);
      if (termRef.current) termRef.current.write(text);
    },
    onOpen: () => {
      if (termRef.current) {
        termRef.current.write(`\r\n\x1b[1;34m┌─ Connected to ${hostname}\x1b[0m\r\n`);
        termRef.current.write(`\x1b[1;34m├─ OS: ${agent?.os || 'unknown'} | Uptime: ${agent?.uptime || 'N/A'}\x1b[0m\r\n`);
        termRef.current.write(`\x1b[1;34m└─ Interactive shell — type commands and press Enter\x1b[0m\r\n\r\n`);
        termRef.current.focus();
      }
    },
  });

  const termRef = useRef<any>(null);

  const handleData = useCallback((data: string) => {
    send(new TextEncoder().encode(data));
  }, [send]);

  return (
    <div className="flex flex-col h-[calc(100vh-7rem)] -m-6 bg-[#060a14]">
      {/* Header */}
      <div className="flex items-center gap-3 border-b border-white/[0.06] bg-white/[0.02] px-4 py-3">
        <button
          onClick={() => navigate(-1)}
          className="p-1.5 rounded-lg text-white/40 hover:text-white hover:bg-white/[0.06] transition-colors"
        >
          <ArrowLeft size={16} />
        </button>
        <Monitor size={18} className="text-indigo-400" />
        <h2 className="text-sm font-semibold text-white">{hostname}</h2>
        <span className="text-[10px] text-white/30">{agent?.os}</span>
        <div className="ml-auto flex items-center gap-2">
          <div className="flex items-center gap-1.5">
            {connected ? <Wifi size={12} className="text-emerald-400" /> : <WifiOff size={12} className="text-rose-400" />}
            <span className={`text-xs font-medium ${connected ? 'text-emerald-400' : reconnecting ? 'text-amber-400' : 'text-rose-400'}`}>
              {connected ? t('terminal.connected') : reconnecting ? t('terminal.reconnecting') : t('terminal.disconnected')}
            </span>
          </div>
        </div>
      </div>

      {/* Terminal */}
      <div className="flex-1 min-h-0">
        <TerminalComponent onData={handleData} />
      </div>
    </div>
  );
}
