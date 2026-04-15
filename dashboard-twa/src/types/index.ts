export interface Agent {
  id: string;
  hostname: string;
  os: string;
  status: 'online' | 'offline';
  lastSeen: string;
  ip: string;
  version: string;
}

export interface Alert {
  id: string;
  command: string;
  user: string;
  agentId: string;
  agentHost: string;
  riskScore: number;
  threatLevel: 'low' | 'medium' | 'high' | 'critical';
  timestamp: string;
  status: 'pending' | 'approved' | 'rejected';
  forensic?: {
    fullCommand: string;
    cwd: string;
    shell: string;
    parentProcess: string;
    networkConnections: string[];
  };
}

export interface AuditEvent {
  id: string;
  type: 'command' | 'alert' | 'approved' | 'denied' | 'agent_join' | 'agent_leave' | 'policy_change';
  message: string;
  timestamp: string;
  agent?: string;
  user?: string;
  details?: string;
}

export interface DashboardStats {
  agentsOnline: number;
  agentsTotal: number;
  activeAlerts: number;
  commandsToday: number;
  shieldStatus: 'active' | 'degraded' | 'offline';
}

export type TabId = 'overview' | 'shield' | 'agents' | 'audit' | 'plans' | 'settings' | 'transactions' | 'notifications' | 'menu';

export interface AccountInfo {
  plan_id?: string;
  plan_name?: string;
  active: boolean;
  servers_count: number;
  user: {
    id: string;
    name: string;
    email: string;
    avatar_url?: string;
  };
  created_at: number;
  last_login: number;
}

