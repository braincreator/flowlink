import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useAuth } from '../hooks/useAuth';
import { api } from '../api/client';
import { useToast } from '../hooks/useToast';
import { User, Mail, Shield, Calendar, LogOut, Link2, CheckCircle } from 'lucide-react';

interface UserInfo {
  account_id: string;
  email?: string;
  name?: string;
  sub: string;
  exp?: number;
  active?: boolean;
  plan?: string;
  created_at?: string;
  last_login?: string;
}

export default function Profile() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { logout } = useAuth();
  const { success, error: showToastError } = useToast();
  const [user, setUser] = useState<UserInfo | null>(null);
  const [accountInfo, setAccountInfo] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [linkEmail, setLinkEmail] = useState('');
  const [linkingEmail, setLinkingEmail] = useState(false);
  const [linkEmailError, setLinkEmailError] = useState('');
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const [twoFAEnabled, setTwoFAEnabled] = useState(false);

  useEffect(() => {
    loadProfile();
    api.get2FAStatus().then(d => setTwoFAEnabled(d.enabled)).catch(() => {});
  }, []);

  const loadProfile = async () => {
    setLoading(true);
    setError('');
    try {
      const [meRes, accountRes] = await Promise.allSettled([
        api.getAuthMe(),
        api.getAccountInfo(),
      ]);
      if (meRes.status === 'fulfilled') setUser(meRes.value);
      if (accountRes.status === 'fulfilled') setAccountInfo(accountRes.value);
    } catch {
      setError(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleLinkEmail = async () => {
    if (!linkEmail.includes('@')) return;
    setLinkingEmail(true);
    setLinkEmailError('');
    try {
      const res = await api.linkEmail(linkEmail);
      if (res.ok) {
        success('Email привязан', linkEmail);
        setLinkEmail('');
        loadProfile(); // refresh to show new email
      }
    } catch {
      setLinkEmailError('Не удалось привязать email');
    } finally {
      setLinkingEmail(false);
    }
  };

  const handleDeleteAccount = async () => {
    setDeleting(true);
    try {
      await api.deleteAccount();
      showToastError('Аккаунт деактивирован');
      logout();
      window.location.href = '/dashboard/login';
    } catch {
      showToastError('Не удалось удалить аккаунт');
    } finally {
      setDeleting(false);
    }
  };

  const handleLogout = async () => {
    try {
      const refreshToken = localStorage.getItem('flowlink_refresh_token');
      await fetch(`${api.getApiBase()}/api/auth/logout`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(api.getToken() ? { Authorization: `Bearer ${api.getToken()}` } : {}),
        },
        body: JSON.stringify({ refresh_token: refreshToken || '' }),
      });
    } catch { /* ignore */ }
    logout();
    window.location.href = '/dashboard/login';
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center p-12">
        <div className="animate-spin h-8 w-8 border-2 border-[var(--color-accent)] border-t-transparent rounded-full" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">{t('profile.title', 'Профиль')}</h1>
        <button
          onClick={handleLogout}
          className="flex items-center gap-2 rounded-lg border border-rose-500/20 bg-rose-500/10 px-3 py-2 text-sm text-rose-400 transition-colors hover:bg-rose-500/20"
        >
          <LogOut size={14} />
          {t('common.sign_out', 'Выйти')}
        </button>
      </div>

      {error && (
        <div className="rounded-lg bg-rose-500/10 border border-rose-500/20 px-4 py-3 text-sm text-rose-400">
          {error}
        </div>
      )}

      {/* User card */}
      <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6">
        <div className="flex items-start gap-4">
          <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-indigo-600 text-xl font-bold text-white">
            {(user?.name || user?.email || '?')[0].toUpperCase()}
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="text-lg font-semibold truncate">
              {user?.name || user?.email || t('profile.unknown', 'Пользователь')}
            </h2>
            {user?.email && (
              <p className="flex items-center gap-1.5 text-sm text-[var(--color-dim)]">
                <Mail size={12} />
                {user.email}
              </p>
            )}
            <p className="mt-1 text-xs text-[var(--color-dim)]">
              ID: {user?.account_id || '—'}
            </p>
          </div>
          <div className={`rounded-full px-2.5 py-1 text-xs font-medium ${
            user?.active !== false
              ? 'bg-emerald-500/10 text-emerald-400'
              : 'bg-rose-500/10 text-rose-400'
          }`}>
            {user?.active !== false ? t('profile.active', 'Активен') : t('profile.inactive', 'Неактивен')}
          </div>
        </div>
      </div>

      {/* 2FA Status */}
      <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className={`flex h-10 w-10 items-center justify-center rounded-xl ${twoFAEnabled ? 'bg-green-500/10 text-green-400' : 'bg-[var(--color-bg)] text-[var(--color-dim)]'}`}>
              <Shield size={18} />
            </div>
            <div>
              <p className="text-sm font-medium">{t('2fa.title', 'Двухфакторная аутентификация')}</p>
              <p className={`text-xs ${twoFAEnabled ? 'text-green-400' : 'text-[var(--color-dim)]'}`}>
                {twoFAEnabled ? t('2fa.enabled', 'Включена') : t('2fa.disabled', 'Отключена')}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {twoFAEnabled && <CheckCircle size={16} className="text-green-400" />}
            <button
              onClick={() => navigate('/settings/2fa')}
              className="rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs font-medium text-[var(--color-text)] transition-colors hover:bg-[var(--color-surface2)]"
            >
              {twoFAEnabled ? t('common.settings', 'Настройки') : t('2fa.setup', 'Настроить')}
            </button>
          </div>
        </div>
      </div>

      {/* Email linking (for OAuth users without email) */}
      {!user?.email && (
        <div className="rounded-2xl border border-amber-500/20 bg-amber-500/5 p-5">
          <h3 className="flex items-center gap-2 text-sm font-semibold mb-2 text-amber-400">
            <Link2 size={14} />
            {t('profile.link_email_title', 'Привяжите email')}
          </h3>
          <p className="text-xs text-[var(--color-dim)] mb-3">
            {t('profile.link_email_desc', 'Email нужен для восстановления доступа к аккаунту')}
          </p>
          <div className="flex gap-2">
            <input
              type="email"
              value={linkEmail}
              onChange={e => setLinkEmail(e.target.value)}
              placeholder="you@example.com"
              className="flex-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none"
            />
            <button
              onClick={handleLinkEmail}
              disabled={linkingEmail || !linkEmail.includes('@')}
              className="rounded-lg bg-amber-500/20 px-3 py-2 text-sm font-medium text-amber-400 transition-colors hover:bg-amber-500/30 disabled:opacity-50"
            >
              {linkingEmail ? '...' : t('profile.link', 'Привязать')}
            </button>
          </div>
          {linkEmailError && (
            <p className="mt-2 text-xs text-rose-400">{linkEmailError}</p>
          )}
        </div>
      )}

      {/* Details grid */}
      <div className="grid gap-4 sm:grid-cols-2">
        {/* Account info */}
        {accountInfo && (
          <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
            <h3 className="flex items-center gap-2 text-sm font-semibold mb-4">
              <User size={14} />
              {t('profile.account', 'Аккаунт')}
            </h3>
            <dl className="space-y-3 text-sm">
              <div className="flex justify-between">
                <dt className="text-[var(--color-dim)]">{t('profile.plan', 'Тариф')}</dt>
                <dd className="font-medium capitalize">{accountInfo.plan || 'Free'}</dd>
              </div>
              {accountInfo.created_at && (
                <div className="flex justify-between">
                  <dt className="text-[var(--color-dim)]">{t('profile.created', 'Создан')}</dt>
                  <dd className="font-medium">{new Date(accountInfo.created_at).toLocaleDateString()}</dd>
                </div>
              )}
              {accountInfo.last_login && (
                <div className="flex justify-between">
                  <dt className="flex items-center gap-1 text-[var(--color-dim)]">
                    <Calendar size={12} />
                    {t('profile.last_login', 'Последний вход')}
                  </dt>
                  <dd className="font-medium">
                    {(() => {
                      const d = new Date(accountInfo.last_login);
                      const now = new Date();
                      const diffMs = now.getTime() - d.getTime();
                      const diffMin = Math.floor(diffMs / 60000);
                      if (diffMin < 1) return t('profile.just_now', 'Только что');
                      if (diffMin < 60) return t('profile.minutes_ago', '{{m}} мин назад', { m: diffMin });
                      const diffH = Math.floor(diffMin / 60);
                      if (diffH < 24) return t('profile.hours_ago', '{{h}}ч назад', { h: diffH });
                      return d.toLocaleDateString();
                    })()}
                  </dd>
                </div>
              )}
            </dl>
          </div>
        )}

        {/* Session info */}
        <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <h3 className="flex items-center gap-2 text-sm font-semibold mb-4">
            <Shield size={14} />
            {t('profile.session', 'Сессия')}
          </h3>
          <dl className="space-y-3 text-sm">
            <div className="flex justify-between">
              <dt className="text-[var(--color-dim)]">{t('profile.provider', 'Провайдер')}</dt>
              <dd className="font-medium">
                {user?.email ? 'Email' : 'API Token'}
              </dd>
            </div>
            {user?.exp && (
              <div className="flex justify-between">
                <dt className="text-[var(--color-dim)]">{t('profile.token_expires', 'Токен истекает')}</dt>
                <dd className="font-medium">
                  {new Date(user.exp * 1000).toLocaleString()}
                </dd>
              </div>
            )}
            <div className="flex justify-between">
              <dt className="text-[var(--color-dim)]">{t('profile.token_refresh', 'Автообновление')}</dt>
              <dd className={`font-medium ${
                localStorage.getItem('flowlink_refresh_token') ? 'text-emerald-400' : 'text-[var(--color-dim)]'
              }`}>
                {localStorage.getItem('flowlink_refresh_token')
                  ? t('profile.enabled', 'Включено')
                  : t('profile.disabled', 'Выключено')}
              </dd>
            </div>
          </dl>
        </div>
      </div>

      {/* Danger Zone */}
      <div className="rounded-2xl border border-rose-500/20 bg-rose-500/5 p-5">
        <h3 className="text-sm font-semibold text-rose-400 mb-2">{t('profile.danger_zone', 'Опасная зона')}</h3>
        <p className="text-xs text-[var(--color-dim)] mb-3">
          {t('profile.delete_desc', 'Деактивация аккаунта необратима. Все данные будут удалены.')}
        </p>
        {!showDeleteConfirm ? (
          <button
            onClick={() => setShowDeleteConfirm(true)}
            className="rounded-lg border border-rose-500/30 px-3 py-2 text-sm text-rose-400 transition-colors hover:bg-rose-500/10"
          >
            {t('profile.delete_account', 'Удалить аккаунт')}
          </button>
        ) : (
          <div className="flex items-center gap-3">
            <span className="text-xs text-rose-400">{t('profile.delete_confirm', 'Вы уверены?')}</span>
            <button
              onClick={handleDeleteAccount}
              disabled={deleting}
              className="rounded-lg bg-rose-500 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-rose-600 disabled:opacity-50"
            >
              {deleting ? '...' : t('profile.yes_delete', 'Да, удалить')}
            </button>
            <button
              onClick={() => setShowDeleteConfirm(false)}
              className="rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs font-medium text-[var(--color-dim)] transition-colors hover:bg-[var(--color-surface2)]"
            >
              {t('common.cancel', 'Отмена')}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
