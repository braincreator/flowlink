import { jsx as _jsx } from "react/jsx-runtime";
import { useEffect, useRef, useCallback } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import '@xterm/xterm/css/xterm.css';
export default function Terminal({ className = '', onData, onResize }) {
    const containerRef = useRef(null);
    const termRef = useRef(null);
    const fitRef = useRef(null);
    const onDataRef = useRef(onData);
    const onResizeRef = useRef(onResize);
    onDataRef.current = onData;
    onResizeRef.current = onResize;
    useEffect(() => {
        if (!containerRef.current)
            return;
        const container = containerRef.current;
        const term = new XTerm({
            theme: {
                background: '#0a0e1a',
                foreground: '#e1e4ed',
                cursor: '#6366f1',
                selectionBackground: '#6366f13d',
                black: '#3b3d57',
                red: '#f43f5e',
                green: '#34d399',
                yellow: '#fbbf24',
                blue: '#60a5fa',
                magenta: '#c084fc',
                cyan: '#22d3ee',
                white: '#e1e4ed',
                brightBlack: '#6b7194',
                brightRed: '#fb7185',
                brightGreen: '#6ee7b7',
                brightYellow: '#fde68a',
                brightBlue: '#93c5fd',
                brightMagenta: '#d8b4fe',
                brightCyan: '#67e8f9',
                brightWhite: '#f1f5f9',
            },
            fontFamily: '"SF Mono", "Fira Code", "Cascadia Code", Menlo, monospace',
            fontSize: 14,
            lineHeight: 1.4,
            cursorBlink: true,
            scrollback: 10000,
            allowProposedApi: true,
        });
        const fit = new FitAddon();
        const webLinks = new WebLinksAddon();
        term.loadAddon(fit);
        term.loadAddon(webLinks);
        term.open(container);
        // Delay fit to ensure container is rendered
        setTimeout(() => {
            try {
                fit.fit();
            }
            catch { }
            onResizeRef.current?.(term.cols, term.rows);
        }, 50);
        term.onData(data => onDataRef.current?.(data));
        term.onResize(({ cols, rows }) => onResizeRef.current?.(cols, rows));
        // Custom right-click for paste
        container.addEventListener('contextmenu', (e) => {
            e.preventDefault();
            const ce = e;
            if (ce.clipboardData) {
                const text = ce.clipboardData.getData('text');
                if (text)
                    term.write(text);
            }
        });
        termRef.current = term;
        fitRef.current = fit;
        const ro = new ResizeObserver(() => {
            try {
                fit.fit();
            }
            catch { }
        });
        ro.observe(container);
        return () => {
            ro.disconnect();
            term.dispose();
            termRef.current = null;
            fitRef.current = null;
        };
    }, []);
    const write = useCallback((data) => {
        if (termRef.current) {
            termRef.current.write(typeof data === 'string' ? data : new TextDecoder().decode(data));
        }
    }, []);
    const clear = useCallback(() => termRef.current?.clear(), []);
    const focus = useCallback(() => termRef.current?.focus(), []);
    // Expose methods via ref-like pattern
    containerRef.current?.__terminal && (containerRef.current.__terminal = { write, clear, focus });
    return _jsx("div", { ref: containerRef, className: `w-full h-full ${className}`, style: { minHeight: '300px' } });
}
// Helper to get terminal instance from ref
export function getTerminalFromContainer(el) {
    return el?.__terminal || null;
}
