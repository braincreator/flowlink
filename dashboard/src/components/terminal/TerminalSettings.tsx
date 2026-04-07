import { useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Palette, MousePointerClick, Wifi, LayoutGrid, Check } from 'lucide-react';
import { themes, getTheme } from './themes';
import type { TerminalTheme } from './themes';
import ThemePreview from './ThemePreview';
import { useTerminalSettings, type TerminalSettings } from '../../hooks/useTerminalSettings';

interface TerminalSettingsProps {
  open: boolean;
  onClose: () => void;
}

const FONTS = ['"SF Mono", "Fira Code", "Cascadia Code", Menlo, monospace', '"Fira Code", Menlo, monospace', '"JetBrains Mono", Menlo, monospace', '"Cascadia Code", Menlo, monospace', 'Menlo, Monaco, monospace', 'Monaco, Menlo, monospace', '"Courier New", monospace'];
const FONT_LABELS = ['SF Mono', 'Fira Code', 'JetBrains Mono', 'Cascadia Code', 'Menlo', 'Monaco', 'Courier New'];

type Tab = 'appearance' | 'behavior' | 'session' | 'soc';

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-9 rounded-full transition-colors flex-shrink-0 ${checked ? 'bg-indigo-500' : 'bg-[var(--color-surface3)]'}`}
    >
      <span className={`absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white shadow transition-transform ${checked ? 'translate-x-4' : ''}`} />
    </button>
  );
}

function Slider({ value, min, max, step, onChange, label }: { value: number; min: number; max: number; step: number; onChange: (v: number) => void; label?: string }) {
  return (
    <div className="flex items-center gap-3">
      <input type="range" min={min} max={max} step={step} value={value}
        onChange={e => onChange(parseFloat(e.target.value))}
        className="flex-1 accent-indigo-500 h-1.5" />
      {label !== undefined && <span className="text-xs text-[var(--color-dim)] w-10 text-right">{label}</span>}
    </div>
  );
}

function SettingRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex items-center justify-between gap-4 py-2">
      <span className="text-sm text-[var(--color-text)] whitespace-nowrap">{label}</span>
      <div className="flex items-center gap-2">{children}</div>
    </label>
  );
}

function Input({ value, onChange, type = 'text', min, max }: { value: string | number; onChange: (v: string) => void; type?: string; min?: number; max?: number }) {
  return (
    <input type={type} value={value} min={min} max={max}
      onChange={e => onChange(e.target.value)}
      className="w-28 rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm font-mono text-right focus:border-indigo-500 focus:outline-none" />
  );
}

function ThemeCard({ theme, selected, onClick }: { theme: TerminalTheme; selected: boolean; onClick: () => void }) {
  const colors = [theme.colors.black, theme.colors.red, theme.colors.green, theme.colors.yellow, theme.colors.blue, theme.colors.magenta, theme.colors.cyan, theme.colors.white];
  return (
    <button
      onClick={onClick}
      className={`relative rounded-lg border-2 p-2 text-left transition-all hover:scale-[1.02] ${
        selected ? 'border-indigo-500 ring-1 ring-indigo-500/30' : 'border-[var(--color-border)] hover:border-[var(--color-dim)]'
      }`}
      style={{ background: theme.colors.background }}
    >
      {selected && (
        <div className="absolute top-1 right-1 flex h-4 w-4 items-center justify-center rounded-full bg-indigo-500">
          <Check size={10} className="text-white" />
        </div>
      )}
      <div className="text-xs font-medium mb-1.5 truncate pr-5" style={{ color: theme.colors.foreground }}>{theme.name}</div>
      <div className="grid grid-cols-8 gap-0.5">
        {colors.map((c, i) => (
          <div key={i} className="h-3 rounded-sm" style={{ background: c }} />
        ))}
      </div>
    </button>
  );
}

export default function TerminalSettings({ open, onClose }: TerminalSettingsProps) {
  const { t } = useTranslation();
  const { settings, update, reset } = useTerminalSettings();
  const [tab, setTab] = useState<Tab>('appearance');
  const [previewTheme, setPreviewTheme] = useState<TerminalTheme>(() => getTheme(settings.themeId));

  const tabs: { key: Tab; icon: typeof Palette; label: string }[] = [
    { key: 'appearance', icon: Palette, label: t('ts.appearance') },
    { key: 'behavior', icon: MousePointerClick, label: t('ts.behavior') },
    { key: 'session', icon: Wifi, label: t('ts.session_settings') },
    { key: 'soc', icon: LayoutGrid, label: t('ts.soc_settings') },
  ];

  const cursorStyles: { value: TerminalSettings['cursorStyle']; label: string }[] = [
    { value: 'block', label: `▮ ${t('ts.block')}` },
    { value: 'underline', label: `▁ ${t('ts.underline')}` },
    { value: 'bar', label: `│ ${t('ts.bar')}` },
  ];

  const gridLayouts = ['1x1', '2x2', '3x2', '3x3'];
  const feedHeights: TerminalSettings['socFeedHeight'][] = ['compact', 'normal', 'expanded'];

  const darkThemes = useMemo(() => themes.filter(th => th.colors.background.startsWith('#0') || th.colors.background.startsWith('#1') || th.colors.background.startsWith('#2')), []);
  const lightThemes = useMemo(() => themes.filter(th => !darkThemes.includes(th)), [darkThemes]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <div className="absolute inset-0 bg-black/40" onClick={onClose} />
      <div className="relative z-10 w-full max-w-lg bg-[var(--color-surface)] border-l border-[var(--color-border)] shadow-2xl flex flex-col max-h-screen">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--color-border)]">
          <h2 className="text-sm font-semibold">{t('ts.terminal_settings')}</h2>
          <div className="flex items-center gap-2">
            <button onClick={reset} className="text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
              {t('common.reset_defaults', 'Reset defaults')}
            </button>
            <button onClick={onClose} className="p-1 rounded-md hover:bg-[var(--color-surface2)] transition-colors">
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-[var(--color-border)] px-5">
          {tabs.map(tb => (
            <button key={tb.key} onClick={() => setTab(tb.key)}
              className={`flex items-center gap-1.5 px-3 py-2.5 text-xs font-medium border-b-2 transition-colors ${
                tab === tb.key
                  ? 'border-indigo-500 text-indigo-400'
                  : 'border-transparent text-[var(--color-dim)] hover:text-[var(--color-text)]'
              }`}>
              <tb.icon size={13} /> {tb.label}
            </button>
          ))}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-4">
          {/* Appearance Tab */}
          {tab === 'appearance' && (
            <>
              <div>
                <h3 className="text-xs font-semibold text-[var(--color-dim)] uppercase tracking-wider mb-3">{t('ts.theme')}</h3>
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                  {themes.map(th => (
                    <ThemeCard key={th.id} theme={th} selected={settings.themeId === th.id}
                      onClick={() => { update({ themeId: th.id }); setPreviewTheme(th); }} />
                  ))}
                </div>
              </div>

              {/* Preview */}
              <div>
                <h3 className="text-xs font-semibold text-[var(--color-dim)] uppercase tracking-wider mb-2">{t('ts.preview', 'Preview')}</h3>
                <ThemePreview theme={previewTheme} className="border border-[var(--color-border)]" />
              </div>

              <div className="space-y-3 pt-2">
                <SettingRow label={t('ts.font_family')}>
                  <select value={settings.fontFamily} onChange={e => update({ fontFamily: e.target.value })}
                    className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm focus:border-indigo-500 focus:outline-none">
                    {FONT_LABELS.map((f, i) => <option key={i} value={FONTS[i]}>{f}</option>)}
                  </select>
                </SettingRow>

                <SettingRow label={t('ts.font_size')}>
                  <Slider value={settings.fontSize} min={10} max={24} step={1}
                    onChange={v => update({ fontSize: v })} label={`${settings.fontSize}px`} />
                </SettingRow>

                <SettingRow label={t('ts.line_height')}>
                  <Slider value={settings.lineHeight} min={1} max={2} step={0.05}
                    onChange={v => update({ lineHeight: v })} label={settings.lineHeight.toFixed(2)} />
                </SettingRow>

                <div className="py-2">
                  <span className="text-sm text-[var(--color-text)] block mb-2">{t('ts.cursor_style')}</span>
                  <div className="flex gap-2">
                    {cursorStyles.map(cs => (
                      <button key={cs.value} onClick={() => update({ cursorStyle: cs.value })}
                        className={`px-3 py-1.5 rounded-md text-xs font-mono transition-colors ${
                          settings.cursorStyle === cs.value
                            ? 'bg-indigo-500/20 text-indigo-400 border border-indigo-500/50'
                            : 'border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)]'
                        }`}>
                        {cs.label}
                      </button>
                    ))}
                  </div>
                </div>

                <SettingRow label={t('ts.cursor_blink')}>
                  <Toggle checked={settings.cursorBlink} onChange={v => update({ cursorBlink: v })} />
                </SettingRow>

                <SettingRow label={t('ts.scrollback')}>
                  <Input value={settings.scrollback} onChange={v => update({ scrollback: parseInt(v) || 10000 })} type="number" min={100} max={50000} />
                </SettingRow>
              </div>
            </>
          )}

          {/* Behavior Tab */}
          {tab === 'behavior' && (
            <div className="space-y-1">
              <SettingRow label={t('ts.copy_on_select')}>
                <Toggle checked={settings.copyOnSelect} onChange={v => update({ copyOnSelect: v })} />
              </SettingRow>
              <SettingRow label={t('ts.paste_middle_click')}>
                <Toggle checked={settings.pasteWithMiddleClick} onChange={v => update({ pasteWithMiddleClick: v })} />
              </SettingRow>
              <SettingRow label={t('ts.word_separators')}>
                <Input value={settings.wordSeparators} onChange={v => update({ wordSeparators: v })} />
              </SettingRow>
              <SettingRow label={t('ts.scroll_sensitivity')}>
                <Slider value={settings.scrollSensitivity} min={0.5} max={5} step={0.5}
                  onChange={v => update({ scrollSensitivity: v })} label={`${settings.scrollSensitivity}x`} />
              </SettingRow>
              <SettingRow label={t('ts.audible_bell')}>
                <Toggle checked={settings.audibleBell} onChange={v => update({ audibleBell: v })} />
              </SettingRow>
            </div>
          )}

          {/* Session Tab */}
          {tab === 'session' && (
            <div className="space-y-1">
              <SettingRow label={t('ts.auto_reconnect')}>
                <Toggle checked={settings.autoReconnect} onChange={v => update({ autoReconnect: v })} />
              </SettingRow>
              <SettingRow label={t('ts.reconnect_delay')}>
                <Input value={settings.reconnectDelay} onChange={v => update({ reconnectDelay: parseInt(v) || 3000 })} type="number" min={500} max={30000} />
                <span className="text-xs text-[var(--color-dim)]">ms</span>
              </SettingRow>
              <SettingRow label={t('ts.close_on_disconnect')}>
                <Toggle checked={settings.closeOnDisconnect} onChange={v => update({ closeOnDisconnect: v })} />
              </SettingRow>
              <SettingRow label={t('ts.notification_on_disconnect')}>
                <Toggle checked={settings.notificationOnDisconnect} onChange={v => update({ notificationOnDisconnect: v })} />
              </SettingRow>
              <SettingRow label={t('ts.recording_format')}>
                <select value={settings.recordingFormat} onChange={e => update({ recordingFormat: e.target.value as any })}
                  className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-sm focus:border-indigo-500 focus:outline-none">
                  <option value="asciicast">asciicast v2</option>
                  <option value="raw">raw</option>
                </select>
              </SettingRow>
              <SettingRow label={t('ts.auto_record')}>
                <Toggle checked={settings.autoRecord} onChange={v => update({ autoRecord: v })} />
              </SettingRow>
            </div>
          )}

          {/* SOC Tab */}
          {tab === 'soc' && (
            <div className="space-y-1">
              <div className="py-2">
                <span className="text-sm text-[var(--color-text)] block mb-2">{t('ts.default_grid_layout')}</span>
                <div className="flex gap-2">
                  {gridLayouts.map(gl => (
                    <button key={gl} onClick={() => update({ socGridLayout: gl })}
                      className={`px-3 py-1.5 rounded-md text-xs font-mono transition-colors ${
                        settings.socGridLayout === gl
                          ? 'bg-indigo-500/20 text-indigo-400 border border-indigo-500/50'
                          : 'border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)]'
                      }`}>
                      {gl}
                    </button>
                  ))}
                </div>
              </div>
              <SettingRow label={t('ts.auto_add_agents')}>
                <Toggle checked={settings.socAutoAdd} onChange={v => update({ socAutoAdd: v })} />
              </SettingRow>
              <SettingRow label={t('ts.show_disconnected')}>
                <Toggle checked={settings.socShowDisconnected} onChange={v => update({ socShowDisconnected: v })} />
              </SettingRow>
              <SettingRow label={t('ts.alert_flash_duration')}>
                <Slider value={settings.socAlertFlash} min={1} max={10} step={0.5}
                  onChange={v => update({ socAlertFlash: v })} label={`${settings.socAlertFlash}s`} />
              </SettingRow>
              <div className="py-2">
                <span className="text-sm text-[var(--color-text)] block mb-2">{t('ts.feed_card_height')}</span>
                <div className="flex gap-2">
                  {feedHeights.map(fh => (
                    <button key={fh} onClick={() => update({ socFeedHeight: fh })}
                      className={`px-3 py-1.5 rounded-md text-xs transition-colors ${
                        settings.socFeedHeight === fh
                          ? 'bg-indigo-500/20 text-indigo-400 border border-indigo-500/50'
                          : 'border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)]'
                      }`}>
                      {t(`ts.height_${fh}`)}
                    </button>
                  ))}
                </div>
              </div>
              <SettingRow label={t('ts.status_update_interval')}>
                <Slider value={settings.socStatusInterval} min={5} max={60} step={5}
                  onChange={v => update({ socStatusInterval: v })} label={`${settings.socStatusInterval}s`} />
              </SettingRow>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export { themes };
