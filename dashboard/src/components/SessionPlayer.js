import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { Play, Pause, Maximize2 } from 'lucide-react';
export default function SessionPlayer({ castData, className = '', theme, fontSize = 14, autoPlay = false, }) {
    const containerRef = useRef(null);
    const termRef = useRef(null);
    const [playing, setPlaying] = useState(autoPlay);
    const [speed, setSpeed] = useState(1);
    const [loaded, setLoaded] = useState(false);
    const [seek, setSeek] = useState(0);
    const rafRef = useRef(0);
    const startTimeRef = useRef(0);
    const pauseOffsetRef = useRef(0);
    const events = useMemo(() => {
        try {
            const lines = castData.trim().split('\n');
            const header = JSON.parse(lines[0]);
            const evts = lines.slice(1).map(l => JSON.parse(l)).filter(e => e[1] === 'o');
            return { header, events: evts, totalDuration: evts.length > 0 ? evts[evts.length - 1][0] : 0 };
        }
        catch {
            return null;
        }
    }, [castData]);
    // Init terminal
    useEffect(() => {
        if (!containerRef.current || !events)
            return;
        let disposed = false;
        (async () => {
            const { Terminal } = await import('@xterm/xterm');
            // xterm CSS should already be loaded
            // @ts-ignore
            if (disposed || !containerRef.current)
                return;
            const term = new Terminal({
                cols: events.header.width || 80,
                rows: events.header.height || 24,
                fontSize,
                fontFamily: 'JetBrains Mono, Fira Code, SF Mono, Menlo, monospace',
                theme: theme || undefined,
                scrollback: 1000,
                convertEol: true,
            });
            term.open(containerRef.current);
            termRef.current = term;
            setLoaded(true);
        })();
        return () => {
            disposed = true;
            if (termRef.current) {
                termRef.current.dispose();
                termRef.current = null;
            }
        };
    }, [events, theme, fontSize]);
    // Playback loop
    const stopPlayback = useCallback(() => {
        if (rafRef.current)
            cancelAnimationFrame(rafRef.current);
    }, []);
    const startPlayback = useCallback((fromOffset = 0) => {
        if (!events || !termRef.current)
            return;
        stopPlayback();
        // Reset terminal if starting from beginning
        if (fromOffset === 0) {
            termRef.current.clear();
            termRef.current.reset?.();
        }
        startTimeRef.current = performance.now();
        pauseOffsetRef.current = fromOffset;
        let eventIndex = 0;
        // Skip events before offset
        if (fromOffset > 0) {
            while (eventIndex < events.events.length && events.events[eventIndex][0] <= fromOffset) {
                termRef.current.write(events.events[eventIndex][2]);
                eventIndex++;
            }
        }
        const frame = () => {
            if (!playing)
                return;
            const elapsed = pauseOffsetRef.current + (performance.now() - startTimeRef.current) / 1000 * speed;
            while (eventIndex < events.events.length && events.events[eventIndex][0] <= elapsed) {
                termRef.current?.write(events.events[eventIndex][2]);
                eventIndex++;
            }
            setSeek(Math.min(elapsed, events.totalDuration));
            if (eventIndex < events.events.length) {
                rafRef.current = requestAnimationFrame(frame);
            }
            else {
                setPlaying(false);
            }
        };
        rafRef.current = requestAnimationFrame(frame);
    }, [events, speed, playing, stopPlayback]);
    // Toggle play/pause
    useEffect(() => {
        if (!loaded || !events)
            return;
        if (playing) {
            startPlayback(seek);
        }
        else {
            // Save current position as offset
            stopPlayback();
            pauseOffsetRef.current = seek;
        }
        return stopPlayback;
    }, [playing, loaded]);
    // Update seek on speed change
    useEffect(() => {
        if (playing && loaded) {
            pauseOffsetRef.current = seek;
            startPlayback(seek);
        }
    }, [speed]);
    if (!events)
        return null;
    const speedOptions = [0.5, 1, 2, 4];
    return (_jsxs("div", { className: `rounded-xl border border-white/[0.06] bg-[#0d1117] overflow-hidden ${className}`, children: [_jsx("div", { ref: containerRef, className: "min-h-[200px]" }), loaded && (_jsxs("div", { className: "flex items-center gap-3 border-t border-white/[0.06] bg-white/[0.02] px-3 py-2", children: [_jsx("button", { onClick: () => {
                            if (seek >= events.totalDuration) {
                                setSeek(0);
                                setPlaying(true);
                            }
                            else {
                                setPlaying(p => !p);
                            }
                        }, className: "rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-white/5 hover:text-white transition-colors", children: playing ? _jsx(Pause, { size: 14 }) : _jsx(Play, { size: 14 }) }), _jsx("input", { type: "range", min: 0, max: events.totalDuration, step: 0.1, value: seek, onChange: (e) => {
                            const val = parseFloat(e.target.value);
                            setSeek(val);
                            pauseOffsetRef.current = val;
                            if (playing)
                                startPlayback(val);
                        }, className: "flex-1 h-1 accent-[var(--color-accent)]" }), _jsxs("span", { className: "text-xs text-[var(--color-dim)] tabular-nums min-w-[80px] text-right", children: [formatTime(seek), " / ", formatTime(events.totalDuration)] }), _jsx("select", { value: speed, onChange: (e) => setSpeed(parseFloat(e.target.value)), className: "rounded-lg bg-white/5 border-none text-xs text-[var(--color-dim)] px-2 py-1 outline-none", children: speedOptions.map(s => (_jsxs("option", { value: s, children: [s, "x"] }, s))) }), _jsx("button", { onClick: () => containerRef.current?.parentElement?.requestFullscreen?.(), className: "rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-white/5 hover:text-white transition-colors", children: _jsx(Maximize2, { size: 14 }) })] }))] }));
}
function formatTime(seconds) {
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
}
