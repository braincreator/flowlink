import { jsx as _jsx } from "react/jsx-runtime";
import { useRef, useEffect } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { toXtermTheme } from './themes';
import '@xterm/xterm/css/xterm.css';
const SAMPLE = [
    '\x1b[1;34m$ ls -la\x1b[0m',
    'drwxr-xr-x  2 root root 4096 Apr  7 \x1b[1;34m.\x1b[0m',
    '-rw-r--r--  1 root root 1234 Apr  7 file.txt',
    '\x1b[1;34m$ echo \x1b[33m"Hello, FlowLink!"\x1b[0m',
    '\x1b[32mHello, FlowLink!\x1b[0m',
].join('\r\n');
export default function ThemePreview({ theme, className = '' }) {
    const containerRef = useRef(null);
    const termRef = useRef(null);
    useEffect(() => {
        if (!containerRef.current)
            return;
        const container = containerRef.current;
        const term = new XTerm({
            theme: toXtermTheme(theme),
            fontFamily: '"SF Mono", "Fira Code", Menlo, monospace',
            fontSize: 11,
            lineHeight: 1.3,
            scrollback: 0,
            allowProposedApi: true,
            disableStdin: true,
            cursorBlink: false,
            cursorStyle: 'block',
        });
        const fit = new FitAddon();
        term.loadAddon(fit);
        term.open(container);
        setTimeout(() => {
            try {
                fit.fit();
            }
            catch { }
            term.write(SAMPLE);
        }, 30);
        termRef.current = term;
        const ro = new ResizeObserver(() => { try {
            fit.fit();
        }
        catch { } });
        ro.observe(container);
        return () => {
            ro.disconnect();
            term.dispose();
            termRef.current = null;
        };
    }, [theme]);
    return (_jsx("div", { ref: containerRef, className: `rounded-lg overflow-hidden ${className}`, style: { minHeight: '120px' } }));
}
