import { useState, useEffect, useCallback, useRef } from 'react';
export function useApi(fetcher, deps = []) {
    const [data, setData] = useState(fetcher());
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState(null);
    const refresh = useCallback(() => {
        setLoading(true);
        setError(null);
        try {
            setData(fetcher());
        }
        catch (e) {
            setError(e.message);
        }
        finally {
            setLoading(false);
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, deps);
    return { data, loading, error, refresh };
}
export function usePolling(callback, intervalMs, enabled = true) {
    const savedCallback = useRef(callback);
    savedCallback.current = callback;
    useEffect(() => {
        if (!enabled)
            return;
        const tick = () => savedCallback.current();
        tick();
        const id = setInterval(tick, intervalMs);
        return () => clearInterval(id);
    }, [intervalMs, enabled]);
}
export function useSSE(url, onMessage) {
    useEffect(() => {
        if (!url)
            return;
        const es = new EventSource(url);
        es.onmessage = (e) => {
            try {
                onMessage(JSON.parse(e.data));
            }
            catch {
                onMessage(e.data);
            }
        };
        es.onerror = () => es.close();
        return () => es.close();
    }, [url, onMessage]);
}
