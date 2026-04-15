import { AccountInfo } from '../types';

const API_BASE = (import.meta as any).env?.VITE_API_URL || window.location.origin;

class ApiClient {
  private token: string | null = null;
  
  constructor() {
    this.token = new URLSearchParams(window.location.search).get('token') || localStorage.getItem('flowlink_token');
    if (this.token) localStorage.setItem('flowlink_token', this.token);
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
      const text = await res.text().catch(() => '');
      throw new Error(`HTTP ${res.status}: ${text || res.statusText}`);
    }
    // Handle empty responses
    const contentType = res.headers.get('content-type');
    if (!contentType || !contentType.includes('json')) return {} as T;
    return res.json();
  }

  // Auth
  async sendEmailCode(email: string) {
    return this.request<{ ok: boolean; message?: string; error?: string }>('POST', '/api/auth/email/send-code', { email });
  }
  
  async verifyEmailCode(email: string, code: string) {
    return this.request<{ token: string; refresh_token: string; user: { account_id: string; email: string } }>('POST', '/api/auth/email/verify', { email, code });
  }

  // Health
  getHealth() { return this.request<{ status: string }>('GET', '/health'); }

  // Agents
  getAgents() { return this.request<any[]>('GET', '/api/agents'); }

  // Account
  getAccountInfo() { return this.request<AccountInfo>('GET', '/api/account/info'); }
  getAccountSettings() { return this.request<any>('GET', '/api/account/settings'); }
  updateAccountSettings(settings: any) { return this.request('PUT', '/api/account/settings', settings); }

  // Billing
  getBillingInfo() { return this.request<any>('GET', '/api/billing'); }
  getPlans() { return this.request<any[]>('GET', '/api/billing/plans'); }
  getSubscription() { return this.request<any>('GET', '/api/billing/subscription'); }
  getInvoices() { return this.request<any[]>('GET', '/api/billing/invoices'); }
  getPaymentMethods() { return this.request<any[]>('GET', '/api/billing/payments/methods'); }
  async subscribe(planId: string, paymentMethod: any) {
    return this.request('POST', '/api/billing/subscribe', { plan_id: planId, payment_method: paymentMethod });
  }
  async cancelSubscription() {
    return this.request('DELETE', '/api/billing/subscription');
  }
  async pauseSubscription() {
    return this.request('POST', '/api/billing/subscription/pause');
  }
  async resumeSubscription() {
    return this.request('POST', '/api/billing/subscription/resume');
  }
  async changePlan(newPlanId: string) {
    return this.request('POST', '/api/billing/subscription/change-plan', { new_plan_id: newPlanId });
  }

  // Transactions (invoices as transactions)
  getTransactions(limit: number = 20) { return this.request<any[]>('GET', `/api/billing/invoices?limit=${limit}`); }

  // Notifications
  getNotifications() { return this.request<any[]>('GET', '/api/account/notifications'); }
  markNotificationRead(id: string) { return this.request('POST', `/api/account/notifications/${id}/read`); }

  // Shield
  getAlerts() { return this.request<any[]>('GET', '/api/shield/alerts'); }
  approveAlert(pid: string) { return this.request('POST', `/api/shield/approve/${pid}`); }
  rejectAlert(pid: string) { return this.request('POST', `/api/shield/reject/${pid}`); }

  // Audit
  getAuditEvents() { return this.request<any[]>('GET', '/api/audit?limit=50'); }
}

export const api = new ApiClient();
