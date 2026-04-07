import { useCallback, useRef } from 'react';

let audioCtx: AudioContext | null = null;

function getCtx() {
  if (!audioCtx || audioCtx.state === 'closed') {
    audioCtx = new AudioContext();
  }
  if (audioCtx.state === 'suspended') audioCtx.resume();
  return audioCtx;
}

export type SoundType = 'l3_alert' | 'agent_disconnect' | 'approval' | 'info';

const SOUNDS: Record<SoundType, { freq: number; dur: number; type: OscillatorType; repeat?: number; gap?: number }> = {
  l3_alert: { freq: 800, dur: 200, type: 'square', repeat: 2, gap: 150 },
  agent_disconnect: { freq: 400, dur: 300, type: 'sine' },
  approval: { freq: 1000, dur: 100, type: 'sine', repeat: 2, gap: 100 },
  info: { freq: 600, dur: 150, type: 'sine' },
};

export function playBeep(type: SoundType = 'info', volume = 0.1) {
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
  } catch {}
}

export function useSound() {
  const volumeRef = useRef(0.1);
  const enabledRef = useRef(true);

  const play = useCallback((type: SoundType) => {
    if (enabledRef.current) playBeep(type, volumeRef.current);
  }, []);

  const setVolume = useCallback((v: number) => { volumeRef.current = v; }, []);
  const setEnabled = useCallback((v: boolean) => { enabledRef.current = v; }, []);

  return { play, setVolume, setEnabled };
}
