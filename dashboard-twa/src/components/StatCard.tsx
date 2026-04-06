interface Props {
  label: string;
  value: string | number;
  icon: string;
  color?: string;
}

export default function StatCard({ label, value, icon, color }: Props) {
  return (
    <div className="bg-tg-surface rounded-xl p-4 flex-1 min-w-0">
      <div className="flex items-center gap-2 mb-1">
        <span className="text-lg">{icon}</span>
        <span className="text-xs text-tg-hint truncate">{label}</span>
      </div>
      <span className="text-2xl font-bold" style={color ? { color } : undefined}>{value}</span>
    </div>
  );
}
