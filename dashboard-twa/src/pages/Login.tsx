import { useState } from 'react';
import { useAuth } from '../contexts/AuthContext';

export default function Login() {
  const { sendCode, login } = useAuth();
  const [email, setEmail] = useState('');
  const [code, setCode] = useState('');
  const [step, setStep] = useState<'email' | 'code'>('email');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSendCode = async () => {
    if (!email.trim()) {
      setError('Введите email');
      return;
    }
    setLoading(true);
    setError('');
    try {
      await sendCode(email.trim());
      setStep('code');
    } catch (e: any) {
      setError(e.message || 'Ошибка отправки кода');
    } finally {
      setLoading(false);
    }
  };

  const handleVerify = async () => {
    if (!code.trim()) {
      setError('Введите код');
      return;
    }
    setLoading(true);
    setError('');
    try {
      await login(email.trim(), code.trim());
    } catch (e: any) {
      setError(e.message || 'Неверный код');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex flex-col items-center justify-center px-6">
      <div className="w-full max-w-sm">
        {/* Logo */}
        <div className="text-center mb-8">
          <h1 className="text-2xl font-bold mb-2">FlowLink</h1>
          <p className="text-sm text-tg-hint">
            {step === 'email' ? 'Войдите с помощью email' : 'Введите код подтверждения'}
          </p>
        </div>

        {error && (
          <div className="mb-4 p-3 rounded-lg bg-tg-danger/20 text-tg-danger text-sm">
            {error}
          </div>
        )}

        {step === 'email' ? (
          <>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Email"
              className="w-full px-4 py-3 rounded-xl bg-tg-hint/10 border border-tg-button/30 text-tg-button-text placeholder:text-tg-hint/60 focus:outline-none focus:ring-2 focus:ring-tg-button mb-4"
              onKeyDown={(e) => e.key === 'Enter' && handleSendCode()}
            />
            <button
              onClick={handleSendCode}
              disabled={loading}
              className="w-full px-4 py-3 rounded-xl bg-tg-button text-tg-button-text font-medium disabled:opacity-60"
            >
              {loading ? 'Отправка...' : 'Получить код'}
            </button>
          </>
        ) : (
          <>
            <p className="text-sm text-tg-hint mb-4 text-center">
              Код отправлен на <span className="text-tg-button-text">{email}</span>
            </p>
            <input
              type="text"
              value={code}
              onChange={(e) => setCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
              placeholder="000000"
              maxLength={6}
              className="w-full px-4 py-3 rounded-xl bg-tg-hint/10 border border-tg-button/30 text-tg-button-text placeholder:text-tg-hint/60 focus:outline-none focus:ring-2 focus:ring-tg-button mb-4 text-center text-xl tracking-[0.5em] font-mono"
              onKeyDown={(e) => e.key === 'Enter' && handleVerify()}
              autoFocus
            />
            <button
              onClick={handleVerify}
              disabled={loading || code.length < 4}
              className="w-full px-4 py-3 rounded-xl bg-tg-button text-tg-button-text font-medium disabled:opacity-60"
            >
              {loading ? 'Проверка...' : 'Войти'}
            </button>
            <button
              onClick={() => { setStep('email'); setCode(''); setError(''); }}
              className="w-full mt-2 px-4 py-2 text-sm text-tg-hint"
            >
              ← Изменить email
            </button>
          </>
        )}

        <p className="text-xs text-tg-hint text-center mt-8">
          Нажимая кнопку, вы соглашаетесь с условиями использования
        </p>
      </div>
    </div>
  );
}
