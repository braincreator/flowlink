import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Wrench, Play, Server, TerminalSquare } from 'lucide-react';
import { StatCard, Badge, LoadingSkeleton, EmptyState, DataTable } from '../components/Layout';

interface McpServer {
  name: string;
  url: string;
  tools: number;
  status: string;
}

export default function MCP() {
  const { t } = useTranslation();
  const [toolInput, setToolInput] = useState('');
  const [result, setResult] = useState<string | null>(null);

  const servers: McpServer[] = [];
  const loading = false;

  if (loading) {
    return <LoadingSkeleton lines={6} />;
  }

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard label={t("common.servers")} value="0" color="accent" icon={<Server size={24} />} />
        <StatCard label={t("common.tools")} value="0" color="green" icon={<Wrench size={24} />} />
        <StatCard label={t('dashboard.commands_today')} value="—" color="blue" icon={<TerminalSquare size={24} />} />
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6">
        <h3 className="mb-4 font-semibold">{t("common.tool_execution")}</h3>
        <div className="flex gap-3">
          <input
            type="text"
            placeholder='{"tool": "name", "args": {}}}'
            value={toolInput}
            onChange={e => setToolInput(e.target.value)}
            className="flex-1 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm font-mono placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none transition-colors"
          />
          <button className="flex items-center gap-2 rounded-lg bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:opacity-90 transition-opacity">
            <Play size={14} /> {t('common.execute')}
          </button>
        </div>
        {result && (
          <pre className="mt-4 rounded-lg bg-[var(--color-bg)] border border-[var(--color-border)] p-4 text-xs font-mono text-[var(--color-dim)] overflow-x-auto max-h-64">
            {result}
          </pre>
        )}
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]">
        <div className="border-b border-[var(--color-border)] px-6 py-4">
          <h3 className="font-semibold">{t("common.connected_mcp")}</h3>
        </div>
        {servers.length === 0 ? (
          <EmptyState
            icon={<Wrench size={40} />}
            title={t("common.no_mcp")}
            description={t("common.no_mcp_desc")}
          />
        ) : (
          <DataTable
            columns={[
              { key: 'name', label: 'Server' },
              { key: 'url', label: 'URL' },
              { key: 'tools', label: 'Tools' },
              { key: 'status', label: 'Status', render: (row) => (
                <Badge variant={row.status === 'connected' ? 'green' : 'red'}>{row.status}</Badge>
              )},
            ]}
            data={servers}
            searchPlaceholder={t("common.search_servers")}
          />
        )}
      </div>
    </div>
  );
}
