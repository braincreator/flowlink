import { useState } from 'react';
import { api } from '../api/client';
import { useApi } from '../hooks/useApi';

export default function Plans() {
  const { data: plans, loading, error, refresh } = useApi(() => api.getPlans());
  const { data: sub } = useApi(() => api.getSubscription());
  const [subscribing, setSubscribing] = useState<string | null>(null);

  const currentPlanId = (sub as any)?.plan_id || 'free';

  const handleSubscribe = async (planId: string) => {
    setSubscribing(planId);
    try {
      const phone = prompt('Введите номер телефона для оплаты через СБП:');
      if (!phone) { setSubscribing(null); return; }
      const result = await api.subscribe(planId, { Sbp: { phone } });
      if ((result as any)?.payment_url) {
        window.open((result as any).payment_url, '_blank');
      }
      alert('Подписка оформлена! Проверьте СБП для подтверждения оплаты.');
      refresh();
    } catch (e: any) {
      alert(`Ошибка: ${e.message}`);
    } finally {
      setSubscribing(null);
    }
  };

  const handleCancel = async () => {
    if (!confirm('Отменить подписку?')) return;
    try {
      await api.cancelSubscription();
      alert('Подписка отменена');
      refresh();
    } catch (e: any) { alert(`Ошибка: ${e.message}`); }
  };

  const handlePause = async () => {
    try {
      await api.pauseSubscription();
      refresh();
    } catch (e: any) { alert(`Ошибка: ${e.message}`); }
  };

  const handleResume = async () => {
    try {
      await api.resumeSubscription();
      refresh();
    } catch (e: any) { alert(`Ошибка: ${e.message}`); }
  };

  if (loading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-tg-button border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-tg-hint mt-3">Загрузка тарифов...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <span className="text-3xl block mb-3">⚠️</span>
        <p className="text-sm text-tg-danger mb-1">{error}</p>
        <button onClick={refresh} className="mt-2 px-4 py-2 rounded-xl bg-tg-button text-tg-button-text text-sm font-medium">
          Повторить
        </button>
      </div>
    );
  }

  const plansList = (plans || []) as any[];

  return (
    <div className="px-4 pt-4">
      <h1 className="text-2xl font-semibold mb-2">Тарифы</h1>
      <p className="text-sm text-tg-hint mb-6">Выберите подходящий план</p>

      <div className="space-y-4">
        {plansList.map((plan: any) => {
          const isCurrent = plan.id === currentPlanId;
          return (
            <div
              key={plan.id}
              className={`p-4 rounded-xl border ${isCurrent ? 'border-tg-button bg-tg-button/10' : 'border-tg-button/20 bg-tg-hint/10'}`}
            >
              <div className="flex items-center justify-between mb-2">
                <h3 className="font-semibold">{plan.name || plan.id}</h3>
                {isCurrent && (
                  <span className="px-2 py-1 rounded-full bg-tg-button text-tg-button-text text-xs font-medium">Текущий</span>
                )}
              </div>

              <p className="text-2xl font-bold mb-2">
                {plan.price ? `${Math.round(plan.price / 100)} ₽/мес` : 'Бесплатно'}
              </p>

              {plan.features && plan.features.length > 0 && (
                <ul className="space-y-1 mb-4">
                  {plan.features.map((f: string, i: number) => (
                    <li key={i} className="text-sm text-tg-hint flex items-center gap-2">
                      <span>✓</span> {f}
                    </li>
                  ))}
                </ul>
              )}

              {!isCurrent && (
                <button
                  onClick={() => handleSubscribe(plan.id)}
                  disabled={subscribing === plan.id}
                  className="w-full py-3 rounded-xl bg-tg-button text-tg-button-text font-medium disabled:opacity-60"
                >
                  {subscribing === plan.id ? 'Оформление...' : 'Выбрать'}
                </button>
              )}
            </div>
          );
        })}

        {sub && (sub as any)?.status === 'active' && (
          <div className="p-4 rounded-xl border border-tg-button bg-tg-button/10 mt-4">
            <h3 className="font-semibold mb-3">Управление подпиской</h3>
            <div className="flex gap-2">
              <button onClick={handlePause} className="flex-1 py-2 rounded-lg bg-tg-hint/20 text-sm font-medium">⏸ Пауза</button>
              <button onClick={handleCancel} className="flex-1 py-2 rounded-lg bg-tg-danger/20 text-tg-danger text-sm font-medium">✕ Отменить</button>
            </div>
          </div>
        )}
        {sub && (sub as any)?.status === 'paused' && (
          <div className="p-4 rounded-xl border border-tg-button bg-tg-button/10 mt-4">
            <h3 className="font-semibold mb-3">Подписка на паузе</h3>
            <button onClick={handleResume} className="w-full py-2 rounded-lg bg-tg-button text-tg-button-text text-sm font-medium">▶ Возобновить</button>
          </div>
        )}
        {plansList.length === 0 && (
          <div className="text-center py-12">
            <span className="text-4xl">📋</span>
            <p className="text-sm text-tg-hint mt-2">Тарифы пока не настроены</p>
          </div>
        )}
      </div>
    </div>
  );
}
