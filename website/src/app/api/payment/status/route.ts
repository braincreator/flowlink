import { NextRequest, NextResponse } from 'next/server';

const RELAY_URL = process.env.RELAY_URL || 'http://localhost:8080';

/**
 * GET /api/payment/status?order_id=xxx
 * Checks payment status by proxying to the relay server.
 */
export async function GET(request: NextRequest) {
  const orderId = request.nextUrl.searchParams.get('order_id');

  if (!orderId) {
    return NextResponse.json({ error: 'order_id query parameter is required' }, { status: 400 });
  }

  try {
    const res = await fetch(`${RELAY_URL}/api/billing/payment/${encodeURIComponent(orderId)}`, {
      cache: 'no-store',
      headers: { 'Authorization': `Bearer ${process.env.RELAY_API_KEY || ''}` },
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: 'Relay unreachable' }, { status: 503 });
  }
}
