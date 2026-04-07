import { useState, useRef, useEffect, useCallback } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';

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

    const term = new XTerm({
      theme: {
        background: '#060a14',
        foreground: '#c8cdd8',
        cursor: interactive ? '#6366f1' : 'transparent',
        selectionBackground: '#6366f13d',
        black: '#3b3d57', red: '#f43f5e', green: '#34d399', yellow: '#fbbf24',
        blue: '#60a5fa', magenta: '#c084fc', cyan: '#22d3ee', white: '#e1e4ed',
        brightBlack: '#6b7194', brightRed: '#fb7185', brightGreen: '#6ee7b7',
        brightYellow: '#fde68a', brightBlue: '#93c5fd', brightMagenta: '#d8b4fe',
        brightCyan: '#67e8f9', brightWhite: '#f1f5f9',
      },
      fontFamily: '"SF Mono", "Fira Code", "Cascadia Code", Menlo, monospace',
      fontSize: 12,
      lineHeight: 1.35,
      cursorBlink: interactive,
      scrollback: interactive ? 10000 : 100,
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
