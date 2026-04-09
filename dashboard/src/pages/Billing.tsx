import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CreditCard, Server, Users, Shield, ArrowUpRight, Check } from 'lucide-react';
import { StatCard } from '../components/Layout';

const PLANS = [
  {
    key: 'trial',
    featured: false,
    icon: Shield,
    cta: null,
  },
  {
    key: 'starter',
    featured: false,
    icon: CreditCard,
    cta: 'upgrade',
  },
  {
    key: 'pro',
    featured: true,
    icon: Shield,
    cta: 'upgrade',
  },
];

interface BillingLimits {
  max_hosts: number;
  max_users: number;
  retention_days: number;
  audit_retention_days: number;
  backup_storage_mb: number;
  max_snapshots: number;
  shield_level: string;
}

interface BillingUsage {
  active_agents: number;
}

interface BillingData {
  plan_id: string;
  plan_name: string;
  active: boolean;
  balance_rub: string;
  expires_at: string | null;
  usage: BillingUsage;
  limits: BillingLimits;
}

export default function Billing() {
  const { t } = useTranslation();
  const [billing, setBilling] = useState<BillingData | null>(null);

  useEffect(() => {
    fetch('/api/billing')
      .then(r => r.json())
      .then(setBilling)
      .catch(() => {});
  }, []);

  const hostsUsed = billing?.usage?.active_agents ?? 0;
  const hostsMax = billing?.limits?.max_hosts ?? 1;
  const usersMax = billing?.limits?.max_users ?? 1;
  const retention = billing?.limits?.retention_days ?? 3;

  return (
    <div className="space-y-6">
      {/* Current plan header */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600">
                <CreditCard size={20} className="text-white" />
              </div>
              <div>
                <h2 className="text-lg font-semibold">
                  {t('billing.current_plan')}
                  {billing?.plan_name && (
                    <span className="ml-2 text-[var(--color-accent)]">{billing.plan_name}</span>
                  )}
                </h2>
                <p className="text-sm text-[var(--color-dim)]">
                  {billing?.balance_rub && `${t('billing.current_plan_desc')} · ${billing.balance_rub}`}
                </p>
              </div>
            </div>
          </div>
          <button className="flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white hover:opacity-90 transition-opacity">
            {t('billing.upgrade')} <ArrowUpRight size={14} />
          </button>
        </div>
      </div>

      {/* Usage stats — from API, not hardcoded */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard
          label={t('billing.hosts_connected')}
          value={`${hostsUsed} / ${hostsMax === 0 ? '∞' : hostsMax}`}
          color="accent"
          icon={<Server size={24} />}
        />
        <StatCard
          label={t('billing.team_members')}
          value={`— / ${usersMax === 0 ? '∞' : usersMax}`}
          color="green"
          icon={<Users size={24} />}
        />
        <StatCard
          label={t('billing.log_retention')}
          value={retention === 0 ? '∞' : `${retention} дн.`}
          color="blue"
          icon={<Shield size={24} />}
        />
      </div>

      {/* Plan cards */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        {PLANS.map((plan) => {
          const Icon = plan.icon;
          const features = t(`billing.${plan.key}_features`, { returnObjects: true }) as string[];
          const isCurrent = billing?.plan_id === plan.key;
          return (
            <div
              key={plan.key}
              className={`rounded-xl border p-6 ${
                plan.featured
                  ? 'border-[var(--color-accent)] bg-gradient-to-br from-[var(--color-surface)] to-[var(--color-accent)]/5 ring-1 ring-[var(--color-accent)]/20'
                  : 'border-[var(--color-border)] bg-[var(--color-surface)]'
              } ${isCurrent ? 'ring-2 ring-[var(--color-accent)]' : ''}`}
            >
              <div className="flex items-center gap-3 mb-4">
                <div className={`flex h-9 w-9 items-center justify-center rounded-lg ${
                  plan.featured ? 'bg-[var(--color-accent)]' : 'bg-[var(--color-border)]'
                }`}>
                  <Icon size={18} className="text-white" />
                </div>
                <div>
                  <h3 className="font-semibold">
                    {t(`billing.${plan.key}_name`)}
                    {isCurrent && (
                      <span className="ml-2 text-xs text-[var(--color-accent)]">● {t('billing.current_plan')}</span>
                    )}
                  </h3>
                  <p className="text-xs text-[var(--color-dim)]">{t(`billing.${plan.key}_price`)}</p>
                </div>
              </div>

              <ul className="space-y-2 mb-5">
                {Array.isArray(features) && features.map((f, i) => (
                  <li key={i} className="flex items-start gap-2 text-sm">
                    <Check size={14} className="text-green-400 mt-0.5 shrink-0" />
                    <span>{f}</span>
                  </li>
                ))}
              </ul>

              {plan.cta && !isCurrent && (
                <button className={`w-full rounded-lg py-2.5 text-sm font-medium transition-opacity ${
                  plan.featured
                    ? 'bg-[var(--color-accent)] text-white hover:opacity-90'
                    : 'border border-[var(--color-border)] hover:border-[var(--color-accent)]'
                }`}>
                  {t('billing.upgrade')}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
