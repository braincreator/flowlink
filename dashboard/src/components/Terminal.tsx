import { useEffect, useRef, useCallback } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import '@xterm/xterm/css/xterm.css';
import { getTheme, toXtermTheme } from './terminal/themes';
import { useTerminalSettings } from '../hooks/useTerminalSettings';

interface TerminalProps {
  className?: string;
  onData?: (data: string) => void;
  onResize?: (cols: number, rows: number) => void;
}

export default function Terminal({ className = '', onData, onResize }: TerminalProps) {
  const { settings } = useTerminalSettings();
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const onDataRef = useRef(onData);
  const onResizeRef = useRef(onResize);

  onDataRef.current = onData;
  onResizeRef.current = onResize;

  useEffect(() => {
    if (!containerRef.current) return;
    const container = containerRef.current;

    const theme = getTheme(settings.themeId);
    const term = new XTerm({
      theme: toXtermTheme(theme),
      fontFamily: settings.fontFamily,
      fontSize: settings.fontSize,
      lineHeight: settings.lineHeight,
      cursorStyle: settings.cursorStyle,
      cursorBlink: settings.cursorBlink,
      scrollback: settings.scrollback,
      allowProposedApi: true,
    });

    const fit = new FitAddon();
    const webLinks = new WebLinksAddon();
    term.loadAddon(fit);
    term.loadAddon(webLinks);

    term.open(container);
    // Delay fit to ensure container is rendered
    setTimeout(() => {
      try { fit.fit(); } catch {}
      onResizeRef.current?.(term.cols, term.rows);
    }, 50);

    term.onData(data => onDataRef.current?.(data));
    term.onResize(({ cols, rows }) => onResizeRef.current?.(cols, rows));

    // Custom right-click for paste
    container.addEventListener('contextmenu', (e: Event) => {
      e.preventDefault();
      const ce = e as ClipboardEvent;
      if (ce.clipboardData) {
        const text = ce.clipboardData.getData('text');
        if (text) term.write(text);
      }
    });

    termRef.current = term;
    fitRef.current = fit;

    const ro = new ResizeObserver(() => {
      try { fit.fit(); } catch {}
    });
    ro.observe(container);

    return () => {
      ro.disconnect();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, []);

  const write = useCallback((data: string | Uint8Array) => {
    if (termRef.current) {
      termRef.current.write(typeof data === 'string' ? data : new TextDecoder().decode(data));
    }
  }, []);

  const clear = useCallback(() => termRef.current?.clear(), []);
  const focus = useCallback(() => termRef.current?.focus(), []);

  // Expose methods via ref-like pattern
  (containerRef.current as any)?.__terminal && ((containerRef.current as any).__terminal = { write, clear, focus });

  return <div ref={containerRef} className={`w-full h-full ${className}`} style={{ minHeight: '300px' }} />;
}

// Helper to get terminal instance from ref
export function getTerminalFromContainer(el: HTMLDivElement | null) {
  return (el as any)?.__terminal || null;
}
