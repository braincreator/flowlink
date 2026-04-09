import { NextResponse } from 'next/server';

const RELAY_URL = process.env.RELAY_URL || 'http://localhost:8080';

/**
 * GET /api/payment/plans
 * Returns available billing plans from the relay server.
 * Cached for 60 seconds via Next.js revalidation.
 */
export const revalidate = 60;

export async function GET() {
  try {
    const res = await fetch(`${RELAY_URL}/api/billing/plans`, {
      headers: { 'Authorization': `Bearer ${process.env.RELAY_API_KEY || ''}` },
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: 'Relay unreachable', plans: [] }, { status: 503 });
  }
}
