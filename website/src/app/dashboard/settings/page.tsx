"use client";

import React, { useState, useEffect, useCallback } from "react";

const API_BASE = "/api";

interface DeletionStatus {
  deletion_requested_at: string | null;
  deleted_at: string | null;
}

export default function SettingsContent() {
  const [deletionStatus, setDeletionStatus] = useState<DeletionStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [showHardModal, setShowHardModal] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState(false);

  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/account/info`, {
        credentials: "include",
      });
      if (!res.ok) return;
      const data = await res.json();
      setDeletionStatus({
        deletion_requested_at: data.deletion_requested_at ?? null,
        deleted_at: data.deleted_at ?? null,
      });
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  const requestDeletion = async () => {
    setActionLoading(true);
    setError(null);
    try {
      const res = await fetch(`${API_BASE}/account`, {
        method: "DELETE",
        credentials: "include",
      });
      const data = await res.json();
      if (!res.ok) {
        setError(data.error || "Ошибка");
        return;
      }
      setShowModal(false);
      fetchStatus();
    } catch {
      setError("Ошибка сети");
    } finally {
      setActionLoading(false);
    }
  };

  const cancelDeletion = async () => {
    setActionLoading(true);
    setError(null);
    try {
      const res = await fetch(`${API_BASE}/account/cancel-deletion`, {
        method: "POST",
        credentials: "include",
      });
      const data = await res.json();
      if (!res.ok) {
        setError(data.error || "Ошибка");
        return;
      }
      fetchStatus();
    } catch {
      setError("Ошибка сети");
    } finally {
      setActionLoading(false);
    }
  };

  const hardDelete = async () => {
    setActionLoading(true);
    setError(null);
    try {
      const res = await fetch(`${API_BASE}/account/hard`, {
        method: "DELETE",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ confirmation_code: "DELETE_MY_ACCOUNT" }),
      });
      const data = await res.json();
      if (!res.ok) {
        setError(data.error || "Ошибка");
        return;
      }
      // Account deleted — redirect
      window.location.href = "/api/auth/logout";
    } catch {
      setError("Ошибка сети");
    } finally {
      setActionLoading(false);
    }
  };

  const getRemainingDays = (): number | null => {
    if (!deletionStatus?.deleted_at) return null;
    const deletedAt = new Date(deletionStatus.deleted_at);
    const now = new Date();
    const diff = deletedAt.getTime() - now.getTime();
    return Math.max(0, Math.ceil(diff / (1000 * 60 * 60 * 24)));
  };

  if (loading) {
    return (
      <div className="p-6 max-w-2xl mx-auto">
        <p className="text-gray-500">Загрузка...</p>
      </div>
    );
  }

  const remainingDays = getRemainingDays();
  const isPending = deletionStatus?.deletion_requested_at != null && remainingDays !== null && remainingDays > 0;

  return (
    <div className="p-6 max-w-2xl mx-auto">
      <h1 className="text-2xl font-bold mb-6">Настройки</h1>

      {/* Danger Zone */}
      <div className="border border-red-300 rounded-lg p-6 bg-red-50">
        <h2 className="text-lg font-semibold text-red-700 mb-2">⚠️ Опасная зона</h2>
        <p className="text-sm text-red-600 mb-4">
          Удаление аккаунта необратимо. Все ваши данные, организации и агенты будут удалены.
        </p>

        {error && (
          <div className="mb-4 p-3 bg-red-100 border border-red-400 rounded text-red-700 text-sm">
            {error}
          </div>
        )}

        {!isPending && (
          <>
            <button
              onClick={() => setShowModal(true)}
              className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 transition-colors text-sm font-medium"
            >
              Удалить аккаунт
            </button>
            <p className="text-xs text-red-500 mt-2">
              После запроса аккаунт будет удалён через 30 дней. Вы сможете отменить удаление в этот период.
            </p>
          </>
        )}

        {isPending && (
          <div className="space-y-3">
            <div className="p-3 bg-red-100 border border-red-300 rounded">
              <p className="text-sm text-red-700 font-medium">
                Аккаунт запланирован к удалению
              </p>
              <p className="text-sm text-red-600">
                Осталось дней: <strong>{remainingDays}</strong>
              </p>
            </div>

            <div className="flex gap-3">
              <button
                onClick={cancelDeletion}
                disabled={actionLoading}
                className="px-4 py-2 bg-gray-200 text-gray-700 rounded hover:bg-gray-300 transition-colors text-sm font-medium"
              >
                Отменить удаление
              </button>

              <button
                onClick={() => setShowHardModal(true)}
                disabled={actionLoading}
                className="px-4 py-2 bg-red-800 text-white rounded hover:bg-red-900 transition-colors text-sm font-medium"
              >
                Удалить немедленно
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Soft Delete Confirmation Modal */}
      {showModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-md mx-4 shadow-xl">
            <h3 className="text-lg font-bold mb-2">Удалить аккаунт?</h3>
            <p className="text-sm text-gray-600 mb-4">
              Ваш аккаунт будет удалён через 30 дней. В течение этого периода вы сможете отменить удаление и восстановить доступ.
            </p>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setShowModal(false)}
                className="px-4 py-2 bg-gray-200 text-gray-700 rounded hover:bg-gray-300 text-sm"
              >
                Отмена
              </button>
              <button
                onClick={requestDeletion}
                disabled={actionLoading}
                className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 text-sm"
              >
                {actionLoading ? "Удаление..." : "Удалить"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Hard Delete Confirmation Modal */}
      {showHardModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg p-6 max-w-md mx-4 shadow-xl">
            <h3 className="text-lg font-bold mb-2 text-red-700">Мгновенное удаление!</h3>
            <p className="text-sm text-gray-600 mb-4">
              Это действие <strong>необратимо</strong>. Все ваши данные будут удалены навсегда.
            </p>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setShowHardModal(false)}
                className="px-4 py-2 bg-gray-200 text-gray-700 rounded hover:bg-gray-300 text-sm"
              >
                Отмена
              </button>
              <button
                onClick={hardDelete}
                disabled={actionLoading}
                className="px-4 py-2 bg-red-800 text-white rounded hover:bg-red-900 text-sm"
              >
                {actionLoading ? "Удаление..." : "Удалить навсегда"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
