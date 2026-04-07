import { useState, useRef, useEffect, useCallback } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import { getTheme, toXtermTheme } from './terminal/themes';
import { useTerminalSettings } from '../hooks/useTerminalSettings';

interface Props {
  agentId: string;
  interactive?: boolean;
  onData?: (data: string) => void;
  onMount?: (term: XTerm) => void;
  className?: string;
}

export default function TerminalFeed({ agentId, interactive = false, onData, onMount, className = '' }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const onDataRef = useRef(onData);
  onDataRef.current = onData;

  useEffect(() => {
    if (!containerRef.current) return;
    const container = containerRef.current;

    const { settings } = useTerminalSettings();
    const theme = getTheme(settings.themeId);
    const xtermTheme = toXtermTheme(theme);
    // Slightly dim background for feed cards
    xtermTheme.background = theme.colors.background;
    xtermTheme.cursor = interactive ? theme.colors.cursor : 'transparent';

    const term = new XTerm({
      theme: xtermTheme,
      fontFamily: settings.fontFamily,
      fontSize: settings.fontSize - 2,
      lineHeight: settings.lineHeight,
      cursorStyle: settings.cursorStyle,
      cursorBlink: interactive && settings.cursorBlink,
      scrollback: interactive ? settings.scrollback : 100,
      allowProposedApi: true,
      disableStdin: !interactive,
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(container);
    setTimeout(() => { try { fit.fit(); } catch {} }, 80);

    if (interactive) {
      term.onData(data => onDataRef.current?.(data));
      term.focus();
    }

    termRef.current = term;
    onMount?.(term);

    const ro = new ResizeObserver(() => { try { fit.fit(); } catch {} });
    ro.observe(container);

    return () => {
      ro.disconnect();
      term.dispose();
      termRef.current = null;
    };
  }, [agentId, interactive, onMount]);

  const write = useCallback((data: string | Uint8Array) => {
    termRef.current?.write(typeof data === 'string' ? data : new TextDecoder().decode(data));
  }, []);

  const clear = useCallback(() => termRef.current?.clear(), []);
  const focus = useCallback(() => termRef.current?.focus(), []);

  useEffect(() => {
    const el = containerRef.current;
    if (el) (el as any).__feed = { write, clear, focus, term: termRef };
  }, [write, clear, focus]);

  return <div ref={containerRef} className={`w-full h-full ${className}`} />;
}
