import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Radio, Settings } from 'lucide-react';
import TerminalComponent from '../components/Terminal';
import TerminalSettingsPanel from '../components/terminal/TerminalSettings';
import { useWebSocket } from '../hooks/useWebSocket';
import { api } from '../api/client';
const RELAY_COMMANDS = ['status', 'agents list', 'shield stats', 'config', 'help'];
export default function TerminalRelay() {
    const { t } = useTranslation();
    const termRef = useRef(null);
    const [commandHistory, setCommandHistory] = useState([]);
    const [settingsOpen, setSettingsOpen] = useState(false);
    const wsUrl = `${api.getApiBase().replace(/^http/, 'ws')}/api/relay/shell`;
    const { connected, send } = useWebSocket({
        url: wsUrl,
        onMessage: (data) => {
            const text = new TextDecoder().decode(data);
            if (termRef.current)
                termRef.current.write(text);
        },
        onOpen: () => {
            if (termRef.current) {
                termRef.current.write(`\r\n\x1b[1;34m┌─ FlowLink Relay Console\x1b[0m\r\n`);
                termRef.current.write(`\x1b[1;34m├─ Commands: status, agents list, shield stats, config, help\x1b[0m\r\n`);
                termRef.current.write(`\x1b[1;34m└─ Type a command and press Enter\x1b[0m\r\n\r\n`);
                termRef.current.focus();
            }
        },
    });
    const handleData = useCallback((data) => {
        send(new TextEncoder().encode(data));
    }, [send]);
    return (_jsxs("div", { className: "flex flex-col h-[calc(100vh-7rem)] -m-6 bg-[#060a14]", children: [_jsx(TerminalSettingsPanel, { open: settingsOpen, onClose: () => setSettingsOpen(false) }), _jsxs("div", { className: "flex items-center gap-3 border-b border-white/[0.06] bg-white/[0.02] px-4 py-3", children: [_jsx(Radio, { size: 18, className: "text-indigo-400" }), _jsx("h2", { className: "text-sm font-semibold text-white", children: t('terminal_soc.relay_console') }), _jsx("span", { className: `ml-2 text-xs font-medium ${connected ? 'text-emerald-400' : 'text-rose-400'}`, children: connected ? t('terminal.connected') : t('terminal.disconnected') }), _jsxs("div", { className: "ml-auto flex items-center gap-2", children: [_jsx("button", { onClick: () => setSettingsOpen(true), className: "p-1.5 rounded-md hover:bg-white/10 transition-colors", title: "Terminal Settings", children: _jsx(Settings, { size: 14, className: "text-white/50" }) }), _jsx("div", { className: "flex gap-1", children: RELAY_COMMANDS.map(cmd => (_jsx("button", { onClick: () => {
                                        if (termRef.current)
                                            termRef.current.write(`${cmd}\r\n`);
                                        send(new TextEncoder().encode(`${cmd}\n`));
                                    }, className: "px-2 py-1 rounded text-[10px] text-white/40 bg-white/[0.04] hover:bg-white/[0.08] hover:text-white/70 transition-colors", children: cmd }, cmd))) })] })] }), _jsx("div", { className: "flex-1 min-h-0", children: _jsx(TerminalComponent, { onData: handleData }) })] }));
}
