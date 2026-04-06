import { useCallback } from 'react';

declare global { interface Window { Telegram?: { WebApp?: { close: () => void; sendData: (s: string) => void; BackButton?: { show: () => void; hide: () => void } } } } }

export function useTelegram() {
  const close = useCallback(() => {
    try { window.Telegram?.WebApp?.close(); } catch {}
  }, []);

  const sendData = useCallback((data: object) => {
    try { window.Telegram?.WebApp?.sendData(JSON.stringify(data)); } catch {}
  }, []);

  const back = useCallback(() => {
    try { window.Telegram?.WebApp?.BackButton?.show(); } catch {}
  }, []);

  const hideBack = useCallback(() => {
    try { window.Telegram?.WebApp?.BackButton?.hide(); } catch {}
  }, []);

  return { close, sendData, back, hideBack };
}
