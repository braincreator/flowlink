"use client";

import React from "react";

type PlanKey = "pro" | "enterprise";
type Period = "month" | "year";
type Method = "card" | "sbp";

const planInfo: Record<PlanKey, { name: string; monthly: number; annual: number }> = {
  pro: { name: "Pro", monthly: 299.9, annual: 2999 },
  enterprise: { name: "Enterprise", monthly: 999.9, annual: 9999 },
};

export default function CheckoutPage() {
  const params = new URLSearchParams(typeof window !== "undefined" ? window.location.search : "");
  const plan = (params.get("plan") ?? "pro") as PlanKey;
  const period = (params.get("period") ?? "month") as Period;
  const info = planInfo[plan] ?? planInfo.pro;
  const price = period === "year" ? info.annual : info.monthly;

  const [method, setMethod] = React.useState<Method>("card");
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const handlePay = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/payment/create", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ plan, period, method }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error ?? "Ошибка оплаты");
      if (data.payment_url) {
        window.location.href = data.payment_url;
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Неизвестная ошибка");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <section className="container" style={{ paddingTop: 120, paddingBottom: 80, maxWidth: 520 }}>
        <h2 style={{ textAlign: "left" }}>Оформление заказа</h2>

        <div className="checkout-card">
          <div className="checkout-row">
            <span className="text-muted">Тариф</span>
            <span style={{ color: "var(--text-primary)", fontWeight: 600 }}>{info.name}</span>
          </div>
          <div className="checkout-row">
            <span className="text-muted">Период</span>
            <span style={{ color: "var(--text-primary)" }}>{period === "year" ? "Год" : "Месяц"}</span>
          </div>
          <div className="checkout-row" style={{ borderBottom: "none" }}>
            <span className="text-muted">Итого</span>
            <span style={{ color: "var(--accent)", fontWeight: 700, fontSize: 20 }}>
              {price.toLocaleString("ru-RU")} ₽
            </span>
          </div>
        </div>

        <div className="checkout-methods">
          <p className="text-muted" style={{ fontSize: 13, marginBottom: 12 }}>Способ оплаты</p>
          <div className="method-grid">
            <button
              className={"method-btn" + (method === "card" ? " active" : "")}
              onClick={() => setMethod("card")}
              type="button"
            >
              💳 Банковская карта
            </button>
            <button
              className={"method-btn" + (method === "sbp" ? " active" : "")}
              onClick={() => setMethod("sbp")}
              type="button"
            >
              🏦 СБП
            </button>
          </div>
        </div>

        {error && (
          <div className="checkout-error">{error}</div>
        )}

        <button
          className="btn btn-primary"
          style={{ width: "100%", justifyContent: "center", marginTop: 24, padding: "14px 24px" }}
          onClick={handlePay}
          disabled={loading}
        >
          {loading ? "Перенаправление…" : `Оплатить ${price.toLocaleString("ru-RU")} ₽`}
        </button>

        <p className="text-muted" style={{ textAlign: "center", marginTop: 16, fontSize: 12 }}>
          Нажимая кнопку, вы соглашаетесь с условиями сервиса
        </p>
      </section>
    </div>
  );
}
