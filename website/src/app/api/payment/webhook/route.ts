import { NextRequest, NextResponse } from 'next/server';
import { createHmac } from 'crypto';

const RELAY_URL = process.env.RELAY_URL || 'http://localhost:8080';

/**
 * Verify the HMAC-SHA256 signature from Tochka Bank webhook.
 * Compares the X-Signature header against HMAC-SHA256(secret, rawBody).
 */
function verifySignature(rawBody: string, signature: string | null): boolean {
  const secret = process.env.TOCHKA_WEBHOOK_SECRET;
  if (!secret || !signature) return false;
  const expected = createHmac('sha256', secret).update(rawBody).digest('hex');
  return expected === signature;
}

/**
 * POST /api/payment/webhook
 * Receives webhook callbacks from Tochka Bank, verifies HMAC-SHA256
 * signature, then forwards verified data to the relay server.
 */
export async function POST(request: NextRequest) {
  const rawBody = await request.text();
  const signature = request.headers.get('X-Signature');

  if (!verifySignature(rawBody, signature)) {
    return NextResponse.json({ error: 'Invalid signature' }, { status: 401 });
  }

  let payload: unknown;
  try {
    payload = JSON.parse(rawBody);
  } catch {
    return NextResponse.json({ error: 'Invalid JSON payload' }, { status: 400 });
  }

  try {
    await fetch(`${RELAY_URL}/api/billing/webhook`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${process.env.RELAY_API_KEY || ''}`,
      },
      body: JSON.stringify(payload),
    });
  } catch {
    // Still return 200 to Tochka so they don't retry; relay will reconcile
  }

  return NextResponse.json({ ok: true }, { status: 200 });
}
