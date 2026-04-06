import { ReactNode } from 'react';
import type { ToastMessage } from '../types';

export function StatCard({ label, value, trend, sparkline, icon, color = 'accent' }: {
  label: string; value: string | number; trend?: { value: number; label: string };
  sparkline?: ReactNode; icon?: ReactNode; color?: string;
}) {
  const colors: Record<string, string> = {
    accent: 'from-indigo-500/20 to-indigo-600/5 border-indigo-500/30',
    green: 'from-emerald-500/20 to-emerald-600/5 border-emerald-500/30',
    red: 'from-rose-500/20 to-rose-600/5 border-rose-500/30',
    amber: 'from-amber-500/20 to-amber-600/5 border-amber-500/30',
    blue: 'from-blue-500/20 to-blue-600/5 border-blue-500/30',
  };
  return (
    <div className={`relative overflow-hidden rounded-xl border bg-gradient-to-br p-5 transition-all duration-200 hover:scale-[1.01] ${colors[color] || colors.accent}`}>
      {icon && <div className="absolute top-4 right-4 opacity-40">{icon}</div>}
      <div className="text-xs font-medium uppercase tracking-wider text-[var(--color-dim)] mb-2">{label}</div>
      <div className="text-3xl font-bold tracking-tight">{value}</div>
      {trend && (
        <div className={`mt-2 text-xs font-medium ${trend.value >= 0 ? 'text-emerald-400' : 'text-rose-400'}`}>
          {trend.value >= 0 ? '↑' : '↓'} {Math.abs(trend.value)}% {trend.label}
        </div>
      )}
      {sparkline && <div className="mt-3 h-8">{sparkline}</div>}
    </div>
  );
}

export function Badge({ children, variant = 'default', className = '' }: { children: ReactNode; variant?: 'default' | 'green' | 'red' | 'amber' | 'blue' | 'purple'; className?: string }) {
  const styles: Record<string, string> = {
    default: 'bg-surface3 text-dim',
    green: 'bg-emerald-500/15 text-emerald-400',
    red: 'bg-rose-500/15 text-rose-400',
    amber: 'bg-amber-500/15 text-amber-400',
    blue: 'bg-blue-500/15 text-blue-400',
    purple: 'bg-indigo-500/15 text-indigo-400',
  };
  return <span className={`inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-semibold ${styles[variant]} ${className}`}>{children}</span>;
}

export function DataTable<T extends Record<string, any>>({ columns, data, onRowClick, emptyText = 'No data', searchPlaceholder }: {
  columns: { key: string; label: string; render?: (row: T) => ReactNode; className?: string }[];
  data: T[]; onRowClick?: (row: T) => void; emptyText?: string; searchPlaceholder?: string;
}) {
  const [search, setSearch] = useState('');
  const [sortKey, setSortKey] = useState<string | null>(null);
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('asc');
  const [page, setPage] = useState(0);
  const pageSize = 10;

  const filtered = data.filter(row =>
    !search || Object.values(row).some(v => String(v).toLowerCase().includes(search.toLowerCase()))
  );

  const sorted = [...filtered].sort((a, b) => {
    if (!sortKey) return 0;
    const av = a[sortKey], bv = b[sortKey];
    const cmp = String(av).localeCompare(String(bv));
    return sortDir === 'asc' ? cmp : -cmp;
  });

  const paged = sorted.slice(page * pageSize, (page + 1) * pageSize);
  const totalPages = Math.ceil(sorted.length / pageSize);

  const toggleSort = (key: string) => {
    if (sortKey === key) setSortDir(d => d === 'asc' ? 'desc' : 'asc');
    else { setSortKey(key); setSortDir('asc'); }
    setPage(0);
  };

  return (
    <div>
      {searchPlaceholder && (
        <div className="mb-4">
          <input
            type="text" placeholder={searchPlaceholder} value={search}
            onChange={e => { setSearch(e.target.value); setPage(0); }}
            className="w-full max-w-xs rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] px-3 py-2 text-sm placeholder-[var(--color-dim)] focus:border-[var(--color-accent)] focus:outline-none transition-colors"
          />
        </div>
      )}
      <div className="overflow-x-auto rounded-xl border border-[var(--color-border)]">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-[var(--color-border)] bg-[var(--color-surface)]">
              {columns.map(col => (
                <th key={col.key} onClick={() => toggleSort(col.key)}
                  className={`cursor-pointer px-4 py-3 text-left text-xs font-medium uppercase tracking-wider text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors ${col.className || ''}`}>
                  {col.label} {sortKey === col.key && (sortDir === 'asc' ? '↑' : '↓')}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {paged.length === 0 ? (
              <tr><td colSpan={columns.length} className="px-4 py-12 text-center text-[var(--color-dim)]">{emptyText}</td></tr>
            ) : paged.map((row, i) => (
              <tr key={i} onClick={() => onRowClick?.(row)}
                className={`border-b border-[var(--color-border)] transition-colors hover:bg-[var(--color-surface2)] ${onRowClick ? 'cursor-pointer' : ''}`}>
                {columns.map(col => (
                  <td key={col.key} className={`px-4 py-3 ${col.className || ''}`}>
                    {col.render ? col.render(row) : String(row[col.key] ?? '')}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {totalPages > 1 && (
        <div className="mt-4 flex items-center justify-between text-sm text-[var(--color-dim)]">
          <span>{filtered.length} results</span>
          <div className="flex gap-2">
            <button onClick={() => setPage(p => Math.max(0, p - 1))} disabled={page === 0}
              className="rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs hover:bg-[var(--color-surface2)] disabled:opacity-30 transition-colors">Prev</button>
            <span className="flex items-center px-2">{page + 1} / {totalPages}</span>
            <button onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))} disabled={page >= totalPages - 1}
              className="rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs hover:bg-[var(--color-surface2)] disabled:opacity-30 transition-colors">Next</button>
          </div>
        </div>
      )}
    </div>
  );
}

import { useState } from 'react';

export function Modal({ open, onClose, title, children, actions }: {
  open: boolean; onClose: () => void; title: string; children: ReactNode;
  actions?: ReactNode;
}) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" onClick={onClose}>
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />
      <div className="relative w-full max-w-lg rounded-2xl border border-[var(--color-border)] bg-[var(--color-surface)] p-6 shadow-2xl fade-in" onClick={e => e.stopPropagation()}>
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-lg font-semibold">{title}</h3>
          <button onClick={onClose} className="rounded-lg p-1 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors">✕</button>
        </div>
        <div className="space-y-4">{children}</div>
        {actions && <div className="mt-6 flex justify-end gap-3">{actions}</div>}
      </div>
    </div>
  );
}

export function SlidePanel({ open, onClose, title, children, width = 'w-[480px]' }: {
  open: boolean; onClose: () => void; title: string; children: ReactNode; width?: string;
}) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50" onClick={onClose}>
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm" />
      <div className={`absolute right-0 top-0 bottom-0 ${width} overflow-y-auto border-l border-[var(--color-border)] bg-[var(--color-surface)] shadow-2xl slide-in-right`} onClick={e => e.stopPropagation()}>
        <div className="sticky top-0 z-10 flex items-center justify-between border-b border-[var(--color-border)] bg-[var(--color-surface)] px-6 py-4">
          <h3 className="text-lg font-semibold">{title}</h3>
          <button onClick={onClose} className="rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors">✕</button>
        </div>
        <div className="p-6">{children}</div>
      </div>
    </div>
  );
}

export function RiskGauge({ score, size = 100 }: { score: number; size?: number }) {
  const r = (size - 12) / 2;
  const circ = 2 * Math.PI * r;
  const offset = circ * (1 - score / 100);
  const color = score >= 70 ? 'var(--color-red)' : score >= 40 ? 'var(--color-amber)' : 'var(--color-green)';
  return (
    <div className="inline-flex flex-col items-center gap-1">
      <svg width={size} height={size} className="-rotate-90">
        <circle cx={size/2} cy={size/2} r={r} fill="none" stroke="var(--color-surface3)" strokeWidth="6" />
        <circle cx={size/2} cy={size/2} r={r} fill="none" stroke={color} strokeWidth="6"
          strokeDasharray={circ} strokeDashoffset={offset} strokeLinecap="round"
          className="transition-all duration-700" />
      </svg>
      <span className="text-lg font-bold" style={{ color }}>{score}</span>
    </div>
  );
}

export function TerminalOutput({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const copy = () => { navigator.clipboard.writeText(text); setCopied(true); setTimeout(() => setCopied(false), 2000); };
  return (
    <div className="relative rounded-xl border border-[var(--color-border)] bg-[#0d0e14] p-4 font-mono text-sm">
      <button onClick={copy} className="absolute top-2 right-2 rounded-md bg-[var(--color-surface2)] px-2 py-1 text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
        {copied ? '✓ Copied' : 'Copy'}
      </button>
      <pre className="whitespace-pre-wrap break-all text-[var(--color-text)] max-h-80 overflow-auto">{text}</pre>
    </div>
  );
}

export function EmptyState({ icon, title, description }: { icon: ReactNode; title: string; description?: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="mb-4 text-4xl opacity-40">{icon}</div>
      <h3 className="text-lg font-semibold text-[var(--color-dim)]">{title}</h3>
      {description && <p className="mt-2 max-w-sm text-sm text-[var(--color-dim)] opacity-70">{description}</p>}
    </div>
  );
}

export function LoadingSkeleton({ lines = 3 }: { lines?: number }) {
  return (
    <div className="space-y-3">
      {Array.from({ length: lines }, (_, i) => (
        <div key={i} className="shimmer h-4 rounded" style={{ width: `${60 + Math.random() * 40}%` }} />
      ))}
    </div>
  );
}

export function Toast({ toasts, onRemove }: { toasts: ToastMessage[]; onRemove: (id: string) => void }) {
  const icons: Record<string, string> = { success: '✓', error: '✕', info: 'ℹ', warning: '⚠' };
  const colors: Record<string, string> = {
    success: 'border-emerald-500/40 bg-emerald-500/10',
    error: 'border-rose-500/40 bg-rose-500/10',
    info: 'border-blue-500/40 bg-blue-500/10',
    warning: 'border-amber-500/40 bg-amber-500/10',
  };
  return (
    <div className="fixed bottom-6 right-6 z-[100] flex flex-col gap-2">
      {toasts.map(t => (
        <div key={t.id} className={`flex items-center gap-3 rounded-xl border px-4 py-3 shadow-lg fade-in ${colors[t.type]}`}>
          <span className="text-lg">{icons[t.type]}</span>
          <div>
            <div className="text-sm font-medium">{t.title}</div>
            {t.message && <div className="text-xs text-[var(--color-dim)]">{t.message}</div>}
          </div>
          <button onClick={() => onRemove(t.id)} className="ml-2 text-[var(--color-dim)] hover:text-[var(--color-text)]">✕</button>
        </div>
      ))}
    </div>
  );
}

export function YamlEditor({ value, onChange, readOnly = false }: { value: string; onChange?: (v: string) => void; readOnly?: boolean }) {
  const [val, setVal] = useState(value);
  const update = (v: string) => { setVal(v); onChange?.(v); };
  return (
    <textarea
      value={val} onChange={e => update(e.target.value)} readOnly={readOnly} spellCheck={false}
      className="h-96 w-full rounded-xl border border-[var(--color-border)] bg-[#0d0e14] p-4 font-mono text-sm leading-relaxed text-[var(--color-text)] focus:border-[var(--color-accent)] focus:outline-none resize-none"
    />
  );
}
