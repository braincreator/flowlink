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
    features: [
      "1 хост",
      "1 пользователь",
      "Pattern blocking (50+ паттернов)",
      "Ручной бэкап",
      "500MB backup storage",
      "Basic sandbox",
      "E2EE (X25519 + AES-256)",
      "Rate limiting",
      "Config hot-reload",
    ],
    cta: "Начать бесплатно",
    ctaHref: "/signup",
  },
  {
    id: "individual",
    name: "Individual",
    monthlyPrice: 1990,
    annualPrice: 15920,
    features: [
      "До 3 хостов",
      "До 2 пользователей",
      "AST + Interpreter анализ",
      "Canary honeypots",
      "Approval workflow",
      "Custom policies (до 10)",
      "Auto backup + Smart backup + Dedup",
      "5GB backup storage",
      "Device trust",
      "MCP protocol",
      "Multi-backend LLM (до 3)",
      "30-day audit log",
      "14 дней trial",
    ],
    popular: true,
    cta: "Начать trial",
    ctaHref: "/checkout",
  },
  {
    id: "business",
    name: "Business",
    monthlyPrice: 4990,
    annualPrice: 39920,
    features: [
      "До 25 хостов",
      "До 10 пользователей",
      "eBPF kernel-level shield",
      "Policy DSL",
      "Forensics",
      "K8s operator",
      "GitOps drift detection",
      "SIEM export (CEF/LEEF/JSON)",
      "RBAC (10 users)",
      "Telegram approval",
      "Auto restore",
      "LLM failover",
      "Global kill switch",
      "PostgreSQL audit",
      "Prometheus metrics",
      "20GB backup storage",
      "90-day audit log",
      "14 дней trial",
    ],
    cta: "Начать trial",
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
