import { useAuth } from '../contexts/AuthContext';

export default function Menu() {
  const { logout, user } = useAuth();

  const items = [
    { icon: '🔗', label: 'Привязать устройство', desc: 'Добавить нового агента по QR-коду' },
    { icon: '📜', label: 'Политики', desc: 'Правила команд и сети' },
    { icon: '👥', label: 'RBAC', desc: 'Управление ролями' },
    { icon: '🔔', label: 'Уведомления', desc: 'Настройки оповещений' },
    { icon: '📖', label: 'Помощь', desc: 'Документация и поддержка' },
  ];

  return (
    <div className="px-4 pt-4">
      <h1 className="font-bold text-lg mb-4">Меню</h1>

      {user && (
        <div className="mb-4 p-3 bg-tg-hint/10 rounded-xl">
          <p className="text-sm text-tg-hint">Аккаунт</p>
          <p className="font-medium">{user.email}</p>
        </div>
      )}

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

      <button
        onClick={logout}
        className="w-full mt-6 py-3 rounded-xl bg-tg-danger/20 text-tg-danger font-medium"
      >
        Выйти
      </button>

      <div className="mt-8 text-center text-xs text-tg-hint">
        <p>FlowLink Dashboard</p>
        <p className="mt-1">v1.0.0 · © 2025</p>
      </div>
    </div>
  );
}
