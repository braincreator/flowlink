const API_BASE = (import.meta as any).env?.VITE_API_URL || 'http://localhost:8080';

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
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  }

  getHealth() { return this.request<{ status: string }>('GET', '/health'); }
  getAgents() { return this.request<any[]>('GET', '/api/agents'); }
  getAlerts() { return this.request<any[]>('GET', '/api/shield/alerts'); }
  getAuditEvents() { return this.request<any[]>('GET', '/api/audit?limit=50'); }
  approveAlert(pid: string) { return this.request('POST', `/api/shield/approve/${pid}`); }
  rejectAlert(pid: string) { return this.request('POST', `/api/shield/reject/${pid}`); }
}

export const api = new ApiClient();
