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
    // Clients
    getClients() { return this.request('GET', '/api/clients'); }
    // LLM
    getLlmBackends() { return this.request('GET', '/api/llm/backends'); }
    getLlmHealth() { return this.request('GET', '/api/llm/health'); }
    postLlmChat(data) { return this.request('POST', '/api/llm', data); }
    // Shield
    getAlerts() { return this.request('GET', '/api/shield/alerts'); }
    approveAlert(pid) { return this.request('POST', `/api/shield/approve/${pid}`); }
    rejectAlert(pid) { return this.request('POST', `/api/shield/reject/${pid}`); }
    resolveAlert(data) { return this.request('POST', '/api/shield/resolve', data); }
    getShieldStats() { return this.request('GET', '/api/shield/stats'); }
    getPolicies() { return this.request('GET', '/api/shield/policies'); }
    getCanaries() { return this.request('GET', '/api/shield/canaries'); }
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
    // RBAC
    getRbacUsers() { return this.request('GET', '/api/rbac/users'); }
    // Sessions
    getSessions() { return this.request('GET', '/api/sessions'); }
    // Backups
    getBackups() { return this.request('GET', '/api/backups'); }
    // Devices
    getDevices() { return this.request('GET', '/api/devices'); }
    pairDevice(data) { return this.request('POST', '/api/devices/pair', data); }
    confirmPairing(data) { return this.request('POST', '/api/devices/confirm', data); }
    removeDevice(id) { return this.request('DELETE', `/api/devices/${id}`); }
    // System
    getSystemInfo() { return this.request('GET', '/api/system/info'); }
    // Billing
    getBillingInfo() { return this.request('GET', '/api/billing'); }
    getUsage() { return this.request('GET', '/api/billing/usage'); }
    getPlans() { return this.request('GET', '/api/plans'); }
    getBillingPlans() { return this.request('GET', '/api/billing/plans'); }
    changePlan(planId) { return this.request('POST', '/api/billing/change-plan', { plan_id: planId }); }
    getInvoices() { return this.request('GET', '/api/billing/invoices'); }
    getInvoice(id) { return this.request('GET', `/api/billing/invoices/${id}`); }
    getPaymentMethods() { return this.request('GET', '/api/billing/payments/methods'); }
    getSubscriptions() { return this.request('GET', '/api/billing/subscriptions'); }
    createSubscription(data) { return this.request('POST', '/api/billing/subscriptions', data); }
    cancelSubscription(id) { return this.request('POST', `/api/billing/subscriptions/${id}/cancel`); }
    getOrders() { return this.request('GET', '/api/billing/orders'); }
    createOrder(data) { return this.request('POST', '/api/billing/orders', data); }
    // Control Plane — Agents/Servers
    getControlPlaneAgents() { return this.request('GET', '/api/v1/agents'); }
    getControlPlaneAgent(id) { return this.request('GET', `/api/v1/agents/${id}`); }
    signupAgent(data) { return this.request('POST', '/api/v1/signup', data); }
    sendHeartbeat(data) { return this.request('POST', '/api/v1/heartbeat', data); }
    // Config
    getConfig() { return this.request('GET', '/api/config'); }
    reloadConfig() { return this.request('POST', '/api/config/reload'); }
    pushConfig(agentId, config) { return this.request('POST', `/api/config/push/${agentId}`, config); }
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
