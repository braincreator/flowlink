import { useState, useEffect } from 'react';
import { api } from '../api/client';
import { useTranslation } from 'react-i18next';
import {
  MessageSquare, Bot, Github, Link, Code, Puzzle,
  Plus, Trash2, Check, AlertCircle, Clock, Settings,
  ChevronRight, X, Loader2, Shield, Zap, CreditCard,
  Server
} from 'lucide-react';

// ── Types ──

interface IntegrationMeta {
  kind: string;
  display_name: string;
  description: string;
  icon: string;
  category: 'messenger' | 'monitoring' | 'ci_cd' | 'productivity' | 'storage' | 'custom';
  config_schema: any;
  available_events: { event_type: string; display_name: string; description: string }[];
  supports_user_instances: boolean;
  supports_org_instances: boolean;
  requires_oauth: boolean;
  oauth_config?: { authorize_url: string; client_id: string; scopes: string };
  author: string;
  version: string;
}

interface InstalledIntegration {
  id: string;
  kind: string;
  name: string;
  status: string;
  config: any;
  subscribed_events: string[];
  org_id?: string;
  requires_oauth: boolean;
  has_tokens: boolean;
  created_at: string;
}

const CATEGORY_ICONS: Record<string, typeof MessageSquare> = {
  messenger: MessageSquare,
  monitoring: Shield,
  ci_cd: Code,
  productivity: Zap,
  storage: Server,
  custom: Link,
};

const KIND_ICONS: Record<string, typeof Bot> = {
  telegram: Bot,
  slack: MessageSquare,
  discord: MessageSquare,
  github: Github,
  max: MessageSquare,
  webhook: Link,
};

const STATUS_STYLES: Record<string, { bg: string; text: string; label: string }> = {
  active: { bg: 'bg-green-500/20', text: 'text-green-400', label: '● Активна' },
  configured: { bg: 'bg-blue-500/20', text: 'text-blue-400', label: '● Настроена' },
  pending_auth: { bg: 'bg-yellow-500/20', text: 'text-yellow-400', label: '● Ожидает авторизации' },
  paused: { bg: 'bg-gray-500/20', text: 'text-gray-400', label: '● Приостановлена' },
  token_expired: { bg: 'bg-orange-500/20', text: 'text-orange-400', label: '● Токен истёк' },
  error: { bg: 'bg-red-500/20', text: 'text-red-400', label: '● Ошибка' },
  installed: { bg: 'bg-gray-500/20', text: 'text-gray-400', label: '● Установлена' },
};

export default function Integrations() {
  const { t } = useTranslation();
  const [catalog, setCatalog] = useState<IntegrationMeta[]>([]);
  const [installed, setInstalled] = useState<InstalledIntegration[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'catalog' | 'installed'>('catalog');
  const [showInstallModal, setShowInstallModal] = useState(false);
  const [selectedKind, setSelectedKind] = useState<IntegrationMeta | null>(null);
  const [installing, setInstalling] = useState(false);
  const [configForm, setConfigForm] = useState<Record<string, any>>({});
  const [selectedEvents, setSelectedEvents] = useState<string[]>([]);
  const [error, setError] = useState('');

  useEffect(() => {
    loadData();
  }, []);

  // Handle OAuth callback status
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const status = params.get('status');
    const err = params.get('error');
    if (status === 'connected') {
      setActiveTab('installed');
      loadData();
    }
    if (err) {
      setError(decodeURIComponent(err));
    }
    // Clean URL
    if (status || err) {
      const clean = new URL(window.location.href);
      clean.searchParams.delete('status');
      clean.searchParams.delete('error');
      window.history.replaceState({}, '', clean.pathname + clean.hash);
    }
  }, []);

  async function loadData() {
    setLoading(true);
    try {
      const [cat, inst] = await Promise.all([
        api.getIntegrationCatalog(),
        api.getIntegrations().catch(() => []),
      ]);
      setCatalog(cat.integrations || []);
      setInstalled(Array.isArray(inst) ? inst : []);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }

  function openInstall(meta: IntegrationMeta) {
    setSelectedKind(meta);
    setConfigForm({});
    setSelectedEvents(meta.available_events.map(e => e.event_type));
    setError('');
    setShowInstallModal(true);
  }

  async function handleInstall() {
    if (!selectedKind) return;
    setInstalling(true);
    setError('');

    try {
      if (selectedKind.requires_oauth) {
        // OAuth flow: begin → redirect
        const res = await api.beginOAuthIntegration({
          kind: selectedKind.kind,
          name: selectedKind.display_name,
          subscribed_events: selectedEvents,
        });
        window.location.href = res.authorize_url;
        return;
      }

      // Direct install (Telegram, Webhook)
      await api.installIntegration({
        kind: selectedKind.kind,
        name: selectedKind.display_name,
        config: configForm,
        subscribed_events: selectedEvents,
      });

      setShowInstallModal(false);
      await loadData();
      setActiveTab('installed');
    } catch (e: any) {
      setError(e.message);
    } finally {
      setInstalling(false);
    }
  }

  async function handleUninstall(id: string) {
    if (!confirm('Удалить интеграцию?')) return;
    try {
      await api.uninstallIntegration(id);
      await loadData();
    } catch (e: any) {
      setError(e.message);
    }
  }

  const installedKinds = new Set(installed.map(i => i.kind));

  // ── Render ──

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-2">
            <Puzzle className="w-6 h-6 text-blue-400" />
            {t('nav.integrations', 'Интеграции')}
          </h1>
          <p className="text-sm text-gray-400 mt-1">
            Подключайте мессенджеры, CI/CD и другие сервисы для уведомлений и автоматизации
          </p>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 flex items-center gap-2 text-red-400 text-sm">
          <AlertCircle className="w-4 h-4 shrink-0" />
          {error}
          <button onClick={() => setError('')} className="ml-auto"><X className="w-4 h-4" /></button>
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-1 bg-gray-800/50 rounded-lg p-1 w-fit">
        <button
          onClick={() => setActiveTab('catalog')}
          className={`px-4 py-2 rounded-md text-sm font-medium transition ${
            activeTab === 'catalog' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-white'
          }`}
        >
          📦 Каталог ({catalog.length})
        </button>
        <button
          onClick={() => setActiveTab('installed')}
          className={`px-4 py-2 rounded-md text-sm font-medium transition ${
            activeTab === 'installed' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-white'
          }`}
        >
          ✅ Мои ({installed.length})
        </button>
      </div>

      {loading ? (
        <div className="flex items-center justify-center py-20 text-gray-400">
          <Loader2 className="w-6 h-6 animate-spin mr-2" /> Загрузка...
        </div>
      ) : activeTab === 'catalog' ? (
        /* ── Catalog Grid ── */
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {catalog.map(meta => {
            const isInstalled = installedKinds.has(meta.kind);
            const CatIcon = CATEGORY_ICONS[meta.category] || Puzzle;
            const KindIcon = KIND_ICONS[meta.kind] || CatIcon;

            return (
              <div key={meta.kind} className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-5 hover:border-gray-600 transition group">
                <div className="flex items-start justify-between mb-3">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-lg bg-gray-700/50 flex items-center justify-center text-xl">
                      {meta.icon}
                    </div>
                    <div>
                      <h3 className="font-semibold text-white">{meta.display_name}</h3>
                      <span className="text-xs text-gray-500 uppercase">{meta.category.replace('_', '/')}</span>
                    </div>
                  </div>
                  {meta.requires_oauth && (
                    <span className="text-[10px] px-2 py-0.5 rounded-full bg-purple-500/20 text-purple-400 font-medium">
                      OAuth2
                    </span>
                  )}
                </div>

                <p className="text-sm text-gray-400 mb-4 line-clamp-2">{meta.description}</p>

                <div className="flex items-center justify-between">
                  <span className="text-xs text-gray-500">v{meta.version} · {meta.author}</span>
                  {isInstalled ? (
                    <span className="text-xs text-green-400 flex items-center gap-1">
                      <Check className="w-3 h-3" /> Установлена
                    </span>
                  ) : (
                    <button
                      onClick={() => openInstall(meta)}
                      className="flex items-center gap-1 text-sm px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white rounded-lg transition"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      {meta.requires_oauth ? 'Подключить' : 'Установить'}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        /* ── Installed List ── */
        installed.length === 0 ? (
          <div className="text-center py-16 text-gray-500">
            <Puzzle className="w-12 h-12 mx-auto mb-3 opacity-50" />
            <p className="text-lg">Нет установленных интеграций</p>
            <p className="text-sm mt-1">Перейдите в каталог, чтобы подключить сервис</p>
            <button
              onClick={() => setActiveTab('catalog')}
              className="mt-4 text-blue-400 hover:text-blue-300 text-sm flex items-center gap-1 mx-auto"
            >
              Открыть каталог <ChevronRight className="w-3 h-3" />
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            {installed.map(integ => {
              const status = STATUS_STYLES[integ.status] || STATUS_STYLES.error;
              const meta = catalog.find(c => c.kind === integ.kind);
              const KindIcon = KIND_ICONS[integ.kind] || Puzzle;

              return (
                <div key={integ.id} className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4 flex items-center justify-between">
                  <div className="flex items-center gap-4">
                    <div className="w-10 h-10 rounded-lg bg-gray-700/50 flex items-center justify-center text-xl">
                      {meta?.icon || '🔌'}
                    </div>
                    <div>
                      <h3 className="font-medium text-white flex items-center gap-2">
                        {integ.name || integ.kind}
                        <span className={`text-[11px] px-2 py-0.5 rounded-full ${status.bg} ${status.text}`}>
                          {status.label}
                        </span>
                        {integ.org_id && (
                          <span className="text-[10px] px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-400">
                            Организация
                          </span>
                        )}
                      </h3>
                      <p className="text-xs text-gray-500 mt-0.5">
                        {integ.kind} · {new Date(integ.created_at).toLocaleDateString('ru-RU')}
                        {integ.subscribed_events.length > 0 && ` · ${integ.subscribed_events.length} событий`}
                      </p>
                    </div>
                  </div>
                  <button
                    onClick={() => handleUninstall(integ.id)}
                    className="p-2 text-gray-500 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition"
                    title="Удалить"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              );
            })}
          </div>
        )
      )}

      {/* ── Install Modal ── */}
      {showInstallModal && selectedKind && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4" onClick={() => setShowInstallModal(false)}>
          <div className="bg-gray-900 border border-gray-700 rounded-2xl max-w-lg w-full max-h-[80vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
            <div className="p-6">
              {/* Header */}
              <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                  <span className="text-2xl">{selectedKind.icon}</span>
                  <div>
                    <h2 className="text-lg font-bold text-white">{selectedKind.display_name}</h2>
                    <p className="text-xs text-gray-500">{selectedKind.description}</p>
                  </div>
                </div>
                <button onClick={() => setShowInstallModal(false)} className="text-gray-500 hover:text-white">
                  <X className="w-5 h-5" />
                </button>
              </div>

              {selectedKind.requires_oauth ? (
                /* OAuth flow info */
                <div className="space-y-4">
                  <div className="bg-purple-500/10 border border-purple-500/30 rounded-lg p-4">
                    <p className="text-sm text-purple-300">
                      Для подключения {selectedKind.display_name} требуется авторизация через OAuth2.
                      Вы будете перенаправлены на сайт {selectedKind.display_name} для предоставления доступа.
                    </p>
                  </div>
                  {selectedKind.oauth_config && (
                    <div className="text-xs text-gray-500">
                      Запрашиваемые права: <code className="text-gray-400">{selectedKind.oauth_config.scopes}</code>
                    </div>
                  )}
                </div>
              ) : (
                /* Config form */
                <div className="space-y-4">
                  {Object.entries(selectedKind.config_schema?.properties || {}).map(([key, field]: [string, any]) => (
                    <div key={key}>
                      <label className="block text-sm font-medium text-gray-300 mb-1">
                        {field.title || key}
                        {selectedKind.config_schema?.required?.includes(key) && <span className="text-red-400 ml-1">*</span>}
                      </label>
                      <input
                        type={field.type === 'integer' ? 'number' : 'text'}
                        placeholder={field.description || ''}
                        value={configForm[key] ?? ''}
                        onChange={e => setConfigForm({ ...configForm, [key]: e.target.value })}
                        className="w-full px-3 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white text-sm placeholder-gray-500 focus:border-blue-500 focus:outline-none"
                      />
                    </div>
                  ))}
                </div>
              )}

              {/* Event subscriptions */}
              <div className="mt-6">
                <h3 className="text-sm font-medium text-gray-300 mb-3">Подписка на события</h3>
                <div className="grid grid-cols-2 gap-2">
                  {selectedKind.available_events.map(ev => (
                    <label key={ev.event_type} className="flex items-center gap-2 text-sm text-gray-400 cursor-pointer hover:text-gray-300">
                      <input
                        type="checkbox"
                        checked={selectedEvents.includes(ev.event_type)}
                        onChange={e => {
                          if (e.target.checked) {
                            setSelectedEvents([...selectedEvents, ev.event_type]);
                          } else {
                            setSelectedEvents(selectedEvents.filter(s => s !== ev.event_type));
                          }
                        }}
                        className="rounded border-gray-600 bg-gray-800 text-blue-500 focus:ring-blue-500"
                      />
                      {ev.display_name}
                    </label>
                  ))}
                </div>
              </div>

              {/* Actions */}
              <div className="mt-6 flex justify-end gap-3">
                <button
                  onClick={() => setShowInstallModal(false)}
                  className="px-4 py-2 text-sm text-gray-400 hover:text-white transition"
                >
                  Отмена
                </button>
                <button
                  onClick={handleInstall}
                  disabled={installing}
                  className="px-6 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white text-sm font-medium rounded-lg transition flex items-center gap-2"
                >
                  {installing && <Loader2 className="w-4 h-4 animate-spin" />}
                  {selectedKind.requires_oauth ? 'Подключить через OAuth' : 'Установить'}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
