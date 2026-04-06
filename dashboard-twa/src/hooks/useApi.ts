import { useState, useEffect, useCallback, useRef } from 'react';

export function useApi<T>(
  fetcher: () => Promise<T>,
  opts: { pollMs?: number } = {}
) {
  const { pollMs } = opts;
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await fetcherRef.current();
      setData(result);
    } catch (e: any) {
      setError(e?.message || 'Connection failed');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    if (pollMs) {
      const id = setInterval(refresh, pollMs);
      return () => clearInterval(id);
    }
  }, [refresh, pollMs]);

  return { data, loading, error, refresh };
}
