import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Shield, LogOut, Smartphone, Monitor, Tablet, Laptop, Globe, Trash2 } from 'lucide-react';
import { api } from '../api/client';
import { useToast } from '../hooks/useToast';
import TwoFASetup from './2FASetup';

interface AuthSession {
  session_id: string;
  ip: string;
  user_agent: string;
  created_at: string;
  email: string;
}

function parseDevice(ua: string): { name: string; icon: React.ReactNode } {
  if (/iPhone/i.test(ua)) return { name: 'iPhone', icon: <Smartphone size={16} /> };
  if (/iPad/i.test(ua)) return { name: 'iPad', icon: <Tablet size={16} /> };
  if (/Android/i.test(ua)) return { name: 'Android', icon: <Smartphone size={16} /> };
  if (/Macintosh/i.test(ua)) return { name: 'macOS', icon: <Laptop size={16} /> };
  if (/Windows/i.test(ua)) return { name: 'Windows', icon: <Monitor size={16} /> };
  if (/Linux/i.test(ua)) return { name: 'Linux', icon: <Monitor size={16} /> };
  return { name: 'Unknown', icon: <Globe size={16} /> };
}

function parseBrowser(ua: string): string {
  const m = ua.match(/(Firefox|Chrome|Safari|Edge|Opera|YaBrowser)\/[\d.]+/);
  return m ? m[1] : 'Unknown browser';
}

type Tab = 'sessions' | '2fa';

export default function Security() {
  const { t } = useTranslation();
  const toast = useToast();
  const [tab, setTab] = useState<Tab>('sessions');
  const [sessions, setSessions] = useState<AuthSession[]>([]);
  const [currentId, setCurrentId] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [confirmRevoke, setConfirmRevoke] = useState<string | 'all' | null>(null);

  const loadSessions = useCallback(async () => {
    try {
      setLoading(true);
      const data = await api.getAuthSessions();
      setSessions(data.sessions || []);
      setCurrentId(data.current_session_id || '');
    } catch {
      toast.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t, toast]);

  useEffect(() => { loadSessions(); }, [loadSessions]);

  const revokeSession = async (id: string) => {
    try {
      await api.revokeAuthSession(id);
      setSessions(s => s.filter(x => x.session_id !== id));
      setConfirmRevoke(null);
      toast.success('Сессия завершена');
    } catch {
      toast.error(t('common.error'));
    }
  };

  const revokeAllOthers = async () => {
    try {
      await api.revokeOtherAuthSessions();
      setSessions(s => s.filter(x => x.session_id === currentId));
      setConfirmRevoke(null);
      toast.success('Все другие сессии завершены');
    } catch {
      toast.error(t('common.error'));
    }
  };

  const tabs: { key: Tab; label: string }[] = [
    { key: 'sessions', label: t('security.sessions', 'Сессии') },
    { key: '2fa', label: t('security.2fa', '2FA') },
  ];

  const formatDate = (d: string) => new Date(d).toLocaleDateString('ru-RU', { day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit' });

  return (
    <div className="mx-auto max-w-5xl space-y-6">
      <div className="flex items-center gap-3">
        <Shield size={24} className="text-[var(--color-accent)]" />
        <h2 className="text-xl font-bold">{t('security.title', 'Безопасность')}</h2>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 rounded-xl bg-[var(--color-bg)] border border-[var(--color-border)] p-1 w-fit">
        {tabs.map(tb => (
          <button
            key={tb.key}
            onClick={() => setTab(tb.key)}
            className={`rounded-lg px-4 py-2 text-sm font-medium transition-all ${
              tab === tb.key
                ? 'bg-[var(--color-surface)] text-[var(--color-text)] shadow-sm'
                : 'text-[var(--color-dim)] hover:text-[var(--color-text)]'
            }`}
          >
            {tb.label}
          </button>
        ))}
      </div>

      {tab === 'sessions' && (
        <div className="space-y-4">
          {/* Revoke all button */}
          {sessions.filter(s => s.session_id !== currentId).length > 0 && (
            <div className="flex justify-end">
              <button
                onClick={() => setConfirmRevoke('all')}
                className="flex items-center gap-2 rounded-lg border border-rose-500/30 px-4 py-2 text-sm font-medium text-rose-400 transition-all hover:bg-rose-500/10"
              >
                <Trash2 size={16} />
                {t('security.revoke_all', 'Завершить все другие сессии')}
              </button>
            </div>
          )}

          {/* Sessions table */}
          <div className="overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-[var(--color-border)] bg-[var(--color-bg)]">
                  <th className="px-4 py-3 text-left font-medium text-[var(--color-dim)]">{t('security.device', 'Устройство')}</th>
                  <th className="px-4 py-3 text-left font-medium text-[var(--color-dim)]">{t('security.ip', 'IP')}</th>
                  <th className="px-4 py-3 text-left font-medium text-[var(--color-dim)]">{t('security.email', 'Email')}</th>
                  <th className="px-4 py-3 text-left font-medium text-[var(--color-dim)]">{t('security.created', 'Создана')}</th>
                  <th className="px-4 py-3 text-right font-medium text-[var(--color-dim)]"></th>
                </tr>
              </thead>
              <tbody>
                {loading ? (
                  <tr><td colSpan={5} className="px-4 py-8 text-center text-[var(--color-dim)]">{t('common.loading')}</td></tr>
                ) : sessions.length === 0 ? (
                  <tr><td colSpan={5} className="px-4 py-8 text-center text-[var(--color-dim)]">{t('security.no_sessions', 'Нет активных сессий')}</td></tr>
                ) : (
                  sessions.map(s => {
                    const device = parseDevice(s.user_agent);
                    const browser = parseBrowser(s.user_agent);
                    const isCurrent = s.session_id === currentId;
                    return (
                      <tr key={s.session_id} className="border-b border-[var(--color-border)] last:border-0 hover:bg-[var(--color-bg)]/50 transition-colors">
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-3">
                            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-[var(--color-bg)] border border-[var(--color-border)] text-[var(--color-dim)]">
                              {device.icon}
                            </div>
                            <div>
                              <div className="flex items-center gap-2">
                                <span className="font-medium text-[var(--color-text)]">{device.name}</span>
                                {isCurrent && (
                                  <span className="rounded-full bg-green-500/10 px-2 py-0.5 text-[10px] font-semibold text-green-400">
                                    {t('security.current', 'Текущая')}
                                  </span>
                                )}
                              </div>
                              <span className="text-xs text-[var(--color-dim)]">{browser}</span>
                            </div>
                          </div>
                        </td>
                        <td className="px-4 py-3 font-mono text-xs text-[var(--color-dim)]">{s.ip}</td>
                        <td className="px-4 py-3 text-[var(--color-text)]">{s.email}</td>
                        <td className="px-4 py-3 text-[var(--color-dim)]">{formatDate(s.created_at)}</td>
                        <td className="px-4 py-3 text-right">
                          {!isCurrent && (
                            <button
                              onClick={() => setConfirmRevoke(s.session_id)}
                              className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs font-medium text-[var(--color-dim)] transition-all hover:border-rose-500/30 hover:text-rose-400 hover:bg-rose-500/10 ml-auto"
                            >
                              <LogOut size={14} />
                              {t('security.revoke', 'Завершить')}
                            </button>
                          )}
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {tab === '2fa' && <TwoFASetup />}

      {/* Confirmation modal */}
      {confirmRevoke && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm" onClick={() => setConfirmRevoke(null)}>
          <div className="mx-4 w-full max-w-sm rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6 shadow-xl" onClick={e => e.stopPropagation()}>
            <h3 className="mb-2 text-lg font-semibold text-[var(--color-text)]">
              {confirmRevoke === 'all'
                ? t('security.confirm_revoke_all_title', 'Завершить все другие сессии?')
                : t('security.confirm_revoke_title', 'Завершить сессию?')}
            </h3>
            <p className="mb-6 text-sm text-[var(--color-dim)]">
              {confirmRevoke === 'all'
                ? t('security.confirm_revoke_all_desc', 'Все устройства, кроме текущего, будут отключены от аккаунта.')
                : t('security.confirm_revoke_desc', 'Это устройство будет отключено от аккаунта.')}
            </p>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setConfirmRevoke(null)}
                className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm font-medium text-[var(--color-dim)] transition-all hover:bg-[var(--color-bg)] hover:text-[var(--color-text)]"
              >
                {t('common.cancel', 'Отмена')}
              </button>
              <button
                onClick={() => confirmRevoke === 'all' ? revokeAllOthers() : revokeSession(confirmRevoke)}
                className="rounded-lg bg-rose-500 px-4 py-2 text-sm font-medium text-white transition-all hover:bg-rose-400"
              >
                {t('security.confirm', 'Завершить')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
