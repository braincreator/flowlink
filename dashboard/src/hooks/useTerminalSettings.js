import { useState, useCallback, useEffect } from 'react';
const STORAGE_KEY = 'flowlink_terminal_settings';
const defaults = {
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
function load() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (raw)
            return { ...defaults, ...JSON.parse(raw) };
    }
    catch { }
    return { ...defaults };
}
function save(s) {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
    }
    catch { }
}
export function useTerminalSettings() {
    const [settings, setSettings] = useState(load);
    useEffect(() => { save(settings); }, [settings]);
    const update = useCallback((patch) => {
        setSettings(prev => ({ ...prev, ...patch }));
    }, []);
    const reset = useCallback(() => {
        setSettings({ ...defaults });
    }, []);
    return { settings, update, reset };
}
