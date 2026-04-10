"use client";

import React, { useState, useEffect, useCallback } from "react";
import { UsageBar, OrderRow, ConfirmModal } from "../components/billing";

interface SubscriptionInfo {
  plan: string;
  status: string;
  next_billing?: string;
  period?: string;
}

interface UsageMetric {
  label: string;
  used: number;
  total: number;
  unit?: string;
}

type InvoiceStatus = "success" | "failed" | "pending";

interface Invoice {
  date: string;
  plan: string;
  amount: number;
  status: InvoiceStatus;
}

const FALLBACK_SUBSCRIPTION: SubscriptionInfo = {
  plan: "Free",
  status: "active",
  period: "month",
};

const FALLBACK_USAGE: UsageMetric[] = [
  { label: "Агенты", used: 0, total: 1 },
  { label: "Хосты", used: 0, total: 1 },
  { label: "Запросы сегодня", used: 0, total: 1000 },
];

const FALLBACK_ORDERS: Invoice[] = [];

export default function BillingPage() {
  const [subscription, setSubscription] = useState<SubscriptionInfo | null>(null);
  const [usage, setUsage] = useState<UsageMetric[]>(FALLBACK_USAGE);
  const [orders, setOrders] = useState<Invoice[]>(FALLBACK_ORDERS);
  const [showCancel, setShowCancel] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchBilling = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [billingRes, usageRes, invoicesRes] = await Promise.allSettled([
        fetch("/api/billing"),
        fetch("/api/billing/usage"),
        fetch("/api/billing/invoices"),
      ]);

      // Subscription info
      if (billingRes.status === "fulfilled" && billingRes.value.ok) {
        const data = await billingRes.value.json();
        if (data?.plan) {
          setSubscription({
            plan: data.plan,
            status: data.status || "active",
            next_billing: data.next_billing,
            period: data.period || "month",
          });
        }
      } else {
        setSubscription(FALLBACK_SUBSCRIPTION);
      }

      // Usage
      if (usageRes.status === "fulfilled" && usageRes.value.ok) {
        const data = await usageRes.value.json();
        if (Array.isArray(data) && data.length > 0) {
          setUsage(
            data.map((u: Record<string, unknown>) => ({
              label: u.label as string || u.name as string,
              used: Number(u.used ?? u.current ?? 0),
              total: Number(u.total ?? u.limit ?? 100),
              unit: u.unit as string | undefined,
            }))
          );
        }
      }

      // Invoices
      if (invoicesRes.status === "fulfilled" && invoicesRes.value.ok) {
        const data = await invoicesRes.value.json();
        if (Array.isArray(data)) {
          setOrders(
            data.slice(0, 20).map((o: Record<string, unknown>) => ({
              date: String(o.date ?? o.created_at ?? ""),
              plan: String(o.plan ?? o.description ?? "—"),
              amount: Number(o.amount ?? 0) / 100,
              status: (String(o.status ?? "success") as InvoiceStatus),
            }))
          );
        }
      }
    } catch {
      setSubscription(FALLBACK_SUBSCRIPTION);
      setError("Не удалось загрузить данные биллинга");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchBilling();
  }, [fetchBilling]);

  const handleCancel = async () => {
    setShowCancel(false);
    try {
      const res = await fetch("/api/billing/cancel", { method: "POST" });
      if (res.ok) {
        alert("Подписка будет отменена в конце расчётного периода.");
        fetchBilling();
      }
    } catch {
      setError("Ошибка при отмене подписки");
    }
  };

  const sub = subscription || FALLBACK_SUBSCRIPTION;
  const statusTag =
    sub.status === "active"
      ? "tag tag-green"
      : sub.status === "trial"
        ? "tag tag-amber"
        : "tag";

  if (loading) {
    return (
      <div>
        <section className="container" style={{ paddingTop: 120, paddingBottom: 80 }}>
          <h2 style={{ textAlign: "left" }}>Биллинг</h2>
          <p className="text-muted">Загрузка...</p>
        </section>
      </div>
    );
  }

  return (
    <div>
      <section className="container" style={{ paddingTop: 120, paddingBottom: 80 }}>
        <h2 style={{ textAlign: "left" }}>Биллинг</h2>

        {error && (
          <div
            style={{
              padding: "12px 16px",
              borderRadius: 8,
              background: "#1a0a0a",
              border: "1px solid #3a1a1a",
              color: "#ff6666",
              marginBottom: 24,
              fontSize: 14,
            }}
          >
            {error}
          </div>
        )}

        {/* Current plan */}
        <div className="billing-card" style={{ marginBottom: 32 }}>
          <div className="billing-card-header">
            <h3 style={{ fontSize: 16 }}>Текущий тариф</h3>
            <span className={statusTag}>
              {sub.status === "active"
                ? "Активен"
                : sub.status === "trial"
                  ? "Пробный"
                  : sub.status}
            </span>
          </div>
          <div className="billing-card-body">
            <div className="billing-info-grid">
              <div>
                <span className="text-muted">Тариф</span>
                <strong
                  style={{ color: "var(--text-primary)", fontSize: 18 }}
                >
                  {sub.plan}
                </strong>
              </div>
              <div>
                <span className="text-muted">Период</span>
                <strong style={{ color: "var(--text-primary)" }}>
                  {sub.period === "year" ? "Годовой" : "Ежемесячный"}
                </strong>
              </div>
              {sub.next_billing && (
                <div>
                  <span className="text-muted">Следующее списание</span>
                  <strong style={{ color: "var(--text-primary)" }}>
                    {sub.next_billing}
                  </strong>
                </div>
              )}
            </div>
            <div
              style={{
                display: "flex",
                gap: 12,
                marginTop: 20,
                flexWrap: "wrap",
              }}
            >
              <a href="/pricing" className="btn btn-secondary">
                Изменить план
              </a>
              {sub.plan !== "Free" && (
                <button
                  className="btn btn-danger"
                  onClick={() => setShowCancel(true)}
                >
                  Отменить подписку
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Usage */}
        <h3
          style={{
            fontSize: 16,
            color: "var(--text-primary)",
            marginBottom: 16,
          }}
        >
          Использование
        </h3>
        <div className="billing-card" style={{ marginBottom: 32 }}>
          {usage.map((u) => (
            <UsageBar
              key={u.label}
              label={u.label}
              used={u.used}
              total={u.total}
              unit={u.unit ?? "шт."}
            />
          ))}
        </div>

        {/* Order history */}
        <h3
          style={{
            fontSize: 16,
            color: "var(--text-primary)",
            marginBottom: 16,
          }}
        >
          История платежей
        </h3>
        <div className="billing-card" style={{ overflowX: "auto" }}>
          {orders.length === 0 ? (
            <p className="text-muted" style={{ padding: "16px 0", textAlign: "center" }}>
              Нет платежей
            </p>
          ) : (
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
          )}
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
