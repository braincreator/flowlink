import { lazy, Suspense, useEffect } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Layout } from './components/Sidebar';
import { LoadingSkeleton } from './components/Layout';
import { NotificationProvider } from './hooks/useNotifications';
import { ToastProvider } from './hooks/useToast';
import { api, isAdmin } from './api/client';
import Login from './pages/Login';
import Onboarding from './pages/Onboarding';

function RequireAdmin({ children }: { children: React.ReactNode }) {
  if (!isAdmin()) return <Navigate to="/" replace />;
  return <>{children}</>;
}
import Agents from './pages/Agents';
import Audit from './pages/Audit';
import Sessions from './pages/Sessions';
import Backups from './pages/Backups';
import Policies from './pages/Policies';
import Devices from './pages/Devices';
import RBAC from './pages/RBAC';
import Settings from './pages/Settings';
import Profile from './pages/Profile';
import TwoFASetup from './pages/2FASetup';
import Billing from './pages/Billing';
import LLM from './pages/LLM';
import MCP from './pages/MCP';
import TerminalPage from './pages/Terminal';
import TerminalSOC from './pages/TerminalSOC';
import TerminalRelay from './pages/TerminalRelay';
import TerminalAgent from './pages/TerminalAgent';
import Admin from './pages/Admin';
import Security from './pages/Security';

// Recharts-heavy pages — lazy loaded for smaller initial bundle
const Dashboard = lazy(() => import('./pages/Dashboard'));
const Metrics = lazy(() => import('./pages/Metrics'));
const Shield = lazy(() => import('./pages/Shield'));

export default function App() {
  // Handle OAuth callback: extract tokens from URL and store them
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const accessToken = params.get('access_token');
    const refreshToken = params.get('refresh_token');
    const requires2FA = params.get('requires_2fa');
    const tempToken = params.get('temp_token');
    if (accessToken) {
      api.setTokens(accessToken, refreshToken, 900); // default 15min
      // Clean URL — remove tokens from address bar
      const clean = new URL(window.location.href);
      clean.searchParams.delete('access_token');
      clean.searchParams.delete('refresh_token');
      window.history.replaceState({}, '', clean.pathname + clean.hash);
    } else if (requires2FA === '1' && tempToken) {
      // Store temp token for 2FA verification
      (window as any).__twofa_temp_token = tempToken;
      const clean = new URL(window.location.href);
      clean.searchParams.delete('requires_2fa');
      clean.searchParams.delete('temp_token');
      window.history.replaceState({}, '', clean.pathname + clean.hash);
    }
  }, []);

  return (
    <NotificationProvider>
    <ToastProvider>
    <BrowserRouter basename="/dashboard">
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
          <Route path="profile" element={<Profile />} />
          <Route path="security" element={<Security />} />
          <Route path="settings/2fa" element={<TwoFASetup />} />
          <Route path="billing" element={<Billing />} />
          <Route path="llm" element={<LLM />} />
          <Route path="mcp" element={<MCP />} />
          <Route path="terminal" element={<TerminalPage />} />
          <Route path="terminal/soc" element={<TerminalSOC />} />
          <Route path="terminal/relay" element={<TerminalRelay />} />
          <Route path="terminal/agent/:id" element={<TerminalAgent />} />
          <Route path="admin" element={<RequireAdmin><Admin /></RequireAdmin>} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </BrowserRouter>
    </ToastProvider>
    </NotificationProvider>
  );
}
