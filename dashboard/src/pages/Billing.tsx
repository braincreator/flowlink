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

export default function Billing() {
  const { t } = useTranslation();

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
                <h2 className="text-lg font-semibold">{t('billing.current_plan')}</h2>
                <p className="text-sm text-[var(--color-dim)]">{t('billing.current_plan_desc')}</p>
              </div>
            </div>
          </div>
          <button className="flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white hover:opacity-90 transition-opacity">
            {t('billing.upgrade')} <ArrowUpRight size={14} />
          </button>
        </div>
      </div>

      {/* Usage stats */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard label={t('billing.hosts_connected')} value="1 / 1" color="accent" icon={<Server size={24} />} />
        <StatCard label={t('billing.team_members')} value="1 / 1" color="green" icon={<Users size={24} />} />
        <StatCard label={t('billing.log_retention')} value="3 дня" color="blue" icon={<Shield size={24} />} />
      </div>

      {/* Plan cards */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        {PLANS.map((plan) => {
          const Icon = plan.icon;
          const features = t(`billing.${plan.key}_features`, { returnObjects: true }) as string[];
          return (
            <div
              key={plan.key}
              className={`rounded-xl border p-6 ${
                plan.featured
                  ? 'border-[var(--color-accent)] bg-gradient-to-br from-[var(--color-surface)] to-[var(--color-accent)]/5 ring-1 ring-[var(--color-accent)]/20'
                  : 'border-[var(--color-border)] bg-[var(--color-surface)]'
              }`}
            >
              <div className="flex items-center gap-3 mb-4">
                <div className={`flex h-9 w-9 items-center justify-center rounded-lg ${
                  plan.featured ? 'bg-[var(--color-accent)]' : 'bg-[var(--color-border)]'
                }`}>
                  <Icon size={18} className="text-white" />
                </div>
                <div>
                  <h3 className="font-semibold">{t(`billing.${plan.key}_name`)}</h3>
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

              {plan.cta && (
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
