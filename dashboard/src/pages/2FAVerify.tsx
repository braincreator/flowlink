import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Shield, X } from 'lucide-react';
import { api } from '../api/client';

interface TwoFAVerifyProps {
  tempToken: string;
  onSuccess: () => void;
  onCancel: () => void;
}

export default function TwoFAVerify({ tempToken, onSuccess, onCancel }: TwoFAVerifyProps) {
  const { t } = useTranslation();
  const [code, setCode] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (code.trim().length < 6) return;
    setLoading(true);
    setError('');
    try {
      const data = await api.complete2FA(tempToken, code.trim());
      if (data.access_token) {
        onSuccess();
      } else {
        setError(data.error || t('common.error'));
      }
    } catch (e: any) {
      setError(e.message || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <div className="w-full max-w-sm rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8 relative">
        <button
          onClick={onCancel}
          className="absolute top-4 right-4 text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors"
        >
          <X size={18} />
        </button>

        <div className="mb-6 flex flex-col items-center gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-indigo-500/10 text-indigo-400">
            <Shield size={24} />
          </div>
          <div className="text-center">
            <h2 className="text-lg font-semibold">{t('2fa.verify_title', 'Двухфакторная проверка')}</h2>
            <p className="mt-1 text-sm text-[var(--color-dim)]">
              {t('2fa.verify_desc', 'Введите код из приложения для аутентификации')}
            </p>
          </div>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <input
            type="text"
            inputMode="numeric"
            maxLength={6}
            value={code}
            onChange={e => setCode(e.target.value.replace(/\D/g, ''))}
            placeholder="000000"
            autoFocus
            autoComplete="one-time-code"
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-3 text-center font-mono text-2xl tracking-[0.5em] placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none"
          />

          {error && (
            <div className="rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-sm text-rose-400">
              {error}
            </div>
          )}

          <button
            type="submit"
            disabled={loading || code.length < 6}
            className="w-full rounded-xl bg-indigo-500 py-2.5 text-sm font-medium text-white transition-all hover:bg-indigo-400 disabled:opacity-50"
          >
            {loading ? t('common.loading') : t('2fa.verify_btn', 'Подтвердить')}
          </button>
        </form>
      </div>
    </div>
  );
}
