import { NextResponse } from "next/server";

const RELAY_URL = process.env.RELAY_URL || "http://127.0.0.1:8080";

// Cache plans for 5 minutes (ISR-like)
let plansCache: { data: unknown; ts: number } | null = null;
const CACHE_TTL = 5 * 60 * 1000;

export async function GET() {
  // Return cached plans if fresh
  if (plansCache && Date.now() - plansCache.ts < CACHE_TTL) {
    return NextResponse.json(plansCache.data);
  }

  try {
    const res = await fetch(`${RELAY_URL}/api/plans`, {
      next: { revalidate: 300 }, // ISR cache
    });

    if (!res.ok) {
      return NextResponse.json(getFallbackPlans());
    }

    const data = await res.json();
    plansCache = { data, ts: Date.now() };
    return NextResponse.json(data);
  } catch {
    return NextResponse.json(getFallbackPlans());
  }
}

// Mirrors Rust plans.rs — same data, same strategy
function getFallbackPlans() {
  const sharedFeatures = [
    "Pattern blocking",
    "AST-анализ обфускации",
    "E2EE шифрование",
    "Telegram бот",
    "Web dashboard",
    "Device trust",
    "MCP protocol",
    "Audit log + HMAC",
  ];

  return [
    {
      id: "trial",
      name: "Trial",
      price_kopecks: 0,
      annual_price_kopecks: null,
      features: sharedFeatures,
      trial_days: 7,
      tier: 0,
      available: true,
      legacy: false,
      description: "Попробуйте FlowLink бесплатно",
      limits: {
        max_hosts: 1,
        max_users: 1,
        retention_days: 3,
        audit_retention_days: 3,
        backup_storage_mb: 500,
        max_snapshots: 5,
      },
    },
    {
      id: "starter",
      name: "Starter",
      price_kopecks: 199000,
      annual_price_kopecks: 1910400,
      features: sharedFeatures,
      trial_days: null,
      tier: 1,
      available: true,
      legacy: false,
      description: "Для фрилансеров и small teams",
      limits: {
        max_hosts: 5,
        max_users: 5,
        retention_days: 30,
        audit_retention_days: 30,
        backup_storage_mb: 5120,
        max_snapshots: 50,
      },
    },
    {
      id: "pro",
      name: "Pro",
      price_kopecks: 599000,
      annual_price_kopecks: 5750800,
      features: sharedFeatures,
      trial_days: null,
      tier: 2,
      available: true,
      legacy: false,
      description: "Для стартапов, IT-отделов и DevOps teams",
      limits: {
        max_hosts: 50,
        max_users: 25,
        retention_days: 365,
        audit_retention_days: 365,
        backup_storage_mb: 0, // unlimited
        max_snapshots: 0,
      },
    },
  ];
}
