import { useState, useCallback, useMemo } from 'react';
import { Alert } from '../types';
import AlertCard from '../components/AlertCard';
import BottomSheet from '../components/BottomSheet';
import RiskGauge from '../components/RiskGauge';
import { useHaptic } from '../hooks/useHaptic';
import { showToast } from '../tg';
import { api } from '../api/client';
import { useApi } from '../hooks/useApi';

export default function Shield() {
  const { data, loading, error, refresh } = useApi(() => api.getAlerts(), { pollMs: 5000 });
  const [selected, setSelected] = useState<Alert | null>(null);
  const { success, error: hapticError } = useHaptic();

  const alerts: Alert[] = useMemo(() => (data || []).map((a: any) => ({
    id: a.id || a.pid || String(Math.random()),
    command: a.command || a.cmd || '',
    user: a.user || a.username || 'unknown',
    agentId: a.agent_id || a.agentId || '',
    agentHost: a.agent_host || a.hostname || a.agentId || '',
    riskScore: a.risk_score ?? a.riskScore ?? 50,
    threatLevel: a.threat_level || a.threatLevel || 'medium',
    timestamp: a.timestamp || a.created_at || new Date().toISOString(),
    status: a.status || 'pending',
    forensic: a.forensic || a.forensics,
  })), [data]);

  const pending = alerts.filter(a => a.status === 'pending');

  const handleApprove = useCallback(async (id: string) => {
    try {
      await api.approveAlert(id);
      success?.();
      showToast('Alert approved');
      refresh();
    } catch (e: any) {
      hapticError?.();
      showToast('Failed to approve');
    }
  }, [refresh, success, hapticError]);

  const handleReject = useCallback(async (id: string) => {
    try {
      await api.rejectAlert(id);
      success?.();
      showToast('Alert rejected');
      refresh();
    } catch (e: any) {
      hapticError?.();
      showToast('Failed to reject');
    }
  }, [refresh, success, hapticError]);

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-tg-button border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-tg-hint mt-3">Loading alerts...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <span className="text-3xl block mb-3">⚠️</span>
        <p className="text-sm text-tg-danger mb-1">{error}</p>
        <button onClick={refresh} className="mt-2 px-4 py-2 rounded-xl bg-tg-button text-tg-button-text text-sm font-medium">
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="px-4 pt-4">
      <h1 className="font-bold text-lg mb-1">Shield</h1>
      <p className="text-xs text-tg-hint mb-4">{pending.length} pending alert{pending.length !== 1 ? 's' : ''} — swipe to act</p>

      {pending.length === 0 ? (
        <div className="text-center py-12 text-tg-hint">
          <span className="text-4xl block mb-3">✨</span>
          <p className="text-sm">All clear! No pending alerts.</p>
        </div>
      ) : (
        pending.map(alert => (
          <AlertCard key={alert.id} alert={alert} onApprove={handleApprove} onReject={handleReject} onTap={setSelected} />
        ))
      )}

      <BottomSheet open={!!selected} onClose={() => setSelected(null)} title="Alert Details">
        {selected && (
          <div className="space-y-4">
            <div className="flex items-center gap-3">
              <RiskGauge score={selected.riskScore} size={64} />
              <div>
                <span className="text-xs text-tg-hint">Threat Level</span>
                <p className="font-semibold capitalize">{selected.threatLevel}</p>
              </div>
            </div>
            <div>
              <span className="text-xs text-tg-hint">Command</span>
              <code className="block mt-1 p-3 bg-tg-surface rounded-lg text-sm font-mono break-all">{selected.command}</code>
            </div>
            <div className="grid grid-cols-2 gap-3 text-sm">
              <div><span className="text-xs text-tg-hint">User</span><p>{selected.user}</p></div>
              <div><span className="text-xs text-tg-hint">Agent</span><p>{selected.agentHost}</p></div>
            </div>
            {selected.forensic && (
              <div>
                <span className="text-xs text-tg-hint font-semibold">Forensic Info</span>
                <div className="mt-2 space-y-1 text-sm">
                  {selected.forensic.cwd && <p><span className="text-tg-hint">CWD:</span> {selected.forensic.cwd}</p>}
                  {selected.forensic.shell && <p><span className="text-tg-hint">Shell:</span> {selected.forensic.shell}</p>}
                  {selected.forensic.parentProcess && <p><span className="text-tg-hint">Parent:</span> {selected.forensic.parentProcess}</p>}
                  {selected.forensic.networkConnections?.length > 0 && (
                    <p><span className="text-tg-hint">Network:</span> {selected.forensic.networkConnections.join(', ')}</p>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </BottomSheet>
    </div>
  );
}
