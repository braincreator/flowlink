import { NextResponse } from 'next/server';

const RELAY_URL = process.env.RELAY_URL || 'http://localhost:8080';

export async function GET() {
  try {
    const res = await fetch(`${RELAY_URL}/api/billing/usage`, {
      cache: 'no-store',
      headers: { 'Authorization': `Bearer ${process.env.RELAY_API_KEY || ''}` },
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ usage: {}, error: 'Relay unreachable' }, { status: 503 });
  }
}
