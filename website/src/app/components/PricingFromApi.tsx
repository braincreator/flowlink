"use client";

import React from "react";

interface ApiPlan {
  id: string;
  name: string;
  price_kopecks: number;
  annual_price_kopecks: number | null;
  features: string[];
  trial_days: number | null;
  tier: number;
  available: boolean;
  legacy: boolean;
}

const FALLBACK_PLANS: ApiPlan[] = [
  {
    id: "trial",
    name: "Trial",
    price_kopecks: 0,
    annual_price_kopecks: null,
    features: ["1 хост", "1 пользователь", "Pattern blocking", "E2EE шифрование", "Community support"],
    trial_days: 7,
    tier: 0,
    available: true,
    legacy: false,
  },
  {
    id: "starter",
    name: "Starter",
    price_kopecks: 99000,
    annual_price_kopecks: 950400,
    features: ["3 хоста", "3 пользователя", "AST-анализ обфускации", "Telegram бот", "Web dashboard", "Email поддержка"],
    trial_days: null,
    tier: 1,
    available: true,
    legacy: false,
  },
  {
    id: "pro",
    name: "Pro",
    price_kopecks: 499000,
    annual_price_kopecks: 4790400,
    features: ["25 хостов", "10 пользователей", "eBPF kernel-level", "K8s operator", "GitOps rollback", "SIEM export", "RBAC + Audit", "Priority поддержка"],
    trial_days: null,
    tier: 2,
    available: true,
    legacy: false,
  },
];

function formatPrice(kopecks: number): string {
  return (kopecks / 100).toLocaleString("ru-RU");
}

export function PricingFromApi() {
  const [plans, setPlans] = React.useState<ApiPlan[]>(FALLBACK_PLANS);

  React.useEffect(() => {
    fetch("/api/plans")
      .then((r) => {
        if (!r.ok) throw new Error(`${r.status}`);
        return r.json();
      })
      .then((data: ApiPlan[]) => {
        if (Array.isArray(data) && data.length > 0) {
          setPlans(
            data
              .filter((p) => p.available !== false && !p.legacy)
              .sort((a, b) => a.tier - b.tier)
          );
        }
      })
      .catch(() => {
        // Use fallback — already set
      });
  }, []);

  return (
    <div className="pricing-grid">
      {plans.map((p, i) => {
        const monthly = p.price_kopecks / 100;
        const isTrial = p.price_kopecks === 0;
        const isFeatured = i === 1;
        const annualMonthly = p.annual_price_kopecks
          ? Math.round(p.annual_price_kopecks / 100 / 12)
          : monthly;
        const annualSavings = p.annual_price_kopecks
          ? Math.round((1 - annualMonthly / monthly) * 100)
          : 0;

        return (
          <div key={p.id} className={`pricing-card${isFeatured ? " featured" : ""}`}>
            <h3>{p.name}</h3>
            <div className="price">
              {isTrial ? "0 ₽" : `${formatPrice(p.price_kopecks)} ₽`}
            </div>
            {!isTrial ? (
              <>
                <div className="price-note">/мес</div>
                {p.annual_price_kopecks && (
                  <p className="price-yearly">
                    {formatPrice(p.annual_price_kopecks)} ₽ /год
                    {annualSavings > 0 && ` (-${annualSavings}%)`}
                  </p>
                )}
              </>
            ) : (
              <div className="price-note">7 дней бесплатно</div>
            )}
            <ul>
              {(p.features || []).map((f, j) => (
                <li key={j}>{f}</li>
              ))}
            </ul>
          </div>
        );
      })}
    </div>
  );
}
