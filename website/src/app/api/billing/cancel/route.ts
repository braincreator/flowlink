import { NextRequest, NextResponse } from "next/server";

const RELAY_URL = process.env.RELAY_URL || "http://127.0.0.1:8080";

// POST /api/billing/cancel — cancel subscription
export async function POST(request: NextRequest) {
  try {
    // Get subscription ID from body or use default
    const body = await request.json().catch(() => ({}));
    const subId = body.subscription_id;

    const url = subId
      ? `${RELAY_URL}/api/billing/subscriptions/${subId}/cancel`
      : `${RELAY_URL}/api/billing/subscriptions/cancel`;

    const res = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: request.headers.get("Authorization") || "",
      },
      body: JSON.stringify(body),
    });

    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json(
      { error: "Cancel service unavailable" },
      { status: 503 }
    );
  }
}
