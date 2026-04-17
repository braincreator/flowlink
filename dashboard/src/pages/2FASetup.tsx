import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { api } from '../api/client';
import { Shield, ArrowLeft, Copy, CheckCircle, XCircle } from 'lucide-react';

type Step = 'status' | 'scan' | 'verify' | 'disable-confirm';

export default function TwoFASetup() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>('status');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const [enabled, setEnabled] = useState(false);
  const [configured, setConfigured] = useState(false);
  const [secret, setSecret] = useState('');
  const [otpauthUri, setOtpauthUri] = useState('');
  const [code, setCode] = useState('');
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    loadStatus();
  }, []);

  const loadStatus = async () => {
    try {
      const data = await api.get2FAStatus();
      setEnabled(data.enabled);
      setConfigured(data.configured);
      if (data.enabled) setStep('status');
    } catch {
      setError(t('common.error'));
    }
  };

  const handleSetup = async () => {
    setLoading(true);
    setError('');
    try {
      const data = await api.setup2FA();
      setSecret(data.secret);
      setOtpauthUri(data.otpauth_uri);
      setStep('scan');
    } catch (e: any) {
      setError(e.message || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleEnable = async () => {
    if (!code.trim()) return;
    setLoading(true);
    setError('');
    try {
      const data = await api.enable2FA(code.trim());
      if (data.ok) {
        setEnabled(true);
        setStep('status');
      }
    } catch (e: any) {
      setError(e.message || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleDisable = async () => {
    if (!code.trim()) return;
    setLoading(true);
    setError('');
    try {
      const data = await api.disable2FA(code.trim());
      if (data.ok) {
        setEnabled(false);
        setConfigured(false);
        setCode('');
        setStep('status');
      }
    } catch (e: any) {
      setError(e.message || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const copySecret = () => {
    navigator.clipboard.writeText(secret);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // ── Scan QR Code Step ──
  if (step === 'scan') {
    return (
      <div className="flex min-h-[60vh] items-center justify-center p-4">
        <div className="w-full max-w-md rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8">
          <button
            onClick={() => { setStep('status'); setCode(''); }}
            className="mb-4 flex items-center gap-1 text-sm text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors"
          >
            <ArrowLeft size={14} /> {t('common.back')}
          </button>

          <div className="mb-6 flex items-center gap-2">
            <Shield size={20} className="text-indigo-400" />
            <h2 className="text-lg font-semibold">{t('2fa.scan_title', 'Отсканируйте QR-код')}</h2>
          </div>

          {error && (
            <div className="mb-4 rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-sm text-rose-400">
              {error}
            </div>
          )}

          <div className="mb-4 flex justify-center">
            <img
              src={`https://api.qrserver.com/v1/create-qr-code/?size=200x200&data=${encodeURIComponent(otpauthUri)}`}
              alt="QR Code"
              className="rounded-lg border border-[var(--color-border)]"
              width={200}
              height={200}
            />
          </div>

          <div className="mb-4">
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t('2fa.manual_secret', 'Секретный ключ')}</label>
            <div className="flex items-center gap-2">
              <code className="flex-1 rounded-lg bg-[var(--color-bg)] border border-[var(--color-border)] px-3 py-2 font-mono text-sm break-all">
                {secret}
              </code>
              <button
                onClick={copySecret}
                className="flex h-10 w-10 items-center justify-center rounded-lg border border-[var(--color-border)] hover:bg-[var(--color-surface2)] transition-colors"
                title={t('common.copy', 'Копировать')}
              >
                {copied ? <CheckCircle size={16} className="text-green-400" /> : <Copy size={16} />}
              </button>
            </div>
          </div>

          <div className="space-y-3">
            <div>
              <label className="mb-1.5 block text-sm text-[var(--color-dim)]">
                {t('2fa.enter_code', 'Введите код из приложения')}
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
              onClick={handleEnable}
              disabled={loading || code.length < 6}
              className="w-full rounded-xl bg-indigo-500 py-2.5 text-sm font-medium text-white transition-all hover:bg-indigo-400 disabled:opacity-50"
            >
              {loading ? t('common.loading') : t('2fa.enable', 'Включить 2FA')}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ── Disable Confirm Step ──
  if (step === 'disable-confirm') {
    return (
      <div className="flex min-h-[60vh] items-center justify-center p-4">
        <div className="w-full max-w-sm rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8">
          <button
            onClick={() => { setStep('status'); setCode(''); }}
            className="mb-4 flex items-center gap-1 text-sm text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors"
          >
            <ArrowLeft size={14} /> {t('common.back')}
          </button>

          <div className="mb-4 flex items-center gap-2">
            <XCircle size={20} className="text-rose-400" />
            <h2 className="text-lg font-semibold">{t('2fa.disable_title', 'Отключить 2FA')}</h2>
          </div>

          <p className="mb-4 text-sm text-[var(--color-dim)]">
            {t('2fa.disable_desc', 'Введите текущий код из приложения для подтверждения.')}
          </p>

          {error && (
            <div className="mb-4 rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-sm text-rose-400">
              {error}
            </div>
          )}

          <div className="space-y-3">
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
            <button
              onClick={handleDisable}
              disabled={loading || code.length < 6}
              className="w-full rounded-xl bg-rose-500 py-2.5 text-sm font-medium text-white transition-all hover:bg-rose-400 disabled:opacity-50"
            >
              {loading ? t('common.loading') : t('2fa.disable', 'Отключить 2FA')}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ── Status Step (default) ──
  return (
    <div className="flex min-h-[60vh] items-center justify-center p-4">
      <div className="w-full max-w-sm rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8">
        <button
          onClick={() => navigate('/profile')}
          className="mb-4 flex items-center gap-1 text-sm text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors"
        >
          <ArrowLeft size={14} /> {t('common.back')}
        </button>

        <div className="mb-6 flex items-center gap-2">
          <Shield size={20} className={enabled ? 'text-green-400' : 'text-[var(--color-dim)]'} />
          <h2 className="text-lg font-semibold">{t('2fa.title', 'Двухфакторная аутентификация')}</h2>
        </div>

        {error && (
          <div className="mb-4 rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-sm text-rose-400">
            {error}
          </div>
        )}

        <div className="mb-6 rounded-xl border border-[var(--color-border)] p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {enabled ? (
                <CheckCircle size={18} className="text-green-400" />
              ) : (
                <XCircle size={18} className="text-[var(--color-dim)]" />
              )}
              <span className="text-sm font-medium">
                {enabled
                  ? t('2fa.enabled', '2FA включена')
                  : t('2fa.disabled', '2FA отключена')}
              </span>
            </div>
            <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${enabled ? 'bg-green-500/10 text-green-400' : 'bg-[var(--color-bg)] text-[var(--color-dim)]'}`}>
              {enabled ? 'ON' : 'OFF'}
            </span>
          </div>
        </div>

        <p className="mb-4 text-sm text-[var(--color-dim)]">
          {enabled
            ? t('2fa.enabled_desc', 'Ваш аккаунт защищён двухфакторной аутентификацией.')
            : t('2fa.disabled_desc', 'Включите 2FA для дополнительной безопасности вашего аккаунта.')}
        </p>

        {!enabled && !configured && (
          <button
            onClick={handleSetup}
            disabled={loading}
            className="w-full rounded-xl bg-indigo-500 py-2.5 text-sm font-medium text-white transition-all hover:bg-indigo-400 disabled:opacity-50"
          >
            {loading ? t('common.loading') : t('2fa.setup', 'Настроить 2FA')}
          </button>
        )}

        {!enabled && configured && (
          <div className="space-y-2">
            <p className="text-xs text-amber-400">{t('2fa.incomplete', '2FA настроена, но не включена. Пройдите верификацию.')}</p>
            <button
              onClick={handleSetup}
              disabled={loading}
              className="w-full rounded-xl bg-indigo-500 py-2.5 text-sm font-medium text-white transition-all hover:bg-indigo-400 disabled:opacity-50"
            >
              {loading ? t('common.loading') : t('2fa.continue_setup', 'Продолжить настройку')}
            </button>
          </div>
        )}

        {enabled && (
          <button
            onClick={() => { setStep('disable-confirm'); setCode(''); }}
            className="w-full rounded-xl border border-rose-500/30 py-2.5 text-sm font-medium text-rose-400 transition-all hover:bg-rose-500/10"
          >
            {t('2fa.disable_btn', 'Отключить 2FA')}
          </button>
        )}
      </div>
    </div>
  );
}
