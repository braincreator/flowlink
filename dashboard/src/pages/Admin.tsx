import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../api/client';
import { useToast } from '../hooks/useToast';
import {
  LineChart, BarChart, Line, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer,
} from 'recharts';
import {
  Users, UserCheck, TrendingUp, Shield, ShieldAlert, ShieldCheck,
  Search, RefreshCw, ChevronLeft, ChevronRight, Filter,
  Plus, Pencil, Trash2, X, LayoutDashboard, CreditCard, Repeat, ShoppingCart,
} from 'lucide-react';

const fmt = (n: number) => new Intl.NumberFormat('ru-RU').format(n);
const fmtDate = (d: string) => new Date(d).toLocaleDateString('ru-RU');
const fmtMoney = (n: number) => `${fmt(Math.round(n))} ₽`;

const PLANS = ['free', 'starter', 'pro', 'business', 'enterprise'];
const PLAN_COLORS: Record<string, string> = {
  free: 'bg-gray-500/20 text-gray-400',
  starter: 'bg-blue-500/20 text-blue-400',
  pro: 'bg-purple-500/20 text-purple-400',
  business: 'bg-amber-500/20 text-amber-400',
  enterprise: 'bg-emerald-500/20 text-emerald-400',
};
const PLAN_BAR_COLORS: Record<string, string> = {
  free: '#6b7280',
  starter: '#3b82f6',
  pro: '#a855f7',
  business: '#f59e0b',
  enterprise: '#10b981',
};

type TabId = 'dashboard' | 'plans' | 'subscriptions' | 'orders';

const TABS: { id: TabId; label: string; icon: typeof LayoutDashboard }[] = [
  { id: 'dashboard', label: 'Дашборд', icon: LayoutDashboard },
  { id: 'plans', label: 'Тарифы', icon: CreditCard },
  { id: 'subscriptions', label: 'Подписки', icon: Repeat },
  { id: 'orders', label: 'Заказы', icon: ShoppingCart },
];

interface Account {
  account_id: string;
  plan_id: string;
  active: boolean;
  email: string;
  tg_id: string | null;
  totp_enabled: boolean;
  last_login: string;
  created_at: string;
}

interface Stats {
  total_users: number;
  active_users: number;
  users_2fa: number;
  total_revenue_rub: number;
  mrr_rub: number;
  arr_rub: number;
  active_subscriptions: number;
  churned_this_month: number;
  plan_distribution: { plan: string; count: number }[];
  new_users_chart: { date: string; count: number }[];
  revenue_chart: { month: string; revenue_rub: number }[];
}

interface Plan {
  id: string;
  name: string;
  description: string;
  tier: number;
  price_kopecks: number;
  annual_price_kopecks: number;
  period: string;
  trial_days: number;
  is_active: boolean;
  sort_order: number;
  limits: Record<string, any>;
  features: string[];
  created_at: string;
  updated_at: string;
}

interface PlanFormData {
  name: string;
  description: string;
  tier: number;
  price_kopecks: number;
  annual_price_kopecks: number;
  period: string;
  trial_days: number;
  is_active: boolean;
  sort_order: number;
  limits: string;
  features: string;
}

const EMPTY_PLAN: PlanFormData = {
  name: '',
  description: '',
  tier: 0,
  price_kopecks: 0,
  annual_price_kopecks: 0,
  period: 'month',
  trial_days: 0,
  is_active: true,
  sort_order: 0,
  limits: '{}',
  features: '[]',
};

interface Subscription {
  subscription_id: string;
  account_id: string;
  email?: string;
  plan_id: string;
  status: string;
  period: string;
  amount_kopecks: number;
  started_at: string;
  expires_at: string;
  cancelled_at: string | null;
}

interface Order {
  order_id: string;
  account_id: string;
  email?: string;
  plan_id: string;
  amount_kopecks: number;
  status: string;
  created_at: string;
  paid_at: string | null;
}

const PAGE_SIZE = 20;

// ─── Status badge helper ───
function Badge({ status, map }: { status: string; map: Record<string, string> }) {
  const cls = map[status] || 'bg-gray-500/20 text-gray-400';
  return <span className={`inline-block rounded-full px-2.5 py-0.5 text-xs font-medium ${cls}`}>{status}</span>;
}

const SUB_STATUS_MAP: Record<string, string> = {
  active: 'bg-emerald-500/20 text-emerald-400',
  cancelled: 'bg-amber-500/20 text-amber-400',
  expired: 'bg-red-500/20 text-red-400',
  trialing: 'bg-blue-500/20 text-blue-400',
  past_due: 'bg-orange-500/20 text-orange-400',
};

const ORDER_STATUS_MAP: Record<string, string> = {
  paid: 'bg-emerald-500/20 text-emerald-400',
  pending: 'bg-amber-500/20 text-amber-400',
  failed: 'bg-red-500/20 text-red-400',
  refunded: 'bg-gray-500/20 text-gray-400',
};

// ─── Plan form panel ───
function PlanPanel({
  data, onChange, onSave, onClose, isEdit,
}: {
  data: PlanFormData;
  onChange: (d: PlanFormData) => void;
  onSave: () => void;
  onClose: () => void;
  isEdit: boolean;
}) {
  const set = <K extends keyof PlanFormData>(k: K, v: PlanFormData[K]) =>
    onChange({ ...data, [k]: v });

  const inputCls =
    'w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]';
  const labelCls = 'block text-xs font-medium text-[var(--color-dim)] mb-1';

  return (
    <div className="fixed inset-0 z-50 flex justify-end" onClick={onClose}>
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40" />
      {/* Panel */}
      <div
        className="relative w-full max-w-lg overflow-y-auto bg-[var(--color-surface)] border-l border-[var(--color-border)] p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-lg font-semibold text-[var(--color-text)]">
            {isEdit ? 'Редактировать тариф' : 'Новый тариф'}
          </h2>
          <button onClick={onClose} className="text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
            <X size={20} />
          </button>
        </div>

        <div className="space-y-4">
          <div>
            <label className={labelCls}>Название</label>
            <input className={inputCls} value={data.name} onChange={(e) => set('name', e.target.value)} />
          </div>
          <div>
            <label className={labelCls}>Описание</label>
            <textarea className={inputCls + ' min-h-[60px] resize-y'} value={data.description} onChange={(e) => set('description', e.target.value)} />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className={labelCls}>Tier</label>
              <input type="number" className={inputCls} value={data.tier} onChange={(e) => set('tier', Number(e.target.value))} />
            </div>
            <div>
              <label className={labelCls}>Sort order</label>
              <input type="number" className={inputCls} value={data.sort_order} onChange={(e) => set('sort_order', Number(e.target.value))} />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className={labelCls}>Цена (копейки)</label>
              <input type="number" className={inputCls} value={data.price_kopecks} onChange={(e) => set('price_kopecks', Number(e.target.value))} />
            </div>
            <div>
              <label className={labelCls}>Годовая цена (коп.)</label>
              <input type="number" className={inputCls} value={data.annual_price_kopecks} onChange={(e) => set('annual_price_kopecks', Number(e.target.value))} />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className={labelCls}>Период</label>
              <select className={inputCls} value={data.period} onChange={(e) => set('period', e.target.value)}>
                <option value="month">Месяц</option>
                <option value="year">Год</option>
                <option value="lifetime">Навсегда</option>
              </select>
            </div>
            <div>
              <label className={labelCls}>Триал (дни)</label>
              <input type="number" className={inputCls} value={data.trial_days} onChange={(e) => set('trial_days', Number(e.target.value))} />
            </div>
          </div>
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="plan-active"
              checked={data.is_active}
              onChange={(e) => set('is_active', e.target.checked)}
              className="rounded border-[var(--color-border)]"
            />
            <label htmlFor="plan-active" className="text-sm text-[var(--color-text)]">Активен</label>
          </div>
          <div>
            <label className={labelCls}>Лимиты (JSON)</label>
            <textarea
              className={inputCls + ' min-h-[80px] font-mono text-xs resize-y'}
              value={data.limits}
              onChange={(e) => set('limits', e.target.value)}
            />
          </div>
          <div>
            <label className={labelCls}>Фичи (JSON массив)</label>
            <textarea
              className={inputCls + ' min-h-[80px] font-mono text-xs resize-y'}
              value={data.features}
              onChange={(e) => set('features', e.target.value)}
            />
          </div>
          <button
            onClick={onSave}
            className="w-full rounded-lg bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white hover:opacity-90 transition-opacity"
          >
            {isEdit ? 'Сохранить' : 'Создать'}
          </button>
        </div>
      </div>
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════════
export default function Admin() {
  const { t } = useTranslation();
  const toast = useToast();

  // Tab
  const [tab, setTab] = useState<TabId>('dashboard');

  // Date range for stats
  const [from, setFrom] = useState(() => {
    const d = new Date();
    d.setMonth(d.getMonth() - 1);
    return d.toISOString().slice(0, 10);
  });
  const [to, setTo] = useState(() => new Date().toISOString().slice(0, 10));

  // Stats
  const [stats, setStats] = useState<Stats | null>(null);
  const [statsLoading, setStatsLoading] = useState(true);

  // Accounts
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [accountsLoading, setAccountsLoading] = useState(false);
  const [accountPage, setAccountPage] = useState(1);
  const [planFilter, setPlanFilter] = useState('');
  const [activeFilter, setActiveFilter] = useState<string>('');
  const [searchFilter, setSearchFilter] = useState('');
  const [accFrom, setAccFrom] = useState('');
  const [accTo, setAccTo] = useState('');

  // Plans
  const [plans, setPlans] = useState<Plan[]>([]);
  const [plansLoading, setPlansLoading] = useState(false);
  const [planPanelOpen, setPlanPanelOpen] = useState(false);
  const [planEditing, setPlanEditing] = useState<Plan | null>(null);
  const [planForm, setPlanForm] = useState<PlanFormData>(EMPTY_PLAN);

  // Subscriptions
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [subsLoading, setSubsLoading] = useState(false);
  const [subStatusFilter, setSubStatusFilter] = useState('');

  // Orders
  const [orders, setOrders] = useState<Order[]>([]);
  const [ordersLoading, setOrdersLoading] = useState(false);
  const [orderStatusFilter, setOrderStatusFilter] = useState('');

  // ─── Fetchers ───

  const fetchStats = useCallback(async () => {
    setStatsLoading(true);
    try {
      const res = await api.getAdminStats(from, to);
      setStats(res);
    } catch {
      toast.error('Ошибка загрузки статистики');
    } finally {
      setStatsLoading(false);
    }
  }, [from, to, toast]);

  const fetchAccounts = useCallback(async (page: number) => {
    setAccountsLoading(true);
    try {
      const filters: Record<string, string> = {};
      if (planFilter) filters.plan = planFilter;
      if (activeFilter) filters.active = activeFilter;
      if (searchFilter) filters.search = searchFilter;
      if (accFrom) filters.from = accFrom;
      if (accTo) filters.to = accTo;
      const res = await api.getAdminAccounts(filters);
      setAccounts(res.accounts || []);
      setAccountPage(page);
    } catch {
      toast.error('Ошибка загрузки аккаунтов');
    } finally {
      setAccountsLoading(false);
    }
  }, [planFilter, activeFilter, searchFilter, accFrom, accTo, toast]);

  const fetchPlans = useCallback(async () => {
    setPlansLoading(true);
    try {
      const res = await api.adminGetPlans();
      setPlans(res);
    } catch {
      toast.error('Ошибка загрузки тарифов');
    } finally {
      setPlansLoading(false);
    }
  }, [toast]);

  const fetchSubscriptions = useCallback(async () => {
    setSubsLoading(true);
    try {
      const res = await api.adminGetSubscriptions();
      setSubscriptions(res);
    } catch {
      toast.error('Ошибка загрузки подписок');
    } finally {
      setSubsLoading(false);
    }
  }, [toast]);

  const fetchOrders = useCallback(async () => {
    setOrdersLoading(true);
    try {
      const res = await api.adminGetOrders();
      setOrders(res);
    } catch {
      toast.error('Ошибка загрузки заказов');
    } finally {
      setOrdersLoading(false);
    }
  }, [toast]);

  // ─── Effects ───

  useEffect(() => { fetchStats(); }, [fetchStats]);
  useEffect(() => { fetchAccounts(1); }, [fetchAccounts]);
  useEffect(() => {
    if (tab === 'plans') fetchPlans();
    else if (tab === 'subscriptions') fetchSubscriptions();
    else if (tab === 'orders') fetchOrders();
  }, [tab, fetchPlans, fetchSubscriptions, fetchOrders]);

  // ─── Account actions ───

  const handleToggleActive = async (id: string) => {
    try {
      await api.adminToggleActive(id);
      toast.success('Статус обновлён');
      fetchAccounts(accountPage);
      fetchStats();
    } catch {
      toast.error('Ошибка обновления статуса');
    }
  };

  const handleChangePlan = async (id: string, planId: string) => {
    try {
      await api.adminChangePlan(id, planId);
      toast.success('Тариф изменён');
      fetchAccounts(accountPage);
      fetchStats();
    } catch {
      toast.error('Ошибка изменения тарифа');
    }
  };

  // ─── Plan actions ───

  const openNewPlan = () => {
    setPlanEditing(null);
    setPlanForm(EMPTY_PLAN);
    setPlanPanelOpen(true);
  };

  const openEditPlan = (plan: Plan) => {
    setPlanEditing(plan);
    setPlanForm({
      name: plan.name,
      description: plan.description,
      tier: plan.tier,
      price_kopecks: plan.price_kopecks,
      annual_price_kopecks: plan.annual_price_kopecks,
      period: plan.period,
      trial_days: plan.trial_days,
      is_active: plan.is_active,
      sort_order: plan.sort_order,
      limits: JSON.stringify(plan.limits || {}, null, 2),
      features: JSON.stringify(plan.features || [], null, 2),
    });
    setPlanPanelOpen(true);
  };

  const handleSavePlan = async () => {
    if (!planForm.name.trim()) {
      toast.error('Название обязательно');
      return;
    }
    let limits: Record<string, any>;
    let features: string[];
    try { limits = JSON.parse(planForm.limits); } catch { toast.error('Невалидный JSON в лимитах'); return; }
    try { features = JSON.parse(planForm.features); } catch { toast.error('Невалидный JSON в фичах'); return; }

    const payload = {
      name: planForm.name,
      description: planForm.description,
      tier: planForm.tier,
      price_kopecks: planForm.price_kopecks,
      annual_price_kopecks: planForm.annual_price_kopecks,
      period: planForm.period,
      trial_days: planForm.trial_days,
      is_active: planForm.is_active,
      sort_order: planForm.sort_order,
      limits,
      features,
    };

    try {
      if (planEditing) {
        await api.adminUpdatePlan(planEditing.id, payload);
        toast.success('Тариф обновлён');
      } else {
        await api.adminCreatePlan(payload);
        toast.success('Тариф создан');
      }
      setPlanPanelOpen(false);
      fetchPlans();
    } catch {
      toast.error('Ошибка сохранения тарифа');
    }
  };

  const handleDeletePlan = async (plan: Plan) => {
    if (!confirm(`Деактивировать тариф «${plan.name}»?`)) return;
    try {
      await api.adminDeletePlan(plan.id);
      toast.success('Тариф деактивирован');
      fetchPlans();
    } catch {
      toast.error('Ошибка деактивации тарифа');
    }
  };

  // ─── Dashboard content ───

  const maxPlanCount = stats ? Math.max(...stats.plan_distribution.map((p) => p.count), 1) : 1;

  const kpis = stats
    ? [
        { label: 'Всего пользователей', value: fmt(stats.total_users), icon: Users, color: 'text-blue-400' },
        { label: 'Активные', value: fmt(stats.active_users), icon: UserCheck, color: 'text-emerald-400' },
        { label: 'MRR', value: fmtMoney(stats.mrr_rub), icon: TrendingUp, color: 'text-purple-400' },
        { label: 'ARR', value: fmtMoney(stats.arr_rub), icon: TrendingUp, color: 'text-indigo-400' },
        { label: 'Активные подписки', value: fmt(stats.active_subscriptions), icon: Shield, color: 'text-amber-400' },
        { label: 'Отток (мес)', value: fmt(stats.churned_this_month), icon: ShieldAlert, color: 'text-red-400' },
      ]
    : [];

  const tickStyle = { fill: 'var(--color-dim)', fontSize: 11 };
  const gridStroke = 'var(--color-border)';
  const tooltipStyle = {
    backgroundColor: 'var(--color-surface)',
    border: '1px solid var(--color-border)',
    borderRadius: 8,
    color: 'var(--color-text)',
    fontSize: 12,
  };

  const renderDashboard = () => (
    <>
      {/* Date Range */}
      <div className="flex flex-wrap items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <Filter size={16} className="text-[var(--color-dim)]" />
        <span className="text-sm text-[var(--color-dim)]">Период:</span>
        <input
          type="date"
          value={from}
          onChange={(e) => setFrom(e.target.value)}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
        />
        <span className="text-[var(--color-dim)]">—</span>
        <input
          type="date"
          value={to}
          onChange={(e) => setTo(e.target.value)}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
        />
        <button
          onClick={fetchStats}
          disabled={statsLoading}
          className="ml-2 flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface2)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-accent)]/10 transition-colors disabled:opacity-50"
        >
          <RefreshCw size={14} className={statsLoading ? 'animate-spin' : ''} />
          Обновить
        </button>
      </div>

      {/* KPI Cards */}
      {statsLoading && !stats ? (
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="h-24 animate-pulse rounded-xl bg-[var(--color-surface)] border border-[var(--color-border)]" />
          ))}
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
          {kpis.map((kpi) => (
            <div key={kpi.label} className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
              <div className="flex items-center justify-between mb-2">
                <kpi.icon size={18} className={kpi.color} />
              </div>
              <div className="text-xl font-bold text-[var(--color-text)]">{kpi.value}</div>
              <div className="text-xs text-[var(--color-dim)] mt-1">{kpi.label}</div>
            </div>
          ))}
        </div>
      )}

      {/* Charts */}
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <h3 className="text-sm font-semibold text-[var(--color-text)] mb-4">Новые пользователи</h3>
          {stats?.new_users_chart?.length ? (
            <ResponsiveContainer width="100%" height={240}>
              <LineChart data={stats.new_users_chart}>
                <XAxis dataKey="date" tick={tickStyle} tickFormatter={(d: string) => d.slice(5)} stroke={gridStroke} tickLine={false} axisLine={false} />
                <YAxis tick={tickStyle} stroke={gridStroke} tickLine={false} axisLine={false} allowDecimals={false} />
                <Tooltip contentStyle={tooltipStyle} formatter={(v: number) => [v, 'Пользователей']} labelFormatter={(l: string) => `Дата: ${l}`} />
                <Line type="monotone" dataKey="count" stroke="var(--color-accent)" strokeWidth={2} dot={false} activeDot={{ r: 4 }} />
              </LineChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex h-[240px] items-center justify-center text-sm text-[var(--color-dim)]">Нет данных</div>
          )}
        </div>

        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
          <h3 className="text-sm font-semibold text-[var(--color-text)] mb-4">Выручка</h3>
          {stats?.revenue_chart?.length ? (
            <ResponsiveContainer width="100%" height={240}>
              <BarChart data={stats.revenue_chart}>
                <XAxis dataKey="month" tick={tickStyle} tickFormatter={(d: string) => d.slice(2)} stroke={gridStroke} tickLine={false} axisLine={false} />
                <YAxis tick={tickStyle} stroke={gridStroke} tickLine={false} axisLine={false} tickFormatter={(v: number) => `${(v / 1000).toFixed(0)}к`} />
                <Tooltip contentStyle={tooltipStyle} formatter={(v: number) => [fmtMoney(v), 'Выручка']} labelFormatter={(l: string) => `Месяц: ${l}`} />
                <Bar dataKey="revenue_rub" fill="var(--color-accent)" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex h-[240px] items-center justify-center text-sm text-[var(--color-dim)]">Нет данных</div>
          )}
        </div>
      </div>

      {/* Plan Distribution */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <h3 className="text-sm font-semibold text-[var(--color-text)] mb-4">Распределение по тарифам</h3>
        {stats?.plan_distribution?.length ? (
          <div className="space-y-3">
            {stats.plan_distribution.map((p) => (
              <div key={p.plan} className="flex items-center gap-3">
                <span className="w-20 text-sm text-[var(--color-dim)] font-medium">{p.plan}</span>
                <div className="flex-1 h-6 rounded-full bg-[var(--color-bg)] overflow-hidden">
                  <div
                    className="h-full rounded-full transition-all duration-500"
                    style={{ width: `${(p.count / maxPlanCount) * 100}%`, backgroundColor: PLAN_BAR_COLORS[p.plan] || 'var(--color-accent)' }}
                  />
                </div>
                <span className="w-10 text-right text-sm text-[var(--color-text)] font-medium">{p.count}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="flex h-16 items-center justify-center text-sm text-[var(--color-dim)]">Нет данных</div>
        )}
      </div>

      {/* Accounts Table */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]">
        <div className="flex flex-wrap items-center gap-3 border-b border-[var(--color-border)] p-4">
          <Search size={16} className="text-[var(--color-dim)]" />
          <input
            type="text"
            placeholder="Поиск по email..."
            value={searchFilter}
            onChange={(e) => setSearchFilter(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)] w-48"
          />
          <select
            value={planFilter}
            onChange={(e) => setPlanFilter(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
          >
            <option value="">Все тарифы</option>
            {PLANS.map((p) => <option key={p} value={p}>{p}</option>)}
          </select>
          <select
            value={activeFilter}
            onChange={(e) => setActiveFilter(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
          >
            <option value="">Все статусы</option>
            <option value="true">Активные</option>
            <option value="false">Неактивные</option>
          </select>
          <input
            type="date"
            value={accFrom}
            onChange={(e) => setAccFrom(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
          />
          <input
            type="date"
            value={accTo}
            onChange={(e) => setAccTo(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
          />
          <button
            onClick={() => fetchAccounts(1)}
            className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface2)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-accent)]/10 transition-colors"
          >
            <RefreshCw size={14} className={accountsLoading ? 'animate-spin' : ''} />
            Найти
          </button>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-[var(--color-border)] text-left text-[var(--color-dim)]">
                <th className="px-4 py-3 font-medium">Email</th>
                <th className="px-4 py-3 font-medium">Тариф</th>
                <th className="px-4 py-3 font-medium">Статус</th>
                <th className="px-4 py-3 font-medium">2FA</th>
                <th className="px-4 py-3 font-medium">Последний вход</th>
                <th className="px-4 py-3 font-medium">Создан</th>
                <th className="px-4 py-3 font-medium">Действия</th>
              </tr>
            </thead>
            <tbody>
              {accountsLoading ? (
                Array.from({ length: 5 }).map((_, i) => (
                  <tr key={i} className="border-b border-[var(--color-border)]">
                    {Array.from({ length: 7 }).map((_, j) => (
                      <td key={j} className="px-4 py-3"><div className="h-4 w-24 animate-pulse rounded bg-[var(--color-bg)]" /></td>
                    ))}
                  </tr>
                ))
              ) : accounts.length === 0 ? (
                <tr><td colSpan={7} className="px-4 py-8 text-center text-[var(--color-dim)]">Нет аккаунтов</td></tr>
              ) : (
                accounts.slice(0, PAGE_SIZE).map((acc) => (
                  <tr key={acc.account_id} className="border-b border-[var(--color-border)] hover:bg-[var(--color-surface2)]/50 transition-colors">
                    <td className="px-4 py-3">
                      <div className="font-medium text-[var(--color-text)]">{acc.email}</div>
                      <div className="text-xs text-[var(--color-dim)] font-mono">{acc.account_id.slice(0, 12)}…</div>
                    </td>
                    <td className="px-4 py-3">
                      <span className={`inline-block rounded-full px-2.5 py-0.5 text-xs font-medium ${PLAN_COLORS[acc.plan_id] || 'bg-gray-500/20 text-gray-400'}`}>
                        {acc.plan_id}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <span className={`inline-block rounded-full px-2.5 py-0.5 text-xs font-medium ${acc.active ? 'bg-emerald-500/20 text-emerald-400' : 'bg-red-500/20 text-red-400'}`}>
                        {acc.active ? 'Активен' : 'Неактивен'}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      {acc.totp_enabled
                        ? <ShieldCheck size={16} className="text-emerald-400" />
                        : <Shield size={16} className="text-[var(--color-dim)] opacity-40" />}
                    </td>
                    <td className="px-4 py-3 text-[var(--color-dim)]">{acc.last_login ? fmtDate(acc.last_login) : '—'}</td>
                    <td className="px-4 py-3 text-[var(--color-dim)]">{acc.created_at ? fmtDate(acc.created_at) : '—'}</td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() => handleToggleActive(acc.account_id)}
                          className={`rounded-lg border px-2 py-1 text-xs font-medium transition-colors ${acc.active
                            ? 'border-red-500/30 text-red-400 hover:bg-red-500/10'
                            : 'border-emerald-500/30 text-emerald-400 hover:bg-emerald-500/10'}`}
                        >
                          {acc.active ? 'Откл.' : 'Акт.'}
                        </button>
                        <select
                          defaultValue={acc.plan_id}
                          onChange={(e) => e.target.value !== acc.plan_id && handleChangePlan(acc.account_id, e.target.value)}
                          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-xs text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
                        >
                          {PLANS.map((p) => <option key={p} value={p}>{p}</option>)}
                        </select>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </>
  );

  // ─── Plans tab ───

  const renderPlans = () => (
    <>
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Тарифы</h2>
        <button
          onClick={openNewPlan}
          className="flex items-center gap-1.5 rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90 transition-opacity"
        >
          <Plus size={16} />
          Добавить тариф
        </button>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-[var(--color-border)] text-left text-[var(--color-dim)]">
              <th className="px-4 py-3 font-medium">Название</th>
              <th className="px-4 py-3 font-medium">Цена</th>
              <th className="px-4 py-3 font-medium">Период</th>
              <th className="px-4 py-3 font-medium">Триал</th>
              <th className="px-4 py-3 font-medium">Tier</th>
              <th className="px-4 py-3 font-medium">Статус</th>
              <th className="px-4 py-3 font-medium">Действия</th>
            </tr>
          </thead>
          <tbody>
            {plansLoading ? (
              Array.from({ length: 5 }).map((_, i) => (
                <tr key={i} className="border-b border-[var(--color-border)]">
                  {Array.from({ length: 7 }).map((_, j) => (
                    <td key={j} className="px-4 py-3"><div className="h-4 w-24 animate-pulse rounded bg-[var(--color-bg)]" /></td>
                  ))}
                </tr>
              ))
            ) : plans.length === 0 ? (
              <tr><td colSpan={7} className="px-4 py-8 text-center text-[var(--color-dim)]">Нет тарифов</td></tr>
            ) : (
              plans.map((p) => (
                <tr key={p.id} className="border-b border-[var(--color-border)] hover:bg-[var(--color-surface2)]/50 transition-colors">
                  <td className="px-4 py-3">
                    <div className="font-medium text-[var(--color-text)]">{p.name}</div>
                    {p.description && <div className="text-xs text-[var(--color-dim)] mt-0.5 max-w-[200px] truncate">{p.description}</div>}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text)]">{fmtMoney(p.price_kopecks / 100)}</td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{p.period === 'month' ? 'Месяц' : p.period === 'year' ? 'Год' : p.period}</td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{p.trial_days || '—'}</td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{p.tier}</td>
                  <td className="px-4 py-3">
                    <Badge status={p.is_active ? 'Активен' : 'Неактивен'} map={{
                      'Активен': 'bg-emerald-500/20 text-emerald-400',
                      'Неактивен': 'bg-gray-500/20 text-gray-400',
                    }} />
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <button onClick={() => openEditPlan(p)} className="rounded-lg border border-[var(--color-border)] px-2 py-1 text-xs text-[var(--color-text)] hover:bg-[var(--color-accent)]/10 transition-colors">
                        <Pencil size={14} />
                      </button>
                      {p.is_active && (
                        <button onClick={() => handleDeletePlan(p)} className="rounded-lg border border-red-500/30 px-2 py-1 text-xs text-red-400 hover:bg-red-500/10 transition-colors">
                          <Trash2 size={14} />
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {planPanelOpen && (
        <PlanPanel
          data={planForm}
          onChange={setPlanForm}
          onSave={handleSavePlan}
          onClose={() => setPlanPanelOpen(false)}
          isEdit={!!planEditing}
        />
      )}
    </>
  );

  // ─── Subscriptions tab ───

  const filteredSubs = subStatusFilter
    ? subscriptions.filter((s) => s.status === subStatusFilter)
    : subscriptions;

  const renderSubscriptions = () => (
    <>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Подписки</h2>
        <div className="flex items-center gap-2">
          <select
            value={subStatusFilter}
            onChange={(e) => setSubStatusFilter(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
          >
            <option value="">Все статусы</option>
            <option value="active">Активные</option>
            <option value="cancelled">Отменённые</option>
            <option value="expired">Истёкшие</option>
            <option value="trialing">Триал</option>
            <option value="past_due">Просроченные</option>
          </select>
          <button onClick={fetchSubscriptions} className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-accent)]/10 transition-colors">
            <RefreshCw size={14} className={subsLoading ? 'animate-spin' : ''} />
          </button>
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-[var(--color-border)] text-left text-[var(--color-dim)]">
              <th className="px-4 py-3 font-medium">Аккаунт</th>
              <th className="px-4 py-3 font-medium">Тариф</th>
              <th className="px-4 py-3 font-medium">Статус</th>
              <th className="px-4 py-3 font-medium">Период</th>
              <th className="px-4 py-3 font-medium">Сумма</th>
              <th className="px-4 py-3 font-medium">Начало</th>
              <th className="px-4 py-3 font-medium">Истекает</th>
              <th className="px-4 py-3 font-medium">Отменена</th>
            </tr>
          </thead>
          <tbody>
            {subsLoading ? (
              Array.from({ length: 5 }).map((_, i) => (
                <tr key={i} className="border-b border-[var(--color-border)]">
                  {Array.from({ length: 8 }).map((_, j) => (
                    <td key={j} className="px-4 py-3"><div className="h-4 w-24 animate-pulse rounded bg-[var(--color-bg)]" /></td>
                  ))}
                </tr>
              ))
            ) : filteredSubs.length === 0 ? (
              <tr><td colSpan={8} className="px-4 py-8 text-center text-[var(--color-dim)]">Нет подписок</td></tr>
            ) : (
              filteredSubs.map((s) => (
                <tr key={s.subscription_id} className="border-b border-[var(--color-border)] hover:bg-[var(--color-surface2)]/50 transition-colors">
                  <td className="px-4 py-3">
                    <div className="text-[var(--color-text)]">{s.email || s.account_id}</div>
                    {s.email && <div className="text-xs text-[var(--color-dim)] font-mono">{s.account_id.slice(0, 12)}…</div>}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text)]">{s.plan_id}</td>
                  <td className="px-4 py-3"><Badge status={s.status} map={SUB_STATUS_MAP} /></td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{s.period}</td>
                  <td className="px-4 py-3 text-[var(--color-text)]">{fmtMoney(s.amount_kopecks / 100)}</td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{fmtDate(s.started_at)}</td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{fmtDate(s.expires_at)}</td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{s.cancelled_at ? fmtDate(s.cancelled_at) : '—'}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </>
  );

  // ─── Orders tab ───

  const filteredOrders = orderStatusFilter
    ? orders.filter((o) => o.status === orderStatusFilter)
    : orders;

  const renderOrders = () => (
    <>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-lg font-semibold text-[var(--color-text)]">Заказы</h2>
        <div className="flex items-center gap-2">
          <select
            value={orderStatusFilter}
            onChange={(e) => setOrderStatusFilter(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
          >
            <option value="">Все статусы</option>
            <option value="paid">Оплаченные</option>
            <option value="pending">Ожидают</option>
            <option value="failed">Ошибка</option>
            <option value="refunded">Возврат</option>
          </select>
          <button onClick={fetchOrders} className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-accent)]/10 transition-colors">
            <RefreshCw size={14} className={ordersLoading ? 'animate-spin' : ''} />
          </button>
        </div>
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-[var(--color-border)] text-left text-[var(--color-dim)]">
              <th className="px-4 py-3 font-medium">ID</th>
              <th className="px-4 py-3 font-medium">Аккаунт</th>
              <th className="px-4 py-3 font-medium">Тариф</th>
              <th className="px-4 py-3 font-medium">Сумма</th>
              <th className="px-4 py-3 font-medium">Статус</th>
              <th className="px-4 py-3 font-medium">Создан</th>
              <th className="px-4 py-3 font-medium">Оплачен</th>
            </tr>
          </thead>
          <tbody>
            {ordersLoading ? (
              Array.from({ length: 5 }).map((_, i) => (
                <tr key={i} className="border-b border-[var(--color-border)]">
                  {Array.from({ length: 7 }).map((_, j) => (
                    <td key={j} className="px-4 py-3"><div className="h-4 w-24 animate-pulse rounded bg-[var(--color-bg)]" /></td>
                  ))}
                </tr>
              ))
            ) : filteredOrders.length === 0 ? (
              <tr><td colSpan={7} className="px-4 py-8 text-center text-[var(--color-dim)]">Нет заказов</td></tr>
            ) : (
              filteredOrders.map((o) => (
                <tr key={o.order_id} className="border-b border-[var(--color-border)] hover:bg-[var(--color-surface2)]/50 transition-colors">
                  <td className="px-4 py-3 font-mono text-xs text-[var(--color-dim)]">{o.order_id.slice(0, 12)}…</td>
                  <td className="px-4 py-3">
                    <div className="text-[var(--color-text)]">{o.email || o.account_id}</div>
                    {o.email && <div className="text-xs text-[var(--color-dim)] font-mono">{o.account_id.slice(0, 12)}…</div>}
                  </td>
                  <td className="px-4 py-3 text-[var(--color-text)]">{o.plan_id}</td>
                  <td className="px-4 py-3 text-[var(--color-text)]">{fmtMoney(o.amount_kopecks / 100)}</td>
                  <td className="px-4 py-3"><Badge status={o.status} map={ORDER_STATUS_MAP} /></td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{fmtDate(o.created_at)}</td>
                  <td className="px-4 py-3 text-[var(--color-dim)]">{o.paid_at ? fmtDate(o.paid_at) : '—'}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </>
  );

  // ─── Render ───

  return (
    <div className="space-y-6">
      {/* Tabs */}
      <div className="flex flex-wrap gap-1 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-1.5">
        {TABS.map((t) => {
          const active = tab === t.id;
          return (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={`flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                active
                  ? 'bg-[var(--color-accent)] text-white'
                  : 'text-[var(--color-dim)] hover:text-[var(--color-text)] hover:bg-[var(--color-surface2)]'
              }`}
            >
              <t.icon size={16} />
              {t.label}
            </button>
          );
        })}
      </div>

      {/* Tab content */}
      <div className="space-y-6">
        {tab === 'dashboard' && renderDashboard()}
        {tab === 'plans' && renderPlans()}
        {tab === 'subscriptions' && renderSubscriptions()}
        {tab === 'orders' && renderOrders()}
      </div>
    </div>
  );
}
