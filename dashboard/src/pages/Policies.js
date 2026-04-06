import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useState } from 'react';
import { Plus, Upload, Download, Play, FileCode } from 'lucide-react';
import { Badge, Modal, YamlEditor, LoadingSkeleton } from '../components/Layout';
import { useApi } from '../hooks/useApi';
import { mockPolicies } from '../api/client';
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
    const [testResult, setTestResult] = useState(null);
    const { data: policies, loading, isLive } = useApi(async () => mockPolicies, mockPolicies);
    const runTest = () => {
        if (!testCmd)
            return;
        if (testCmd.includes('rm') || testCmd.includes('chmod 777') || testCmd.includes('iptables')) {
            setTestResult('⛔ DENIED — matches "Block dangerous rm" (priority 100)');
        }
        else if (testCmd.includes('sudo')) {
            setTestResult('🛡 INTERCEPT — matches "Intercept sudo commands" (priority 90)');
        }
        else {
            setTestResult('✅ ALLOW — no matching deny/intercept rule');
        }
    };
    if (loading)
        return _jsx(LoadingSkeleton, { lines: 6 });
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [!isLive && (_jsx("div", { className: "rounded-xl border border-amber-500/30 bg-amber-500/5 px-4 py-3 text-sm text-amber-400", children: "\u26A0\uFE0F Connected to mock data. Start relay for live data." })), _jsxs("div", { className: "flex flex-wrap gap-3", children: [_jsxs("button", { onClick: () => setAddOpen(true), className: "flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]", children: [_jsx(Plus, { size: 16 }), " Add Rule"] }), _jsxs("button", { onClick: () => setTestOpen(true), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Play, { size: 16 }), " Test Rule"] }), _jsxs("button", { className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Upload, { size: 16 }), " Import YAML"] }), _jsxs("button", { className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Download, { size: 16 }), " Export YAML"] })] }), _jsxs("div", { className: "grid grid-cols-1 gap-6 xl:grid-cols-2", children: [_jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsx("h3", { className: "mb-3 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: "Policy Editor" }), _jsx(YamlEditor, { value: yaml, onChange: setYaml })] }), _jsxs("div", { className: "space-y-3", children: [_jsxs("h3", { className: "text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: ["Active Rules (", policies.length, ")"] }), policies.map((p, i) => ( // eslint-disable-next-line
                            _jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4 transition-all hover:border-[var(--color-accent)]/30", children: [_jsxs("div", { className: "flex items-center justify-between mb-2", children: [_jsxs("div", { className: "flex items-center gap-2", children: [_jsx(FileCode, { size: 16, className: "text-[var(--color-accent)]" }), _jsx("span", { className: "font-medium", children: p.name })] }), _jsx(Badge, { variant: p.action === 'deny' ? 'red' : p.action === 'intercept' ? 'amber' : p.action === 'allow' ? 'green' : 'blue', children: p.action })] }), _jsxs("div", { className: "text-xs text-[var(--color-dim)] mb-1", children: ["Priority: ", p.priority, " \u00B7 ", p.enabled ? '✅ Enabled' : '❌ Disabled'] }), _jsx("div", { className: "rounded-lg bg-[var(--color-bg)] p-2 font-mono text-[10px] text-[var(--color-dim)]", children: Object.entries(p.conditions || {}).map(([k, v]) => _jsxs("div", { children: [_jsx("span", { className: "text-[var(--color-accent-light)]", children: k }), ": ", String(v)] }, k)) })] }, i)))] })] }), _jsx(Modal, { open: addOpen, onClose: () => setAddOpen(false), title: "Add Policy Rule", actions: _jsxs(_Fragment, { children: [_jsx("button", { onClick: () => setAddOpen(false), className: "rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm", children: "Cancel" }), _jsx("button", { onClick: () => setAddOpen(false), className: "rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white", children: "Add Rule" })] }), children: _jsxs("div", { className: "space-y-3", children: [_jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: "Rule Name" }), _jsx("input", { type: "text", placeholder: "e.g. Block curl pipe bash", className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" })] }), _jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: "Action" }), _jsxs("select", { className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none", children: [_jsx("option", { children: "deny" }), _jsx("option", { children: "intercept" }), _jsx("option", { children: "alert" }), _jsx("option", { children: "allow" })] })] }), _jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: "Priority" }), _jsx("input", { type: "number", defaultValue: 50, className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" })] }), _jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: "Command Pattern (regex)" }), _jsx("input", { type: "text", placeholder: "curl.*\\\\|.*bash", className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" })] })] }) }), _jsxs(Modal, { open: testOpen, onClose: () => { setTestOpen(false); setTestResult(null); }, title: "Test Rule Match", actions: _jsxs(_Fragment, { children: [_jsx("button", { onClick: () => { setTestOpen(false); setTestResult(null); }, className: "rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm", children: "Close" }), _jsx("button", { onClick: runTest, className: "rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white", children: "Test" })] }), children: [_jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: "Enter command to test" }), _jsx("input", { type: "text", value: testCmd, onChange: e => setTestCmd(e.target.value), placeholder: "e.g. rm -rf /tmp/old", className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none", onKeyDown: e => e.key === 'Enter' && runTest() })] }), testResult && (_jsx("div", { className: `mt-3 rounded-lg p-3 text-sm ${testResult.includes('DENIED') ? 'bg-rose-500/10 border border-rose-500/20' : testResult.includes('INTERCEPT') ? 'bg-amber-500/10 border border-amber-500/20' : 'bg-emerald-500/10 border border-emerald-500/20'}`, children: testResult }))] })] }));
}
