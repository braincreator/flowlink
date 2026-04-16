import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
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
export default function Billing() {
    const { t } = useTranslation();
    const [billing, setBilling] = useState(null);
    useEffect(() => {
        fetch('/api/billing')
            .then(r => r.json())
            .then(setBilling)
            .catch(() => { });
    }, []);
    const hostsUsed = billing?.usage?.active_agents ?? 0;
    const hostsMax = billing?.limits?.max_hosts ?? 1;
    const usersMax = billing?.limits?.max_users ?? 1;
    const retention = billing?.limits?.retention_days ?? 3;
    return (_jsxs("div", { className: "space-y-6", children: [_jsx("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6", children: _jsxs("div", { className: "flex items-center justify-between", children: [_jsx("div", { children: _jsxs("div", { className: "flex items-center gap-3", children: [_jsx("div", { className: "flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600", children: _jsx(CreditCard, { size: 20, className: "text-white" }) }), _jsxs("div", { children: [_jsxs("h2", { className: "text-lg font-semibold", children: [t('billing.current_plan'), billing?.plan_name && (_jsx("span", { className: "ml-2 text-[var(--color-accent)]", children: billing.plan_name }))] }), _jsx("p", { className: "text-sm text-[var(--color-dim)]", children: billing?.balance_rub && `${t('billing.current_plan_desc')} · ${billing.balance_rub}` })] })] }) }), _jsxs("button", { className: "flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white hover:opacity-90 transition-opacity", children: [t('billing.upgrade'), " ", _jsx(ArrowUpRight, { size: 14 })] })] }) }), _jsxs("div", { className: "grid grid-cols-1 gap-4 sm:grid-cols-3", children: [_jsx(StatCard, { label: t('billing.hosts_connected'), value: `${hostsUsed} / ${hostsMax === 0 ? '∞' : hostsMax}`, color: "accent", icon: _jsx(Server, { size: 24 }) }), _jsx(StatCard, { label: t('billing.team_members'), value: `— / ${usersMax === 0 ? '∞' : usersMax}`, color: "green", icon: _jsx(Users, { size: 24 }) }), _jsx(StatCard, { label: t('billing.log_retention'), value: retention === 0 ? '∞' : `${retention} дн.`, color: "blue", icon: _jsx(Shield, { size: 24 }) })] }), _jsx("div", { className: "grid grid-cols-1 gap-4 lg:grid-cols-3", children: PLANS.map((plan) => {
                    const Icon = plan.icon;
                    const features = t(`billing.${plan.key}_features`, { returnObjects: true });
                    const isCurrent = billing?.plan_id === plan.key;
                    return (_jsxs("div", { className: `rounded-xl border p-6 ${plan.featured
                            ? 'border-[var(--color-accent)] bg-gradient-to-br from-[var(--color-surface)] to-[var(--color-accent)]/5 ring-1 ring-[var(--color-accent)]/20'
                            : 'border-[var(--color-border)] bg-[var(--color-surface)]'} ${isCurrent ? 'ring-2 ring-[var(--color-accent)]' : ''}`, children: [_jsxs("div", { className: "flex items-center gap-3 mb-4", children: [_jsx("div", { className: `flex h-9 w-9 items-center justify-center rounded-lg ${plan.featured ? 'bg-[var(--color-accent)]' : 'bg-[var(--color-border)]'}`, children: _jsx(Icon, { size: 18, className: "text-white" }) }), _jsxs("div", { children: [_jsxs("h3", { className: "font-semibold", children: [t(`billing.${plan.key}_name`), isCurrent && (_jsxs("span", { className: "ml-2 text-xs text-[var(--color-accent)]", children: ["\u25CF ", t('billing.current_plan')] }))] }), _jsx("p", { className: "text-xs text-[var(--color-dim)]", children: t(`billing.${plan.key}_price`) })] })] }), _jsx("ul", { className: "space-y-2 mb-5", children: Array.isArray(features) && features.map((f, i) => (_jsxs("li", { className: "flex items-start gap-2 text-sm", children: [_jsx(Check, { size: 14, className: "text-green-400 mt-0.5 shrink-0" }), _jsx("span", { children: f })] }, i))) }), plan.cta && !isCurrent && (_jsx("button", { className: `w-full rounded-lg py-2.5 text-sm font-medium transition-opacity ${plan.featured
                                    ? 'bg-[var(--color-accent)] text-white hover:opacity-90'
                                    : 'border border-[var(--color-border)] hover:border-[var(--color-accent)]'}`, children: t('billing.upgrade') }))] }, plan.key));
                }) })] }));
}
