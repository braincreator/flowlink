import { useState, useCallback, useEffect, createContext, useContext, createElement } from 'react';
import { useSound } from './useSound';
const defaultSettings = {
    browserEnabled: true,
    soundEnabled: true,
    volume: 0.1,
    events: { l3: true, l2: false, agentEvents: true, errors: true },
};
function loadSettings() {
    try {
        const s = localStorage.getItem('flowlink_notif_settings');
        return s ? { ...defaultSettings, ...JSON.parse(s) } : defaultSettings;
    }
    catch {
        return defaultSettings;
    }
}
function saveSettings(s) {
    localStorage.setItem('flowlink_notif_settings', JSON.stringify(s));
}
const NotificationContext = createContext(null);
export const useNotifications = () => {
    const ctx = useContext(NotificationContext);
    if (!ctx)
        throw new Error('useNotifications must be used within NotificationProvider');
    return ctx;
};
export function useNotificationProvider() {
    const [notifications, setNotifications] = useState([]);
    const [settings, setSettings] = useState(loadSettings);
    const { play, setVolume, setEnabled } = useSound();
    useEffect(() => { setVolume(settings.volume); setEnabled(settings.soundEnabled); }, [settings.volume, settings.soundEnabled, setVolume, setEnabled]);
    // Request browser notification permission
    useEffect(() => {
        if (settings.browserEnabled && 'Notification' in window && Notification.permission === 'default') {
            Notification.requestPermission();
        }
    }, [settings.browserEnabled]);
    useEffect(() => { saveSettings(settings); }, [settings]);
    const addNotification = useCallback((n) => {
        const notif = { ...n, id: `notif_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`, timestamp: Date.now(), read: false };
        setNotifications(prev => [notif, ...prev].slice(0, 100));
        // Browser notification for critical events
        if (settings.browserEnabled && 'Notification' in window && Notification.permission === 'granted') {
            const isCritical = (notif.risk_score ?? 0) >= 80 || notif.level === 'L3' || notif.type === 'error';
            const matchesEvents = (notif.level === 'L3' && settings.events.l3) ||
                (notif.level === 'L2' && settings.events.l2) ||
                ((notif.type === 'agent_online' || notif.type === 'agent_offline') && settings.events.agentEvents) ||
                (notif.type === 'error' && settings.events.errors) ||
                notif.type === 'approval';
            if (isCritical && matchesEvents) {
                new Notification('FlowLink: ' + notif.title, {
                    body: notif.body,
                    icon: '/favicon.svg',
                    tag: notif.id,
                });
            }
        }
        // Sound
        if (settings.soundEnabled) {
            let sound = 'info';
            if (notif.level === 'L3' && settings.events.l3)
                sound = 'l3_alert';
            else if (notif.type === 'agent_offline' && settings.events.agentEvents)
                sound = 'agent_disconnect';
            else if (notif.type === 'approval')
                sound = 'approval';
            else if (notif.type === 'error' && settings.events.errors)
                sound = 'agent_disconnect';
            play(sound);
        }
    }, [settings, play]);
    const markAllRead = useCallback(() => setNotifications(prev => prev.map(n => ({ ...n, read: true }))), []);
    const markRead = useCallback((id) => setNotifications(prev => prev.map(n => n.id === id ? { ...n, read: true } : n)), []);
    const clearAll = useCallback(() => setNotifications([]), []);
    const updateSettings = useCallback((s) => setSettings(prev => ({ ...prev, ...s })), []);
    const unread = notifications.filter(n => !n.read).length;
    return { notifications, unread, addNotification, markAllRead, markRead, clearAll, settings, updateSettings };
}
export function NotificationProvider({ children }) {
    const value = useNotificationProvider();
    return createElement(NotificationContext.Provider, { value }, children);
}
export { NotificationContext, defaultSettings };
