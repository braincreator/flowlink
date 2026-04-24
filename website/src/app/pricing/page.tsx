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
  limits?: {
    max_hosts: number;
    max_users: number;
    retention_days: number;
    backup_storage_mb?: number;
  };
}

function mapPlan(p: ApiPlan, index: number): Plan {
  const monthlyPrice = p.price_kopecks / 100;
  const annualPrice = p.annual_price_kopecks
    ? p.annual_price_kopecks / 100 / 12
    : monthlyPrice;
  const isTrial = p.tier === 0;

  // Build display features: resource limits first (value proposition), then tech features
  const displayFeatures: string[] = [];
  if (p.limits) {
    // Resource limits (most important for customers)
    displayFeatures.push(`${p.limits.max_hosts} хост${p.limits.max_hosts === 1 ? "" : p.limits.max_hosts < 5 ? "а" : "ов"}`);
    displayFeatures.push(`${p.limits.max_users} пользователь`);
    if (p.limits.retention_days === 365) {
      displayFeatures.push("Бессрочное хранение логов");
    } else if (p.limits.retention_days > 0) {
      displayFeatures.push(`Логи хранятся ${p.limits.retention_days} дней`);
    }
    if (p.limits?.backup_storage_mb === 0) {
      displayFeatures.push("Безлимитное облако для бэкапов");
    } else if (p.limits?.backup_storage_mb && p.limits.backup_storage_mb > 1024) {
      const gbLabel = [p.limits.backup_storage_mb / 1024, ' ГБ облако для бэкапов'].join('');
      displayFeatures.push(gbLabel);
    } else if (p.limits?.backup_storage_mb && p.limits.backup_storage_mb > 0) {
      const mbLabel = [p.limits.backup_storage_mb, ' МБ облако для бэкапов'].join('');
      displayFeatures.push(mbLabel);
    }
  }
  // Add tech features (now identical across plans)
  displayFeatures.push(...(p.features || []));

  return {
    id: p.id,
    name: p.name,
    monthlyPrice,
    annualPrice: Math.round(annualPrice),
    features: displayFeatures,
    popular: index === 1,
    cta: isTrial ? "Начать бесплатно" : "Начать trial",
    ctaHref: isTrial ? "/signup" : `/checkout?plan=${p.id}`,
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
        // Fallback — same structure as API response
        setPlans([
          {
            id: "trial",
            name: "Trial",
            monthlyPrice: 0,
            annualPrice: 0,
            features: [
              "1 хост",
              "1 пользователь",
              "3 дней логов",
              "Pattern blocking",
              "AST-анализ обфускации",
              "E2EE шифрование",
              "Telegram бот",
              "Web dashboard",
              "Device trust",
              "MCP protocol",
              "Audit log + HMAC",
            ],
            cta: "Начать бесплатно",
            ctaHref: "/signup",
          },
          {
            id: "starter",
            name: "Starter",
            monthlyPrice: 1990,
            annualPrice: 1592,
            features: [
              "5 хостов",
              "5 пользователей",
              "30 дней логов",
              "Pattern blocking",
              "AST-анализ обфускации",
              "E2EE шифрование",
              "Telegram бот",
              "Web dashboard",
              "Device trust",
              "MCP protocol",
              "Audit log + HMAC",
            ],
            popular: true,
            cta: "Начать trial",
            ctaHref: "/checkout?plan=starter",
          },
          {
            id: "pro",
            name: "Pro",
            monthlyPrice: 5990,
            annualPrice: 4792,
            features: [
              "50 хостов",
              "25 пользователей",
              "Годовое хранение логов",
              "Pattern blocking",
              "AST-анализ обфускации",
              "E2EE шифрование",
              "Telegram бот",
              "Web dashboard",
              "Device trust",
              "MCP protocol",
              "Audit log + HMAC",
            ],
            cta: "Начать trial",
            ctaHref: "/checkout?plan=pro",
          },
        ]);
      })
      .finally(() => setLoading(false));
  }, []);

  return (
    <div>
      <section className="container" style={{ paddingTop: 120, paddingBottom: 80 }}>
        <h2>Тарифы</h2>
        <p className="section-sub">Все функции — в каждом плане. Плати только за масштаб.</p>

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
