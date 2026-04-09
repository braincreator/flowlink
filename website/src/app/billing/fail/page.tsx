"use client";

import React from "react";

export default function BillingFailPage() {
  return (
    <div>
      <section className="container" style={{ paddingTop: 140, paddingBottom: 80, textAlign: "center" }}>
        <div className="fail-icon">⚠</div>
        <h2 style={{ color: "var(--critical)", marginBottom: 12 }}>Оплата не удалась</h2>
        <p className="text-muted" style={{ fontSize: 15, marginBottom: 24 }}>
          Деньги не списаны. Попробуйте ещё раз или выберите другой способ оплаты.
        </p>
        <div style={{ display: "flex", gap: 12, justifyContent: "center", flexWrap: "wrap" }}>
          <a href="/pricing" className="btn btn-primary">
            Попробовать снова
          </a>
          <a href="/billing" className="btn btn-secondary">
            К биллингу
          </a>
        </div>
      </section>
    </div>
  );
}
