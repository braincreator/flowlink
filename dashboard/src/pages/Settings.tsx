import { useState } from 'react';
import { Save, Server, Shield, Bell, Info } from 'lucide-react';
import { mockSystemInfo } from '../api/client';

export default function Settings() {
  const info = mockSystemInfo;
  const [saved, setSaved] = useState(false);

  const handleSave = () => { setSaved(true); setTimeout(() => setSaved(false), 2000); };

  return (
    <div className="mx-auto max-w-3xl space-y-6 fade-in">
      {/* System Info */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Info size={16} className="text-[var(--color-accent)]" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">System Information</h3>
        </div>
        <div className="grid grid-cols-2 gap-4">
          {[
            { label: 'Version', value: info.version },
            { label: 'Uptime', value: info.uptime },
            { label: 'Memory Usage', value: `${info.memory_usage}%` },
            { label: 'CPU Usage', value: `${info.cpu_usage}%` },
          ].map(s => (
            <div key={s.label} className="rounded-lg bg-[var(--color-bg)] p-3">
              <div className="text-xs text-[var(--color-dim)]">{s.label}</div>
              <div className="mt-1 font-medium">{s.value}</div>
            </div>
          ))}
        </div>
      </div>

      {/* Relay Config */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Server size={16} className="text-[var(--color-accent)]" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Relay Configuration</h3>
        </div>
        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Listen Address</label>
            <input type="text" defaultValue="0.0.0.0:8080" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">TLS Certificate Path</label>
            <input type="text" defaultValue="/etc/flowlink/tls.crt" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Log Level</label>
            <select defaultValue="info" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
              <option>trace</option><option>debug</option><option value="info">info</option><option>warn</option><option>error</option>
            </select>
          </div>
        </div>
      </div>

      {/* Shield Config */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Shield size={16} className="text-rose-400" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Shield Configuration</h3>
        </div>
        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Shield Mode</label>
            <select defaultValue="intercept" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
              <option value="monitor">Monitor (log only)</option><option value="alert">Alert</option><option value="intercept">Intercept</option><option value="enforce">Enforce</option>
            </select>
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Policy File Path</label>
            <input type="text" defaultValue="/etc/flowlink/shield-policy.yaml" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Relay URL</label>
            <input type="text" defaultValue="wss://relay.flowlink.local" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
        </div>
      </div>

      {/* Notification Settings */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
        <div className="flex items-center gap-2 mb-4">
          <Bell size={16} className="text-amber-400" />
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Notifications</h3>
        </div>
        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Telegram Bot Token</label>
            <input type="password" defaultValue="7234567890:AAH..." className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div>
            <label className="mb-1.5 block text-sm text-[var(--color-dim)]">Telegram Channel</label>
            <input type="text" defaultValue="@flowlink-alerts" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" />
          </div>
          <div className="flex flex-wrap gap-3">
            {['L3 Alerts', 'L2 Alerts', 'Canary Triggers', 'Agent Offline', 'Policy Changes'].map(label => (
              <label key={label} className="flex items-center gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm cursor-pointer">
                <input type="checkbox" defaultChecked className="rounded accent-[var(--color-accent)]" />
                {label}
              </label>
            ))}
          </div>
        </div>
      </div>

      {/* Save */}
      <button onClick={handleSave} className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-6 py-3 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)] hover:shadow-lg hover:shadow-indigo-500/20">
        <Save size={16} /> {saved ? '✓ Saved!' : 'Save Configuration'}
      </button>
    </div>
  );
}
