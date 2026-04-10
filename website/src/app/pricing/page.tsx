"use client";

import React from "react";
import { PlanCard, PricingToggle } from "../components/billing";
import type { Plan } from "../components/billing";

interface ApiPlan {
  id: string;
  name: string;
  description: string;
  tier: number;
  price_kopecks: number;
  annual_price_kopecks: number | null;
  features: string[];
  trial_days: number | null;
  available: boolean;
  legacy: boolean;
}

function mapPlan(p: ApiPlan, index: number): Plan {
  const monthlyPrice = p.price_kopecks / 100;
  const annualPrice = p.annual_price_kopecks
    ? p.annual_price_kopecks / 100 / 12
    : monthlyPrice;
  const isTrial = p.tier === 0;

  return {
    id: p.id,
    name: p.name,
    monthlyPrice,
    annualPrice: Math.round(annualPrice),
    features: p.features || [],
    popular: index === 1,
    cta: isTrial ? "Начать бесплатно" : "Начать trial",
    ctaHref: isTrial ? "/signup" : "/checkout",
  };
}

export default function PricingPage() {
  const [annual, setAnnual] = React.useState(false);
  const [plans, setPlans] = React.useState<Plan[]>([]);
  const [loading, setLoading] = React.useState(true);

  React.useEffect(() => {
    fetch("/api/plans")
      .then((r) => r.json())
      .then((data: ApiPlan[]) => {
        setPlans(
          (Array.isArray(data) ? data : [])
            .filter((p) => p.available !== false && !p.legacy)
            .sort((a, b) => a.tier - b.tier)
            .map(mapPlan)
        );
      })
      .catch(() => {
        setPlans([
          {
            id: "trial",
            name: "Trial",
            monthlyPrice: 0,
            annualPrice: 0,
            features: [
              "1 сервер",
              "1 пользователь",
              "Unlimited undo",
              "Policy Engine",
              "Community support",
            ],
            cta: "Начать бесплатно",
            ctaHref: "/signup",
          },
          {
            id: "starter",
            name: "Starter",
            monthlyPrice: 990,
            annualPrice: 792,
            features: [
              "3 сервера",
              "3 пользователя",
              "Telegram бот",
              "Web dashboard",
              "E2EE шифрование",
              "Device trust",
              "MCP protocol",
              "Email поддержка",
            ],
            popular: true,
            cta: "Начать trial",
            ctaHref: "/checkout",
          },
          {
            id: "pro",
            name: "Pro",
            monthlyPrice: 4990,
            annualPrice: 3992,
            features: [
              "25 серверов",
              "10 пользователей",
              "K8s operator",
              "SIEM export",
              "RBAC",
              "Approval workflow",
              "Forensics",
              "Audit log + HMAC",
              "Priority поддержка",
            ],
            cta: "Начать trial",
            ctaHref: "/checkout",
          },
        ]);
      })
      .finally(() => setLoading(false));
  }, []);

  return (
    <div>
      <section className="container" style={{ paddingTop: 120, paddingBottom: 80 }}>
        <h2>Тарифы</h2>
        <p className="section-sub">Начни бесплатно — масштабируйся когда нужно</p>

        <div style={{ display: "flex", justifyContent: "center", marginBottom: 40 }}>
          <PricingToggle annual={annual} onChange={setAnnual} />
        </div>

        {loading ? (
          <p style={{ textAlign: "center", opacity: 0.5 }}>Загрузка тарифов...</p>
        ) : (
          <div className="pricing-grid" style={{ gridTemplateColumns: `repeat(${plans.length}, 1fr)` }}>
            {plans.map((p) => (
              <PlanCard key={p.id} plan={p} annual={annual} />
            ))}
          </div>
        )}

        <p style={{ textAlign: "center", marginTop: 32, opacity: 0.7 }}>
          Нужен безлимит? <a href="mailto:hello@flowlink.app">Свяжитесь с нами</a>
        </p>
      </section>

      <footer className="container">
        <p>
          <a href="/">← На главную</a> · Agent: MIT · FlowMasters © 2026
        </p>
      </footer>
    </div>
  );
}
