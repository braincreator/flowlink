import { NextRequest, NextResponse } from "next/server";

const RELAY_URL = process.env.RELAY_URL || "http://127.0.0.1:8080";

// GET /api/billing/usage — current usage metrics
export async function GET(request: NextRequest) {
  try {
    const res = await fetch(`${RELAY_URL}/api/billing/usage`, {
      headers: {
        Authorization: request.headers.get("Authorization") || "",
      },
      cache: "no-store",
    });

    if (!res.ok) {
      return NextResponse.json(
        { error: "Usage unavailable" },
        { status: res.status }
      );
    }

    const data = await res.json();
    return NextResponse.json(data);
  } catch {
    return NextResponse.json(
      { error: "Usage service unavailable" },
      { status: 503 }
    );
  }
}
