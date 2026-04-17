import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CreditCard, Server, Users, Shield, ArrowUpRight, Check } from 'lucide-react';
import { StatCard } from '../components/Layout';
import { api } from '../api/client';
function formatRubles(kopecks) {
    return `${(kopecks / 100).toLocaleString('ru-RU')} ₽`;
}
export default function Billing() {
    const { t } = useTranslation();
    const [billing, setBilling] = useState(null);
    const [plans, setPlans] = useState([]);
    const [loading, setLoading] = useState(true);
    useEffect(() => {
        Promise.all([
            api.getBillingInfo().catch(() => null),
            api.getBillingPlans().catch(() => []),
        ]).then(([b, p]) => {
            if (b)
                setBilling(b);
            if (Array.isArray(p) && p.length > 0)
                setPlans(p);
            setLoading(false);
        });
    }, []);
    const activePlans = plans.filter(p => p.is_active).sort((a, b) => a.sort_order - b.sort_order);
    const hostsUsed = billing?.usage?.active_agents ?? 0;
    const hostsMax = billing?.limits?.max_hosts ?? 1;
    const usersMax = billing?.limits?.max_users ?? 1;
    const retention = billing?.limits?.retention_days ?? 3;
    const currentPlan = activePlans.find(p => p.id === billing?.plan_id);
    const fmt = (n) => new Intl.NumberFormat('ru-RU').format(n);
    if (loading) {
        return (_jsx("div", { className: "space-y-6", children: [1, 2, 3, 4].map(i => (_jsx("div", { className: "h-32 rounded-xl bg-[var(--color-surface)] border border-[var(--color-border)] animate-pulse" }, i))) }));
    }
    return (_jsxs("div", { className: "space-y-6", children: [_jsx("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6", children: _jsxs("div", { className: "flex items-center justify-between", children: [_jsx("div", { children: _jsxs("div", { className: "flex items-center gap-3", children: [_jsx("div", { className: "flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600", children: _jsx(CreditCard, { size: 20, className: "text-white" }) }), _jsxs("div", { children: [_jsx("h2", { className: "text-lg font-semibold", children: currentPlan?.name || billing?.plan_name || t('billing.current_plan') }), _jsxs("p", { className: "text-sm text-[var(--color-dim)]", children: [billing?.balance_rub && `${fmt(Number(billing.balance_rub))} ₽`, currentPlan && ` · ${formatRubles(currentPlan.price_kopecks)}/${currentPlan.period === 'year' ? 'год' : 'мес'}`] })] })] }) }), _jsxs("button", { className: "flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white hover:opacity-90 transition-opacity", children: [t('billing.upgrade'), " ", _jsx(ArrowUpRight, { size: 14 })] })] }) }), _jsxs("div", { className: "grid grid-cols-1 gap-4 sm:grid-cols-3", children: [_jsx(StatCard, { label: t('billing.hosts_connected'), value: `${hostsUsed} / ${hostsMax === 0 ? '∞' : hostsMax}`, color: "accent", icon: _jsx(Server, { size: 24 }) }), _jsx(StatCard, { label: t('billing.team_members'), value: `— / ${usersMax === 0 ? '∞' : usersMax}`, color: "green", icon: _jsx(Users, { size: 24 }) }), _jsx(StatCard, { label: t('billing.log_retention'), value: retention === 0 ? '∞' : `${retention} дн.`, color: "blue", icon: _jsx(Shield, { size: 24 }) })] }), activePlans.length > 0 ? (_jsx("div", { className: "grid grid-cols-1 gap-4 lg:grid-cols-3", children: activePlans.map((plan) => {
                    const isCurrent = billing?.plan_id === plan.id;
                    const isFeatured = plan.tier >= 2;
                    const features = Array.isArray(plan.features) ? plan.features : [];
                    return (_jsxs("div", { className: `rounded-xl border p-6 ${isFeatured
                            ? 'border-[var(--color-accent)] bg-gradient-to-br from-[var(--color-surface)] to-[var(--color-accent)]/5 ring-1 ring-[var(--color-accent)]/20'
                            : 'border-[var(--color-border)] bg-[var(--color-surface)]'} ${isCurrent ? 'ring-2 ring-[var(--color-accent)]' : ''}`, children: [_jsxs("div", { className: "flex items-center gap-3 mb-4", children: [_jsx("div", { className: `flex h-9 w-9 items-center justify-center rounded-lg ${isFeatured ? 'bg-[var(--color-accent)]' : 'bg-[var(--color-border)]'}`, children: _jsx(Shield, { size: 18, className: "text-white" }) }), _jsxs("div", { children: [_jsxs("h3", { className: "font-semibold", children: [plan.name, isCurrent && (_jsxs("span", { className: "ml-2 text-xs text-[var(--color-accent)]", children: ["\u25CF ", t('billing.current_plan')] }))] }), _jsxs("p", { className: "text-xs text-[var(--color-dim)]", children: [plan.price_kopecks === 0
                                                        ? (plan.trial_days > 0 ? `Бесплатно · ${plan.trial_days} дней` : 'Бесплатно')
                                                        : `${formatRubles(plan.price_kopecks)}/${plan.period === 'year' ? 'год' : 'мес'}`, plan.annual_price_kopecks && plan.period === 'month' && (_jsxs("span", { className: "ml-1 text-green-400", children: ["(", formatRubles(plan.annual_price_kopecks), "/\u0433\u043E\u0434)"] }))] })] })] }), plan.description && (_jsx("p", { className: "text-xs text-[var(--color-dim)] mb-3", children: plan.description })), _jsx("ul", { className: "space-y-2 mb-5", children: features.map((f, i) => (_jsxs("li", { className: "flex items-start gap-2 text-sm", children: [_jsx(Check, { size: 14, className: "text-green-400 mt-0.5 shrink-0" }), _jsx("span", { children: f })] }, i))) }), !isCurrent && plan.price_kopecks > 0 && (_jsx("button", { className: `w-full rounded-lg py-2.5 text-sm font-medium transition-opacity ${isFeatured
                                    ? 'bg-[var(--color-accent)] text-white hover:opacity-90'
                                    : 'border border-[var(--color-border)] hover:border-[var(--color-accent)]'}`, children: t('billing.upgrade') }))] }, plan.id));
                }) })) : (_jsx("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-8 text-center", children: _jsx("p", { className: "text-[var(--color-dim)]", children: "\u0422\u0430\u0440\u0438\u0444\u044B \u0432\u0440\u0435\u043C\u0435\u043D\u043D\u043E \u043D\u0435\u0434\u043E\u0441\u0442\u0443\u043F\u043D\u044B" }) }))] }));
}
