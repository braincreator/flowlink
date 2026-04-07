import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Save, Server, Shield, Bell, Info, Globe, Volume2, VolumeX } from 'lucide-react';
import { useApi } from '../hooks/useApi';
import { useAuth } from '../hooks/useAuth';
import { api } from '../api/client';
import { useNotifications } from '../hooks/useNotifications';

export default function Settings() {
  const { t, i18n } = useTranslation();
  const { logout } = useAuth();
  const { data: systemInfo, loading: infoLoading } = useApi(
    () => api.getSystemInfo(),
  );
  const { data: health, error: healthError } = useApi(
    () => api.getHealth(),
    { pollMs: 15000 }
  );

  const info = (systemInfo as any) || {};
  const healthStatus = (health as any)?.status === 'ok';
  const [saved, setSaved] = useState(false);

  const handleSave = () => { setSaved(true); setTimeout(() => setSaved(false), 2000); };
  const { settings: notifSettings, updateSettings } = useNotifications();

  return (
    <div className="mx-auto max-w-3xl space-y-6 fade-in">
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

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Server size={16} className="text-[var(--color-accent)]" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('settings.relay')}</h3>
        </div>
        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.listen_address")}</label>
            <input type="text" defaultValue="0.0.0.0:8080" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.tls_cert")}</label>
            <input type="text" defaultValue="/etc/flowlink/tls.crt" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.log_level")}</label>
            <select defaultValue="info" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
              <option>trace</option><option>debug</option><option value="info">info</option><option>warn</option><option>error</option>
            </select>
          </div>
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Shield size={16} className="text-rose-400" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('settings.shield_config')}</h3>
        </div>
        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.shield_mode")}</label>
            <select defaultValue="intercept" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
              <option value="monitor">Monitor (log only)</option><option value="alert">Alert</option><option value="intercept">Intercept</option><option value="enforce">Enforce</option>
            </select>
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.policy_file")}</label>
            <input type="text" defaultValue="/etc/flowlink/shield-policy.yaml" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Bell size={16} className="text-amber-400" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('settings.notifications')}</h3>
        </div>
        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.telegram_bot_token")}</label>
            <input type="password" defaultValue="" placeholder="Enter token..." className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">{t("settings.telegram_channel")}</label>
            <input type="text" defaultValue="@flowlink-alerts" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
        </div>
      </div>

      <NotificationPreferences />

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
        <button onClick={() => { logout(); window.location.href = '/login'; }}
          className="rounded-xl border border-rose-500/30 bg-rose-500/10 px-6 py-3 text-sm font-medium text-rose-400 transition-colors hover:bg-rose-500/20">
          {t('common.sign_out')}
        </button>
      </div>
    </div>
  );
}

function NotificationPreferences() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useNotifications();

  const Toggle = ({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) => (
    <label className="flex items-center justify-between py-2">
      <span className="text-sm text-[var(--color-text)]">{label}</span>
      <button
        onClick={() => onChange(!checked)}
        className={`relative h-6 w-11 rounded-full transition-colors ${checked ? 'bg-[var(--color-accent)]' : 'bg-[var(--color-surface3)]'}`}
      >
        <span className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${checked ? 'translate-x-5' : ''}`} />
      </button>
    </label>
  );

  return (
    <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
      <div className="flex items-center gap-2 mb-4">
        {settings.soundEnabled ? <Volume2 size={16} className="text-indigo-400" /> : <VolumeX size={16} className="text-[var(--color-dim)]" />}
        <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">{t('settings.notifications_browser')}</h3>
      </div>
      <div className="space-y-1">
        <Toggle label="Browser notifications" checked={settings.browserEnabled} onChange={v => updateSettings({ browserEnabled: v })} />
        <Toggle label={t('settings.notifications_sound')} checked={settings.soundEnabled} onChange={v => updateSettings({ soundEnabled: v })} />
        <div className="py-2">
          <label className="mb-2 block text-sm text-[var(--color-text)]">{t('settings.volume')}</label>
          <input
            type="range" min={0} max={0.5} step={0.01} value={settings.volume}
            onChange={e => updateSettings({ volume: parseFloat(e.target.value) })}
            className="w-full max-w-xs accent-[var(--color-accent)]"
          />
          <span className="text-xs text-[var(--color-dim)]">{Math.round(settings.volume * 200)}%</span>
        </div>
        <div className="border-t border-[var(--color-border)] pt-2 mt-2">
          <div className="text-xs text-[var(--color-dim)] uppercase tracking-wider mb-2">{t("settings.events")}</div>
          <Toggle label={t('settings.events_l3')} checked={settings.events.l3} onChange={v => updateSettings({ events: { ...settings.events, l3: v } })} />
          <Toggle label={t('settings.events_l2')} checked={settings.events.l2} onChange={v => updateSettings({ events: { ...settings.events, l2: v } })} />
          <Toggle label={t('settings.events_agent')} checked={settings.events.agentEvents} onChange={v => updateSettings({ events: { ...settings.events, agentEvents: v } })} />
          <Toggle label={t('settings.events_errors')} checked={settings.events.errors} onChange={v => updateSettings({ events: { ...settings.events, errors: v } })} />
        </div>
      </div>
    </div>
  );
}
