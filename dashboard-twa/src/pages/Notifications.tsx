import { useEffect, useState } from 'react';
import { api } from '../api/client';

interface Notification {
  id: string;
  type: 'info' | 'success' | 'warning' | 'alert';
  title: string;
  message: string;
  created_at: string;
  read: boolean;
}

export default function Notifications() {
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const loadNotifications = async () => {
    setLoading(true);
    setError('');
    try {
      const data = await api.getNotifications();
      const notifs: Notification[] = (data || []).map((n: any) => ({
        id: n.id || n.notification_id || Math.random().toString(),
        type: n.type || 'info',
        title: n.title || 'Notification',
        message: n.message || n.body || 'No details',
        created_at: n.created_at || n.timestamp || new Date().toISOString(),
        read: n.read !== undefined ? n.read : false,
      }));
      setNotifications(notifs);
    } catch (e: any) {
      setError(e.message || 'Failed to load notifications');
    } finally {
      setLoading(false);
    }
  };

  const markAsRead = async (id: string) => {
    try {
      await api.markNotificationRead(id);
      setNotifications(notifications.map(n => 
        n.id === id ? { ...n, read: true } : n
      ));
    } catch (e: any) {
      console.error('Failed to mark notification as read:', e);
    }
  };

  const markAllAsRead = async () => {
    const unread = notifications.filter(n => !n.read);
    for (const n of unread) {
      try {
        await api.markNotificationRead(n.id);
      } catch (e) {
        console.error('Failed to mark notification as read:', e);
      }
    }
    setNotifications(notifications.map(n => ({ ...n, read: true })));
  };

  useEffect(() => {
    loadNotifications();
  }, []);

  const getTypeIcon = (type: string) => {
    switch (type) {
      case 'info': return 'ℹ️';
      case 'success': return '✅';
      case 'warning': return '⚠️';
      case 'alert': return '🚨';
      default: return '💬';
    }
  };

  const getTypeColor = (type: string) => {
    switch (type) {
      case 'info': return 'bg-blue-500/20';
      case 'success': return 'bg-tg-success/20';
      case 'warning': return 'bg-yellow-500/20';
      case 'alert': return 'bg-tg-danger/20';
      default: return 'bg-tg-hint/20';
    }
  };

  const unreadCount = notifications.filter(n => !n.read).length;

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-tg-button border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-tg-hint mt-3">Loading notifications...</p>
      </div>
    );
  }

  return (
    <div className="px-4 pt-4">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Notifications</h1>
        {unreadCount > 0 && (
          <button
            onClick={markAllAsRead}
            className="text-sm px-3 py-1 rounded-lg bg-tg-button text-tg-button-text font-medium"
          >
            Mark All Read ({unreadCount})
          </button>
        )}
      </div>

      {error && (
        <div className="mb-4 p-3 rounded-lg bg-tg-danger/20 text-tg-danger">
          {error}
        </div>
      )}

      {/* Notifications List */}
      <div className="space-y-3">
        {notifications.length === 0 ? (
          <div className="text-center py-12">
            <span className="text-4xl">📬</span>
            <p className="text-sm text-tg-hint mt-2">No notifications yet</p>
          </div>
        ) : (
          notifications.map((n) => (
            <div
              key={n.id}
              onClick={() => !n.read && markAsRead(n.id)}
              className={`p-4 rounded-lg border border-tg-button/30 cursor-pointer transition-colors ${
                n.read ? 'bg-transparent opacity-60' : 'bg-white'
              }`}
            >
              <div className="flex items-start gap-3">
                <div className={`w-8 h-8 rounded-full flex items-center justify-center ${getTypeColor(n.type)}`}>
                  <span className="text-lg">{getTypeIcon(n.type)}</span>
                </div>
                <div className="flex-1">
                  <p className="font-semibold text-sm">{n.title}</p>
                  <p className="text-sm text-tg-hint mt-1">{n.message}</p>
                  <p className="text-xs text-tg-button-text/60 mt-2">
                    {new Date(n.created_at).toLocaleString('ru-RU')}
                  </p>
                </div>
                {!n.read && (
                  <div className="w-2 h-2 rounded-full bg-tg-button" />
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
