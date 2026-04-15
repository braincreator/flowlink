import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Save, Server, Shield, Bell, Info, Globe, Volume2, VolumeX, TerminalSquare, ArrowRight, RefreshCw } from 'lucide-react';
import { useApi } from '../hooks/useApi';
import { useAuth } from '../hooks/useAuth';
import { useSettings, BillingInfo, ServerInfo } from '../hooks/useSettings';
import { useNotifications } from '../hooks/useNotifications';
import { useTerminalSettings } from '../hooks/useTerminalSettings';
import { getTheme, themes } from '../components/terminal/themes';
import ThemePreview from '../components/terminal/ThemePreview';

export default function Settings() {
  const { t, i18n } = useTranslation();
  const { token } = useAuth();
  const { data: systemInfo, loading: infoLoading } = useApi(
    () => api.getSystemInfo(),
  );
  const { data: health, error: healthError } = useApi(
    () => api.getHealth(),
    { pollMs: 15000 }
  );
  const {
    billingInfo,
    servers,
    usage,
    loading,
    error,
    changePlan,
    refresh,
  } = useSettings();

  const info = (systemInfo as any) || {};
  const healthStatus = (health as any)?.status === 'ok';
  const [saved, setSaved] = useState(false);
  const [relaySettings, setRelaySettings] = useState({
    listenAddress: '0.0.0.0:8080',
    tlsCert: '/etc/flowlink/tls.crt',
    logLevel: 'info',
  });
  const [shieldSettings, setShieldSettings] = useState({
    mode: 'intercept',
    policyFile: '/etc/flowlink/shield-policy.yaml',
  });
  const [notifConfig, setNotifConfig] = useState({
    botToken: '',
    channel: '@flowlink-alerts',
  });

  const handleSave = () => {
    console.log('Saving settings:', { relaySettings, shieldSettings, notifConfig });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const { settings: notifSettings, updateSettings } = useNotifications();
  const { settings: termSettings, update: updateTermSettings } = useTerminalSettings();
  const navigate = useNavigate();
  const currentTheme = getTheme(termSettings.themeId);

  const formatMemory = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  const formatCurrency = (amount: string) => {
    return amount.replace(/RUB/g, '₽').replace(/(\d+)/g, (match) => {
      return parseInt(match).toLocaleString();
    });
  };

  return (
    <div className="mx-auto max-w-3xl space-y-6 fade-in">
      {/* Billing Information */}
      {billingInfo && (
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <Info size={16} className="text-[var(--color-accent)]" />
              <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">
                {t('settings.billing')}
              </h3>
              <span className={`px-2 py-1 rounded text-xs font-medium ${
                billingInfo.active 
                  ? 'bg-green-500/20 text-green-400' 
                  : 'bg-red-500/20 text-red-400'
              }`}>
                {billingInfo.active ? t('settings.active') : t('settings.inactive')}
              </span>
            </div>
            <button 
              onClick={refresh}
              className="flex items-center gap-1.5 text-xs text-indigo-400 hover:text-indigo-300 transition-colors"
              disabled={loading}
            >
              <RefreshCw size={12} className={loading ? 'animate-spin' : ''} />
              {t('common.refresh')}
            </button>
          </div>
          
          <div className="grid grid-cols-2 gap-4 mb-4">
            <div className="rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)]">{t('settings.current_plan')}</div>
              <div className="mt-1 font-medium">{billingInfo.plan_name}</div>
            </div>
            <div className="rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)]">{t('settings.billing_balance')}</div>
              <div className="mt-1 font-medium text-green-400">{formatCurrency(billingInfo.balance_rub)}</div>
            </div>
            {billingInfo.expires_at && (
              <div className="rounded-lg bg-[var(--color-bg)] p-3">
                <div className="text-xs text-[var(--color-dim)]">{t('settings.expires_at')}</div>
                <div className="mt-1 font-medium">
                  {new Date(billingInfo.expires_at).toLocaleDateString()}
                </div>
              </div>
            )}
            <div className="rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)]">{t('settings.active_agents')}</div>
              <div className="mt-1 font-medium">{usage?.tracker?.active_agents || 0}</div>
            </div>
          </div>

          {/* Plan Selection */}
          <div className="border-t border-[var(--color-border)] pt-4">
            <h4 className="text-sm font-medium text-[var(--color-dim)] mb-3">{t('settings.available_plans')}</h4>
            <div className="grid grid-cols-1 gap-3">
              {billingInfo.available_plans.map((plan) => (
                <div
                  key={plan.id}
                  className={`rounded-lg border p-3 transition-colors ${
                    billingInfo.plan_id === plan.id
                      ? 'border-[var(--color-accent)] bg-[var(--color-accent)]/5'
                      : 'border-[var(--color-border)] hover:border-[var(--color-dim)]'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div>
                      <div className="font-medium">{plan.name}</div>
                      <div className="text-sm text-[var(--color-dim)]">{formatCurrency(plan.price_rub)}</div>
                    </div>
                    {billingInfo.plan_id !== plan.id && (
                      <button
                        onClick={() => changePlan(plan.id)}
                        className="px-3 py-1 text-sm bg-[var(--color-accent)] text-white rounded hover:bg-[var(--color-accent-light)] transition-colors"
                      >
                        {t('settings.upgrade')}
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Server Management */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Server size={16} className="text-[var(--color-accent)]" />
            <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">
              {t('settings.servers')}
            </h3>
            <span className="text-xs text-[var(--color-dim)]">
              {servers.filter(s => s.status === 'online').length} online
            </span>
          </div>
        </div>
        
        <div className="space-y-3">
          {servers.length > 0 ? (
            servers.map((server) => (
              <div
                key={server.id}
                className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-4"
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <div
                      className={`h-2 w-2 rounded-full ${
                        server.status === 'online' ? 'bg-green-400' : 'bg-red-400'
                      }`}
                    />
                    <span className="font-medium">{server.name}</span>
                    <span className={`text-xs px-2 py-1 rounded ${
                      server.status === 'online' 
                        ? 'bg-green-500/20 text-green-400' 
                        : 'bg-red-500/20 text-red-400'
                    }`}>
                      {server.status}
                    </span>
                  </div>
                  <span className="text-xs text-[var(--color-dim)]">
                    {new Date(server.last_seen).toLocaleString()}
                  </span>
                </div>
                
                <div className="grid grid-cols-3 gap-4 text-xs">
                  <div>
                    <div className="text-[var(--color-dim)]">CPU</div>
                    <div className="font-medium">{server.cpu_percent.toFixed(1)}%</div>
                  </div>
                  <div>
                    <div className="text-[var(--color-dim)]">Memory</div>
                    <div className="font-medium">{formatMemory(server.memory_used)}</div>
                  </div>
                  <div>
                    <div className="text-[var(--color-dim)]">Commands</div>
                    <div className="font-medium">{server.commands_processed}</div>
                  </div>
                </div>
              </div>
            ))
          ) : (
            <div className="text-center py-8 text-[var(--color-dim)]">
              <Server size={24} className="mx-auto mb-2 opacity-50" />
              <p>{t('settings.no_servers')}</p>
            </div>
          )}
        </div>
      </div>

      {/* System Information */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Info size={16} className="text-[var(--color-accent)]" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('settings.title')} — System</h3>
        </div>
        <div className="grid grid-cols-2 gap-4">
          {[
            { label: t('settings.version'), value: info.version || '—' },
            { label: t('settings.uptime'), value: info.uptime || '—' },
            { label: t('settings.memory_usage'), value: info.memory_usage != null ? `${info.memory_usage}%` : '—' },
            { label: t('settings.cpu_usage'), value: info.cpu_usage != null ? `${info.cpu_usage}%` : '—' },
            { label: t('settings.api_url'), value: api.getApiBase() },
            { label: t('settings.connection'), value: healthError ? t('settings.offline_status') : healthStatus ? t('settings.connected_status') : infoLoading ? t('settings.connecting_status') : t('settings.unknown_status') },
          ].map(s => (
            <div key={s.label} className="rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)]">{s.label}</div>
              <div className="mt-1 font-medium">{s.value}</div>
            </div>
          ))}
        </div>
      </div>

      {/* Usage Statistics */}
      {usage && (
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <div className="flex items-center gap-2 mb-4">
            <RefreshCw size={16} className="text-[var(--color-accent)]" />
            <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">
              {t('settings.usage_stats')}
            </h3>
          </div>
          
          <div className="grid grid-cols-3 gap-4">
            <div className="rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)]">{t('settings.daily_requests')}</div>
              <div className="mt-1 font-medium">{usage.tracker.daily_requests.toLocaleString()}</div>
            </div>
            <div className="rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)]">{t('settings.daily_tokens')}</div>
              <div className="mt-1 font-medium">{usage.tracker.daily_tokens.toLocaleString()}</div>
            </div>
            <div className="rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)]">{t('settings.active_agents')}</div>
              <div className="mt-1 font-medium">{usage.tracker.active_agents}</div>
            </div>
          </div>
        </div>
      )}

      {/* Relay Configuration */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Server size={16} className="text-[var(--color-accent)]" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('settings.relay')}</h3>
        </div>
        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.listen_address")}</label>
            <input type="text" value={relaySettings.listenAddress} onChange={e => setRelaySettings(s => ({...s, listenAddress: e.target.value}))} className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.tls_cert")}</label>
            <input type="text" value={relaySettings.tlsCert} onChange={e => setRelaySettings(s => ({...s, tlsCert: e.target.value}))} className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.log_level")}</label>
            <select value={relaySettings.logLevel} onChange={e => setRelaySettings(s => ({...s, logLevel: e.target.value}))} className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
              <option>trace</option><option>debug</option><option value="info">info</option><option>warn</option><option>error</option>
            </select>
          </div>
        </div>
      </div>

      {/* Terminal Appearance */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <TerminalSquare size={16} className="text-emerald-400" />
            <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('ts.terminal_appearance')}</h3>
          </div>
          <button onClick={() => navigate('/terminal')}
            className="flex items-center gap-1.5 text-xs text-indigo-400 hover:text-indigo-300 transition-colors">
            {t('ts.open_terminal_settings')} <ArrowRight size={12} />
          </button>
        </div>

        {/* Current theme with live preview */}
        <div className="mb-4">
          <ThemePreview theme={currentTheme} className="border border-[var(--color-border)]" />
          <div className="mt-2 flex items-center gap-2">
            <span className="text-xs text-[var(--color-dim)]">{t('ts.current_theme')}:</span>
            <span className="text-xs font-medium text-[var(--color-text)]">{currentTheme.name}</span>
            <span className="text-xs text-[var(--color-dim)]">— {currentTheme.description}</span>
          </div>
        </div>

        {/* Quick theme picker grid */}
        <div className="grid grid-cols-4 sm:grid-cols-8 gap-1.5">
          {themes.map(th => {
            const palette = [th.colors.black, th.colors.red, th.colors.green, th.colors.yellow, th.colors.blue, th.colors.magenta, th.colors.cyan, th.colors.white];
            return (
              <button key={th.id} onClick={() => updateTermSettings({ themeId: th.id })}
                className={`relative rounded-md p-1.5 text-left transition-all hover:scale-105 ${
                  termSettings.themeId === th.id
                    ? 'ring-2 ring-indigo-500 ring-offset-1 ring-offset-[var(--color-surface)]'
                    : 'ring-1 ring-[var(--color-border)] hover:ring-[var(--color-dim)]'
                }`}
                style={{ background: th.colors.background }}
                title={th.name}>
                {termSettings.themeId === th.id && (
                  <div className="absolute -top-1 -right-1 h-3 w-3 rounded-full bg-indigo-500 border border-[var(--color-surface)]" />
                )}
                <div className="grid grid-cols-8 gap-px">
                  {palette.map((c, i) => (
                    <div key={i} className="h-2 rounded-sm" style={{ background: c }} />
                  ))}
                </div>
              </button>
            );
          })}
        </div>
        <p className="mt-3 text-xs text-[var(--color-dim)]">{t('ts.quick_customize')}</p>
      </div>

      {/* Language */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Globe size={16} className="text-[var(--color-accent)]" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('settings.language')}</h3>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => i18n.changeLanguage('en')}
            className={`rounded-lg px-4 py-2 text-sm font-medium transition-colors ${i18n.language === 'en' ? 'bg-[var(--color-accent)] text-white' : 'border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)]'}`}
          >English</button>
          <button
            onClick={() => i18n.changeLanguage('ru')}
            className={`rounded-lg px-4 py-2 text-sm font-medium transition-colors ${i18n.language === 'ru' ? 'bg-[var(--color-accent)] text-white' : 'border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)]'}`}
          >Русский</button>
          <button
            onClick={() => i18n.changeLanguage(navigator.language.startsWith('ru') ? 'ru' : 'en')}
            className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm font-medium text-[var(--color-dim)] hover:bg-[var(--color-surface2)] transition-colors"
          >{t('settings.system')}</button>
        </div>
      </div>

      <div className="flex gap-3">
        <button onClick={handleSave} className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-6 py-3 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] hover:shadow-lg hover:shadow-indigo-500/20">
          <Save size={16} /> {saved ? t('common.saved') : t('common.save_configuration')}
        </button>
        {token && (
          <button onClick={() => { logout(); window.location.href = '/login'; }}
            className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-6 py-3 text-sm font-medium text-rose-400 transition-colors hover:bg-rose-500/20">
            {t('common.sign_out')}
          </button>
        )}
      </div>
    </div>
  );
}