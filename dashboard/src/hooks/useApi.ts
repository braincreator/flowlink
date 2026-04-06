import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '../api/client';

// ═══ Generic async data hook ═══

export function useApi<T>(
  fetcher: () => Promise<T>,
  opts: { pollMs?: number; enabled?: boolean } = {}
) {
  const { pollMs, enabled = true } = opts;
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const refresh = useCallback(async () => {
    if (!enabled) return;
    setLoading(true);
    setError(null);
    try {
      const result = await fetcherRef.current();
      setData(result);
      setLastUpdated(new Date());
      setError(null);
    } catch (e: any) {
      setError(e?.message || 'Connection failed');
    } finally {
      setLoading(false);
    }
  }, [enabled]);

  useEffect(() => {
    refresh();
    if (pollMs && enabled) {
      const id = setInterval(refresh, pollMs);
      return () => clearInterval(id);
    }
  }, [refresh, pollMs, enabled]);

  return { data, setData, loading, error, refresh, lastUpdated };
}

// ═══ SSE hook for real-time events ═══

export function useSSE() {
  const [events, setEvents] = useState<any[]>([]);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
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
