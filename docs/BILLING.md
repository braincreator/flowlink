# FlowLink Billing — Subscription Flow

## Overview

FlowLink uses **Tochka Bank Subscriptions API** for recurring billing. Payments are charged automatically via SBP or bank card on each billing cycle.

## Subscription Lifecycle

1. **Subscribe** — `POST /api/billing/subscribe` with plan_id + payment method
2. **Active** — automatic recurring billing per period (month/year)
3. **Pause/Resume** — user can pause subscription temporarily
4. **Cancel** — stops at end of current period
5. **Webhooks** — Tochka sends callbacks for all events

## Plan Changes

### Upgrade (higher or equal price)
- **Immediate** — old subscription cancelled, new one created
- Prorated difference charged on next billing cycle
- Access to new plan features granted immediately

### Downgrade (lower price)
- **Scheduled** — takes effect at end of current billing period
- No refund for remaining period
- `pending_plan_id` stored in account DB
- Applied automatically via webhook when period ends

## Cancellation Policy
- Subscription remains active until end of current period
- No partial refunds
- Account reverts to Starter plan after expiration
- Can re-subscribe at any time

## Pause/Resume
- **Pause**: billing stops, service access suspended
- **Resume**: billing resumes from current cycle
- No charges during paused period

## Webhook Events

| Event | Action |
|-------|--------|
| `created` | Activate subscription, update account plan |
| `renewed` | Extend subscription period |
| `payment_failed` | Mark as past_due, retry on next cycle |
| `cancelled` | Deactivate account, revert to Starter plan |
| `paused` | Suspend service |
| `resumed` | Restore service |

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/billing/subscribe` | Create subscription |
| GET | `/api/billing/subscription` | Get current status |
| POST | `/api/billing/subscription/change-plan` | Change plan |
| POST | `/api/billing/subscription/pause` | Pause |
| POST | `/api/billing/subscription/resume` | Resume |
| DELETE | `/api/billing/subscription` | Cancel |
| POST | `/api/billing/webhook/tochka` | Webhook receiver |

## Telegram Bot Commands

| Command | Description |
|---------|-------------|
| `/subscribe [plan]` | Subscribe or view plan picker |
| `/substatus` | Current subscription info |
| `/subcancel` | Cancel with confirmation |
| `/subchange <plan>` | Change plan (shows upgrade/downgrade) |
| `/plans` | List available plans |
| `/billing` | Billing overview |
