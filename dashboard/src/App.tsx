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
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="agents" element={<Agents />} />
          <Route path="shield" element={<Shield />} />
          <Route path="audit" element={<Audit />} />
          <Route path="sessions" element={<Sessions />} />
          <Route path="backups" element={<Backups />} />
          <Route path="policies" element={<Policies />} />
          <Route path="devices" element={<Devices />} />
          <Route path="rbac" element={<RBAC />} />
          <Route path="metrics" element={<Metrics />} />
          <Route path="settings" element={<Settings />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
