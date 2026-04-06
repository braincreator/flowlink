import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Layout } from './components/Sidebar';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import Agents from './pages/Agents';
import Shield from './pages/Shield';
import Audit from './pages/Audit';
import Sessions from './pages/Sessions';
import Backups from './pages/Backups';
import Policies from './pages/Policies';
import Devices from './pages/Devices';
import RBAC from './pages/RBAC';
import Metrics from './pages/Metrics';
import Settings from './pages/Settings';
import Billing from './pages/Billing';
import LLM from './pages/LLM';
import MCP from './pages/MCP';
export default function App() {
    return (_jsx(BrowserRouter, { children: _jsxs(Routes, { children: [_jsx(Route, { path: "/login", element: _jsx(Login, {}) }), _jsxs(Route, { element: _jsx(Layout, {}), children: [_jsx(Route, { index: true, element: _jsx(Dashboard, {}) }), _jsx(Route, { path: "agents", element: _jsx(Agents, {}) }), _jsx(Route, { path: "shield", element: _jsx(Shield, {}) }), _jsx(Route, { path: "audit", element: _jsx(Audit, {}) }), _jsx(Route, { path: "sessions", element: _jsx(Sessions, {}) }), _jsx(Route, { path: "backups", element: _jsx(Backups, {}) }), _jsx(Route, { path: "policies", element: _jsx(Policies, {}) }), _jsx(Route, { path: "devices", element: _jsx(Devices, {}) }), _jsx(Route, { path: "rbac", element: _jsx(RBAC, {}) }), _jsx(Route, { path: "metrics", element: _jsx(Metrics, {}) }), _jsx(Route, { path: "settings", element: _jsx(Settings, {}) }), _jsx(Route, { path: "billing", element: _jsx(Billing, {}) }), _jsx(Route, { path: "llm", element: _jsx(LLM, {}) }), _jsx(Route, { path: "mcp", element: _jsx(MCP, {}) }), _jsx(Route, { path: "*", element: _jsx(Navigate, { to: "/", replace: true }) })] })] }) }));
}
