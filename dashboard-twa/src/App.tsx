import { useState } from 'react';
import { TabId } from './types';
import { AuthProvider, useAuth } from './contexts/AuthContext';
import BottomNav from './components/BottomNav';
import Login from './pages/Login';
import Overview from './pages/Overview';
import Shield from './pages/Shield';
import Agents from './pages/Agents';
import Audit from './pages/Audit';
import Settings from './pages/Settings';
import Transactions from './pages/Transactions';
import Notifications from './pages/Notifications';
import Plans from './pages/Plans';
import Menu from './pages/Menu';

function Dashboard() {
  const { token, loading: authLoading } = useAuth();
  const [tab, setTab] = useState<TabId>('overview');

  if (authLoading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-tg-button border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-tg-hint mt-3">Загрузка...</p>
      </div>
    );
  }

  if (!token) {
    return <Login />;
  }

  return (
    <div className="min-h-screen">
      <div className="pb-2">
        {tab === 'overview' && <Overview />}
        {tab === 'shield' && <Shield />}
        {tab === 'agents' && <Agents />}
        {tab === 'audit' && <Audit />}
        {tab === 'plans' && <Plans />}
        {tab === 'settings' && <Settings />}
        {tab === 'transactions' && <Transactions />}
        {tab === 'notifications' && <Notifications />}
        {tab === 'menu' && <Menu />}
      </div>
      <BottomNav active={tab} onChange={setTab} />
    </div>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <Dashboard />
    </AuthProvider>
  );
}
