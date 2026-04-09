import { NextRequest, NextResponse } from 'next/server';

const RELAY_URL = process.env.RELAY_URL || 'http://localhost:8080';

/**
 * Extract and forward the Authorization header from the incoming request.
 * Returns the token string or null if missing.
 */
function extractToken(request: NextRequest): string | null {
  const header = request.headers.get('Authorization');
  if (!header) return null;
  return header.replace('Bearer ', '');
}

/**
 * GET /api/subscription?account_id=xxx
 * Returns the current subscription for the authenticated account.
 */
export async function GET(request: NextRequest) {
  const token = extractToken(request);
  if (!token) {
    return NextResponse.json({ error: 'Authorization header required' }, { status: 401 });
  }

  const accountId = request.nextUrl.searchParams.get('account_id');
  if (!accountId) {
    return NextResponse.json({ error: 'account_id query parameter is required' }, { status: 400 });
  }

  try {
    const res = await fetch(
      `${RELAY_URL}/api/billing/subscription?account_id=${encodeURIComponent(accountId)}`,
      {
        cache: 'no-store',
        headers: { 'Authorization': `Bearer ${token}` },
      },
    );
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: 'Relay unreachable' }, { status: 503 });
  }
}

/**
 * POST /api/subscription
 * Cancels the current subscription. Expects Authorization header and
 * JSON body with account_id.
 */
export async function POST(request: NextRequest) {
  const token = extractToken(request);
  if (!token) {
    return NextResponse.json({ error: 'Authorization header required' }, { status: 401 });
  }

  let body: { account_id?: string };
  try {
    body = await request.json();
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body' }, { status: 400 });
  }

  if (!body.account_id) {
    return NextResponse.json({ error: 'account_id is required' }, { status: 400 });
  }

  try {
    const res = await fetch(`${RELAY_URL}/api/billing/subscription/cancel`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`,
      },
      body: JSON.stringify(body),
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: 'Relay unreachable' }, { status: 503 });
  }
}
