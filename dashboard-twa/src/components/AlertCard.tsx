import { Alert } from '../types';
import { useSwipe } from '../hooks/useSwipe';
import { useHaptic } from '../hooks/useHaptic';

interface Props {
  alert: Alert;
  onApprove: (id: string) => void;
  onReject: (id: string) => void;
  onTap: (alert: Alert) => void;
}

const levelColors = { low: 'text-tg-success', medium: 'text-tg-warning', high: 'text-orange-400', critical: 'text-tg-danger' };
const levelBg = { low: 'bg-tg-success-bg', medium: 'bg-tg-warning-bg', high: 'bg-orange-500/10', critical: 'bg-tg-danger-bg' };

export default function AlertCard({ alert, onApprove, onReject, onTap }: Props) {
  const { success, error } = useHaptic();
  const swipe = useSwipe({
    threshold: 80,
    onSwipeRight: () => { success(); onApprove(alert.id); },
    onSwipeLeft: () => { error(); onReject(alert.id); },
  });

  const time = new Date(alert.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

  return (
    <div className="relative overflow-hidden rounded-xl mb-3">
      <div className="absolute inset-0 flex">
        <div className="w-1/2 bg-tg-success/20 flex items-center justify-start pl-4">
          <span className="text-tg-success font-semibold text-sm">✓ Approve</span>
        </div>
        <div className="w-1/2 bg-tg-danger/20 flex items-center justify-end pr-4">
          <span className="text-tg-danger font-semibold text-sm">Reject ✗</span>
        </div>
      </div>
      <div {...swipe} onClick={() => onTap(alert)}
        className="relative bg-tg-surface rounded-xl p-4 cursor-pointer active:opacity-80 transition-opacity">
        <div className="flex items-start justify-between mb-2">
          <span className={`text-xs font-semibold px-2 py-0.5 rounded-full ${levelBg[alert.threatLevel]} ${levelColors[alert.threatLevel]}`}>
            {alert.threatLevel.toUpperCase()}
          </span>
          <span className="text-xs text-tg-hint">{time}</span>
        </div>
        <code className="text-sm font-mono text-tg-text block mb-2 truncate">{alert.command}</code>
        <div className="flex items-center justify-between">
          <span className="text-xs text-tg-hint">{alert.user} @ {alert.agentHost}</span>
          <RiskBadge score={alert.riskScore} />
        </div>
      </div>
    </div>
  );
}

function RiskBadge({ score }: { score: number }) {
  const color = score < 30 ? 'text-tg-success' : score < 70 ? 'text-tg-warning' : 'text-tg-danger';
  return <span className={`text-xs font-mono font-bold ${color}`}>Risk {score}</span>;
}
