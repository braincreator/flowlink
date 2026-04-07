import { useTranslation } from 'react-i18next';
import { Brain, RefreshCw, Globe, Server } from 'lucide-react';
import { StatCard, Badge, LoadingSkeleton, EmptyState, DataTable } from '../components/Layout';

interface LlmBackend {
  name: string;
  url: string;
  model: string;
  status: string;
}

export default function LLM() {
  const { t } = useTranslation();
  const loading = false;
  const backends: LlmBackend[] = [];
  const healthy = true;

  if (loading) {
    return <LoadingSkeleton lines={6} />;
  }

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
        <StatCard label={t("common.backends")} value="0" color="accent" icon={<Brain size={24} />} />
        <StatCard label={t('metrics.health')} value={healthy ? 'OK' : 'Degraded'} color={healthy ? 'green' : 'red'} icon={<Server size={24} />} />
        <StatCard label={t("common.models")} value="—" color="blue" icon={<Globe size={24} />} />
      </div>

      <div className="rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]">
        <div className="flex items-center justify-between border-b border-[var(--color-border)] px-6 py-4">
          <h3 className="font-semibold">{`${t("common.backends")} LLM`}</h3>
          <button className="flex items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs text-[var(--color-dim)] hover:bg-[var(--color-surface2)] transition-colors">
            <RefreshCw size={12} /> {t('common.refresh')}
          </button>
        </div>
        {backends.length === 0 ? (
          <EmptyState
            icon={<Brain size={40} />}
            title={t("common.no_backends")}
            description={t("common.no_backends_desc")}
          />
        ) : (
          <DataTable
            columns={[
              { key: 'name', label: 'Name' },
              { key: 'url', label: 'URL' },
              { key: 'model', label: 'Model' },
              { key: 'status', label: 'Status', render: (row) => (
                <Badge variant={row.status === 'healthy' ? 'green' : 'red'}>{row.status}</Badge>
              )},
            ]}
            data={backends}
            searchPlaceholder={t("common.search_backends")}
          />
        )}
      </div>
    </div>
  );
}
