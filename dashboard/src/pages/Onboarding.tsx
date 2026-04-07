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
    { id: 'strict', name: t('policies.strict'), desc: t('onboarding.strict_desc'), icon: <ShieldAlert size={20} />, config: { l3: 'block', l2: 'approve', l1: 'allow' } },
    { id: 'balanced', name: t('policies.balanced'), desc: t('onboarding.balanced_desc'), icon: <Shield size={20} />, config: { l3: 'block', l2: 'approve', l1: 'allow' } },
    { id: 'monitoring', name: t('policies.monitoring'), desc: t('onboarding.monitoring_desc'), icon: <Eye size={20} />, config: { l3: 'log', l2: 'log', l1: 'log' } },
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
    } catch (e: any) {
      setConnError(e.message || t('onboarding.connection_failed'));
    } finally {
      setTesting(false);
    }
  };

  const pollForAgent = useCallback(() => {
    setPolling(true);
    const interval = setInterval(async () => {
      try {
        const agents = await api.getAgents();
        if ((agents as any[]).length > 0) {
          setAgentFound(true);
          setPolling(false);
          clearInterval(interval);
        }
      } catch { /* keep polling */ }
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
    } catch { /* non-critical */ }
    setSaving(false);
    setStep(4);
    setConfetti(true);
  };

  const finish = () => {
    if (dontShow) localStorage.setItem('flowlink_onboarded', 'true');
    navigate('/');
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-[var(--color-bg)] p-6">
      {confetti && (
        <div className="pointer-events-none fixed inset-0 z-50">
          {Array.from({ length: 50 }).map((_, i) => (
            <div
              key={i}
              className="absolute rounded-full"
              style={{
                left: `${Math.random() * 100}%`,
                top: '-10px',
                width: `${6 + Math.random() * 8}px`,
                height: `${6 + Math.random() * 8}px`,
                background: ['#6366f1', '#10b981', '#f59e0b', '#f43f5e', '#3b82f6'][i % 5],
                animation: `confetti-fall ${1.5 + Math.random() * 2}s ease-out forwards`,
                animationDelay: `${Math.random() * 0.5}s`,
              }}
            />
          ))}
          <style>{`
            @keyframes confetti-fall {
              0% { transform: translateY(0) rotate(0deg); opacity: 1; }
              100% { transform: translateY(100vh) rotate(${360 + Math.random() * 720}deg); opacity: 0; }
            }
          `}</style>
        </div>
      )}

      <div className="w-full max-w-lg">
        <div className="mb-8 flex items-center justify-center gap-2">
          {STEPS.map((s, i) => (
            <div key={s} className="flex items-center gap-2">
              <div className={`flex h-8 w-8 items-center justify-center rounded-full text-xs font-bold transition-all ${
                i < step ? 'bg-[var(--color-green)] text-white' :
                i === step ? 'bg-[var(--color-accent)] text-white' :
                'bg-[var(--color-surface2)] text-[var(--color-dim)]'
              }`}>
                {i < step ? <Check size={14} /> : i + 1}
              </div>
              {i < STEPS.length - 1 && (
                <div className={`h-0.5 w-8 transition-colors ${i < step ? 'bg-[var(--color-green)]' : 'bg-[var(--color-surface2)]'}`} />
              )}
            </div>
          ))}
        </div>

        <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8 shadow-2xl">
          {step === 0 && (
            <div className="text-center">
              <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-2xl bg-gradient-to-br from-indigo-500 to-indigo-600 text-4xl shadow-lg shadow-indigo-500/30">
                <Shield />
              </div>
              <h2 className="text-2xl font-bold mb-3">{t('onboarding.welcome')}</h2>
              <p className="text-[var(--color-dim)] mb-8 leading-relaxed">{t('onboarding.welcome_desc')}</p>
              <button onClick={() => setStep(1)}
                className="inline-flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-6 py-3 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)] hover:shadow-lg hover:shadow-indigo-500/20">
                {t('onboarding.get_started')} <ArrowRight size={16} />
              </button>
            </div>
          )}

          {step === 1 && (
            <div>
              <h2 className="text-xl font-bold mb-2">{t('onboarding.connect_relay')}</h2>
              <p className="text-sm text-[var(--color-dim)] mb-6">{t('onboarding.relay_endpoint')}</p>
              <input
                type="text" value={relayUrl} onChange={e => { setRelayUrl(e.target.value); setConnected(false); setConnError(''); }}
                placeholder="http://localhost:8080"
                className="w-full rounded-xl border border-[var(--color-border)] bg-[var(--color-bg)] px-4 py-3 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none transition-colors"
              />
              {connError && <p className="mt-2 text-sm text-[var(--color-red)]">{connError}</p>}
              {connected && (
                <div className="mt-3 flex items-center gap-2 text-sm text-[var(--color-green)]">
                  <Check size={16} /> {t('onboarding.connection_ok')}
                </div>
              )}
              <div className="mt-6 flex justify-between">
                <button onClick={() => setStep(0)} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
                  <ArrowLeft size={16} /> {t('common.back')}
                </button>
                <button onClick={testConnection} disabled={testing || !relayUrl}
                  className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-5 py-2.5 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-50">
                  {testing ? <Loader2 size={16} className="animate-spin" /> : <Zap size={16} />}
                  {connected ? `${t('onboarding.connection_ok')} ✓` : t('onboarding.test_connection')}
                </button>
              </div>
              {connected && (
                <button onClick={() => setStep(2)} className="mt-4 w-full flex items-center justify-center gap-2 rounded-xl bg-[var(--color-green)] px-5 py-2.5 text-sm font-semibold text-white transition-all hover:opacity-90">
                  {t('common.next')}: {t('onboarding.deploy_agent')} <ArrowRight size={16} />
                </button>
              )}
            </div>
          )}

          {step === 2 && (
            <div>
              <h2 className="text-xl font-bold mb-2">{t('onboarding.deploy_agent')}</h2>
              <p className="text-sm text-[var(--color-dim)] mb-6">{t('onboarding.install_command')}</p>
              <div className="relative rounded-xl border border-[var(--color-border)] bg-[#0d0e14] p-4">
                <button onClick={() => { navigator.clipboard.writeText(INSTALL_CMD.replace('RELAY_URL', relayUrl)); }}
                  className="absolute top-2 right-2 rounded-md bg-[var(--color-surface2)] px-2 py-1 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
                  <Copy size={14} />
                </button>
                <code className="block whitespace-pre-wrap break-all font-mono text-sm text-[var(--color-accent-light)]">
                  {INSTALL_CMD.replace('RELAY_URL', relayUrl)}
                </code>
              </div>
              {polling && !agentFound && (
                <div className="mt-4 flex items-center gap-3 text-sm text-[var(--color-dim)]">
                  <Loader2 size={16} className="animate-spin" /> {t('onboarding.waiting_agent')}
                </div>
              )}
              {agentFound && (
                <div className="mt-4 flex items-center gap-2 text-sm text-[var(--color-green)]">
                  <Check size={16} /> {t('onboarding.agent_connected')}
                </div>
              )}
              <div className="mt-6 flex justify-between">
                <button onClick={() => setStep(1)} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
                  <ArrowLeft size={16} /> {t('common.back')}
                </button>
                {agentFound && (
                  <button onClick={() => setStep(3)} className="flex items-center gap-2 rounded-xl bg-[var(--color-green)] px-5 py-2.5 text-sm font-semibold text-white transition-all hover:opacity-90">
                    {t('common.next')}: {t('onboarding.create_policy')} <ArrowRight size={16} />
                  </button>
                )}
              </div>
            </div>
          )}

          {step === 3 && (
            <div>
              <h2 className="text-xl font-bold mb-2">{t('onboarding.create_policy')}</h2>
              <p className="text-sm text-[var(--color-dim)] mb-6">{t("policies.choose_template")}</p>
              <div className="space-y-3">
                {TEMPLATES.map(tpl => (
                  <button key={tpl.id} onClick={() => setSelectedTemplate(tpl.id)}
                    className={`w-full flex items-center gap-4 rounded-xl border p-4 text-left transition-all ${
                      selectedTemplate === tpl.id
                        ? 'border-[var(--color-accent)] bg-[var(--color-accent)]/10'
                        : 'border-[var(--color-border)] hover:bg-[var(--color-surface2)]'
                    }`}>
                    <div className={`flex h-10 w-10 items-center justify-center rounded-lg ${
                      selectedTemplate === tpl.id ? 'bg-[var(--color-accent)] text-white' : 'bg-[var(--color-surface2)] text-[var(--color-dim)]'
                    }`}>
                      {tpl.icon}
                    </div>
                    <div>
                      <div className="font-semibold text-sm">{tpl.name}</div>
                      <div className="text-xs text-[var(--color-dim)] mt-0.5">{tpl.desc}</div>
                    </div>
                    {selectedTemplate === tpl.id && <Check size={16} className="ml-auto text-[var(--color-accent-light)]" />}
                  </button>
                ))}
              </div>
              <div className="mt-6 flex justify-between">
                <button onClick={() => setStep(2)} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
                  <ArrowLeft size={16} /> {t('common.back')}
                </button>
                <button onClick={savePolicy} disabled={saving}
                  className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-5 py-2.5 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)]">
                  {saving ? <Loader2 size={16} className="animate-spin" /> : <ShieldCheck size={16} />}
                  Apply Policy
                </button>
              </div>
            </div>
          )}

          {step === 4 && (
            <div className="text-center">
              <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-gradient-to-br from-emerald-400 to-emerald-600 text-4xl shadow-lg shadow-emerald-500/30">
                <Check />
              </div>
              <h2 className="text-2xl font-bold mb-3">{t('onboarding.complete')}</h2>
              <p className="text-[var(--color-dim)] mb-8">{t('onboarding.complete_desc')}</p>
              <div className="flex justify-center gap-3 mb-8">
                <button onClick={() => navigate('/')} className="rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:bg-[var(--color-accent-light)]">{t('nav.dashboard')}</button>
                <button onClick={() => navigate('/agents')} className="rounded-xl border border-[var(--color-border)] px-4 py-2 text-sm font-medium hover:bg-[var(--color-surface2)]">{t('nav.agents')}</button>
                <button onClick={() => navigate('/policies')} className="rounded-xl border border-[var(--color-border)] px-4 py-2 text-sm font-medium hover:bg-[var(--color-surface2)]">{t('nav.policies')}</button>
              </div>
              <label className="flex items-center justify-center gap-2 text-sm text-[var(--color-dim)] cursor-pointer">
                <input type="checkbox" checked={dontShow} onChange={e => setDontShow(e.target.checked)}
                  className="rounded border-[var(--color-border)]" />
                {t('onboarding.dont_show')}
              </label>
              <button onClick={finish} className="mt-6 w-full rounded-xl bg-[var(--color-accent)] px-6 py-3 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)]">
                {t('onboarding.go_dashboard')}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
