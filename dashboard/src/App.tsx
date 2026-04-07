import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Layout } from './components/Sidebar';
import { LoadingSkeleton } from './components/Layout';
import { NotificationProvider } from './hooks/useNotifications';

import Login from './pages/Login';
import Onboarding from './pages/Onboarding';
import Agents from './pages/Agents';
import Audit from './pages/Audit';
import Sessions from './pages/Sessions';
import Backups from './pages/Backups';
import Policies from './pages/Policies';
import Devices from './pages/Devices';
import RBAC from './pages/RBAC';
import Settings from './pages/Settings';
import Billing from './pages/Billing';
import LLM from './pages/LLM';
import MCP from './pages/MCP';
import TerminalPage from './pages/Terminal';
import TerminalSOC from './pages/TerminalSOC';
import TerminalRelay from './pages/TerminalRelay';
import TerminalAgent from './pages/TerminalAgent';

// Recharts-heavy pages — lazy loaded for smaller initial bundle
const Dashboard = lazy(() => import('./pages/Dashboard'));
const Metrics = lazy(() => import('./pages/Metrics'));
const Shield = lazy(() => import('./pages/Shield'));

export default function App() {
  return (
    <NotificationProvider>
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route element={<Layout />}>
          <Route index element={<Suspense fallback={<LoadingSkeleton lines={8} />}><Dashboard /></Suspense>} />
          <Route path="agents" element={<Agents />} />
          <Route path="shield" element={<Suspense fallback={<LoadingSkeleton lines={8} />}><Shield /></Suspense>} />
          <Route path="audit" element={<Audit />} />
          <Route path="sessions" element={<Sessions />} />
          <Route path="backups" element={<Backups />} />
          <Route path="policies" element={<Policies />} />
          <Route path="devices" element={<Devices />} />
          <Route path="rbac" element={<RBAC />} />
          <Route path="metrics" element={<Suspense fallback={<LoadingSkeleton lines={8} />}><Metrics /></Suspense>} />
          <Route path="onboarding" element={<Onboarding />} />
          <Route path="settings" element={<Settings />} />
          <Route path="billing" element={<Billing />} />
          <Route path="llm" element={<LLM />} />
          <Route path="mcp" element={<MCP />} />
          <Route path="terminal" element={<TerminalPage />} />
          <Route path="terminal/soc" element={<TerminalSOC />} />
          <Route path="terminal/relay" element={<TerminalRelay />} />
          <Route path="terminal/agent/:id" element={<TerminalAgent />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
    </NotificationProvider>
  );
}
