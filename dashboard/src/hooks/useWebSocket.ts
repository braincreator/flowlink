import { useEffect, useRef, useCallback, useState } from 'react';

interface UseWebSocketOptions {
  url: string | null;
  onMessage?: (data: Uint8Array) => void;
  onOpen?: () => void;
  onClose?: () => void;
  reconnectMs?: number;
  maxRetries?: number;
}

export function useWebSocket({ url, onMessage, onOpen, onClose, reconnectMs = 3000, maxRetries = 10 }: UseWebSocketOptions) {
  const wsRef = useRef<WebSocket | null>(null);
  const retryRef = useRef(0);
  const [connected, setConnected] = useState(false);
  const [reconnecting, setReconnecting] = useState(false);

  const connect = useCallback(() => {
    if (!url) return;
    if (wsRef.current?.readyState === WebSocket.OPEN || wsRef.current?.readyState === WebSocket.CONNECTING) return;

    const token = localStorage.getItem('flowlink_token');
    const sep = url.includes('?') ? '&' : '?';
    const fullUrl = token ? `${url}${sep}token=${token}` : url;

    const ws = new WebSocket(fullUrl);
    ws.binaryType = 'arraybuffer';
    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true);
      setReconnecting(false);
      retryRef.current = 0;
      onOpen?.();
    };

    ws.onmessage = (e) => {
      if (e.data instanceof ArrayBuffer) {
        onMessage?.(new Uint8Array(e.data));
      } else if (typeof e.data === 'string') {
        const enc = new TextEncoder();
        onMessage?.(enc.encode(e.data));
      }
    };

    ws.onclose = () => {
      setConnected(false);
      wsRef.current = null;
      onClose?.();
      if (retryRef.current < maxRetries) {
        setReconnecting(true);
        setTimeout(() => {
          retryRef.current++;
          connect();
        }, reconnectMs);
      }
    };

    ws.onerror = () => { ws.close(); };
  }, [url, onMessage, onOpen, onClose, reconnectMs, maxRetries]);

  useEffect(() => {
    connect();
    return () => {
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, [connect]);

  const send = useCallback((data: Uint8Array | string) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(data);
    }
  }, []);

  const sendJson = useCallback((obj: object) => {
    send(JSON.stringify(obj));
  }, [send]);

  return { connected, reconnecting, send, sendJson, ws: wsRef };
}
