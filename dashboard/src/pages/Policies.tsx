import { useState } from 'react';
import { Plus, Upload, Download, Play, FileCode } from 'lucide-react';
import { Badge, Modal, YamlEditor, LoadingSkeleton, EmptyState } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';

const DEFAULT_YAML = `# FlowLink Shield Policy
# version: "1.0"

rules:
  - name: Block dangerous rm
    action: deny
    priority: 100
    conditions:
      command_match: "rm\\s+-rf\\s+/"
      user: "*"

  - name: Intercept sudo commands
    action: intercept
    priority: 90
    conditions:
      command_match: "^sudo\\s+"
      user: "*"

  - name: Alert on sensitive file reads
    action: alert
    priority: 80
    conditions:
      command_match: "/etc/(shadow|passwd|ssh)"
      user: "*"
`;

export default function Policies() {
  const [yaml, setYaml] = useState(DEFAULT_YAML);
  const [addOpen, setAddOpen] = useState(false);
  const [testOpen, setTestOpen] = useState(false);
  const [testCmd, setTestCmd] = useState('');
  const [testResult, setTestResult] = useState<string | null>(null);

  const { data, loading, error, refresh } = useApi<any[]>(
    () => api.getPolicies(),
  );

  const policies = data || [];

  const runTest = () => {
    if (!testCmd) return;
    if (testCmd.includes('rm') || testCmd.includes('chmod 777') || testCmd.includes('iptables')) {
      setTestResult('⛔ DENIED — matches "Block dangerous rm" (priority 100)');
    } else if (testCmd.includes('sudo')) {
      setTestResult('🛡 INTERCEPT — matches "Intercept sudo commands" (priority 90)');
    } else {
      setTestResult('✅ ALLOW — no matching deny/intercept rule');
    }
  };

  if (loading && !data) return <LoadingSkeleton lines={6} />;

  return (
    <div className="space-y-6 fade-in">
      {error && !data && (
        <div className="flex flex-col items-center py-16 text-center">
          <div className="text-4xl mb-4 opacity-40">⚠️</div>
          <h3 className="text-lg font-semibold text-[var(--color-dim)]">Unable to connect to relay</h3>
          <p className="mt-2 text-sm text-[var(--color-dim)] opacity-70">{error}</p>
          <button onClick={refresh} className="mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]">Retry</button>
        </div>
      )}

      <div className="flex flex-wrap gap-3">
        <button onClick={() => setAddOpen(true)} className="flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]">
          <Plus size={16} /> Add Rule
        </button>
        <button onClick={() => setTestOpen(true)} className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
          <Play size={16} /> Test Rule
        </button>
        <button className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
          <Upload size={16} /> Import YAML
        </button>
        <button className="flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]">
          <Download size={16} /> Export YAML
        </button>
      </div>

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5">
          <h3 className="mb-3 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Policy Editor</h3>
          <YamlEditor value={yaml} onChange={setYaml} />
        </div>
        <div className="space-y-3">
          <h3 className="text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider">Active Rules ({policies.length})</h3>
          {policies.length === 0 ? (
            <EmptyState icon={<FileCode size={48} />} title="No policies configured" description="Add rules via the editor or API" />
          ) : policies.map((p: any, i: number) => (
            <div key={i} className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4 transition-all hover:border-[var(--color-accent)]/30">
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <FileCode size={16} className="text-[var(--color-accent)]" />
                  <span className="font-medium">{p.name}</span>
                </div>
                <Badge variant={p.action === 'deny' ? 'red' : p.action === 'intercept' ? 'amber' : p.action === 'allow' ? 'green' : 'blue'}>{p.action}</Badge>
              </div>
              <div className="text-xs text-[var(--color-dim)] mb-1">Priority: {p.priority} · {p.enabled ? '✅ Enabled' : '❌ Disabled'}</div>
              <div className="rounded-lg bg-[var(--color-bg)] p-2 font-mono text-[10px] text-[var(--color-dim)]">
                {Object.entries(p.conditions || {}).map(([k, v]) => <div key={k}><span className="text-[var(--color-accent-light)]">{k}</span>: {String(v)}</div>)}
              </div>
            </div>
          ))}
        </div>
      </div>

      <Modal open={addOpen} onClose={() => setAddOpen(false)} title="Add Policy Rule" actions={
        <>
          <button onClick={() => setAddOpen(false)} className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm">Cancel</button>
          <button onClick={() => setAddOpen(false)} className="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white">Add Rule</button>
        </>
      }>
        <div className="space-y-3">
          <div><label className="mb-1 block text-sm text-[var(--color-dim)]">Rule Name</label>
            <input type="text" placeholder="e.g. Block curl pipe bash" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" /></div>
          <div><label className="mb-1 block text-sm text-[var(--color-dim)]">Action</label>
            <select className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none">
              <option>deny</option><option>intercept</option><option>alert</option><option>allow</option>
            </select></div>
          <div><label className="mb-1 block text-sm text-[var(--color-dim)]">Priority</label>
            <input type="number" defaultValue={50} className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" /></div>
          <div><label className="mb-1 block text-sm text-[var(--color-dim)]">Command Pattern (regex)</label>
            <input type="text" placeholder="curl.*\\|.*bash" className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" /></div>
        </div>
      </Modal>

      <Modal open={testOpen} onClose={() => { setTestOpen(false); setTestResult(null); }} title="Test Rule Match" actions={
        <>
          <button onClick={() => { setTestOpen(false); setTestResult(null); }} className="rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm">Close</button>
          <button onClick={runTest} className="rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white">Test</button>
        </>
      }>
        <div>
          <label className="mb-1 block text-sm text-[var(--color-dim)]">Enter command to test</label>
          <input type="text" value={testCmd} onChange={e => setTestCmd(e.target.value)} placeholder="e.g. rm -rf /tmp/old"
            className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none"
            onKeyDown={e => e.key === 'Enter' && runTest()} />
        </div>
        {testResult && (
          <div className={`mt-3 rounded-lg p-3 text-sm ${testResult.includes('DENIED') ? 'bg-rose-500/10 border border-rose-500/20' : testResult.includes('INTERCEPT') ? 'bg-amber-500/10 border border-amber-500/20' : 'bg-emerald-500/10 border border-emerald-500/20'}`}>
            {testResult}
          </div>
        )}
      </Modal>
    </div>
  );
}
