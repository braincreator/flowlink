// ═══ API Client ═══
const API_BASE = import.meta.env?.VITE_API_URL || 'http://localhost:8080';
// Redirect URL — saved before login, restored after
let _redirectAfterLogin = null;
export function setRedirectAfterLogin(url) { _redirectAfterLogin = url; }
export function getRedirectAfterLogin() { const r = _redirectAfterLogin; _redirectAfterLogin = null; return r; }
class ApiClient {
    token = null;
    refreshToken = null;
    refreshPromise = null;
    expiresAt = 0;
    constructor() {
        const stored = localStorage.getItem('flowlink_token');
        if (stored)
            this.token = stored;
        const storedRefresh = localStorage.getItem('flowlink_refresh_token');
        if (storedRefresh)
            this.refreshToken = storedRefresh;
        const storedExp = localStorage.getItem('flowlink_token_expires_at');
        if (storedExp)
            this.expiresAt = parseInt(storedExp, 10);
    }
    getToken() { return this.token; }
    setToken(token) {
        this.token = token;
        if (token)
            localStorage.setItem('flowlink_token', token);
        else
            localStorage.removeItem('flowlink_token');
    }
    setTokens(access, refresh, expiresIn) {
        this.token = access;
        localStorage.setItem('flowlink_token', access);
        if (refresh) {
            this.refreshToken = refresh;
            localStorage.setItem('flowlink_refresh_token', refresh);
        }
        // Set expiry with 30s buffer
        this.expiresAt = Date.now() + (expiresIn * 1000) - 30000;
        localStorage.setItem('flowlink_token_expires_at', String(this.expiresAt));
    }
    clearTokens() {
        this.token = null;
        this.refreshToken = null;
        this.expiresAt = 0;
        localStorage.removeItem('flowlink_token');
        localStorage.removeItem('flowlink_refresh_token');
        localStorage.removeItem('flowlink_token_expires_at');
    }
    isTokenExpired() {
        return Date.now() > this.expiresAt;
    }
    async refreshAccessToken() {
        if (!this.refreshToken)
            return null;
        try {
            const res = await fetch(`${API_BASE}/api/auth/refresh`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ refresh_token: this.refreshToken }),
            });
            if (!res.ok) {
                this.clearTokens();
                return null;
            }
            const data = await res.json();
            this.setTokens(data.access_token, data.refresh_token, data.expires_in);
            return this.token;
        }
        catch {
            return null;
        }
    }
    async getValidToken() {
        // If token is fresh, use it
        if (this.token && !this.isTokenExpired())
            return this.token;
        // If refresh in progress, wait for it
        if (this.refreshPromise)
            return this.refreshPromise;
        // Try to refresh
        this.refreshPromise = this.refreshAccessToken().finally(() => { this.refreshPromise = null; });
        return this.refreshPromise;
    }
    headers() {
        const h = { 'Content-Type': 'application/json' };
        if (this.token)
            h['Authorization'] = `Bearer ${this.token}`;
        return h;
    }
    async request(method, path, body, retry = true) {
        // Try to get a valid token before the request
        const validToken = await this.getValidToken();
        const h = { 'Content-Type': 'application/json' };
        if (validToken)
            h['Authorization'] = `Bearer ${validToken}`;
        const res = await fetch(`${API_BASE}${path}`, {
            method,
            headers: h,
            body: body ? JSON.stringify(body) : undefined,
        });
        // On 401, try refresh once
        if (res.status === 401 && retry && this.refreshToken) {
            const newToken = await this.refreshAccessToken();
            if (newToken) {
                const retryHeaders = { 'Content-Type': 'application/json' };
                retryHeaders['Authorization'] = `Bearer ${newToken}`;
                const retryRes = await fetch(`${API_BASE}${path}`, {
                    method,
                    headers: retryHeaders,
                    body: body ? JSON.stringify(body) : undefined,
                });
                if (!retryRes.ok) {
                    const error = await retryRes.json().catch(() => ({ error: retryRes.statusText }));
                    throw new Error(error.error || `HTTP ${retryRes.status}`);
                }
                return retryRes.json();
            }
            // Refresh failed — redirect to login with return_to
            const currentPath = window.location.pathname + window.location.search;
            if (currentPath !== '/dashboard/login' && currentPath !== '/login') {
                setRedirectAfterLogin(currentPath);
            }
            window.location.href = '/dashboard/login';
            throw new Error('Session expired');
        }
        if (!res.ok) {
            const error = await res.json().catch(() => ({ error: res.statusText }));
            throw new Error(error.error || `HTTP ${res.status}`);
        }
        return res.json();
    }
    async requestText(method, path) {
        const validToken = await this.getValidToken();
        const h = {};
        if (validToken)
            h['Authorization'] = `Bearer ${validToken}`;
        const res = await fetch(`${API_BASE}${path}`, { method, headers: h });
        if (!res.ok)
            throw new Error(`HTTP ${res.status}`);
        return res.text();
    }
    // ── Auth ──
    async sendEmailCode(email) {
        const res = await fetch(`${API_BASE}/api/auth/email/send-code`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email }),
        });
        return res.json();
    }
    async verifyEmailCode(email, code) {
        const res = await fetch(`${API_BASE}/api/auth/email/verify`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email, code }),
        });
        const data = await res.json();
        if (res.ok && data.access_token) {
            this.setTokens(data.access_token, data.refresh_token, data.expires_in);
        }
        return data;
    }
    getProviders() { return this.request('GET', '/api/auth/providers'); }
    getOAuthUrl(provider, redirect) {
        return this.request('GET', `/api/auth/oauth-url?provider=${provider}&redirect=${encodeURIComponent(redirect)}`);
    }
    getAuthMe() { return this.request('GET', '/api/auth/me'); }
    getAccountInfo() { return this.request('GET', '/api/account/info'); }
    linkEmail(email) { return this.request('POST', '/api/auth/link-email', { email }); }
    deleteAccount() { return this.request('DELETE', '/api/account'); }
    // 2FA
    setup2FA() { return this.request('POST', '/api/auth/2fa/setup'); }
    enable2FA(code) { return this.request('POST', '/api/auth/2fa/enable', { code }); }
    disable2FA(code) { return this.request('POST', '/api/auth/2fa/disable', { code }); }
    get2FAStatus() { return this.request('GET', '/api/auth/2fa/status'); }
    complete2FA(tempToken, code) {
        return fetch(`${API_BASE}/api/auth/2fa/complete`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ temp_token: tempToken, code }),
        }).then(async (res) => {
            const data = await res.json();
            if (res.ok && data.access_token) {
                this.setTokens(data.access_token, data.refresh_token, data.expires_in);
            }
            return data;
        });
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
    // Admin
    getAdminStats(from, to) {
        const qs = from ? `?from=${from}&to=${to || ''}` : '';
        return this.request('GET', `/api/admin/dashboard-stats${qs}`);
    }
    getAdminAccounts(filters) {
        const qs = new URLSearchParams(filters || {}).toString();
        return this.request('GET', `/api/admin/accounts${qs ? '?' + qs : ''}`);
    }
    adminChangePlan(id, planId) {
        return this.request('PUT', `/api/admin/accounts/${id}/plan`, { plan_id: planId });
    }
    adminToggleActive(id) {
        return this.request('POST', `/api/admin/accounts/${id}/toggle`);
    }
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
/** Check if current user has admin role (from JWT claims) */
export function isAdmin() {
    try {
        const t = api.getToken();
        if (!t)
            return false;
        const payload = JSON.parse(atob(t.split('.')[1]));
        return payload.is_admin === true;
    }
    catch {
        return false;
    }
}
