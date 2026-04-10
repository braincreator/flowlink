import { NextResponse } from "next/server";

const RELAY_URL = process.env.RELAY_URL || "http://127.0.0.1:8080";

export async function GET() {
  try {
    const res = await fetch(`${RELAY_URL}/healthz`, { cache: "no-store" });

    if (!res.ok) {
      return NextResponse.json(
        { status: "unhealthy", relay: false },
        { status: 503 }
      );
    }

    // Also fetch agent count from /api/agents
    let agents = 0;
    try {
      const agentsRes = await fetch(`${RELAY_URL}/api/agents`, {
        cache: "no-store",
      });
      if (agentsRes.ok) {
        const agentsData = await agentsRes.json();
        agents = Array.isArray(agentsData) ? agentsData.length : 0;
      }
    } catch {
      // Non-critical
    }

    return NextResponse.json({
      status: "healthy",
      relay: true,
      agents,
      timestamp: new Date().toISOString(),
    });
  } catch {
    return NextResponse.json(
      { status: "unhealthy", relay: false },
      { status: 503 }
    );
  }
}
