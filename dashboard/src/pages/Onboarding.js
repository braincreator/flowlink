import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Shield, Check, Copy, Loader2, ArrowRight, ArrowLeft, Zap, Eye, ShieldAlert, ShieldCheck } from 'lucide-react';
import { api } from '../api/client';
export default function Onboarding() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const [step, setStep] = useState(0);
    const [relayUrl, setRelayUrl] = useState('http://localhost:8080');
    const [testing, setTesting] = useState(false);
    const [connected, setConnected] = useState(false);
    const [connError, setConnError] = useState('');
    const [selectedTemplate, setSelectedTemplate] = useState('balanced');
    const [polling, setPolling] = useState(false);
    const [agentFound, setAgentFound] = useState(false);
    const [saving, setSaving] = useState(false);
    const [done, setDone] = useState(false);
    const [dontShow, setDontShow] = useState(false);
    const [confetti, setConfetti] = useState(false);
    const STEPS = [
        t('nav.dashboard'),
        t('onboarding.connect_relay'),
        t('onboarding.deploy_agent'),
        t('onboarding.create_policy'),
        t('common.done'),
    ];
    const TEMPLATES = [
        { id: 'strict', name: t('policies.strict'), desc: t('onboarding.strict_desc'), icon: _jsx(ShieldAlert, { size: 20 }), config: { l3: 'block', l2: 'approve', l1: 'allow' } },
        { id: 'balanced', name: t('policies.balanced'), desc: t('onboarding.balanced_desc'), icon: _jsx(Shield, { size: 20 }), config: { l3: 'block', l2: 'approve', l1: 'allow' } },
        { id: 'monitoring', name: t('policies.monitoring'), desc: t('onboarding.monitoring_desc'), icon: _jsx(Eye, { size: 20 }), config: { l3: 'log', l2: 'log', l1: 'log' } },
    ];
    const INSTALL_CMD = 'curl -fsSL https://flowlink.sh/install.sh | bash -s -- --relay RELAY_URL --token TOKEN';
    useEffect(() => {
        if (confetti) {
            const timer = setTimeout(() => setConfetti(false), 3000);
            return () => clearTimeout(timer);
        }
    }, [confetti]);
    const testConnection = async () => {
        setTesting(true);
        setConnError('');
        try {
            await api.getHealth();
            setConnected(true);
        }
        catch (e) {
            setConnError(e.message || t('onboarding.connection_failed'));
        }
        finally {
            setTesting(false);
        }
    };
    const pollForAgent = useCallback(() => {
        setPolling(true);
        const interval = setInterval(async () => {
            try {
                const agents = await api.getAgents();
                if (agents.length > 0) {
                    setAgentFound(true);
                    setPolling(false);
                    clearInterval(interval);
                }
            }
            catch { /* keep polling */ }
        }, 3000);
        return () => clearInterval(interval);
    }, []);
    useEffect(() => {
        if (step === 2 && !agentFound) {
            return pollForAgent();
        }
    }, [step, agentFound, pollForAgent]);
    const savePolicy = async () => {
        setSaving(true);
        try {
            const tpl = TEMPLATES.find(tp => tp.id === selectedTemplate);
            if (tpl) {
                await api.registerAgent({ policy_template: tpl.id, ...tpl.config });
            }
        }
        catch { /* non-critical */ }
        setSaving(false);
        setStep(4);
        setConfetti(true);
    };
    const finish = () => {
        if (dontShow)
            localStorage.setItem('flowlink_onboarded', 'true');
        navigate('/');
    };
    return (_jsxs("div", { className: "flex min-h-screen items-center justify-center bg-[var(--color-bg)] p-6", children: [confetti && (_jsxs("div", { className: "pointer-events-none fixed inset-0 z-50", children: [Array.from({ length: 50 }).map((_, i) => (_jsx("div", { className: "absolute rounded-full", style: {
                            left: `${Math.random() * 100}%`,
                            top: '-10px',
                            width: `${6 + Math.random() * 8}px`,
                            height: `${6 + Math.random() * 8}px`,
                            background: ['#6366f1', '#10b981', '#f59e0b', '#f43f5e', '#3b82f6'][i % 5],
                            animation: `confetti-fall ${1.5 + Math.random() * 2}s ease-out forwards`,
                            animationDelay: `${Math.random() * 0.5}s`,
                        } }, i))), _jsx("style", { children: `
            @keyframes confetti-fall {
              0% { transform: translateY(0) rotate(0deg); opacity: 1; }
              100% { transform: translateY(100vh) rotate(${360 + Math.random() * 720}deg); opacity: 0; }
            }
          ` })] })), _jsxs("div", { className: "w-full max-w-lg", children: [_jsx("div", { className: "mb-8 flex items-center justify-center gap-2", children: STEPS.map((s, i) => (_jsxs("div", { className: "flex items-center gap-2", children: [_jsx("div", { className: `flex h-8 w-8 items-center justify-center rounded-full text-xs font-bold transition-all ${i < step ? 'bg-[var(--color-green)] text-white' :
                                        i === step ? 'bg-[var(--color-accent)] text-white' :
                                            'bg-[var(--color-surface2)] text-[var(--color-dim)]'}`, children: i < step ? _jsx(Check, { size: 14 }) : i + 1 }), i < STEPS.length - 1 && (_jsx("div", { className: `h-0.5 w-8 transition-colors ${i < step ? 'bg-[var(--color-green)]' : 'bg-[var(--color-surface2)]'}` }))] }, s))) }), _jsxs("div", { className: "rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8 shadow-2xl", children: [step === 0 && (_jsxs("div", { className: "text-center", children: [_jsx("div", { className: "mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-indigo-600 text-4xl shadow-lg shadow-indigo-500/30", children: _jsx(Shield, {}) }), _jsx("h2", { className: "text-2xl font-bold mb-3", children: t('onboarding.welcome') }), _jsx("p", { className: "text-[var(--color-dim)] mb-8 leading-relaxed", children: t('onboarding.welcome_desc') }), _jsxs("button", { onClick: () => setStep(1), className: "inline-flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-6 py-3 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)] hover:shadow-lg hover:shadow-indigo-500/20", children: [t('onboarding.get_started'), " ", _jsx(ArrowRight, { size: 16 })] })] })), step === 1 && (_jsxs("div", { children: [_jsx("h2", { className: "text-xl font-bold mb-2", children: t('onboarding.connect_relay') }), _jsx("p", { className: "text-sm text-[var(--color-dim)] mb-6", children: t('onboarding.relay_endpoint') }), _jsx("input", { type: "text", value: relayUrl, onChange: e => { setRelayUrl(e.target.value); setConnected(false); setConnError(''); }, placeholder: "http://localhost:8080", className: "w-full rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] px-4 py-3 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none transition-colors" }), connError && _jsx("p", { className: "mt-2 text-sm text-[var(--color-red)]", children: connError }), connected && (_jsxs("div", { className: "mt-3 flex items-center gap-2 text-sm text-[var(--color-green)]", children: [_jsx(Check, { size: 16 }), " ", t('onboarding.connection_ok')] })), _jsxs("div", { className: "mt-6 flex justify-between", children: [_jsxs("button", { onClick: () => setStep(0), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(ArrowLeft, { size: 16 }), " ", t('common.back')] }), _jsxs("button", { onClick: testConnection, disabled: testing || !relayUrl, className: "flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-5 py-2.5 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-50", children: [testing ? _jsx(Loader2, { size: 16, className: "animate-spin" }) : _jsx(Zap, { size: 16 }), connected ? `${t('onboarding.connection_ok')} ✓` : t('onboarding.test_connection')] })] }), connected && (_jsxs("button", { onClick: () => setStep(2), className: "mt-4 w-full flex items-center justify-center gap-2 rounded-xl bg-[var(--color-green)] px-5 py-2.5 text-sm font-semibold text-white transition-all hover:opacity-90", children: [t('common.next'), ": ", t('onboarding.deploy_agent'), " ", _jsx(ArrowRight, { size: 16 })] }))] })), step === 2 && (_jsxs("div", { children: [_jsx("h2", { className: "text-xl font-bold mb-2", children: t('onboarding.deploy_agent') }), _jsx("p", { className: "text-sm text-[var(--color-dim)] mb-6", children: t('onboarding.install_command') }), _jsxs("div", { className: "relative rounded-xl border border-[var(--color-border)] bg-[#0d0e14] p-4", children: [_jsx("button", { onClick: () => { navigator.clipboard.writeText(INSTALL_CMD.replace('RELAY_URL', relayUrl)); }, className: "absolute top-2 right-2 rounded-md bg-[var(--color-surface2)] px-2 py-1 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors", children: _jsx(Copy, { size: 14 }) }), _jsx("code", { className: "block whitespace-pre-wrap break-all font-mono text-sm text-[var(--color-accent-light)]", children: INSTALL_CMD.replace('RELAY_URL', relayUrl) })] }), polling && !agentFound && (_jsxs("div", { className: "mt-4 flex items-center gap-3 text-sm text-[var(--color-dim)]", children: [_jsx(Loader2, { size: 16, className: "animate-spin" }), " ", t('onboarding.waiting_agent')] })), agentFound && (_jsxs("div", { className: "mt-4 flex items-center gap-2 text-sm text-[var(--color-green)]", children: [_jsx(Check, { size: 16 }), " ", t('onboarding.agent_connected')] })), _jsxs("div", { className: "mt-6 flex justify-between", children: [_jsxs("button", { onClick: () => setStep(1), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(ArrowLeft, { size: 16 }), " ", t('common.back')] }), agentFound && (_jsxs("button", { onClick: () => setStep(3), className: "flex items-center gap-2 rounded-xl bg-[var(--color-green)] px-5 py-2.5 text-sm font-semibold text-white transition-all hover:opacity-90", children: [t('common.next'), ": ", t('onboarding.create_policy'), " ", _jsx(ArrowRight, { size: 16 })] }))] })] })), step === 3 && (_jsxs("div", { children: [_jsx("h2", { className: "text-xl font-bold mb-2", children: t('onboarding.create_policy') }), _jsx("p", { className: "text-sm text-[var(--color-dim)] mb-6", children: t("policies.choose_template") }), _jsx("div", { className: "space-y-3", children: TEMPLATES.map(tpl => (_jsxs("button", { onClick: () => setSelectedTemplate(tpl.id), className: `w-full flex items-center gap-4 rounded-xl border p-4 text-left transition-all ${selectedTemplate === tpl.id
                                                ? 'border-[var(--color-accent)] bg-[var(--color-accent)]/10'
                                                : 'border-[var(--color-border)] hover:bg-[var(--color-surface2)]'}`, children: [_jsx("div", { className: `flex h-10 w-10 items-center justify-center rounded-lg ${selectedTemplate === tpl.id ? 'bg-[var(--color-accent)] text-white' : 'bg-[var(--color-surface2)] text-[var(--color-dim)]'}`, children: tpl.icon }), _jsxs("div", { children: [_jsx("div", { className: "font-semibold text-sm", children: tpl.name }), _jsx("div", { className: "text-xs text-[var(--color-dim)] mt-0.5", children: tpl.desc })] }), selectedTemplate === tpl.id && _jsx(Check, { size: 16, className: "ml-auto text-[var(--color-accent-light)]" })] }, tpl.id))) }), _jsxs("div", { className: "mt-6 flex justify-between", children: [_jsxs("button", { onClick: () => setStep(2), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(ArrowLeft, { size: 16 }), " ", t('common.back')] }), _jsxs("button", { onClick: savePolicy, disabled: saving, className: "flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-5 py-2.5 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)]", children: [saving ? _jsx(Loader2, { size: 16, className: "animate-spin" }) : _jsx(ShieldCheck, { size: 16 }), "Apply Policy"] })] })] })), step === 4 && (_jsxs("div", { className: "text-center", children: [_jsx("div", { className: "mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-gradient-to-br from-emerald-400 to-emerald-600 text-4xl shadow-lg shadow-emerald-500/30", children: _jsx(Check, {}) }), _jsx("h2", { className: "text-2xl font-bold mb-3", children: t('onboarding.complete') }), _jsx("p", { className: "text-[var(--color-dim)] mb-8", children: t('onboarding.complete_desc') }), _jsxs("div", { className: "flex justify-center gap-3 mb-8", children: [_jsx("button", { onClick: () => navigate('/'), className: "rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:bg-[var(--color-accent-light)]", children: t('nav.dashboard') }), _jsx("button", { onClick: () => navigate('/agents'), className: "rounded-xl border border-[var(--color-border)] px-4 py-2 text-sm font-medium hover:bg-[var(--color-surface2)]", children: t('nav.agents') }), _jsx("button", { onClick: () => navigate('/policies'), className: "rounded-xl border border-[var(--color-border)] px-4 py-2 text-sm font-medium hover:bg-[var(--color-surface2)]", children: t('nav.policies') })] }), _jsxs("label", { className: "flex items-center justify-center gap-2 text-sm text-[var(--color-dim)] cursor-pointer", children: [_jsx("input", { type: "checkbox", checked: dontShow, onChange: e => setDontShow(e.target.checked), className: "rounded border-[var(--color-border)]" }), t('onboarding.dont_show')] }), _jsx("button", { onClick: finish, className: "mt-6 w-full rounded-xl bg-[var(--color-accent)] px-6 py-3 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)]", children: t('onboarding.go_dashboard') })] }))] })] })] }));
}
