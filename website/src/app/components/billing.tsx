"use client";

import React from "react";

/* ═══════════════════════════════════════
   Shared Billing Components
   ═══════════════════════════════════════ */

export interface Plan {
  id: string;
  name: string;
  monthlyPrice: number;
  annualPrice: number;
  features: string[];
  popular?: boolean;
  cta: string;
  ctaHref: string;
}

export function PricingToggle({
  annual,
  onChange,
}: {
  annual: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="pricing-toggle">
      <span className={!annual ? "active" : ""}>Месяц</span>
      <button
        className={"toggle-track" + (annual ? " on" : "")}
        onClick={() => onChange(!annual)}
        type="button"
        aria-label="Переключить период"
      >
        <span className="toggle-thumb" />
      </button>
      <span className={annual ? "active" : ""}>
        Год <span className="toggle-badge">−17%</span>
      </span>
    </div>
  );
}

export function PlanCard({
  plan,
  annual,
}: {
  plan: Plan;
  annual: boolean;
}) {
  const price = annual ? plan.annualPrice : plan.monthlyPrice;
  const period = annual ? "/год" : "/мес";

  return (
    <div className={"pricing-card" + (plan.popular ? " featured" : "")}>
      {plan.popular && <div className="popular-badge">Популярный</div>}
      <h3>{plan.name}</h3>
      <div className="price">{price === 0 ? "0 ₽" : `${price.toLocaleString("ru-RU")} ₽`}</div>
      <div className="price-note">{price === 0 ? "навсегда" : period}</div>
      <ul>
        {plan.features.map((f) => (
          <li key={f}>{f}</li>
        ))}
      </ul>
      <a
        href={price === 0 ? plan.ctaHref : `${plan.ctaHref}?plan=${plan.id}&period=${annual ? "year" : "month"}`}
        className={"btn " + (plan.popular ? "btn-primary" : "btn-secondary")}
        style={{ width: "100%", justifyContent: "center", marginTop: "20px" }}
      >
        {plan.cta}
      </a>
    </div>
  );
}

export function UsageBar({
  label,
  used,
  total,
  unit,
}: {
  label: string;
  used: number;
  total: number;
  unit?: string;
}) {
  const pct = Math.min((used / total) * 100, 100);
  const fmt = (n: number) => (n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n.toLocaleString("ru-RU"));

  return (
    <div className="usage-bar-wrapper">
      <div className="usage-bar-header">
        <span>{label}</span>
        <span className="usage-bar-value">
          {fmt(used)} / {fmt(total)} {unit}
        </span>
      </div>
      <div className="usage-bar-track">
        <div
          className={"usage-bar-fill" + (pct > 90 ? " danger" : pct > 70 ? " warning" : "")}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

export function OrderRow({
  date,
  plan,
  amount,
  status,
}: {
  date: string;
  plan: string;
  amount: number;
  status: "success" | "failed" | "pending";
}) {
  const statusMap = {
    success: { label: "Оплачено", cls: "tag-green" },
    failed: { label: "Ошибка", cls: "tag-red" },
    pending: { label: "В процессе", cls: "tag-amber" },
  };
  const s = statusMap[status];
  return (
    <tr>
      <td>{date}</td>
      <td>{plan}</td>
      <td>{amount.toLocaleString("ru-RU")} ₽</td>
      <td>
        <span className={"tag " + s.cls}>{s.label}</span>
      </td>
    </tr>
  );
}

export function ConfirmModal({
  open,
  title,
  message,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  if (!open) return null;
  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <h3>{title}</h3>
        <p>{message}</p>
        <div className="modal-actions">
          <button className="btn btn-secondary" onClick={onCancel}>
            Отмена
          </button>
          <button className="btn btn-danger" onClick={onConfirm}>
            Подтвердить
          </button>
        </div>
      </div>
    </div>
  );
}
