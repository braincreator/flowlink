import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Zap } from 'lucide-react';
import { api } from '../api/client';

export default function Login() {
  const navigate = useNavigate();
  const [token, setToken] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token.trim()) return;
    setLoading(true);
    setError('');
    try {
      api.setToken(token.trim());
      await api.getHealth();
      navigate('/');
    } catch {
      setError('Invalid token or relay unreachable');
      api.setToken(null);
    } finally {
      setLoading(false);
    }
  };

  const handleSkip = () => {
    api.setToken(null);
    navigate('/');
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-[var(--color-bg)] p-4">
      <div className="w-full max-w-sm rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8">
        <div className="mb-6 flex items-center justify-center gap-2.5">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-indigo-600 text-lg font-bold text-white"><Zap /></div>
          <span className="text-xl font-bold"><span className="text-[var(--color-accent-light)]">Flow</span><span className="text-[var(--color-text)]">Link</span></span>
        </div>
        <h2 className="mb-1 text-center text-lg font-semibold">Sign in</h2>
        <p className="mb-6 text-center text-sm text-[var(--color-dim)]">Enter your API token or connect to relay</p>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">API Token</label>
            <input
              type="password" value={token} onChange={e => setToken(e.target.value)}
              placeholder="fl_token_..." autoFocus
              className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none"
            />
          </div>
          {error && <div className="rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-sm text-rose-400">{error}</div>}
          <button type="submit" disabled={loading}
            className="w-full rounded-xl bg-[var(--color-accent)] py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-50">
            {loading ? 'Connecting...' : 'Connect'}
          </button>
        </form>

        <button onClick={handleSkip}
          className="mt-3 w-full rounded-xl border border-[var(--color-border)] py-2.5 text-sm font-medium text-[var(--color-dim)] transition-colors hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)]">
          Skip — use mock data
        </button>
      </div>
    </div>
  );
}
