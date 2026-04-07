import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
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
    const { t } = useTranslation();
    const [yaml, setYaml] = useState(DEFAULT_YAML);
    const [addOpen, setAddOpen] = useState(false);
    const [testOpen, setTestOpen] = useState(false);
    const [testCmd, setTestCmd] = useState('');
    const [testResult, setTestResult] = useState(null);
    const { data, loading, error, refresh } = useApi(() => api.getPolicies());
    const policies = data || [];
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
    if (loading && !data)
        return _jsx(LoadingSkeleton, { lines: 6 });
    return (_jsxs("div", { className: "space-y-6 fade-in", children: [error && !data && (_jsxs("div", { className: "flex flex-col items-center py-16 text-center", children: [_jsx("div", { className: "text-4xl mb-4 opacity-40", children: "\u26A0\uFE0F" }), _jsx("h3", { className: "text-lg font-semibold text-[var(--color-dim)]", children: t('common.unable_connect') }), _jsx("p", { className: "mt-2 text-sm text-[var(--color-dim)] opacity-70", children: error }), _jsx("button", { onClick: refresh, className: "mt-4 rounded-xl bg-[var(--color-accent)] px-4 py-2 text-sm text-white hover:bg-[var(--color-accent-light)]", children: t('common.retry') })] })), _jsxs("div", { className: "flex flex-wrap gap-3", children: [_jsxs("button", { onClick: () => setAddOpen(true), className: "flex items-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-2.5 text-sm font-medium text-white transition-all hover:bg-[var(--color-accent-light)]", children: [_jsx(Plus, { size: 16 }), " ", t('policies.add_rule')] }), _jsxs("button", { onClick: () => setTestOpen(true), className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Play, { size: 16 }), " ", t('policies.test_rule')] }), _jsxs("button", { className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Upload, { size: 16 }), " ", t('policies.import')] }), _jsxs("button", { className: "flex items-center gap-2 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-2.5 text-sm font-medium transition-colors hover:bg-[var(--color-surface2)]", children: [_jsx(Download, { size: 16 }), " ", t('policies.export')] })] }), _jsxs("div", { className: "grid grid-cols-1 gap-6 xl:grid-cols-2", children: [_jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-5", children: [_jsx("h3", { className: "mb-3 text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: t('policies.editor') }), _jsx(YamlEditor, { value: yaml, onChange: setYaml })] }), _jsxs("div", { className: "space-y-3", children: [_jsxs("h3", { className: "text-sm font-semibold text-[var(--color-dim)] uppercase tracking-wider", children: [t("policies.active_rules"), " (", policies.length, ")"] }), policies.length === 0 ? (_jsx(EmptyState, { icon: _jsx(FileCode, { size: 48 }), title: t('policies.no_rules'), description: t("policies.add_rules_desc") })) : policies.map((p, i) => (_jsxs("div", { className: "rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-4 transition-all hover:border-[var(--color-accent)]/30", children: [_jsxs("div", { className: "flex items-center justify-between mb-2", children: [_jsxs("div", { className: "flex items-center gap-2", children: [_jsx(FileCode, { size: 16, className: "text-[var(--color-accent)]" }), _jsx("span", { className: "font-medium", children: p.name })] }), _jsx(Badge, { variant: p.action === 'deny' ? 'red' : p.action === 'intercept' ? 'amber' : p.action === 'allow' ? 'green' : 'blue', children: p.action })] }), _jsxs("div", { className: "text-xs text-[var(--color-dim)] mb-1", children: [t('policies.priority'), ": ", p.priority, " \u00B7 ", p.enabled ? `✅ ${t('shield.enabled')}` : `❌ ${t('shield.disabled')}`] }), _jsx("div", { className: "rounded-lg bg-[var(--color-bg)] p-2 font-mono text-[10px] text-[var(--color-dim)]", children: Object.entries(p.conditions || {}).map(([k, v]) => _jsxs("div", { children: [_jsx("span", { className: "text-[var(--color-accent-light)]", children: k }), ": ", String(v)] }, k)) })] }, i)))] })] }), _jsx(Modal, { open: addOpen, onClose: () => setAddOpen(false), title: t('policies.add_rule'), actions: _jsxs(_Fragment, { children: [_jsx("button", { onClick: () => setAddOpen(false), className: "rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm", children: t('common.cancel') }), _jsx("button", { onClick: () => setAddOpen(false), className: "rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white", children: t('policies.add_rule') })] }), children: _jsxs("div", { className: "space-y-3", children: [_jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: t("policies.rule_name") }), _jsx("input", { type: "text", placeholder: "e.g. Block curl pipe bash", className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" })] }), _jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: t("policies.action") }), _jsxs("select", { className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none", children: [_jsx("option", { children: "deny" }), _jsx("option", { children: "intercept" }), _jsx("option", { children: "alert" }), _jsx("option", { children: "allow" })] })] }), _jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: t("policies.priority") }), _jsx("input", { type: "number", defaultValue: 50, className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 text-sm focus:border-[var(--color-accent)] focus:outline-none" })] }), _jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: t("policies.command_pattern") }), _jsx("input", { type: "text", placeholder: "curl.*\\\\|.*bash", className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none" })] })] }) }), _jsxs(Modal, { open: testOpen, onClose: () => { setTestOpen(false); setTestResult(null); }, title: t('policies.test_rule'), actions: _jsxs(_Fragment, { children: [_jsx("button", { onClick: () => { setTestOpen(false); setTestResult(null); }, className: "rounded-lg border border-[var(--color-border)] px-4 py-2 text-sm", children: t('common.close') }), _jsx("button", { onClick: runTest, className: "rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm text-white", children: t('policies.test_rule') })] }), children: [_jsxs("div", { children: [_jsx("label", { className: "mb-1 block text-sm text-[var(--color-dim)]", children: t("policies.enter_command_test") }), _jsx("input", { type: "text", value: testCmd, onChange: e => setTestCmd(e.target.value), placeholder: "e.g. rm -rf /tmp/old", className: "w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2.5 font-mono text-sm focus:border-[var(--color-accent)] focus:outline-none", onKeyDown: e => e.key === 'Enter' && runTest() })] }), testResult && (_jsx("div", { className: `mt-3 rounded-lg p-3 text-sm ${testResult.includes('DENIED') ? 'bg-rose-500/10 border border-rose-500/20' : testResult.includes('INTERCEPT') ? 'bg-amber-500/10 border border-amber-500/20' : 'bg-emerald-500/10 border border-emerald-500/20'}`, children: testResult }))] })] }));
}
