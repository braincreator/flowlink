export default function Menu() {
  const items = [
    { icon: '🔗', label: 'Pair Device', desc: 'Add a new agent via QR code' },
    { icon: '📜', label: 'Policies', desc: 'Command & network policies' },
    { icon: '👥', label: 'RBAC', desc: 'Role-based access control' },
    { icon: '🔔', label: 'Notifications', desc: 'Alert & event preferences' },
    { icon: '📖', label: 'Help & Support', desc: 'Documentation and support' },
  ];

  return (
    <div className="px-4 pt-4">
      <h1 className="font-bold text-lg mb-4">Menu</h1>

      <div className="space-y-2">
        {items.map(item => (
          <button key={item.label}
            className="w-full flex items-center gap-3 p-4 bg-tg-surface rounded-xl min-h-[56px] active:opacity-80 transition-opacity">
            <span className="text-xl">{item.icon}</span>
            <div className="flex-1 text-left">
              <p className="text-sm font-semibold">{item.label}</p>
              <p className="text-xs text-tg-hint">{item.desc}</p>
            </div>
            <span className="text-tg-hint">›</span>
          </button>
        ))}
      </div>

      <div className="mt-8 text-center text-xs text-tg-hint">
        <p>FlowLink Telegram Mini App</p>
        <p className="mt-1">v1.0.0 · © 2025</p>
      </div>
    </div>
  );
}
