import { useState, useCallback, useEffect, useRef, createContext, useContext, createElement, type ReactNode } from 'react';
import { useSound, type SoundType } from './useSound';

export interface Notification {
  id: string;
  type: 'alert' | 'approval' | 'agent_online' | 'agent_offline' | 'error' | 'info';
  title: string;
  body: string;
  timestamp: number;
  read: boolean;
  link?: string;
  risk_score?: number;
  level?: string;
  agent?: string;
}

interface NotificationSettings {
  browserEnabled: boolean;
  soundEnabled: boolean;
  volume: number;
  events: {
    l3: boolean;
    l2: boolean;
    agentEvents: boolean;
    errors: boolean;
  };
}

const defaultSettings: NotificationSettings = {
  browserEnabled: true,
  soundEnabled: true,
  volume: 0.1,
  events: { l3: true, l2: false, agentEvents: true, errors: true },
};

function loadSettings(): NotificationSettings {
  try {
    const s = localStorage.getItem('flowlink_notif_settings');
    return s ? { ...defaultSettings, ...JSON.parse(s) } : defaultSettings;
  } catch { return defaultSettings; }
}

function saveSettings(s: NotificationSettings) {
  localStorage.setItem('flowlink_notif_settings', JSON.stringify(s));
}

const NotificationContext = createContext<{
  notifications: Notification[];
  unread: number;
  addNotification: (n: Omit<Notification, 'id' | 'timestamp' | 'read'>) => void;
  markAllRead: () => void;
  markRead: (id: string) => void;
  clearAll: () => void;
  settings: NotificationSettings;
  updateSettings: (s: Partial<NotificationSettings>) => void;
} | null>(null);

export const useNotifications = () => {
  const ctx = useContext(NotificationContext);
  if (!ctx) throw new Error('useNotifications must be used within NotificationProvider');
  return ctx;
};

export function useNotificationProvider() {
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [settings, setSettings] = useState<NotificationSettings>(loadSettings);
  const { play, setVolume, setEnabled } = useSound();

  useEffect(() => { setVolume(settings.volume); setEnabled(settings.soundEnabled); }, [settings.volume, settings.soundEnabled, setVolume, setEnabled]);

  // Request browser notification permission
  useEffect(() => {
    if (settings.browserEnabled && 'Notification' in window && Notification.permission === 'default') {
      Notification.requestPermission();
    }
  }, [settings.browserEnabled]);

  useEffect(() => { saveSettings(settings); }, [settings]);

  const addNotification = useCallback((n: Omit<Notification, 'id' | 'timestamp' | 'read'>) => {
    const notif: Notification = { ...n, id: `notif_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`, timestamp: Date.now(), read: false };
    setNotifications(prev => [notif, ...prev].slice(0, 100));

    // Browser notification for critical events
    if (settings.browserEnabled && 'Notification' in window && Notification.permission === 'granted') {
      const isCritical = (notif.risk_score ?? 0) >= 80 || notif.level === 'L3' || notif.type === 'error';
      const matchesEvents =
        (notif.level === 'L3' && settings.events.l3) ||
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
      let sound: SoundType = 'info';
      if (notif.level === 'L3' && settings.events.l3) sound = 'l3_alert';
      else if (notif.type === 'agent_offline' && settings.events.agentEvents) sound = 'agent_disconnect';
      else if (notif.type === 'approval') sound = 'approval';
      else if (notif.type === 'error' && settings.events.errors) sound = 'agent_disconnect';
      play(sound);
    }
  }, [settings, play]);

  const markAllRead = useCallback(() => setNotifications(prev => prev.map(n => ({ ...n, read: true }))), []);
  const markRead = useCallback((id: string) => setNotifications(prev => prev.map(n => n.id === id ? { ...n, read: true } : n)), []);
  const clearAll = useCallback(() => setNotifications([]), []);
  const updateSettings = useCallback((s: Partial<NotificationSettings>) => setSettings(prev => ({ ...prev, ...s })), []);

  const unread = notifications.filter(n => !n.read).length;

  return { notifications, unread, addNotification, markAllRead, markRead, clearAll, settings, updateSettings };
}

export function NotificationProvider({ children }: { children: ReactNode }) {
  const value = useNotificationProvider();
  return createElement(NotificationContext.Provider, { value }, children);
}

export { NotificationContext, defaultSettings };
