import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../api/client';
import { useToast } from '../hooks/useToast';
import {
  LineChart, BarChart, Line, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell,
} from 'recharts';
import {
  Users, UserCheck, TrendingUp, Shield, ShieldAlert, ShieldCheck,
  Search, RefreshCw, ChevronLeft, ChevronRight, Filter,
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

const PAGE_SIZE = 20;

export default function Admin() {
  const { t } = useTranslation();
  const toast = useToast();

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
  const [activeFilter, setActiveFilter] = useState<string>(''); // '', 'true', 'false'
  const [searchFilter, setSearchFilter] = useState('');
  const [accFrom, setAccFrom] = useState('');
  const [accTo, setAccTo] = useState('');

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

  useEffect(() => { fetchStats(); }, [fetchStats]);
  useEffect(() => { fetchAccounts(1); }, [fetchAccounts]);

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

  const maxPlanCount = stats ? Math.max(...stats.plan_distribution.map(p => p.count), 1) : 1;

  const kpis = stats ? [
    { label: 'Всего пользователей', value: fmt(stats.total_users), icon: Users, color: 'text-blue-400' },
    { label: 'Активные', value: fmt(stats.active_users), icon: UserCheck, color: 'text-emerald-400' },
    { label: 'MRR', value: fmtMoney(stats.mrr_rub), icon: TrendingUp, color: 'text-purple-400' },
    { label: 'ARR', value: fmtMoney(stats.arr_rub), icon: TrendingUp, color: 'text-indigo-400' },
    { label: 'Активные подписки', value: fmt(stats.active_subscriptions), icon: Shield, color: 'text-amber-400' },
    { label: 'Отток (мес)', value: fmt(stats.churned_this_month), icon: ShieldAlert, color: 'text-red-400' },
  ] : [];

  const tickStyle = { fill: 'var(--color-dim)', fontSize: 11 };
  const gridStroke = 'var(--color-border)';
  const tooltipStyle = {
    backgroundColor: 'var(--color-surface)',
    border: '1px solid var(--color-border)',
    borderRadius: 8,
    color: 'var(--color-text)',
    fontSize: 12,
  };

  return (
    <div className="space-y-6">
      {/* Date Range */}
      <div className="flex flex-wrap items-center gap-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <Filter size={16} className="text-[var(--color-dim)]" />
        <span className="text-sm text-[var(--color-dim)]">Период:</span>
        <input type="date" value={from} onChange={e => setFrom(e.target.value)}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]" />
        <span className="text-[var(--color-dim)]">—</span>
        <input type="date" value={to} onChange={e => setTo(e.target.value)}
          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]" />
        <button onClick={fetchStats} disabled={statsLoading}
          className="ml-2 flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface2)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-accent)]/10 transition-colors disabled:opacity-50">
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
        {/* New Users Line Chart */}
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

        {/* Revenue Bar Chart */}
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
                  <div className="h-full rounded-full transition-all duration-500"
                    style={{ width: `${(p.count / maxPlanCount) * 100}%`, backgroundColor: PLAN_BAR_COLORS[p.plan] || 'var(--color-accent)' }} />
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
        {/* Filters */}
        <div className="flex flex-wrap items-center gap-3 border-b border-[var(--color-border)] p-4">
          <Search size={16} className="text-[var(--color-dim)]" />
          <input type="text" placeholder="Поиск по email..." value={searchFilter}
            onChange={e => { setSearchFilter(e.target.value); }}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)] w-48" />
          <select value={planFilter} onChange={e => setPlanFilter(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]">
            <option value="">Все тарифы</option>
            {PLANS.map(p => <option key={p} value={p}>{p}</option>)}
          </select>
          <select value={activeFilter} onChange={e => setActiveFilter(e.target.value)}
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]">
            <option value="">Все статусы</option>
            <option value="true">Активные</option>
            <option value="false">Неактивные</option>
          </select>
          <input type="date" value={accFrom} onChange={e => setAccFrom(e.target.value)} placeholder="С"
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]" />
          <input type="date" value={accTo} onChange={e => setAccTo(e.target.value)} placeholder="По"
            className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]" />
          <button onClick={() => fetchAccounts(1)}
            className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface2)] px-3 py-1.5 text-sm text-[var(--color-text)] hover:bg-[var(--color-accent)]/10 transition-colors">
            <RefreshCw size={14} className={accountsLoading ? 'animate-spin' : ''} />
            Найти
          </button>
        </div>

        {/* Table */}
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
                    <td className="px-4 py-3 text-[var(--color-dim)]">
                      {acc.last_login ? fmtDate(acc.last_login) : '—'}
                    </td>
                    <td className="px-4 py-3 text-[var(--color-dim)]">
                      {acc.created_at ? fmtDate(acc.created_at) : '—'}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-2">
                        <button onClick={() => handleToggleActive(acc.account_id)}
                          className={`rounded-lg border px-2 py-1 text-xs font-medium transition-colors ${acc.active
                            ? 'border-red-500/30 text-red-400 hover:bg-red-500/10'
                            : 'border-emerald-500/30 text-emerald-400 hover:bg-emerald-500/10'}`}>
                          {acc.active ? 'Откл.' : 'Акт.'}
                        </button>
                        <select defaultValue={acc.plan_id}
                          onChange={e => e.target.value !== acc.plan_id && handleChangePlan(acc.account_id, e.target.value)}
                          className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1 text-xs text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]">
                          {PLANS.map(p => <option key={p} value={p}>{p}</option>)}
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
    </div>
  );
}
