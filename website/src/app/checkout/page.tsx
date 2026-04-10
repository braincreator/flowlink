"use client";

import React, { useState, useEffect } from "react";

interface ApiPlan {
  id: string;
  name: string;
  price_kopecks: number;
  annual_price_kopecks: number | null;
  features: string[];
  trial_days: number | null;
  tier: number;
  available: boolean;
}

const FALLBACK_PLANS: ApiPlan[] = [
  {
    id: "starter",
    name: "Starter",
    price_kopecks: 99000,
    annual_price_kopecks: 950400,
    features: [],
    trial_days: null,
    tier: 1,
    available: true,
  },
  {
    id: "pro",
    name: "Pro",
    price_kopecks: 499000,
    annual_price_kopecks: 4790400,
    features: [],
    trial_days: null,
    tier: 2,
    available: true,
  },
];

type Period = "month" | "year";
type Method = "card" | "sbp";

export default function CheckoutPage() {
  const params = new URLSearchParams(
    typeof window !== "undefined" ? window.location.search : ""
  );
  const planId = params.get("plan") ?? "pro";
  const period = (params.get("period") ?? "month") as Period;

  const [plans, setPlans] = useState<ApiPlan[]>(FALLBACK_PLANS);
  const [method, setMethod] = useState<Method>("card");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [authToken, setAuthToken] = useState<string | null>(null);

  // Load plans from API
  useEffect(() => {
    fetch("/api/plans")
      .then((r) => (r.ok ? r.json() : FALLBACK_PLANS))
      .then((data: ApiPlan[]) => {
        if (Array.isArray(data) && data.length > 0) setPlans(data);
      })
      .catch(() => {});
  }, []);

  // Load auth token from localStorage (set during login/pairing)
  useEffect(() => {
    if (typeof window !== "undefined") {
      setAuthToken(localStorage.getItem("flowlink_token"));
    }
  }, []);

  const plan = plans.find((p) => p.id === planId) ?? plans[plans.length - 1];
  const monthly = plan.price_kopecks / 100;
  const isAnnual = period === "year";
  const price = isAnnual && plan.annual_price_kopecks
    ? plan.annual_price_kopecks / 100
    : monthly;

  const handlePay = async () => {
    if (!authToken) {
      setError("Войдите в аккаунт перед оплатой");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/payment/create", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${authToken}`,
        },
        body: JSON.stringify({
          plan: plan.id,
          period,
          method,
          amount_kopecks: Math.round(price * 100),
        }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error ?? "Ошибка оплаты");
      if (data.payment_url) {
        window.location.href = data.payment_url;
      } else if (data.order_id) {
        // Order created, redirect to billing
        window.location.href = "/billing?order=" + data.order_id;
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : "Неизвестная ошибка");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <section
        className="container"
        style={{ paddingTop: 120, paddingBottom: 80, maxWidth: 520 }}
      >
        <h2 style={{ textAlign: "left" }}>Оформление заказа</h2>

        <div className="checkout-card">
          <div className="checkout-row">
            <span className="text-muted">Тариф</span>
            <span style={{ color: "var(--text-primary)", fontWeight: 600 }}>
              {plan.name}
            </span>
          </div>
          <div className="checkout-row">
            <span className="text-muted">Период</span>
            <span style={{ color: "var(--text-primary)" }}>
              {isAnnual ? "Год" : "Месяц"}
            </span>
          </div>
          {isAnnual && plan.annual_price_kopecks && (
            <div className="checkout-row">
              <span className="text-muted">Экономия</span>
              <span style={{ color: "#44cc44", fontWeight: 600 }}>
                -
                {Math.round(
                  (1 - (plan.annual_price_kopecks / 100 / 12 / monthly) * 100)
                )}
                %
              </span>
            </div>
          )}
          <div className="checkout-row" style={{ borderBottom: "none" }}>
            <span className="text-muted">Итого</span>
            <span
              style={{
                color: "var(--accent)",
                fontWeight: 700,
                fontSize: 20,
              }}
            >
              {price.toLocaleString("ru-RU")} ₽
            </span>
          </div>
        </div>

        <div className="checkout-methods">
          <p
            className="text-muted"
            style={{ fontSize: 13, marginBottom: 12 }}
          >
            Способ оплаты
          </p>
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

        {error && <div className="checkout-error">{error}</div>}

        {!authToken && (
          <div
            style={{
              padding: "12px 16px",
              borderRadius: 8,
              background: "#1a1a0a",
              border: "1px solid #3a3a1a",
              color: "#ffcc00",
              marginBottom: 16,
              fontSize: 14,
            }}
          >
            Для оплаты необходимо{" "}
            <a href="/billing" style={{ color: "var(--accent)" }}>
              войти в аккаунт
            </a>
          </div>
        )}

        <button
          className="btn btn-primary"
          style={{
            width: "100%",
            justifyContent: "center",
            marginTop: 24,
            padding: "14px 24px",
          }}
          onClick={handlePay}
          disabled={loading || !authToken}
        >
          {loading
            ? "Перенаправление…"
            : `Оплатить ${price.toLocaleString("ru-RU")} ₽`}
        </button>

        <p
          className="text-muted"
          style={{ textAlign: "center", marginTop: 16, fontSize: 12 }}
        >
          Нажимая кнопку, вы соглашаетесь с условиями сервиса
        </p>
      </section>
    </div>
  );
}
