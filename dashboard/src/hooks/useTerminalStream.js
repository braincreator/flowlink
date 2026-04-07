import { useEffect, useRef, useState, useCallback } from 'react';
import { api } from '../api/client';
export function useTerminalStream({ agentId, pollMs = 5000 }) {
    const [feed, setFeed] = useState(null);
    const wsRef = useRef(null);
    const reconnectTimer = useRef(null);
    const connectWs = useCallback(() => {
        if (!agentId)
            return;
        const base = api.getApiBase().replace(/^http/, 'ws');
        const ws = new WebSocket(`${base}/api/agents/${agentId}/shell?mode=readonly`);
        ws.binaryType = 'arraybuffer';
        ws.onmessage = (e) => {
            const text = new TextDecoder().decode(e.data);
            setFeed(prev => prev ? { ...prev, lastOutput: prev.lastOutput + text, status: 'online' } : prev);
        };
        ws.onopen = () => {
            setFeed(prev => prev ? { ...prev, status: 'online' } : prev);
        };
        ws.onclose = () => {
            setFeed(prev => prev ? { ...prev, status: 'disconnected' } : prev);
            reconnectTimer.current = setTimeout(connectWs, 5000);
        };
        ws.onerror = () => ws.close();
        wsRef.current = ws;
    }, [agentId]);
    useEffect(() => {
        // Fetch agent metadata
        if (!agentId)
            return;
        api.getAgents().then((resp) => {
            const agents = Array.isArray(resp) ? resp : [];
            const a = agents.find((ag) => ag.id === agentId);
            if (a) {
                setFeed({
                    agentId: a.id,
                    hostname: a.hostname,
                    status: a.status === 'online' ? 'online' : 'disconnected',
                    tags: a.tags || [],
                    os: a.os || 'linux',
                    uptime: a.uptime || 0,
                    commandCount: a.command_count || 0,
                    alertCount: a.alert_count || 0,
                    lastOutput: '',
                    connectedAt: Date.now(),
                });
            }
        }).catch(() => { });
    }, [agentId]);
    useEffect(() => {
        connectWs();
        return () => {
            if (reconnectTimer.current)
                clearTimeout(reconnectTimer.current);
            wsRef.current?.close();
        };
    }, [connectWs]);
    const disconnect = useCallback(() => {
        if (reconnectTimer.current)
            clearTimeout(reconnectTimer.current);
        wsRef.current?.close();
    }, []);
    return { feed, disconnect };
}
