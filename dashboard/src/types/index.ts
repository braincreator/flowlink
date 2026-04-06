// ═══════════════════════════════════════════════════════
// FlowLink Dashboard — TypeScript Types
// ═══════════════════════════════════════════════════════

export interface Agent {
  id: string;
  hostname: string;
  os: string;
  version: string;
  status: 'online' | 'offline';
  last_heartbeat: string;
  tags: string[];
  cpu?: number;
  ram?: number;
  disk?: number;
  sessions_count?: number;
  ip?: string;
}

export interface ShieldAlert {
  alert_id: string;
  pid: number;
  uid: number;
  username: string;
  command: string;
  rule_name: string;
  action: string;
  snapshot?: string;
  timestamp: number;
  agent_id?: string;
  resolved: boolean;
  approved?: boolean;
  threat_level: 'L1' | 'L2' | 'L3';
  risk_score: number;
}

export interface AuditEvent {
  id: string;
  agent_id: string;
  event_type: string;
  timestamp_nanos: number;
  timestamp_iso: string;
  command?: string;
  user?: string;
  risk_score?: number;
  action?: string;
  result?: string;
  metadata: Record<string, string>;
}

export interface Session {
  id: string;
  agent_id: string;
  user: string;
  origin: string;
  started_at: string;
  duration_ms: number;
  commands_count: number;
  status: 'active' | 'ended';
  terminal?: string;
}

export interface Backup {
  id: string;
  agent_id: string;
  hostname: string;
  files: string[];
  size_bytes: number;
  timestamp: string;
  status: 'completed' | 'failed' | 'in_progress';
}

export interface PolicyRule {
  name: string;
  action: 'allow' | 'deny' | 'intercept' | 'alert';
  conditions: Record<string, string>;
  priority: number;
  enabled: boolean;
}

export interface Device {
  id: string;
  name: string;
  type: string;
  last_active: string;
  status: 'active' | 'inactive';
}

export interface User {
  username: string;
  roles: string[];
  allowed_paths: string[];
  status: 'active' | 'disabled';
  created_at: string;
}

export interface CanaryToken {
  path: string;
  agent_id: string;
  triggers_count: number;
  last_triggered?: string;
}

export interface SystemInfo {
  version: string;
  uptime: string;
  memory_usage: number;
  cpu_usage: number;
}

export interface DashboardStats {
  total_agents: number;
  online_agents: number;
  active_alerts: number;
  commands_today: number;
  uptime_pct: number;
  l1_count: number;
  l2_count: number;
  l3_count: number;
}

export interface ToastMessage {
  id: string;
  type: 'success' | 'error' | 'info' | 'warning';
  title: string;
  message?: string;
}
