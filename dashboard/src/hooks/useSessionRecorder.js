import { useRef, useCallback, useState } from 'react';
export function useSessionRecorder() {
    const [recording, setRecording] = useState(false);
    const [progress, setProgress] = useState(0);
    const abortRef = useRef(false);
    const containerRef = useRef(null);
    const recordSession = useCallback(async (castData, options = {}) => {
        setRecording(true);
        setProgress(0);
        abortRef.current = false;
        const { speed = 1, fps = 30, format = 'webm', } = options;
        return new Promise(async (resolve, reject) => {
            try {
                // 1. Parse cast file
                const lines = castData.trim().split('\n');
                const header = JSON.parse(lines[0]);
                const events = lines.slice(1)
                    .map(l => JSON.parse(l))
                    .filter((e) => e[1] === 'o');
                if (events.length === 0) {
                    setRecording(false);
                    reject(new Error('No events in cast file'));
                    return;
                }
                const totalDuration = events[events.length - 1][0];
                // 2. Create offscreen terminal
                const container = document.createElement('div');
                container.style.position = 'fixed';
                container.style.left = '-9999px';
                container.style.top = '0';
                container.style.width = `${(header.width || 80) * (options.fontSize || 16) * 0.6}px`;
                container.style.height = `${(header.height || 24) * (options.fontSize || 16) * 1.3}px`;
                container.style.zIndex = '-1';
                document.body.appendChild(container);
                containerRef.current = container;
                const { Terminal } = await import('@xterm/xterm');
                // xterm CSS should already be loaded by the app
                // @ts-ignore
                const term = new Terminal({
                    cols: header.width || 80,
                    rows: header.height || 24,
                    fontSize: options.fontSize || 16,
                    fontFamily: options.fontFamily || 'JetBrains Mono, Fira Code, SF Mono, Menlo, monospace',
                    theme: options.theme || undefined,
                    scrollback: 0,
                    convertEol: true,
                });
                term.open(container);
                // 3. Get canvas
                const canvas = container.querySelector('canvas');
                if (!canvas) {
                    term.dispose();
                    document.body.removeChild(container);
                    setRecording(false);
                    reject(new Error('No canvas found in terminal'));
                    return;
                }
                // 4. Setup MediaRecorder
                const stream = canvas.captureStream(fps);
                let mimeType = 'video/webm;codecs=vp9';
                if (!MediaRecorder.isTypeSupported(mimeType))
                    mimeType = 'video/webm;codecs=vp8';
                if (!MediaRecorder.isTypeSupported(mimeType))
                    mimeType = 'video/webm';
                const recorder = new MediaRecorder(stream, {
                    mimeType,
                    videoBitsPerSecond: 2_500_000,
                });
                const chunks = [];
                recorder.ondataavailable = (e) => {
                    if (e.data.size > 0)
                        chunks.push(e.data);
                };
                recorder.onstop = () => {
                    const blob = new Blob(chunks, { type: mimeType });
                    const url = URL.createObjectURL(blob);
                    term.dispose();
                    if (container.parentNode)
                        document.body.removeChild(container);
                    containerRef.current = null;
                    setRecording(false);
                    setProgress(100);
                    resolve({
                        blob,
                        url,
                        duration: totalDuration / speed,
                        size: blob.size,
                        format: 'webm',
                    });
                };
                // 5. Start recording
                recorder.start(100);
                // 6. Playback events with speed control
                const startTime = performance.now();
                let eventIndex = 0;
                const playbackFrame = () => {
                    if (abortRef.current) {
                        recorder.stop();
                        return;
                    }
                    if (eventIndex >= events.length) {
                        setTimeout(() => recorder.stop(), 600);
                        return;
                    }
                    const elapsed = (performance.now() - startTime) / 1000 * speed;
                    while (eventIndex < events.length && events[eventIndex][0] <= elapsed) {
                        term.write(events[eventIndex][2]);
                        eventIndex++;
                    }
                    setProgress(Math.round((eventIndex / events.length) * 100));
                    requestAnimationFrame(playbackFrame);
                };
                // Small delay to let terminal render initial frame
                await new Promise(r => setTimeout(r, 150));
                requestAnimationFrame(playbackFrame);
            }
            catch (err) {
                if (containerRef.current?.parentNode) {
                    document.body.removeChild(containerRef.current);
                    containerRef.current = null;
                }
                setRecording(false);
                reject(err);
            }
        });
    }, []);
    const cancelRecording = useCallback(() => {
        abortRef.current = true;
        setRecording(false);
        setProgress(0);
    }, []);
    const downloadRecording = useCallback((result, filename) => {
        const a = document.createElement('a');
        a.href = result.url;
        a.download = filename || `session-${Date.now()}.webm`;
        a.click();
        // Revoke after a delay
        setTimeout(() => URL.revokeObjectURL(result.url), 30_000);
    }, []);
    const shareToTelegram = useCallback(async (result, chatId) => {
        // TWA: use WebApp share, otherwise download
        const tw = window.Telegram?.WebApp;
        if (tw) {
            try {
                const file = new File([result.blob], 'session.webm', { type: result.blob.type });
                if (navigator.share && navigator.canShare?.({ files: [file] })) {
                    await navigator.share({ files: [file] });
                    return;
                }
            }
            catch { }
        }
        // Fallback: download
        downloadRecording(result);
    }, [downloadRecording]);
    return {
        recordSession,
        cancelRecording,
        downloadRecording,
        shareToTelegram,
        recording,
        progress,
    };
}
