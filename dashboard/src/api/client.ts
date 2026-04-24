import type {
  Agent, ShieldAlert, AuditEvent, Session, Backup,
  PolicyRule, Device, User, CanaryToken, SystemInfo, DashboardStats
} from '../types';

// ═══ API Client ═══

const API_BASE = (import.meta as any).env?.VITE_API_URL || '';

// Redirect URL — saved before login, restored after
let _redirectAfterLogin: string | null = null;
export function setRedirectAfterLogin(url: string | null) { _redirectAfterLogin = url; }
export function getRedirectAfterLogin() { const r = _redirectAfterLogin; _redirectAfterLogin = null; return r; }

class ApiClient {
  private token: string | null = null;
  private refreshToken: string | null = null;
  private refreshPromise: Promise<string | null> | null = null;
  private expiresAt: number = 0;

  constructor() {
    const stored = localStorage.getItem('flowlink_token');
    if (stored) this.token = stored;
    const storedRefresh = localStorage.getItem('flowlink_refresh_token');
    if (storedRefresh) this.refreshToken = storedRefresh;
    const storedExp = localStorage.getItem('flowlink_token_expires_at');
    if (storedExp) this.expiresAt = parseInt(storedExp, 10);
  }

  getToken() { return this.token; }

  setToken(token: string | null) {
    this.token = token;
    if (token) localStorage.setItem('flowlink_token', token);
    else localStorage.removeItem('flowlink_token');
  }

  setTokens(access: string, refresh: string | null, expiresIn: number) {
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

  private isTokenExpired(): boolean {
    return Date.now() > this.expiresAt;
  }

  private async refreshAccessToken(): Promise<string | null> {
    if (!this.refreshToken) return null;
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
    } catch {
      return null;
    }
  }

  private async getValidToken(): Promise<string | null> {
    // If token is fresh, use it
    if (this.token && !this.isTokenExpired()) return this.token;
    // If refresh in progress, wait for it
    if (this.refreshPromise) return this.refreshPromise;
    // Try to refresh
    this.refreshPromise = this.refreshAccessToken().finally(() => { this.refreshPromise = null; });
    return this.refreshPromise;
  }

  private headers(): Record<string, string> {
    const h: Record<string, string> = { 'Content-Type': 'application/json' };
    if (this.token) h['Authorization'] = `Bearer ${this.token}`;
    return h;
  }

  private async request<T>(method: string, path: string, body?: any, retry = true): Promise<T> {
    // Try to get a valid token before the request
    const validToken = await this.getValidToken();
    const h: Record<string, string> = { 'Content-Type': 'application/json' };
    if (validToken) h['Authorization'] = `Bearer ${validToken}`;

    const res = await fetch(`${API_BASE}${path}`, {
      method,
      headers: h,
      body: body ? JSON.stringify(body) : undefined,
    });

    // On 401, try refresh once
    if (res.status === 401 && retry && this.refreshToken) {
      const newToken = await this.refreshAccessToken();
      if (newToken) {
        const retryHeaders: Record<string, string> = { 'Content-Type': 'application/json' };
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

    // 401 with no refresh token — redirect immediately
    if (res.status === 401 && !this.refreshToken) {
      const currentPath = window.location.pathname + window.location.search;
      if (currentPath !== '/dashboard/login' && currentPath !== '/login') {
        setRedirectAfterLogin(currentPath);
      }
      window.location.href = '/dashboard/login';
      throw new Error('Not authenticated');
    }

    if (!res.ok) {
      const error = await res.json().catch(() => ({ error: res.statusText }));
      throw new Error(error.error || `HTTP ${res.status}`);
    }
    return res.json();
  }

  private async requestText(method: string, path: string): Promise<string> {
    const validToken = await this.getValidToken();
    const h: Record<string, string> = {};
    if (validToken) h['Authorization'] = `Bearer ${validToken}`;

    const res = await fetch(`${API_BASE}${path}`, { method, headers: h });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.text();
  }

  // ── Auth ──
  async sendEmailCode(email: string) {
    const res = await fetch(`${API_BASE}/api/auth/email/send-code`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email }),
    });
    return res.json();
  }

  async verifyEmailCode(email: string, code: string) {
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

  getProviders() { return this.request<{ providers: string[] }>('GET', '/api/auth/providers'); }
  getOAuthUrl(provider: string, redirect: string) {
    return this.request<{ url: string; state: string }>('GET', `/api/auth/oauth-url?provider=${provider}&redirect=${encodeURIComponent(redirect)}`);
  }
  getAuthMe() { return this.request<any>('GET', '/api/auth/me'); }
  getAccountInfo() { return this.request<any>('GET', '/api/account/info'); }
  linkEmail(email: string) { return this.request<{ ok: boolean; email: string }>('POST', '/api/auth/link-email', { email }); }
  deleteAccount() { return this.request<{ ok: boolean; message: string }>('DELETE', '/api/account'); }

  // Auth sessions (JWT)
  getAuthSessions() { return this.request<any>('GET', '/api/auth/sessions'); }
  revokeAuthSession(id: string) { return this.request<any>('DELETE', `/api/auth/sessions/${id}`); }
  revokeOtherAuthSessions() { return this.request<any>('DELETE', '/api/auth/sessions'); }

  // 2FA
  setup2FA() { return this.request<{ secret: string; otpauth_uri: string }>('POST', '/api/auth/2fa/setup'); }
  enable2FA(code: string) { return this.request<{ ok: boolean; enabled: boolean }>('POST', '/api/auth/2fa/enable', { code }); }
  disable2FA(code: string) { return this.request<{ ok: boolean; enabled: boolean }>('POST', '/api/auth/2fa/disable', { code }); }
  get2FAStatus() { return this.request<{ enabled: boolean; configured: boolean }>('GET', '/api/auth/2fa/status'); }
  complete2FA(tempToken: string, code: string) {
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
  getHealth() { return this.request<{ status: string }>('GET', '/health'); }

  // Agents
  getAgents() { return this.request<Agent[]>('GET', '/api/agents'); }
  registerAgent(agent: any) { return this.request('POST', '/api/agents/register', agent); }
  removeAgent(id: string) { return this.request('DELETE', `/api/agents/${id}`); }

  // Clients
  getClients() { return this.request<any[]>('GET', '/api/clients'); }

  // LLM
  getLlmBackends() { return this.request<any[]>('GET', '/api/llm/backends'); }
  getLlmHealth() { return this.request<any>('GET', '/api/llm/health'); }
  postLlmChat(data: any) { return this.request<any>('POST', '/api/llm', data); }

  // Shield
  getAlerts() { return this.request<ShieldAlert[]>('GET', '/api/shield/alerts'); }
  approveAlert(pid: string) { return this.request('POST', `/api/shield/approve/${pid}`); }
  rejectAlert(pid: string) { return this.request('POST', `/api/shield/reject/${pid}`); }
  resolveAlert(data: any) { return this.request('POST', '/api/shield/resolve', data); }
  getShieldStats() { return this.request<any>('GET', '/api/shield/stats'); }
  getPolicies() { return this.request<PolicyRule[]>('GET', '/api/shield/policies'); }
  getCanaries() { return this.request<CanaryToken[]>('GET', '/api/shield/canaries'); }

  // Audit
  getAuditEvents(params: Record<string, any> = {}) {
    const qs = new URLSearchParams(
      Object.entries(params).filter(([, v]) => v !== undefined && v !== null && v !== '').map(([k, v]) => [k, String(v)])
    ).toString();
    return this.request<AuditEvent[]>('GET', `/api/audit${qs ? `?${qs}` : ''}`);
  }
  getAuditStats() { return this.request<any>('GET', '/api/audit/stats'); }
  exportAudit(format: string) { return this.requestText('GET', `/api/audit/export?format=${format}`); }

  // Approvals
  getApprovals() { return this.request<any[]>('GET', '/api/approvals'); }
  approveRequest(id: string) { return this.request('POST', `/api/approvals/${id}/approve`); }
  rejectRequest(id: string) { return this.request('POST', `/api/approvals/${id}/reject`); }

  // RBAC
  getRbacUsers() { return this.request<User[]>('GET', '/api/rbac/users'); }

  // Sessions
  getSessions() { return this.request<Session[]>('GET', '/api/sessions'); }

  // Backups
  getBackups() { return this.request<Backup[]>('GET', '/api/backups'); }

  // Devices
  getDevices() { return this.request<Device[]>('GET', '/api/devices'); }
  pairDevice(data: any) { return this.request<any>('POST', '/api/devices/pair', data); }
  confirmPairing(data: any) { return this.request<any>('POST', '/api/devices/confirm', data); }
  removeDevice(id: string) { return this.request('DELETE', `/api/devices/${id}`); }

  // System
  getSystemInfo() { return this.request<SystemInfo>('GET', '/api/system/info'); }

  // Billing
  getBillingInfo() { return this.request<any>('GET', '/api/billing'); }
  getUsage() { return this.request<any>('GET', '/api/billing/usage'); }
  getPlans() { return this.request<any[]>('GET', '/api/plans'); }
  getBillingPlans() { return this.request<any[]>('GET', '/api/billing/plans'); }
  changePlan(planId: string) { return this.request<any>('POST', '/api/billing/change-plan', { plan_id: planId }); }
  getInvoices() { return this.request<any[]>('GET', '/api/billing/invoices'); }
  getInvoice(id: string) { return this.request<any>('GET', `/api/billing/invoices/${id}`); }
  getPaymentMethods() { return this.request<any[]>('GET', '/api/billing/payments/methods'); }
  getSubscriptions() { return this.request<any[]>('GET', '/api/billing/subscriptions'); }
  createSubscription(data: any) { return this.request<any>('POST', '/api/billing/subscriptions', data); }
  cancelSubscription(id: string) { return this.request<any>('POST', `/api/billing/subscriptions/${id}/cancel`); }
  getOrders() { return this.request<any[]>('GET', '/api/billing/orders'); }
  createOrder(data: any) { return this.request<any>('POST', '/api/billing/orders', data); }

  // Control Plane — Agents/Servers
  getControlPlaneAgents() { return this.request<any[]>('GET', '/api/v1/agents'); }
  getControlPlaneAgent(id: string) { return this.request<any>('GET', `/api/v1/agents/${id}`); }
  signupAgent(data: any) { return this.request<any>('POST', '/api/v1/signup', data); }
  sendHeartbeat(data: any) { return this.request<any>('POST', '/api/v1/heartbeat', data); }

  // Config
  getConfig() { return this.request<any>('GET', '/api/config'); }
  reloadConfig() { return this.request<any>('POST', '/api/config/reload'); }
  pushConfig(agentId: string, config: any) { return this.request<any>('POST', `/api/config/push/${agentId}`, config); }

  // Admin
  getAdminStats(from?: string, to?: string) {
    const qs = from ? `?from=${from}&to=${to || ''}` : '';
    return this.request<any>('GET', `/api/admin/dashboard-stats${qs}`);
  }
  getAdminAccounts(filters?: Record<string, string>) {
    const qs = new URLSearchParams(filters || {}).toString();
    return this.request<any>('GET', `/api/admin/accounts${qs ? '?' + qs : ''}`);
  }
  adminChangePlan(id: string, planId: string) {
    return this.request<any>('PUT', `/api/admin/accounts/${id}/plan`, { plan_id: planId });
  }
  adminToggleActive(id: string) {
    return this.request<any>('POST', `/api/admin/accounts/${id}/toggle`);
  }

  // Admin plans CRUD
  adminGetPlans() { return this.request<any[]>('GET', '/api/admin/plans'); }
  adminCreatePlan(plan: any) { return this.request<any>('POST', '/api/admin/plans', plan); }
  adminUpdatePlan(id: string, plan: any) { return this.request<any>('PUT', `/api/admin/plans/${id}`, plan); }
  adminDeletePlan(id: string) { return this.request<any>('DELETE', `/api/admin/plans/${id}`); }

  // Admin invoices & subscriptions
  adminGetSubscriptions() { return this.request<any[]>('GET', '/api/admin/subscriptions'); }
  adminGetOrders() { return this.request<any[]>('GET', '/api/admin/orders'); }
  adminGetInvoices() { return this.request<any[]>('GET', '/api/admin/invoices'); }

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
export function isAdmin(): boolean {
  try {
    const t = api.getToken();
    if (!t) return false;
    const payload = JSON.parse(atob(t.split('.')[1]));
    return payload.is_admin === true;
  } catch { return false; }
}
