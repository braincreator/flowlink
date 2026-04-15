import { TabId } from '../types';

const tabs: { id: TabId; icon: string; label: string }[] = [
  { id: 'overview', icon: '🏠', label: 'Главная' },
  { id: 'shield', icon: '🛡️', label: 'Щит' },
  { id: 'agents', icon: '🤖', label: 'Агенты' },
  { id: 'audit', icon: '📋', label: 'Аудит' },
  { id: 'plans', icon: '💎', label: 'Тарифы' },
  { id: 'settings', icon: '⚙️', label: 'Настройки' },
  { id: 'menu', icon: '☰', label: 'Меню' },
];

interface Props {
  active: TabId;
  onChange: (tab: TabId) => void;
}

export default function BottomNav({ active, onChange }: Props) {
  return (
    <nav className="fixed bottom-0 left-0 right-0 z-30 bg-tg-bg/90 backdrop-blur-lg border-t border-white/5"
         style={{ paddingBottom: 'var(--safe-bottom)' }}>
      <div className="flex justify-around items-center h-16">
        {tabs.map(tab => {
          const isActive = active === tab.id;
          return (
            <button key={tab.id} onClick={() => onChange(tab.id)}
              className="relative flex flex-col items-center justify-center w-full h-full min-h-[44px] transition-colors"
              style={{ color: isActive ? 'var(--tg-button)' : 'var(--tg-hint)' }}>
              <span className="text-xl">{tab.icon}</span>
              <span className="text-[10px] mt-0.5">{tab.label}</span>
            </button>
          );
        })}
      </div>
    </nav>
  );
}
