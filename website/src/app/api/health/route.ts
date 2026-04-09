import { NextResponse } from 'next/server';

const RELAY_URL = process.env.RELAY_URL || 'http://localhost:8080';

export async function GET() {
  try {
    const res = await fetch(`${RELAY_URL}/healthz`, { cache: 'no-store' });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ status: 'error', message: 'Relay unreachable' }, { status: 503 });
  }
}
