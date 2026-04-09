"use client";

import React from "react";
import { UsageBar, OrderRow, ConfirmModal } from "../components/billing";

/* Mock data — replace with real API calls */
const subscription = {
  plan: "Pro",
  status: "active" as const,
  nextBilling: "10 мая 2026",
  period: "month",
};

const usage = [
  { label: "Агенты", used: 3, total: 10 },
  { label: "Запросы сегодня", used: 1240, total: 10000 },
  { label: "Токены", used: 1_200_000, total: 5_000_000, unit: "токенов" },
];

const orders = [
  { date: "10.04.2026", plan: "Pro", amount: 299.9, status: "success" as const },
  { date: "10.03.2026", plan: "Pro", amount: 299.9, status: "success" as const },
  { date: "10.02.2026", plan: "Free", amount: 0, status: "success" as const },
];

export default function BillingPage() {
  const [showCancel, setShowCancel] = React.useState(false);

  const handleCancel = () => {
    // POST /api/subscription/cancel
    setShowCancel(false);
    alert("Подписка будет отменена в конце расчётного периода.");
  };

  return (
    <div>
      <section className="container" style={{ paddingTop: 120, paddingBottom: 80 }}>
        <h2 style={{ textAlign: "left" }}>Биллинг</h2>

        {/* Current plan */}
        <div className="billing-card" style={{ marginBottom: 32 }}>
          <div className="billing-card-header">
            <h3 style={{ fontSize: 16 }}>Текущий тариф</h3>
            <span className={"tag tag-green"}>Активен</span>
          </div>
          <div className="billing-card-body">
            <div className="billing-info-grid">
              <div>
                <span className="text-muted">Тариф</span>
                <strong style={{ color: "var(--text-primary)", fontSize: 18 }}>{subscription.plan}</strong>
              </div>
              <div>
                <span className="text-muted">Период</span>
                <strong style={{ color: "var(--text-primary)" }}>
                  {subscription.period === "year" ? "Годовой" : "Ежемесячный"}
                </strong>
              </div>
              <div>
                <span className="text-muted">Следующее списание</span>
                <strong style={{ color: "var(--text-primary)" }}>{subscription.nextBilling}</strong>
              </div>
            </div>
            <div style={{ display: "flex", gap: 12, marginTop: 20, flexWrap: "wrap" }}>
              <a href="/pricing" className="btn btn-secondary">Изменить план</a>
              <button className="btn btn-danger" onClick={() => setShowCancel(true)}>
                Отменить подписку
              </button>
            </div>
          </div>
        </div>

        {/* Usage */}
        <h3 style={{ fontSize: 16, color: "var(--text-primary)", marginBottom: 16 }}>Использование</h3>
        <div className="billing-card" style={{ marginBottom: 32 }}>
          {usage.map((u) => (
            <UsageBar key={u.label} label={u.label} used={u.used} total={u.total} unit={u.unit ?? "шт."} />
          ))}
        </div>

        {/* Order history */}
        <h3 style={{ fontSize: 16, color: "var(--text-primary)", marginBottom: 16 }}>История платежей</h3>
        <div className="billing-card" style={{ overflowX: "auto" }}>
          <table className="orders-table">
            <thead>
              <tr>
                <th>Дата</th>
                <th>Тариф</th>
                <th>Сумма</th>
                <th>Статус</th>
              </tr>
            </thead>
            <tbody>
              {orders.map((o, i) => (
                <OrderRow key={i} {...o} />
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <ConfirmModal
        open={showCancel}
        title="Отменить подписку?"
        message="Тариф будет действовать до конца оплаченного периода. После этого аккаунт переключится на Free."
        onConfirm={handleCancel}
        onCancel={() => setShowCancel(false)}
      />
    </div>
  );
}
