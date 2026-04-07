import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Bell, Check, CheckCheck, Trash2, X, AlertTriangle, Bot, Shield, AlertCircle, Info } from 'lucide-react';
import { useNotifications, type Notification } from '../hooks/useNotifications';
import { useSSE } from '../hooks/useApi';

const typeIcons: Record<string, typeof Bell> = {
  alert: AlertTriangle,
  approval: Shield,
  agent_online: Bot,
  agent_offline: Bot,
  error: AlertCircle,
  info: Info,
};

const typeColors: Record<string, string> = {
  alert: 'text-amber-400',
  approval: 'text-indigo-400',
  agent_online: 'text-emerald-400',
  agent_offline: 'text-rose-400',
  error: 'text-rose-400',
  info: 'text-blue-400',
};

export default function NotificationCenter() {
  const { t } = useTranslation();
  const { notifications, unread, addNotification, markAllRead, markRead, clearAll } = useNotifications();
  const [open, setOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const navigate = useNavigate();
  const { events: sseEvents } = useSSE();

  // Convert SSE events to notifications
  useEffect(() => {
    for (const ev of sseEvents.slice(0, 5)) {
      const eventType = (ev as any).event_type || (ev as any).type || '';
      let type: Notification['type'] = 'info';
      if (eventType === 'command_intercepted' || eventType === 'policy_violation') type = 'alert';
      else if (eventType === 'canary_triggered') type = 'error';
      else if (eventType === 'agent_heartbeat') {
        type = (ev as any).status === 'online' ? 'agent_online' : 'agent_offline';
      }

      addNotification({
        type,
        title: eventType.replace(/_/g, ' '),
        body: (ev as any).command || (ev as any).message || '',
        link: eventType.includes('command') ? '/shield' : eventType.includes('agent') ? '/agents' : undefined,
        risk_score: (ev as any).risk_score,
        level: (ev as any).threat_level,
        agent: (ev as any).hostname || (ev as any).agent_id,
      });
    }
  }, [sseEvents]);

  // Close dropdown on click outside
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  const handleClick = (n: Notification) => {
    markRead(n.id);
    setOpen(false);
    if (n.link) navigate(n.link);
  };

  const recent = notifications.slice(0, 20);

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        onClick={() => setOpen(!open)}
        className="relative flex h-9 w-9 items-center justify-center rounded-xl border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors"
      >
        <Bell size={16} />
        {unread > 0 && (
          <span className="absolute -top-1 -right-1 flex h-5 min-w-[20px] items-center justify-center rounded-full bg-rose-500 px-1 text-[10px] font-bold text-white">
            {unread > 99 ? '99+' : unread}
          </span>
        )}
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-2 w-80 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] shadow-2xl z-50 fade-in">
          {/* Header */}
          <div className="flex items-center justify-between border-b border-[var(--color-border)] px-4 py-3">
            <h3 className="text-sm font-semibold">{t('settings.notifications')}</h3>
            <div className="flex items-center gap-1">
              {unread > 0 && (
                <button onClick={markAllRead} className="rounded-md p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-emerald-400 transition-colors" title={t('common.mark_all_read')}>
                  <CheckCheck size={14} />
                </button>
              )}
              <button onClick={clearAll} className="rounded-md p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-rose-400 transition-colors" title={t('common.clear_all')}>
                <Trash2 size={14} />
              </button>
            </div>
          </div>

          {/* List */}
          <div className="max-h-96 overflow-y-auto">
            {recent.length === 0 ? (
              <div className="flex flex-col items-center py-8 text-center">
                <Bell size={24} className="mb-2 text-[var(--color-dim)] opacity-40" />
                <p className="text-sm text-[var(--color-dim)]">{t('common.no_notifications')}</p>
              </div>
            ) : recent.map(n => {
              const Icon = typeIcons[n.type] || Info;
              const color = typeColors[n.type] || 'text-[var(--color-dim)]';
              const timeAgo = getTimeAgo(n.timestamp);
              return (
                <button
                  key={n.id}
                  onClick={() => handleClick(n)}
                  className={`w-full text-left px-4 py-3 border-b border-[var(--color-border)] hover:bg-[var(--color-surface2)] transition-colors ${!n.read ? 'bg-[var(--color-accent)]/5' : ''}`}
                >
                  <div className="flex items-start gap-3">
                    <Icon size={14} className={`mt-0.5 flex-shrink-0 ${color}`} />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium truncate">{n.title}</span>
                        {!n.read && <span className="h-2 w-2 flex-shrink-0 rounded-full bg-[var(--color-accent)]" />}
                      </div>
                      <p className="text-xs text-[var(--color-dim)] truncate mt-0.5">{n.body}</p>
                      <div className="flex items-center gap-2 mt-1">
                        <span className="text-[10px] text-[var(--color-dim)] opacity-60">{timeAgo}</span>
                        {n.risk_score && (
                          <span className={`text-[10px] font-mono ${n.risk_score >= 70 ? 'text-rose-400' : 'text-amber-400'}`}>
                            risk:{n.risk_score}
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

function getTimeAgo(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 60000) return 'just now';
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
  return `${Math.floor(diff / 86400000)}d ago`;
}
