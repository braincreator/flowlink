"use client";

import React from "react";

export default function BillingSuccessPage() {
  const [count, setCount] = React.useState(5);

  React.useEffect(() => {
    const id = setInterval(() => setCount((c) => c - 1), 1000);
    const redirect = setTimeout(() => {
      window.location.href = "/billing";
    }, 5000);
    return () => { clearInterval(id); clearTimeout(redirect); };
  }, []);

  return (
    <div>
      <section className="container" style={{ paddingTop: 140, paddingBottom: 80, textAlign: "center" }}>
        <div className="success-icon">✓</div>
        <h2 style={{ color: "var(--accent)", marginBottom: 12 }}>Оплата прошла!</h2>
        <p className="text-muted" style={{ fontSize: 15 }}>
          Ваш тариф активирован. Перенаправление через {count}с…
        </p>
        <a href="/billing" className="btn btn-secondary" style={{ marginTop: 24 }}>
          Перейти в биллинг
        </a>
      </section>
    </div>
  );
}
