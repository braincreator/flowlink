import { NextRequest, NextResponse } from 'next/server';

const RELAY_URL = process.env.RELAY_URL || 'http://localhost:8080';

/** Valid values for payment creation */
const VALID_PLANS = ['pro', 'enterprise'] as const;
const VALID_PERIODS = ['month', 'quarter', 'year'] as const;
const VALID_METHODS = ['card', 'sbp'] as const;

type PlanId = (typeof VALID_PLANS)[number];
type Period = (typeof VALID_PERIODS)[number];
type PaymentMethod = (typeof VALID_METHODS)[number];

interface CreatePaymentBody {
  account_id: string;
  plan_id: PlanId;
  period: Period;
  payment_method: PaymentMethod;
}

/**
 * Validate the incoming payment creation request body.
 * Returns an error message string or null if valid.
 */
function validateBody(body: unknown): string | null {
  if (!body || typeof body !== 'object') return 'Request body must be a JSON object';
  const b = body as Record<string, unknown>;
  if (!b.account_id || typeof b.account_id !== 'string') return 'account_id is required and must be a string';
  if (!VALID_PLANS.includes(b.plan_id as PlanId)) return `plan_id must be one of: ${VALID_PLANS.join(', ')}`;
  if (!VALID_PERIODS.includes(b.period as Period)) return `period must be one of: ${VALID_PERIODS.join(', ')}`;
  if (!VALID_METHODS.includes(b.payment_method as PaymentMethod)) return `payment_method must be one of: ${VALID_METHODS.join(', ')}`;
  return null;
}

/**
 * POST /api/payment/create
 * Creates a payment by proxying to the relay server.
 */
export async function POST(request: NextRequest) {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body' }, { status: 400 });
  }

  const validationError = validateBody(body);
  if (validationError) {
    return NextResponse.json({ error: validationError }, { status: 400 });
  }

  try {
    const res = await fetch(`${RELAY_URL}/api/billing/subscribe`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${process.env.RELAY_API_KEY || ''}`,
      },
      body: JSON.stringify(body),
    });

    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch {
    return NextResponse.json({ error: 'Relay unreachable' }, { status: 503 });
  }
}
