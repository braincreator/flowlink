import { TabId } from '../types';

const tabs: { id: TabId; icon: string; label: string; badge?: number }[] = [
  { id: 'overview', icon: '🏠', label: 'Overview' },
  { id: 'shield', icon: '🛡️', label: 'Shield' },
  { id: 'agents', icon: '🤖', label: 'Agents' },
  { id: 'audit', icon: '📋', label: 'Audit' },
  { id: 'menu', icon: '⚙️', label: 'Menu' },
];

interface Props {
  active: TabId;
  onChange: (tab: TabId) => void;
  alertCount?: number;
}

export default function BottomNav({ active, onChange, alertCount = 0 }: Props) {
  return (
    <nav className="fixed bottom-0 left-0 right-0 z-30 bg-tg-bg/90 backdrop-blur-lg border-t border-white/5"
         style={{ paddingBottom: 'var(--safe-bottom)' }}>
      <div className="flex justify-around items-center h-16">
        {tabs.map(tab => {
          const isActive = active === tab.id;
          const badge = tab.id === 'shield' && alertCount > 0 ? alertCount : 0;
          return (
            <button key={tab.id} onClick={() => onChange(tab.id)}
              className="relative flex flex-col items-center justify-center w-full h-full min-h-[44px] transition-colors"
              style={{ color: isActive ? 'var(--tg-button)' : 'var(--tg-hint)' }}>
              <span className="text-xl">{tab.icon}</span>
              <span className="text-[10px] mt-0.5">{tab.label}</span>
              {badge > 0 && (
                <span className="absolute top-1.5 right-1/4 min-w-[18px] h-[18px] rounded-full bg-tg-danger text-white text-[10px] font-bold flex items-center justify-center px-1">
                  {badge}
                </span>
              )}
            </button>
          );
        })}
      </div>
    </nav>
  );
}
