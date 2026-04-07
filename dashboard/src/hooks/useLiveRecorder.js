import { useRef, useCallback, useState, useEffect } from 'react';
export function useLiveRecorder(terminalContainerRef) {
    const [recording, setRecording] = useState(false);
    const [duration, setDuration] = useState(0);
    const recorderRef = useRef(null);
    const chunksRef = useRef([]);
    const startTimeRef = useRef(0);
    const timerRef = useRef(0);
    const startRecording = useCallback((fps = 30) => {
        const container = terminalContainerRef?.current;
        if (!container)
            return;
        const canvas = container.querySelector('canvas');
        if (!canvas)
            return;
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
        chunksRef.current = [];
        recorder.ondataavailable = (e) => {
            if (e.data.size > 0)
                chunksRef.current.push(e.data);
        };
        recorder.start(100);
        recorderRef.current = recorder;
        startTimeRef.current = Date.now();
        setRecording(true);
        setDuration(0);
        timerRef.current = window.setInterval(() => {
            setDuration(Math.round((Date.now() - startTimeRef.current) / 1000));
        }, 1000);
    }, [terminalContainerRef]);
    const stopRecording = useCallback(() => {
        if (!recorderRef.current || recorderRef.current.state === 'inactive')
            return null;
        clearInterval(timerRef.current);
        return new Promise((resolve) => {
            const recorder = recorderRef.current;
            recorder.onstop = () => {
                const mimeType = recorder.mimeType;
                const blob = new Blob(chunksRef.current, { type: mimeType });
                const url = URL.createObjectURL(blob);
                const dur = (Date.now() - startTimeRef.current) / 1000;
                setRecording(false);
                recorderRef.current = null;
                resolve({
                    blob,
                    url,
                    duration: dur,
                    size: blob.size,
                    format: 'webm',
                });
            };
            recorder.stop();
        });
    }, []);
    // Cleanup on unmount
    useEffect(() => {
        return () => {
            clearInterval(timerRef.current);
            if (recorderRef.current?.state === 'recording') {
                recorderRef.current.stop();
            }
        };
    }, []);
    return { startRecording, stopRecording, recording, duration };
}
