import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components/Sidebar';
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
export default function App() {
    return (_jsx(BrowserRouter, { children: _jsx(Routes, { children: _jsxs(Route, { element: _jsx(Layout, {}), children: [_jsx(Route, { index: true, element: _jsx(Dashboard, {}) }), _jsx(Route, { path: "agents", element: _jsx(Agents, {}) }), _jsx(Route, { path: "shield", element: _jsx(Shield, {}) }), _jsx(Route, { path: "audit", element: _jsx(Audit, {}) }), _jsx(Route, { path: "sessions", element: _jsx(Sessions, {}) }), _jsx(Route, { path: "backups", element: _jsx(Backups, {}) }), _jsx(Route, { path: "policies", element: _jsx(Policies, {}) }), _jsx(Route, { path: "devices", element: _jsx(Devices, {}) }), _jsx(Route, { path: "rbac", element: _jsx(RBAC, {}) }), _jsx(Route, { path: "metrics", element: _jsx(Metrics, {}) }), _jsx(Route, { path: "settings", element: _jsx(Settings, {}) })] }) }) }));
}
