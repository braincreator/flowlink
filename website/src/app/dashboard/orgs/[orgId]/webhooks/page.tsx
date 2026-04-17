"use client";

import React, { useState, useEffect, useCallback } from "react";
import { useParams } from "next/navigation";

const API_BASE = "/api";

interface Webhook {
  id: string;
  org_id: string;
  url: string;
  events: string[];
  is_active: boolean;
  created_at: string;
  last_triggered_at: string | null;
}

const EVENT_TYPES = [
  "account.created",
  "account.updated",
  "account.deleted",
  "org.member_added",
  "org.member_removed",
  "plan.changed",
  "webhook.test",
];

export default function WebhooksPage() {
  const params = useParams();
  const orgId = params.orgId as string;

  const [webhooks, setWebhooks] = useState<Webhook[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);

  // Form state
  const [formUrl, setFormUrl] = useState("");
  const [formEvents, setFormEvents] = useState<string[]>([]);
  const [formLoading, setFormLoading] = useState(false);

  const fetchWebhooks = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(`${API_BASE}/orgs/${orgId}/webhooks`, {
        credentials: "include",
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setWebhooks(data.items || []);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Ошибка загрузки");
    } finally {
      setLoading(false);
    }
  }, [orgId]);

  useEffect(() => {
    fetchWebhooks();
  }, [fetchWebhooks]);

  const maskUrl = (url: string) => {
    try {
      const u = new URL(url);
      const host = u.hostname;
      const masked = host.length > 12 ? host.slice(0, 6) + "***" + host.slice(-4) : host;
      return masked + u.pathname;
    } catch {
      return "***";
    }
  };

  const toggleEvent = (event: string) => {
    setFormEvents((prev) =>
      prev.includes(event) ? prev.filter((e) => e !== event) : [...prev, event]
    );
  };

  const handleCreate = async () => {
    if (!formUrl || formEvents.length === 0) return;
    setFormLoading(true);
    try {
      const res = await fetch(`${API_BASE}/orgs/${orgId}/webhooks`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url: formUrl, events: formEvents }),
      });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.error || `HTTP ${res.status}`);
      }
      setFormUrl("");
      setFormEvents([]);
      setShowForm(false);
      fetchWebhooks();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Ошибка");
    } finally {
      setFormLoading(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Удалить webhook?")) return;
    try {
      const res = await fetch(`${API_BASE}/orgs/${orgId}/webhooks/${id}`, {
        method: "DELETE",
        credentials: "include",
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      fetchWebhooks();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Ошибка");
    }
  };

  const handleTest = async (id: string) => {
    try {
      const res = await fetch(`${API_BASE}/orgs/${orgId}/webhooks/${id}/test`, {
        method: "POST",
        credentials: "include",
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      alert("✅ Тестовый пинг отправлен");
    } catch (e) {
      alert(e instanceof Error ? e.message : "Ошибка");
    }
  };

  const formatDate = (ts: string) => {
    try {
      return new Date(ts).toLocaleString("ru-RU");
    } catch {
      return ts;
    }
  };

  return (
    <div className="min-h-screen bg-[var(--bg-deep)] text-white p-6">
      <div className="max-w-5xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-2xl font-bold">🔗 Вебхуки</h1>
          <button
            onClick={() => setShowForm(!showForm)}
            className="bg-blue-600 hover:bg-blue-500 px-4 py-2 rounded-lg transition"
          >
            {showForm ? "Отмена" : "+ Добавить"}
          </button>
        </div>

        {/* Create form */}
        {showForm && (
          <div className="bg-[var(--bg-card)] border border-white/10 rounded-xl p-6 mb-6">
            <h2 className="text-lg font-semibold mb-4">Новый вебхук</h2>
            <div className="mb-4">
              <label className="block text-sm text-gray-400 mb-1">URL</label>
              <input
                type="url"
                value={formUrl}
                onChange={(e) => setFormUrl(e.target.value)}
                placeholder="https://example.com/webhook"
                className="w-full bg-[var(--bg-elevated)] border border-white/10 rounded-lg px-4 py-2 text-white"
              />
            </div>
            <div className="mb-4">
              <label className="block text-sm text-gray-400 mb-2">События</label>
              <div className="flex flex-wrap gap-2">
                {EVENT_TYPES.map((event) => (
                  <button
                    key={event}
                    onClick={() => toggleEvent(event)}
                    className={`px-3 py-1.5 rounded-lg text-sm border transition ${
                      formEvents.includes(event)
                        ? "bg-blue-600 border-blue-500 text-white"
                        : "bg-[var(--bg-elevated)] border-white/10 text-gray-400 hover:border-white/20"
                    }`}
                  >
                    {event}
                  </button>
                ))}
              </div>
            </div>
            <button
              onClick={handleCreate}
              disabled={formLoading || !formUrl || formEvents.length === 0}
              className="bg-green-600 hover:bg-green-500 disabled:opacity-40 px-6 py-2 rounded-lg transition"
            >
              {formLoading ? "Создание..." : "Создать"}
            </button>
            <p className="text-xs text-gray-500 mt-2">
              Секрет будет сгенерирован автоматически
            </p>
          </div>
        )}

        {/* List */}
        {loading ? (
          <div className="text-center text-gray-400 py-8">Загрузка...</div>
        ) : webhooks.length === 0 ? (
          <div className="text-center text-gray-500 py-8">
            {error || "Нет вебхуков"}
          </div>
        ) : (
          <div className="space-y-3">
            {webhooks.map((wh) => (
              <div
                key={wh.id}
                className="bg-[var(--bg-card)] border border-white/10 rounded-xl p-4 hover:border-white/20 transition"
              >
                <div className="flex items-start justify-between">
                  <div>
                    <div className="font-mono text-sm text-gray-300 mb-1">
                      {maskUrl(wh.url)}
                    </div>
                    <div className="flex flex-wrap gap-1 mb-2">
                      {wh.events.map((e) => (
                        <span
                          key={e}
                          className="inline-block bg-purple-500/20 text-purple-300 px-2 py-0.5 rounded text-xs"
                        >
                          {e}
                        </span>
                      ))}
                    </div>
                    <div className="text-xs text-gray-500">
                      Статус:{" "}
                      <span
                        className={
                          wh.is_active ? "text-green-400" : "text-red-400"
                        }
                      >
                        {wh.is_active ? "активен" : "отключён"}
                      </span>
                      {wh.last_triggered_at && (
                        <>
                          {" "}
                          · Последний вызов:{" "}
                          {formatDate(wh.last_triggered_at)}
                        </>
                      )}
                      {" · Создан: "}
                      {formatDate(wh.created_at)}
                    </div>
                  </div>
                  <div className="flex gap-2">
                    <button
                      onClick={() => handleTest(wh.id)}
                      className="bg-yellow-600/80 hover:bg-yellow-500 px-3 py-1.5 rounded-lg text-sm transition"
                    >
                      🧪 Тест
                    </button>
                    <button
                      onClick={() => handleDelete(wh.id)}
                      className="bg-red-600/80 hover:bg-red-500 px-3 py-1.5 rounded-lg text-sm transition"
                    >
                      🗑 Удалить
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
