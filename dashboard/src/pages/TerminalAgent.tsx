import { useState, useRef, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Monitor, Wifi, WifiOff, ArrowLeft, Settings } from 'lucide-react';
import TerminalComponent from '../components/Terminal';
import TerminalSettingsPanel from '../components/terminal/TerminalSettings';
import { useLiveRecorder } from '../hooks/useLiveRecorder';
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
  const [settingsOpen, setSettingsOpen] = useState(false);
  const terminalContainerRef = useRef<HTMLDivElement>(null);
  const { startRecording, stopRecording, recording, duration } = useLiveRecorder(terminalContainerRef);
  const [recordingResult, setRecordingResult] = useState<any>(null);

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
          {recording && (
            <div className="flex items-center gap-1.5 rounded-lg bg-rose-500/15 px-2.5 py-1">
              <span className="h-2 w-2 rounded-full bg-rose-500 animate-pulse" />
              <span className="text-xs font-medium text-rose-400">{Math.floor(duration / 60)}:{(duration % 60).toString().padStart(2, '0')}</span>
              <button onClick={() => { const res = stopRecording(); if (res) setRecordingResult(res); }} className="text-xs text-rose-400 hover:text-rose-300 ml-1">⏹</button>
            </div>
          )}
          {!recording && (
            <button onClick={() => startRecording()} className="p-1.5 rounded-md hover:bg-white/10 transition-colors" title="Live Record">⏺</button>
          )}
          <button onClick={() => setSettingsOpen(true)} className="p-1.5 rounded-md hover:bg-white/10 transition-colors" title="Terminal Settings">
            <Settings size={14} className="text-white/50" />
          </button>
        </div>
      </div>

      <TerminalSettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />

      {/* Terminal */}
      <div className="flex-1 min-h-0" ref={terminalContainerRef}>
        <TerminalComponent onData={handleData} />
      </div>
      {recordingResult && (
        <div className="fixed bottom-4 right-4 z-50 rounded-xl border border-white/10 bg-[var(--color-surface)] shadow-xl p-3 space-y-2 w-72">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium">🎬 Recording ready</span>
            <button onClick={() => setRecordingResult(null)} className="text-[var(--color-dim)] hover:text-white text-xs">✕</button>
          </div>
          <video src={recordingResult.url} controls className="w-full rounded-lg" />
          <button
            onClick={() => {
              const a = document.createElement('a');
              a.href = recordingResult.url;
              a.download = `terminal-${Date.now()}.webm`;
              a.click();
            }}
            className="w-full rounded-lg bg-[var(--color-accent)] py-1.5 text-xs font-medium text-white"
          >{t('sessions.download')}</button>
        </div>
      )}
    </div>
  );
}
