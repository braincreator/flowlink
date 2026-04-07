import { useCallback, useRef } from 'react';
let audioCtx = null;
function getCtx() {
    if (!audioCtx || audioCtx.state === 'closed') {
        audioCtx = new AudioContext();
    }
    if (audioCtx.state === 'suspended')
        audioCtx.resume();
    return audioCtx;
}
const SOUNDS = {
    l3_alert: { freq: 800, dur: 200, type: 'square', repeat: 2, gap: 150 },
    agent_disconnect: { freq: 400, dur: 300, type: 'sine' },
    approval: { freq: 1000, dur: 100, type: 'sine', repeat: 2, gap: 100 },
    info: { freq: 600, dur: 150, type: 'sine' },
};
export function playBeep(type = 'info', volume = 0.1) {
    try {
        const sound = SOUNDS[type];
        const ctx = getCtx();
        const repeats = sound.repeat || 1;
        for (let i = 0; i < repeats; i++) {
            const osc = ctx.createOscillator();
            const gain = ctx.createGain();
            osc.connect(gain);
            gain.connect(ctx.destination);
            osc.frequency.value = sound.freq;
            osc.type = sound.type;
            gain.gain.value = volume;
            const start = ctx.currentTime + i * ((sound.dur + (sound.gap || 0)) / 1000);
            osc.start(start);
            osc.stop(start + sound.dur / 1000);
        }
    }
    catch { }
}
export function useSound() {
    const volumeRef = useRef(0.1);
    const enabledRef = useRef(true);
    const play = useCallback((type) => {
        if (enabledRef.current)
            playBeep(type, volumeRef.current);
    }, []);
    const setVolume = useCallback((v) => { volumeRef.current = v; }, []);
    const setEnabled = useCallback((v) => { enabledRef.current = v; }, []);
    return { play, setVolume, setEnabled };
}
