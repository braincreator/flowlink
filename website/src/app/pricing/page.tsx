"use client";

import React from "react";
import { PlanCard, PricingToggle } from "../components/billing";
import type { Plan } from "../components/billing";

const plans: Plan[] = [
  {
    id: "free",
    name: "Free",
    monthlyPrice: 0,
    annualPrice: 0,
    features: ["1 агент", "100 запросов/день", "50K токенов", "Community support"],
    cta: "Начать бесплатно",
    ctaHref: "/signup",
  },
  {
    id: "pro",
    name: "Pro",
    monthlyPrice: 299.9,
    annualPrice: 2999,
    features: [
      "10 агентов",
      "10 000 запросов/день",
      "5M токенов",
      "Telegram бот",
      "Web dashboard",
      "Priority support",
    ],
    popular: true,
    cta: "Выбрать план",
    ctaHref: "/checkout",
  },
  {
    id: "enterprise",
    name: "Enterprise",
    monthlyPrice: 999.9,
    annualPrice: 9999,
    features: [
      "Безлимит агентов",
      "Безлимит запросов",
      "Безлимит токенов",
      "E2EE шифрование",
      "Audit log",
      "Self-hosted relay",
      "SLA 99.9%",
      "Dedicated support",
    ],
    cta: "Выбрать план",
    ctaHref: "/checkout",
  },
];

export default function PricingPage() {
  const [annual, setAnnual] = React.useState(false);

  return (
    <div>
      <section className="container" style={{ paddingTop: 120, paddingBottom: 80 }}>
        <h2>Тарифы</h2>
        <p className="section-sub">Начни бесплатно — масштабируйся когда нужно</p>

        <div style={{ display: "flex", justifyContent: "center", marginBottom: 40 }}>
          <PricingToggle annual={annual} onChange={setAnnual} />
        </div>

        <div className="pricing-grid" style={{ gridTemplateColumns: "repeat(3, 1fr)" }}>
          {plans.map((p) => (
            <PlanCard key={p.id} plan={p} annual={annual} />
          ))}
        </div>
      </section>

      <footer className="container">
        <p>
          <a href="/">← На главную</a> · FlowMasters © 2026
        </p>
      </footer>
    </div>
  );
}
