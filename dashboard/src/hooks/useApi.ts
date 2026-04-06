import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '../api/client';

// ═══ Generic async data hook with mock fallback ═══

export function useApi<T>(
  fetcher: () => Promise<T>,
  mockData: T,
  opts: { pollMs?: number; enabled?: boolean } = {}
) {
  const { pollMs, enabled = true } = opts;
  const [data, setData] = useState<T>(mockData);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isLive, setIsLive] = useState(false);
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const refresh = useCallback(async () => {
    if (!enabled) return;
    setLoading(true);
    setError(null);
    try {
      const result = await fetcherRef.current();
      setData(result);
      setIsLive(true);
    } catch {
      setIsLive(false);
      if (!isLive) setData(mockData); // only fall back if never connected
    } finally {
      setLoading(false);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);

  useEffect(() => {
    refresh();
    if (pollMs && enabled) {
      const id = setInterval(refresh, pollMs);
      return () => clearInterval(id);
    }
  }, [refresh, pollMs, enabled]);

  return { data, setData, loading, error, refresh, isLive };
}

// ═══ SSE hook for real-time events ═══

export function useSSE() {
  const [events, setEvents] = useState<any[]>([]);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    const token = api.getToken();
    const url = api.getSSEUrl();
    const es = new EventSource(url);
    es.onopen = () => setConnected(true);
    es.onmessage = (e) => {
      try {
        setEvents(prev => [JSON.parse(e.data), ...prev].slice(0, 100));
      } catch { /* ignore */ }
    };
    es.onerror = () => setConnected(false);
    return () => es.close();
  }, []);

  return { events, connected };
}

// ═══ Polling helper ═══

export function usePolling(callback: () => void, intervalMs: number, enabled = true) {
  const savedCallback = useRef(callback);
  savedCallback.current = callback;

  useEffect(() => {
    if (!enabled) return;
    const tick = () => savedCallback.current();
    tick();
    const id = setInterval(tick, intervalMs);
    return () => clearInterval(id);
  }, [intervalMs, enabled]);
}
