import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Zap, Mail, ArrowLeft } from 'lucide-react';
import { api, getRedirectAfterLogin, setRedirectAfterLogin } from '../api/client';
import { useToast } from '../hooks/useToast';
import TwoFAVerify from './2FAVerify';

// ── Yandex OAuth Button — follows official Yandex ID guidelines ──
// https://yandex.com/dev/id/doc/en/codes/buttons-design
function YandexButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        height: 44, fontSize: 15, padding: '0 16px', gap: 8, borderRadius: 8,
        backgroundColor: '#fff', border: '1px solid rgba(0,0,0,0.15)', color: '#000',
        fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif',
        fontWeight: 500, cursor: 'pointer', transition: 'background-color 0.15s, border-color 0.15s',
        lineHeight: 1, whiteSpace: 'nowrap', width: '100%',
      }}
      onMouseOver={e => { e.currentTarget.style.backgroundColor = '#f5f5f5'; e.currentTarget.style.borderColor = 'rgba(0,0,0,0.3)'; }}
      onMouseOut={e => { e.currentTarget.style.backgroundColor = '#fff'; e.currentTarget.style.borderColor = 'rgba(0,0,0,0.15)'; }}
    >
      <svg width="24" height="24" viewBox="0 0 23 23" xmlns="http://www.w3.org/2000/svg" style={{ flexShrink: 0 }}>
        <path d="m23.29,11.66c0,9.16 -2.29,11.44 -11.48,11.44c-9.19,0 -11.49,-2.28 -11.49,-11.44c0,-9.15 2.29,-11.44 11.49,-11.44c9.19,0 11.48,2.29 11.48,11.44z" fill="#262633"/>
        <path clip-rule="evenodd" d="m7.67,19.14l0,-14.18l-2.48,0l0,14.18l2.48,0zm8.21,-13.81c-0.8,-0.25 -1.68,-0.37 -2.62,-0.37l-3.34,0l0,14.2l3.13,0c0.97,0 1.9,-0.17 2.72,-0.48c0.83,-0.31 1.55,-0.77 2.14,-1.38c0.6,-0.61 1.06,-1.36 1.4,-2.24c0.33,-0.9 0.5,-1.92 0.5,-3.08c0,-1.31 -0.15,-2.41 -0.5,-3.3c-0.32,-0.88 -0.79,-1.61 -1.37,-2.16c-0.58,-0.54 -1.27,-0.95 -2.07,-1.19zm-1.2,11.55c-0.55,0.22 -1.16,0.34 -1.83,0.34l0,-0.02l-0.44,0l0,-10.34l0.65,0c0.64,0 1.22,0.08 1.73,0.26c0.53,0.17 0.97,0.46 1.35,0.85c0.38,0.39 0.67,0.92 0.87,1.56c0.21,0.65 0.31,1.45 0.31,2.4c0,0.88 -0.12,1.65 -0.33,2.31c-0.22,0.66 -0.51,1.21 -0.91,1.65c-0.4,0.42 -0.86,0.77 -1.41,0.99z" fill="white" fill-rule="evenodd"/>
      </svg>
      <span style={{ flexShrink: 0 }}>Войти с Яндекс</span>
    </button>
  );
}

function VkIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" style={{ flexShrink: 0 }}>
      <path d="M12.785 16.241s.288-.032.436-.192c.136-.148.132-.428.132-.428s-.02-1.308.588-1.504c.596-.168 1.364 1.26 2.176 1.816.616.42 1.084.328 1.084.328l2.176-.032s1.14-.072.6-.964c-.044-.072-.316-.668-1.628-1.888-1.372-1.276-1.188-1.068.464-3.248.356-.492.816-.384.852-.288a.59.59 0 01.032.336s.492 2.86-1.148 2.716c-.592-.044-.856-.068-.856-.068s-1.308.22-3.088.168c-2.144-.064-3.944.012-3.944.012s-.28.02-.428.16c-.16.148-.128.46-.128.46s-.02 1.364.6 2.048c.604.68 1.784.648 1.784.648h.784z"/>
    </svg>
  );
}

function GitHubIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" style={{ flexShrink: 0 }}>
      <path d="M12 2C6.477 2 2 6.477 2 12c0 4.42 2.865 8.166 6.839 9.489.5.092.682-.217.682-.482 0-.237-.009-.866-.013-1.7-2.782.604-3.369-1.34-3.369-1.34-.454-1.156-1.11-1.462-1.11-1.462-.908-.62.069-.608.069-.608 1.003.07 1.531 1.03 1.531 1.03.892 1.529 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.11-4.555-4.943 0-1.091.39-1.984 1.029-2.683-.103-.253-.446-1.27.098-2.647 0 0 .84-.268 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.294 2.747-1.026 2.747-1.026.546 1.377.203 2.394.1 2.647.64.699 1.028 1.592 1.028 2.683 0 3.842-2.339 4.687-4.566 4.935.359.309.678.919.678 1.852 0 1.336-.012 2.415-.012 2.743 0 .267.18.578.688.48C19.138 20.161 22 16.416 22 12c0-5.523-4.477-10-10-10z"/>
    </svg>
  );
}

type LoginStep = 'choose' | 'email-code' | 'token';

export default function Login() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [step, setStep] = useState<LoginStep>('choose');
  const [token, setToken] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [providers, setProviders] = useState<string[]>([]);

  // Email magic link state
  const [email, setEmail] = useState('');
  const [code, setCode] = useState('');
  const [codeSent, setCodeSent] = useState(false);
  const [emailSent, setEmailSent] = useState(false);
  const [resendTimer, setResendTimer] = useState(0);
  const [twoFATempToken, setTwoFATempToken] = useState<string | null>(null);

  // Check for OAuth 2FA redirect
  useEffect(() => {
    const t = (window as any).__twofa_temp_token;
    if (t) {
      delete (window as any).__twofa_temp_token;
      setTwoFATempToken(t);
    }
  }, []);

  useEffect(() => {
    const base = (import.meta as any).env?.VITE_API_URL || '';
    fetch(`${base}/api/auth/providers`)
      .then(r => r.json())
      .then(d => setProviders(Array.isArray(d.providers) ? d.providers : []))
      .catch(() => {});
  }, []);

  // Resend countdown timer
  useEffect(() => {
    if (resendTimer <= 0) return;
    const id = setTimeout(() => setResendTimer(resendTimer - 1), 1000);
    return () => clearTimeout(id);
  }, [resendTimer]);

  const base = (import.meta as any).env?.VITE_API_URL || '';
  const { warning } = useToast();

  // Show toast if redirected due to expired session
  useEffect(() => {
    const redirect = getRedirectAfterLogin();
    if (redirect) {
      setRedirectAfterLogin(redirect); // restore it
      warning('Сессия истекла', 'Войдите заново');
    }
  }, []);

  const handleOAuth = (provider: string) => {
    const origin = window.location.origin;
    window.location.href = `${base}/api/auth/oauth-url?provider=${provider}&redirect=${origin}`;
  };

  const handleSendCode = async () => {
    if (!email.trim() || !email.includes('@')) return;
    setLoading(true);
    setError('');
    try {
      const res = await fetch(`${base}/api/auth/email/send-code`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: email.trim() }),
      });
      const data = await res.json();
      if (res.ok && data.ok) {
        setCodeSent(true);
        setEmailSent(true);
        setResendTimer(60);
      } else {
        setError(data.error || t('common.error'));
      }
    } catch {
      setError(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleVerifyCode = async () => {
    if (!code.trim()) return;
    setLoading(true);
    setError('');
    try {
      const res = await fetch(`${base}/api/auth/email/verify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: email.trim(), code: code.trim() }),
      });
      const data = await res.json();
      if (res.ok && data.requires_2fa) {
        setTwoFATempToken(data.temp_token);
        return;
      }
      if (res.ok && data.access_token) {
        api.setTokens(data.access_token, data.refresh_token, data.expires_in);
        const redirect = getRedirectAfterLogin();
        navigate(redirect || '/');
      }
    } catch {
      setError(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleResend = () => {
    if (resendTimer > 0) return;
    setCode('');
    setCodeSent(false);
    handleSendCode();
  };

  const handleTokenSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token.trim()) return;
    setLoading(true);
    setError('');
    try {
      api.setToken(token.trim());
      await api.getHealth();
      const redirect = getRedirectAfterLogin();
      navigate(redirect || '/');
    } catch {
      setError(t('common.error'));
      api.setToken(null);
    } finally {
      setLoading(false);
    }
  };

  const handleSkip = () => {
    api.setToken(null);
    navigate(getRedirectAfterLogin() || '/');
  };

  const oauthProviders = providers.filter(p => p !== 'email');
  const hasOAuth = oauthProviders.length > 0;
  const hasEmail = providers.includes('email');

  // ── Email Code Step ──
  if (step === 'email-code') {
    return (
      <div className="flex min-h-screen items-center justify-center bg-[var(--color-bg)] p-4">
        <div className="w-full max-w-sm rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8">
          <button
            onClick={() => setStep('choose')}
            className="mb-4 flex items-center gap-1 text-sm text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors"
          >
            <ArrowLeft size={14} /> {t('common.back')}
          </button>

          <div className="mb-4 flex items-center gap-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-indigo-500/10 text-indigo-400">
              <Mail size={16} />
            </div>
            <div>
              <h2 className="text-base font-semibold">
                {codeSent ? t('login.enter_code') : t('login.send_code_title')}
              </h2>
              <p className="text-xs text-[var(--color-dim)]">{email}</p>
            </div>
          </div>

          {error && (
            <div className="mb-4 rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-sm text-rose-400">
              {error}
            </div>
          )}

          {codeSent ? (
            <div className="space-y-4">
              <p className="text-sm text-[var(--color-dim)]">
                {t('login.code_sent_desc', 'Код отправлен на {{email}}. Проверьте почту.', { email })}
              </p>
              <div>
                <label className="mb-1.5 block text-sm text-[var(--color-dim)]">
                  {t('login.verification_code', 'Код подтверждения')}
                </label>
                <input
                  type="text"
                  inputMode="numeric"
                  maxLength={6}
                  value={code}
                  onChange={e => setCode(e.target.value.replace(/\D/g, ''))}
                  placeholder="000000"
                  autoFocus
                  className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-center font-mono text-lg tracking-[0.5em] placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none"
                />
              </div>
              <button
                onClick={handleVerifyCode}
                disabled={loading || code.length < 6}
                className="w-full rounded-xl bg-[var(--color-accent)] py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-50"
              >
                {loading ? t('common.loading') : t('common.sign_in')}
              </button>
              <p className="text-center text-xs text-[var(--color-dim)]">
                {resendTimer > 0
                  ? t('login.resend_timer', 'Повторная отправка через {{s}}с', { s: resendTimer })
                  : (
                    <button
                      onClick={handleResend}
                      className="text-indigo-400 hover:text-indigo-300 transition-colors"
                    >
                      {t('login.resend_code', 'Отправить код повторно')}
                    </button>
                  )
                }
              </p>
            </div>
          ) : (
            <div className="space-y-4">
              <button
                onClick={handleSendCode}
                disabled={loading}
                className="w-full rounded-xl bg-[var(--color-accent)] py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-50"
              >
                {loading ? t('common.loading') : t('login.send_code', 'Отправить код')}
              </button>
            </div>
          )}
        </div>
        {twoFATempToken && (
          <TwoFAVerify
            tempToken={twoFATempToken}
            onSuccess={() => {
              setTwoFATempToken(null);
              const redirect = getRedirectAfterLogin();
              navigate(redirect || '/');
            }}
            onCancel={() => setTwoFATempToken(null)}
          />
        )}
      </div>
    );
  }

  // ── Main Login Step ──
  return (
    <div className="flex min-h-screen items-center justify-center bg-[var(--color-bg)] p-4">
      <div className="w-full max-w-sm rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8">
        {/* Logo */}
        <div className="mb-6 flex items-center justify-center gap-2.5">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-indigo-600 text-lg font-bold text-white">
            <Zap />
          </div>
          <span className="text-xl font-bold">
            <span className="text-[var(--color-accent-light)]">Flow</span>
            <span className="text-[var(--color-text)]">Link</span>
          </span>
        </div>

        <h2 className="mb-1 text-center text-lg font-semibold">{t("common.sign_in")}</h2>
        <p className="mb-6 text-center text-sm text-[var(--color-dim)]">{t("onboarding.relay_endpoint")}</p>

        {/* ── Email Login (Magic Link / Code) ── */}
        {hasEmail && (
          <>
            <div className="mb-4 space-y-3">
              <div>
                <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t('login.email_label', 'Email')}</label>
                <input
                  type="email"
                  value={email}
                  onChange={e => setEmail(e.target.value)}
                  placeholder="you@example.com"
                  onKeyDown={e => { if (e.key === 'Enter' && email.includes('@') && !codeSent) { e.preventDefault(); setStep('email-code'); } }}
                  className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none"
                />
              </div>
              <button
                onClick={() => { if (email.includes('@')) { setCodeSent(false); setCode(''); setStep('email-code'); } }}
                disabled={!email.includes('@')}
                className="flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--color-accent)] py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-40"
              >
                <Mail size={16} />
                {t('login.continue_with_email', 'Войти по email')}
              </button>
            </div>

            {/* Divider */}
            <div className="relative my-4">
              <div className="absolute inset-0 flex items-center">
                <div className="w-full border-t border-[var(--color-border)]" />
              </div>
              <div className="relative flex justify-center text-xs">
                <span className="bg-[var(--color-surface)] px-2 text-[var(--color-dim)]">или</span>
              </div>
            </div>
          </>
        )}

        {/* ── OAuth Buttons ── */}
        {hasOAuth && (
          <div className="mb-4 space-y-3">
            {providers.includes('yandex') && (
              <YandexButton onClick={() => handleOAuth('yandex')} />
            )}
            {providers.includes('vk') && (
              <button
                onClick={() => handleOAuth('vk')}
                className="flex w-full items-center justify-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] py-2.5 text-sm font-medium text-[var(--color-text)] transition-colors hover:bg-[var(--color-surface2)]"
              >
                <VkIcon />
                <span>Войти через VK</span>
              </button>
            )}
            {providers.includes('github') && (
              <button
                onClick={() => handleOAuth('github')}
                className="flex w-full items-center justify-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] py-2.5 text-sm font-medium text-[var(--color-text)] transition-colors hover:bg-[var(--color-surface2)]"
              >
                <GitHubIcon />
                <span>Войти через GitHub</span>
              </button>
            )}

            {/* Divider */}
            <div className="relative my-4">
              <div className="absolute inset-0 flex items-center">
                <div className="w-full border-t border-[var(--color-border)]" />
              </div>
              <div className="relative flex justify-center text-xs">
                <span className="bg-[var(--color-surface)] px-2 text-[var(--color-dim)]">или токен API</span>
              </div>
            </div>
          </div>
        )}

        {/* ── Token Login ── */}
        <form onSubmit={handleTokenSubmit} className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("common.api_token")}</label>
            <input
              type="password"
              value={token}
              onChange={e => setToken(e.target.value)}
              placeholder="fl_token_..."
              autoFocus={!hasEmail}
              className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none"
            />
          </div>
          {error && (
            <div className="rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-sm text-rose-400">
              {error}
            </div>
          )}
          <button
            type="submit"
            disabled={loading}
            className="w-full rounded-xl bg-[var(--color-accent)] py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-50"
          >
            {loading ? t('common.loading') : t('common.connect')}
          </button>
        </form>

        <button
          onClick={handleSkip}
          className="mt-3 w-full rounded-xl border border-[var(--color-border)] py-2.5 text-sm font-medium text-[var(--color-dim)] transition-colors hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)]"
        >
          {t('common.skip_mock')}
        </button>
      </div>
      {twoFATempToken && (
        <TwoFAVerify
          tempToken={twoFATempToken}
          onSuccess={() => {
            setTwoFATempToken(null);
            const redirect = getRedirectAfterLogin();
            navigate(redirect || '/');
          }}
          onCancel={() => setTwoFATempToken(null)}
        />
      )}
    </div>
  );
}
