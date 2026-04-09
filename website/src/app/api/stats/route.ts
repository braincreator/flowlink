import { NextResponse } from 'next/server';

const RELAY_URL = process.env.RELAY_URL || 'http://localhost:8080';

export async function GET() {
  try {
    const [healthRes, agentsRes, billingRes] = await Promise.all([
      fetch(`${RELAY_URL}/healthz`, { cache: 'no-store' }),
      fetch(`${RELAY_URL}/api/agents`, {
        cache: 'no-store',
        headers: { 'Authorization': `Bearer ${process.env.RELAY_API_KEY || ''}` },
      }),
      fetch(`${RELAY_URL}/api/billing/usage`, {
        cache: 'no-store',
        headers: { 'Authorization': `Bearer ${process.env.RELAY_API_KEY || ''}` },
      }),
    ]);
    const health = await healthRes.json();
    const agents = await agentsRes.json();
    const billing = await billingRes.json();
    return NextResponse.json({ health, agents, billing });
  } catch {
    return NextResponse.json({ error: 'Relay unreachable' }, { status: 503 });
  }
}
