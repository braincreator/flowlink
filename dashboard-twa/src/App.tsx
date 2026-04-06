import { useState } from 'react';
import { TabId } from './types';
import BottomNav from './components/BottomNav';
import Overview from './pages/Overview';
import Shield from './pages/Shield';
import Agents from './pages/Agents';
import Audit from './pages/Audit';
import Menu from './pages/Menu';

export default function App() {
  const [tab, setTab] = useState<TabId>('overview');
  const [alertCount] = useState(2);

  return (
    <div className="min-h-screen">
      <div className="pb-2">
        {tab === 'overview' && <Overview />}
        {tab === 'shield' && <Shield />}
        {tab === 'agents' && <Agents />}
        {tab === 'audit' && <Audit />}
        {tab === 'menu' && <Menu />}
      </div>
      <BottomNav active={tab} onChange={setTab} alertCount={alertCount} />
    </div>
  );
}
