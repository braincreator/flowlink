import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Zap } from 'lucide-react';
import { api } from '../api/client';
export default function Login() {
    const navigate = useNavigate();
    const [token, setToken] = useState('');
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(false);
    const handleSubmit = async (e) => {
        e.preventDefault();
        if (!token.trim())
            return;
        setLoading(true);
        setError('');
        try {
            api.setToken(token.trim());
            await api.getHealth();
            navigate('/');
        }
        catch {
            setError('Invalid token or relay unreachable');
            api.setToken(null);
        }
        finally {
            setLoading(false);
        }
    };
    const handleSkip = () => {
        api.setToken(null);
        navigate('/');
    };
    return (_jsx("div", { className: "flex min-h-screen items-center justify-center bg-[var(--color-bg)] p-4", children: _jsxs("div", { className: "w-full max-w-sm rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8", children: [_jsxs("div", { className: "mb-6 flex items-center justify-center gap-2.5", children: [_jsx("div", { className: "flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 to-indigo-600 text-lg font-bold text-white", children: _jsx(Zap, {}) }), _jsxs("span", { className: "text-xl font-bold", children: [_jsx("span", { className: "text-[var(--color-accent-light)]", children: "Flow" }), _jsx("span", { className: "text-[var(--color-text)]", children: "Link" })] })] }), _jsx("h2", { className: "mb-1 text-center text-lg font-semibold", children: "Sign in" }), _jsx("p", { className: "mb-6 text-center text-sm text-[var(--color-dim)]", children: "Enter your API token or connect to relay" }), _jsxs("form", { onSubmit: handleSubmit, className: "space-y-4", children: [_jsxs("div", { children: [_jsx("label", { className: "mb-1.5 block text-sm text-[var(--color-dim)]", children: "API Token" }), _jsx("input", { type: "password", value: token, onChange: e => setToken(e.target.value), placeholder: "fl_token_...", autoFocus: true, className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none" })] }), error && _jsx("div", { className: "rounded-lg bg-rose-500/10 border border-rose-500/20 px-3 py-2 text-sm text-rose-400", children: error }), _jsx("button", { type: "submit", disabled: loading, className: "w-full rounded-xl bg-[var(--color-accent)] py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-50", children: loading ? 'Connecting...' : 'Connect' })] }), _jsx("button", { onClick: handleSkip, className: "mt-3 w-full rounded-xl border border-[var(--color-border)] py-2.5 text-sm font-medium text-[var(--color-dim)] transition-colors hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)]", children: "Skip \u2014 use mock data" })] }) }));
}
