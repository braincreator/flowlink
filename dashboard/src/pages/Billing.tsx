import { CreditCard, Zap, Database, Bot, ArrowUpRight } from 'lucide-react';
import { StatCard, LoadingSkeleton, EmptyState } from '../components/Layout';

export default function Billing() {
  return (
    <div className="space-y-6">
      {/* Plan Info */}
      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600">
                <Zap size={20} className="text-white" />
              </div>
              <div>
                <h2 className="text-lg font-semibold">Free Plan</h2>
                <p className="text-sm text-[var(--color-dim)]">Up to 3 agents · 1,000 API calls/month</p>
              </div>
            </div>
          </div>
          <button className="flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white hover:opacity-90 transition-opacity">
            Upgrade to Pro <ArrowUpRight size={14} />
          </button>
        </div>
      </div>

      {/* Usage Stats */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard label="API Calls" value="—" color="accent" icon={<CreditCard size={24} />} />
        <StatCard label="Agents Connected" value="—" color="green" icon={<Bot size={24} />} />
        <StatCard label="Storage Used" value="—" color="blue" icon={<Database size={24} />} />
      </div>

      {/* Coming Soon */}
      <EmptyState
        icon={<CreditCard size={40} />}
        title="Billing coming soon"
        description="Usage tracking, invoices, and plan management will be available in a future update."
      />
    </div>
  );
}
