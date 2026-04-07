import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useRef, useCallback, useEffect } from 'react';
import { Trash2, Download } from 'lucide-react';
import { useWebSocket } from '../hooks/useWebSocket';
import { SlidePanel } from './Layout';
import { api } from '../api/client';
export default function TerminalPanel({ open, onClose, agent, mode = 'shell' }) {
    const [output, setOutput] = useState([]);
    const termRef = useRef(null);
    const wsUrl = agent && mode === 'shell'
        ? `${api.getApiBase().replace(/^http/, 'ws')}/api/agents/${agent.id}/shell`
        : null;
    const { connected, reconnecting, send } = useWebSocket({
        url: wsUrl,
        onMessage: (data) => {
            const text = new TextDecoder().decode(data);
            setOutput(prev => [...prev, text]);
        },
        onOpen: () => {
            if (termRef.current) {
                termRef.current.write(`\x1b[1;34mConnected to ${agent?.hostname}\x1b[0m\r\n`);
            }
        },
    });
    // Log viewer mode: fetch logs on open
    useEffect(() => {
        if (mode === 'logs' && open) {
            api.getAuditEvents({ limit: 50 }).then((events) => {
                const lines = events.map(ev => {
                    const color = ev.risk_score >= 70 ? '\x1b[31m' : ev.risk_score >= 40 ? '\x1b[33m' : ev.event_type?.includes('error') ? '\x1b[31m' : '\x1b[34m';
                    const reset = '\x1b[0m';
                    const time = ev.timestamp_iso ? new Date(ev.timestamp_iso).toISOString().slice(11, 19) : '';
                    return `${color}[${time}]${reset} ${ev.event_type || ''} ${ev.command || ''} ${ev.risk_score ? `risk:${ev.risk_score}` : ''}`;
                });
                setOutput(lines);
            }).catch(() => setOutput(['Failed to fetch logs']));
        }
    }, [mode, open]);
    const handleData = useCallback((data) => {
        if (mode === 'shell') {
            send(new TextEncoder().encode(data));
        }
    }, [mode, send]);
    const handleClear = useCallback(() => {
        setOutput([]);
        if (termRef.current)
            termRef.current.clear();
    }, []);
    const handleDownload = useCallback(() => {
        const text = output.join('\n');
        const blob = new Blob([text], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `flowlink-${mode}-${agent?.hostname || 'logs'}-${new Date().toISOString().slice(0, 19)}.txt`;
        a.click();
        URL.revokeObjectURL(url);
    }, [output, mode, agent]);
    return (_jsx(SlidePanel, { open: open, onClose: onClose, title: mode === 'shell' ? `Terminal: ${agent?.hostname || ''}` : 'Log Viewer', width: "w-[640px]", children: _jsxs("div", { className: "flex flex-col h-full -m-6", children: [_jsxs("div", { className: "flex items-center gap-2 border-b border-[var(--color-border)] bg-[var(--color-bg)] px-4 py-2", children: [mode === 'shell' && (_jsx("span", { className: `text-xs font-medium ${connected ? 'text-emerald-400' : reconnecting ? 'text-amber-400' : 'text-rose-400'}`, children: connected ? '● Connected' : reconnecting ? '◐ Reconnecting...' : '○ Disconnected' })), _jsxs("div", { className: "ml-auto flex items-center gap-1", children: [_jsxs("button", { onClick: handleClear, className: "flex items-center gap-1 rounded-md px-2 py-1 text-xs text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: [_jsx(Trash2, { size: 12 }), " Clear"] }), _jsxs("button", { onClick: handleDownload, className: "flex items-center gap-1 rounded-md px-2 py-1 text-xs text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: [_jsx(Download, { size: 12 }), " Download"] })] })] }), _jsx("div", { className: "flex-1 bg-[#0a0e1a]", style: { height: 'calc(100vh - 140px)' }, children: _jsx(TerminalWrapper, { output: output, onData: handleData, termRef: termRef }) })] }) }));
}
// Inline wrapper that feeds output to xterm
function TerminalWrapper({ output, onData, termRef }) {
    const containerRef = useRef(null);
    const prevLenRef = useRef(0);
    // Write new output to the terminal via a custom approach
    // We use a simpler approach: render output directly
    return (_jsxs("div", { className: "h-full w-full overflow-auto font-mono text-sm p-3 bg-[#0a0e1a] text-[#e1e4ed]", style: { lineHeight: '1.5' }, children: [output.map((line, i) => (_jsx("div", { className: "whitespace-pre-wrap", children: line }, i))), output.length === 0 && (_jsx("div", { className: "text-[var(--color-dim)]", children: "Waiting for connection..." }))] }));
}
