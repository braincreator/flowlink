import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useRef, useCallback } from 'react';
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
    const { id } = useParams();
    const { t } = useTranslation();
    const navigate = useNavigate();
    const { data: agents } = useApi(() => api.getAgents(), { pollMs: 15000 });
    const agentList = (agents || []);
    const agent = agentList.find((a) => a.id === id);
    const hostname = agent?.hostname || id || 'Unknown';
    const [settingsOpen, setSettingsOpen] = useState(false);
    const terminalContainerRef = useRef(null);
    const { startRecording, stopRecording, recording, duration } = useLiveRecorder(terminalContainerRef);
    const [recordingResult, setRecordingResult] = useState(null);
    const wsUrl = id ? `${api.getApiBase().replace(/^http/, 'ws')}/api/agents/${id}/shell` : null;
    const { connected, reconnecting, send } = useWebSocket({
        url: wsUrl,
        onMessage: (data) => {
            const text = new TextDecoder().decode(data);
            if (termRef.current)
                termRef.current.write(text);
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
    const termRef = useRef(null);
    const handleData = useCallback((data) => {
        send(new TextEncoder().encode(data));
    }, [send]);
    return (_jsxs("div", { className: "flex flex-col h-[calc(100vh-7rem)] -m-6 bg-[#060a14]", children: [_jsxs("div", { className: "flex items-center gap-3 border-b border-white/[0.06] bg-white/[0.02] px-4 py-3", children: [_jsx("button", { onClick: () => navigate(-1), className: "p-1.5 rounded-lg text-white/40 hover:text-white hover:bg-white/[0.06] transition-colors", children: _jsx(ArrowLeft, { size: 16 }) }), _jsx(Monitor, { size: 18, className: "text-indigo-400" }), _jsx("h2", { className: "text-sm font-semibold text-white", children: hostname }), _jsx("span", { className: "text-[10px] text-white/30", children: agent?.os }), _jsxs("div", { className: "ml-auto flex items-center gap-2", children: [_jsxs("div", { className: "flex items-center gap-1.5", children: [connected ? _jsx(Wifi, { size: 12, className: "text-emerald-400" }) : _jsx(WifiOff, { size: 12, className: "text-rose-400" }), _jsx("span", { className: `text-xs font-medium ${connected ? 'text-emerald-400' : reconnecting ? 'text-amber-400' : 'text-rose-400'}`, children: connected ? t('terminal.connected') : reconnecting ? t('terminal.reconnecting') : t('terminal.disconnected') })] }), recording && (_jsxs("div", { className: "flex items-center gap-1.5 rounded-lg bg-rose-500/15 px-2.5 py-1", children: [_jsx("span", { className: "h-2 w-2 rounded-full bg-rose-500 animate-pulse" }), _jsxs("span", { className: "text-xs font-medium text-rose-400", children: [Math.floor(duration / 60), ":", (duration % 60).toString().padStart(2, '0')] }), _jsx("button", { onClick: () => { const res = stopRecording(); if (res)
                                            setRecordingResult(res); }, className: "text-xs text-rose-400 hover:text-rose-300 ml-1", children: "\u23F9" })] })), !recording && (_jsx("button", { onClick: () => startRecording(), className: "p-1.5 rounded-md hover:bg-white/10 transition-colors", title: "Live Record", children: "\u23FA" })), _jsx("button", { onClick: () => setSettingsOpen(true), className: "p-1.5 rounded-md hover:bg-white/10 transition-colors", title: "Terminal Settings", children: _jsx(Settings, { size: 14, className: "text-white/50" }) })] })] }), _jsx(TerminalSettingsPanel, { open: settingsOpen, onClose: () => setSettingsOpen(false) }), _jsx("div", { className: "flex-1 min-h-0", ref: terminalContainerRef, children: _jsx(TerminalComponent, { onData: handleData }) }), recordingResult && (_jsxs("div", { className: "fixed bottom-4 right-4 z-50 rounded-xl border border-white/10 bg-[var(--color-surface)] shadow-xl p-3 space-y-2 w-72", children: [_jsxs("div", { className: "flex items-center justify-between", children: [_jsx("span", { className: "text-xs font-medium", children: "\uD83C\uDFAC Recording ready" }), _jsx("button", { onClick: () => setRecordingResult(null), className: "text-[var(--color-dim)] hover:text-white text-xs", children: "\u2715" })] }), _jsx("video", { src: recordingResult.url, controls: true, className: "w-full rounded-lg" }), _jsx("button", { onClick: () => {
                            const a = document.createElement('a');
                            a.href = recordingResult.url;
                            a.download = `terminal-${Date.now()}.webm`;
                            a.click();
                        }, className: "w-full rounded-lg bg-[var(--color-accent)] py-1.5 text-xs font-medium text-white", children: t('sessions.download') })] }))] }));
}
