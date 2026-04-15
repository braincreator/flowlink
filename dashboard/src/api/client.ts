import type {
  Agent, ShieldAlert, AuditEvent, Session, Backup,
  PolicyRule, Device, User, CanaryToken, SystemInfo, DashboardStats
} from '../types';

// ═══ API Client ═══

const API_BASE = (import.meta as any).env?.VITE_API_URL || 'http://localhost:8080';

class ApiClient {
  private token: string | null = null;

  constructor() {
    const stored = localStorage.getItem('flowlink_token');
    if (stored) this.token = stored;
  }

  getToken() { return this.token; }

  setToken(token: string | null) {
    this.token = token;
    if (token) localStorage.setItem('flowlink_token', token);
    else localStorage.removeItem('flowlink_token');
  }

  private headers(): Record<string, string> {
    const h: Record<string, string> = { 'Content-Type': 'application/json' };
    if (this.token) h['Authorization'] = `Bearer ${this.token}`;
    return h;
  }

  private async request<T>(method: string, path: string, body?: any): Promise<T> {
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

  private async requestText(method: string, path: string): Promise<string> {
    const res = await fetch(`${API_BASE}${path}`, { method, headers: this.headers() });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.text();
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
