import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useRef, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { TerminalSquare } from 'lucide-react';
import TerminalComponent from '../components/Terminal';
import { useWebSocket } from '../hooks/useWebSocket';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
export default function TerminalPage() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const { data: agents } = useApi(() => api.getAgents(), { pollMs: 30000 });
    const agentList = (agents || []);
    const onlineAgents = agentList.filter((a) => a.status === 'online');
    const [selectedAgent, setSelectedAgent] = useState(onlineAgents[0]?.id || '');
    const [mode, setMode] = useState('shell');
    const [logLines, setLogLines] = useState([]);
    const hostname = agentList.find((a) => a.id === selectedAgent)?.hostname || '';
    const wsUrl = mode === 'shell' && selectedAgent
        ? `${api.getApiBase().replace(/^http/, 'ws')}/api/agents/${selectedAgent}/shell`
        : null;
    const { connected, reconnecting, send, sendJson } = useWebSocket({
        url: wsUrl,
        onMessage: (data) => {
            const text = new TextDecoder().decode(data);
            if (termRef.current)
                termRef.current.write(text);
        },
        onOpen: () => {
            if (termRef.current)
                termRef.current.write(`\r\n\x1b[1;34m┌─ Connected to ${hostname}\x1b[0m\r\n\x1b[1;34m└─ Type commands and press Enter\x1b[0m\r\n\r\n`);
            if (termRef.current)
                termRef.current.focus();
        },
    });
    const termRef = useRef(null);
    useEffect(() => {
        if (mode === 'logs') {
            api.getAuditEvents({ limit: 50 }).then((events) => {
                const lines = events.map(ev => {
                    const level = ev.risk_score >= 70 ? 'ERROR' : ev.risk_score >= 40 ? 'WARN' : 'INFO';
                    const colors = { ERROR: '\x1b[31m', WARN: '\x1b[33m', INFO: '\x1b[34m', DEBUG: '\x1b[2m' };
                    const time = ev.timestamp_iso ? new Date(ev.timestamp_iso).toISOString().slice(11, 19) : '???:??';
                    return `${colors[level] || ''}[${time}] [${level}] ${ev.event_type || ''} | ${ev.command || ev.user || ''}${ev.risk_score ? ` | risk:${ev.risk_score}` : ''}\x1b[0m`;
                });
                setLogLines(lines);
            }).catch(() => setLogLines(['Failed to fetch logs.']));
        }
    }, [mode]);
    const handleData = useCallback((data) => {
        if (mode === 'shell')
            send(new TextEncoder().encode(data));
    }, [mode, send]);
    const handleClear = useCallback(() => {
        if (termRef.current)
            termRef.current.clear();
    }, []);
    const handleDownload = useCallback(() => {
        const content = logLines.join('\n');
        const blob = new Blob([content], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `flowlink-logs-${hostname}-${new Date().toISOString().slice(0, 19)}.txt`;
        a.click();
        URL.revokeObjectURL(url);
    }, [logLines, hostname]);
    return (_jsxs("div", { className: "flex flex-col h-[calc(100vh-7rem)] -m-6 fade-in", children: [_jsxs("div", { className: "flex items-center gap-3 border-b border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-3", children: [_jsx(TerminalSquare, { size: 18, className: "text-[var(--color-accent)]" }), _jsxs("select", { value: selectedAgent, onChange: e => setSelectedAgent(e.target.value), className: "rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm focus:border-[var(--color-accent)] focus:outline-none", children: [onlineAgents.map(a => (_jsx("option", { value: a.id, children: a.hostname }, a.id))), onlineAgents.length === 0 && _jsx("option", { value: "", children: t('common.no_online_agents') })] }), _jsxs("div", { className: "flex rounded-lg border border-[var(--color-border)] overflow-hidden", children: [_jsx("button", { onClick: () => setMode('shell'), className: `px-3 py-1.5 text-xs font-medium transition-colors ${mode === 'shell' ? 'bg-[var(--color-accent)] text-white' : 'text-[var(--color-dim)] hover:bg-[var(--color-surface2)]'}`, children: t('terminal.shell_mode') }), _jsx("button", { onClick: () => setMode('logs'), className: `px-3 py-1.5 text-xs font-medium transition-colors ${mode === 'logs' ? 'bg-[var(--color-accent)] text-white' : 'text-[var(--color-dim)] hover:bg-[var(--color-surface2)]'}`, children: t('terminal.logs_mode') })] }), mode === 'shell' && (_jsx("span", { className: `text-xs font-medium ${connected ? 'text-emerald-400' : reconnecting ? 'text-amber-400' : 'text-rose-400'}`, children: connected ? t('terminal.connected') : reconnecting ? t('terminal.reconnecting') : t('terminal.disconnected') })), _jsxs("div", { className: "ml-auto flex items-center gap-2", children: [_jsx("button", { onClick: handleClear, className: "rounded-md px-2.5 py-1 text-xs text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: t('terminal.clear') }), mode === 'logs' && (_jsx("button", { onClick: handleDownload, className: "rounded-md px-2.5 py-1 text-xs text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: t('terminal.download') }))] })] }), _jsx("div", { className: "flex-1 bg-[#0a0e1a] overflow-hidden", children: mode === 'shell' ? (_jsx(TerminalComponent, { onData: handleData })) : (_jsxs("div", { className: "h-full overflow-auto font-mono text-sm p-4 text-[#e1e4ed]", style: { lineHeight: '1.6' }, children: [logLines.map((line, i) => (_jsx("div", { className: "whitespace-pre-wrap", children: line }, i))), logLines.length === 0 && _jsx("div", { className: "text-[var(--color-dim)]", children: t("common.loading_logs") })] })) })] }));
}
