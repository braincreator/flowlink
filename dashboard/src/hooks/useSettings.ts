import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

// ═══ Types matching backend response structures ═══

export interface BillingInfo {
  plan_id: string;
  plan_name: string;
  active: boolean;
  balance_rub: string;
  expires_at?: string;
  usage: any;
  limits: any;
  available_plans: Array<{
    id: string;
    name: string;
    price_rub: string;
    tier: string;
  }>;
}

export interface AgentInfo {
  agent_id: string;
  account_id: string;
  os?: string;
  arch?: string;
  version?: string;
  online: boolean;
  last_heartbeat_at: number;
  registered_at: number;
  memory_bytes: number;
  cpu_percent: number;
  commands_processed: number;
  commands_blocked: number;
  backups_created: number;
  backup_storage_bytes: number;
  public_key?: string;
}

export interface ServerInfo {
  id: string;
  name: string;
  status: 'online' | 'offline';
  last_seen: string;
  os?: string;
  arch?: string;
  memory_used: number;
  cpu_percent: number;
  commands_processed: number;
  version?: string;
}

export interface UsageTracker {
  agents: any[];
  daily_requests: number;
  daily_tokens: number;
  active_agents: number;
}

export interface UsageData {
  tracker: UsageTracker;
  billing?: any;
}

export interface ApiError {
  endpoint: string;
  message: string;
  status?: number;
}

// ═══ Hook ═══

export const useSettings = () => {
  const [billingInfo, setBillingInfo] = useState<BillingInfo | null>(null);
  const [servers, setServers] = useState<ServerInfo[]>([]);
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [usage, setUsage] = useState<UsageData | null>(null);
  const [loading, setLoading] = useState(true);
  const [errors, setErrors] = useState<ApiError[]>([]);

  const fetchBillingInfo = useCallback(async () => {
    try {
      const data = await api.getBillingInfo();
      setBillingInfo(data);
      setErrors(prev => prev.filter(e => e.endpoint !== '/api/billing'));
    } catch (err: any) {
      setErrors(prev => {
        const filtered = prev.filter(e => e.endpoint !== '/api/billing');
        return [...filtered, { endpoint: '/api/billing', message: err.message || 'Failed to fetch billing info', status: err.status }];
      });
    }
  }, []);

  const fetchServers = useCallback(async () => {
    try {
      const agentsList = await api.getControlPlaneAgents();
      const formatted: ServerInfo[] = agentsList.map((a: AgentInfo) => ({
        id: a.agent_id,
        name: a.os && a.arch ? `${a.os}/${a.arch}` : `Agent-${a.agent_id.slice(0, 8)}`,
        status: a.online ? 'online' : 'offline',
        last_seen: new Date(a.last_heartbeat_at * 1000).toISOString(),
        os: a.os,
        arch: a.arch,
        memory_used: a.memory_bytes,
        cpu_percent: a.cpu_percent,
        commands_processed: a.commands_processed,
        version: a.version,
      }));
      setServers(formatted);
      setAgents(agentsList);
      setErrors(prev => prev.filter(e => e.endpoint !== '/api/v1/agents'));
    } catch (err: any) {
      setErrors(prev => {
        const filtered = prev.filter(e => e.endpoint !== '/api/v1/agents');
        return [...filtered, { endpoint: '/api/v1/agents', message: err.message || 'Failed to fetch agents', status: err.status }];
      });
    }
  }, []);

  const fetchUsage = useCallback(async () => {
    try {
      const data = await api.getUsage();
      setUsage(data);
      setErrors(prev => prev.filter(e => e.endpoint !== '/api/billing/usage'));
    } catch (err: any) {
      setErrors(prev => {
        const filtered = prev.filter(e => e.endpoint !== '/api/billing/usage');
        return [...filtered, { endpoint: '/api/billing/usage', message: err.message || 'Failed to fetch usage data', status: err.status }];
      });
    }
  }, []);

  const changePlan = useCallback(async (planId: string): Promise<boolean> => {
    try {
      await api.changePlan(planId);
      await fetchBillingInfo();
      return true;
    } catch (err: any) {
      setErrors(prev => [...prev, { endpoint: '/api/billing/change-plan', message: err.message || 'Failed to change plan' }]);
      return false;
    }
  }, [fetchBillingInfo]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setErrors([]);
    await Promise.all([fetchBillingInfo(), fetchServers(), fetchUsage()]);
    setLoading(false);
  }, [fetchBillingInfo, fetchServers, fetchUsage]);

  useEffect(() => {
    const load = async () => {
      setLoading(true);
      await Promise.all([fetchBillingInfo(), fetchServers(), fetchUsage()]);
      setLoading(false);
    };
    load();
  }, [fetchBillingInfo, fetchServers, fetchUsage]);

  // Auto-refresh every 30 seconds
  useEffect(() => {
    const interval = setInterval(() => {
      Promise.all([fetchBillingInfo(), fetchServers(), fetchUsage()]);
    }, 30000);
    return () => clearInterval(interval);
  }, [fetchBillingInfo, fetchServers, fetchUsage]);

  return {
    billingInfo,
    servers,
    agents,
    usage,
    loading,
    errors,
    changePlan,
    refresh,
  };
};