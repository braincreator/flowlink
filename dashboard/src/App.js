import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
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
    return (_jsx(NotificationProvider, { children: _jsx(BrowserRouter, { children: _jsxs(Routes, { children: [_jsx(Route, { path: "/login", element: _jsx(Login, {}) }), _jsxs(Route, { element: _jsx(Layout, {}), children: [_jsx(Route, { index: true, element: _jsx(Suspense, { fallback: _jsx(LoadingSkeleton, { lines: 8 }), children: _jsx(Dashboard, {}) }) }), _jsx(Route, { path: "agents", element: _jsx(Agents, {}) }), _jsx(Route, { path: "shield", element: _jsx(Suspense, { fallback: _jsx(LoadingSkeleton, { lines: 8 }), children: _jsx(Shield, {}) }) }), _jsx(Route, { path: "audit", element: _jsx(Audit, {}) }), _jsx(Route, { path: "sessions", element: _jsx(Sessions, {}) }), _jsx(Route, { path: "backups", element: _jsx(Backups, {}) }), _jsx(Route, { path: "policies", element: _jsx(Policies, {}) }), _jsx(Route, { path: "devices", element: _jsx(Devices, {}) }), _jsx(Route, { path: "rbac", element: _jsx(RBAC, {}) }), _jsx(Route, { path: "metrics", element: _jsx(Suspense, { fallback: _jsx(LoadingSkeleton, { lines: 8 }), children: _jsx(Metrics, {}) }) }), _jsx(Route, { path: "onboarding", element: _jsx(Onboarding, {}) }), _jsx(Route, { path: "settings", element: _jsx(Settings, {}) }), _jsx(Route, { path: "billing", element: _jsx(Billing, {}) }), _jsx(Route, { path: "llm", element: _jsx(LLM, {}) }), _jsx(Route, { path: "mcp", element: _jsx(MCP, {}) }), _jsx(Route, { path: "terminal", element: _jsx(TerminalPage, {}) }), _jsx(Route, { path: "terminal/soc", element: _jsx(TerminalSOC, {}) }), _jsx(Route, { path: "terminal/relay", element: _jsx(TerminalRelay, {}) }), _jsx(Route, { path: "terminal/agent/:id", element: _jsx(TerminalAgent, {}) }), _jsx(Route, { path: "*", element: _jsx(Navigate, { to: "/", replace: true }) })] })] }) }) }));
}
