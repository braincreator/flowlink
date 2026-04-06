import { AuditEvent } from '../types';

const typeIcons: Record<string, string> = {
  command: '⚡', alert: '🚨', approved: '✅', denied: '❌',
  agent_join: '🤖', agent_leave: '👋', policy_change: '📝',
};
const typeColors: Record<string, string> = {
  command: 'text-tg-hint', alert: 'text-tg-danger', approved: 'text-tg-success',
  denied: 'text-tg-danger', agent_join: 'text-tg-success', agent_leave: 'text-tg-warning',
  policy_change: 'text-tg-button',
};

interface Props {
  event: AuditEvent;
  onTap?: (event: AuditEvent) => void;
}

export default function EventItem({ event, onTap }: Props) {
  const time = new Date(event.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const date = new Date(event.timestamp).toLocaleDateString([], { month: 'short', day: 'numeric' });

  return (
    <div onClick={() => onTap?.(event)}
      className="flex items-start gap-3 py-3 cursor-pointer active:opacity-80 transition-opacity">
      <span className="text-lg mt-0.5 shrink-0">{typeIcons[event.type] || '📌'}</span>
      <div className="flex-1 min-w-0">
        <p className="text-sm leading-snug">{event.message}</p>
        {(event.agent || event.user) && (
          <p className="text-xs text-tg-hint mt-0.5">{event.user || event.agent}</p>
        )}
      </div>
      <div className="text-right shrink-0">
        <p className="text-xs text-tg-hint">{time}</p>
        <p className="text-[10px] text-tg-hint">{date}</p>
      </div>
    </div>
  );
}
