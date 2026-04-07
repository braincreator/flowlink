import { useState, useCallback, useEffect } from 'react';

export interface TerminalSettings {
  themeId: string;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  cursorStyle: 'block' | 'underline' | 'bar';
  cursorBlink: boolean;
  scrollback: number;
  copyOnSelect: boolean;
  pasteWithMiddleClick: boolean;
  wordSeparators: string;
  scrollSensitivity: number;
  audibleBell: boolean;
  autoReconnect: boolean;
  reconnectDelay: number;
  closeOnDisconnect: boolean;
  notificationOnDisconnect: boolean;
  recordingFormat: 'asciicast' | 'raw';
  autoRecord: boolean;
  socGridLayout: string;
  socAutoAdd: boolean;
  socShowDisconnected: boolean;
  socAlertFlash: number;
  socFeedHeight: 'compact' | 'normal' | 'expanded';
  socStatusInterval: number;
}

const STORAGE_KEY = 'flowlink_terminal_settings';

const defaults: TerminalSettings = {
  themeId: 'flowlink-dark',
  fontFamily: '"SF Mono", "Fira Code", "Cascadia Code", Menlo, monospace',
  fontSize: 14,
  lineHeight: 1.2,
  cursorStyle: 'block',
  cursorBlink: true,
  scrollback: 10000,
  copyOnSelect: false,
  pasteWithMiddleClick: true,
  wordSeparators: ' ()[]{}\'",;',
  scrollSensitivity: 1,
  audibleBell: false,
  autoReconnect: true,
  reconnectDelay: 3000,
  closeOnDisconnect: false,
  notificationOnDisconnect: true,
  recordingFormat: 'asciicast',
  autoRecord: false,
  socGridLayout: '2x2',
  socAutoAdd: true,
  socShowDisconnected: false,
  socAlertFlash: 3,
  socFeedHeight: 'normal',
  socStatusInterval: 15,
};

function load(): TerminalSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...defaults, ...JSON.parse(raw) };
  } catch {}
  return { ...defaults };
}

function save(s: TerminalSettings) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(s)); } catch {}
}

export function useTerminalSettings() {
  const [settings, setSettings] = useState<TerminalSettings>(load);

  useEffect(() => { save(settings); }, [settings]);

  const update = useCallback((patch: Partial<TerminalSettings>) => {
    setSettings(prev => ({ ...prev, ...patch }));
  }, []);

  const reset = useCallback(() => {
    setSettings({ ...defaults });
  }, []);

  return { settings, update, reset };
}
