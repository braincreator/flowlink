"use client";

import React, { useState, useEffect, useCallback } from "react";
import { useParams } from "next/navigation";

const API_BASE = "/api";

interface AuditEntry {
  id: number;
  org_id: string | null;
  account_id: string;
  action: string;
  resource_type: string | null;
  resource_id: string | null;
  details: Record<string, unknown> | null;
  ip_address: string | null;
  timestamp: string;
}

const ACTIONS = [
  "account.created",
  "account.updated",
  "account.deleted",
  "org.created",
  "org.updated",
  "org.member_added",
  "org.member_removed",
  "webhook.created",
  "webhook.deleted",
  "webhook.test",
  "auth.login",
  "auth.logout",
  "plan.changed",
  "subscription.created",
];

export default function AuditPage() {
  const params = useParams();
  const orgId = params.orgId as string;

  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [actionFilter, setActionFilter] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAudit = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const params2 = new URLSearchParams({
        page: String(page),
        limit: "50",
      });
      if (actionFilter) params2.set("action", actionFilter);

      const res = await fetch(
        `${API_BASE}/orgs/${orgId}/audit?${params2}`,
        { credentials: "include" }
      );
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setEntries(data.items || []);
      setTotal(data.total || 0);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Ошибка загрузки");
    } finally {
      setLoading(false);
    }
  }, [orgId, page, actionFilter]);

  useEffect(() => {
    fetchAudit();
  }, [fetchAudit]);

  const totalPages = Math.ceil(total / 50);

  const formatTimestamp = (ts: string) => {
    try {
      return new Date(ts).toLocaleString("ru-RU", {
        day: "2-digit",
        month: "2-digit",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      });
    } catch {
      return ts;
    }
  };

  const maskDetails = (details: Record<string, unknown> | null) => {
    if (!details) return "—";
    try {
      return JSON.stringify(details, null, 2);
    } catch {
      return "—";
    }
  };

  return (
    <div className="min-h-screen bg-[var(--bg-deep)] text-white p-6">
      <div className="max-w-7xl mx-auto">
        <h1 className="text-2xl font-bold mb-6">📋 Журнал аудита</h1>

        {/* Filters */}
        <div className="flex flex-wrap gap-3 mb-6">
          <select
            value={actionFilter}
            onChange={(e) => {
              setActionFilter(e.target.value);
              setPage(1);
            }}
            className="bg-[var(--bg-card)] border border-white/10 rounded-lg px-4 py-2 text-white"
          >
            <option value="">Все действия</option>
            {ACTIONS.map((a) => (
              <option key={a} value={a}>
                {a}
              </option>
            ))}
          </select>
          <button
            onClick={fetchAudit}
            className="bg-blue-600 hover:bg-blue-500 px-4 py-2 rounded-lg transition"
          >
            Обновить
          </button>
        </div>

        {/* Table */}
        <div className="overflow-x-auto rounded-xl border border-white/10">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-[var(--bg-card)] text-left text-gray-400">
                <th className="px-4 py-3">Время</th>
                <th className="px-4 py-3">Действие</th>
                <th className="px-4 py-3">Аккаунт</th>
                <th className="px-4 py-3">Ресурс</th>
                <th className="px-4 py-3">IP</th>
                <th className="px-4 py-3">Детали</th>
              </tr>
            </thead>
            <tbody>
              {entries.length === 0 && !loading && (
                <tr>
                  <td colSpan={6} className="px-4 py-8 text-center text-gray-500">
                    {error || "Нет записей"}
                  </td>
                </tr>
              )}
              {entries.map((e) => (
                <tr
                  key={e.id}
                  className="border-t border-white/5 hover:bg-white/5 transition"
                >
                  <td className="px-4 py-3 font-mono text-xs text-gray-300">
                    {formatTimestamp(e.timestamp)}
                  </td>
                  <td className="px-4 py-3">
                    <span className="inline-block bg-blue-500/20 text-blue-300 px-2 py-0.5 rounded text-xs">
                      {e.action}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-gray-300">{e.account_id}</td>
                  <td className="px-4 py-3 text-gray-400">
                    {e.resource_type
                      ? `${e.resource_type}${e.resource_id ? `:${e.resource_id.slice(0, 8)}` : ""}`
                      : "—"}
                  </td>
                  <td className="px-4 py-3 font-mono text-xs text-gray-400">
                    {e.ip_address || "—"}
                  </td>
                  <td className="px-4 py-3 max-w-xs truncate font-mono text-xs text-gray-500">
                    <details className="cursor-pointer">
                      <summary className="text-gray-400">Показать</summary>
                      <pre className="mt-1 p-2 bg-black/30 rounded text-xs overflow-auto max-h-32">
                        {maskDetails(e.details)}
                      </pre>
                    </details>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Pagination */}
        <div className="flex items-center justify-between mt-4">
          <span className="text-sm text-gray-400">
            Записей: {total} · Страница {page} из {totalPages || 1}
          </span>
          <div className="flex gap-2">
            <button
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              disabled={page <= 1}
              className="bg-[var(--bg-card)] border border-white/10 px-3 py-1.5 rounded-lg disabled:opacity-40 hover:bg-white/10 transition"
            >
              ← Назад
            </button>
            <button
              onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
              disabled={page >= totalPages}
              className="bg-[var(--bg-card)] border border-white/10 px-3 py-1.5 rounded-lg disabled:opacity-40 hover:bg-white/10 transition"
            >
              Вперёд →
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
