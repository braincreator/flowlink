import { NextRequest, NextResponse } from "next/server";

const RELAY_URL = process.env.RELAY_URL || "http://127.0.0.1:8080";

// POST /api/billing/subscribe — create or change subscription
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const res = await fetch(`${RELAY_URL}/api/billing/change-plan`, {
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
      { error: "Subscription service unavailable" },
      { status: 503 }
    );
  }
}
