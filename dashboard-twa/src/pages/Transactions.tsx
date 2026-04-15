import { useEffect, useState } from 'react';
import { api } from '../api/client';
import StatCard from '../components/StatCard';

interface Transaction {
  id: string;
  type: 'payment' | 'subscription' | 'refund';
  amount: string;
  plan?: string;
  status: 'completed' | 'pending' | 'failed';
  description: string;
  created_at: string;
}

export default function Transactions() {
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);

  const loadTransactions = async (pageNum: number = 1) => {
    setLoading(true);
    setError('');
    try {
      const data = await api.getTransactions(20);
      const txs: Transaction[] = (data || []).map((t: any) => ({
        id: t.id || t.transaction_id || Math.random().toString(),
        type: t.type || 'payment',
        amount: t.amount || t.amount_kopecks || '0',
        plan: t.plan_name || t.plan || undefined,
        status: t.status || 'completed',
        description: t.description || t.description || `${t.type} for ${t.plan_name || 'service'}`,
        created_at: t.created_at || t.timestamp || new Date().toISOString(),
      }));

      setTransactions(pageNum === 1 ? txs : [...transactions, ...txs]);
      setHasMore(txs.length === 20);
      setPage(pageNum);
    } catch (e: any) {
      setError(e.message || 'Failed to load transactions');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadTransactions(1);
  }, []);

  const loadMore = () => {
    if (hasMore && !loading) {
      loadTransactions(page + 1);
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'completed': return 'text-tg-success';
      case 'pending': return 'text-tg-hint';
      case 'failed': return 'text-tg-danger';
      default: return 'text-tg-button-text';
    }
  };

  const getTypeIcon = (type: string) => {
    switch (type) {
      case 'payment': return '💳';
      case 'subscription': return '📦';
      case 'refund': return '↩️';
      default: return '💰';
    }
  };

  const totalSpent = transactions.reduce((sum, tx) => {
    if (tx.status === 'completed' && tx.type === 'payment') {
      const amount = parseInt(tx.amount) || 0;
      return sum + amount;
    }
    return sum;
  }, 0);

  if (loading && transactions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <div className="w-8 h-8 border-2 border-tg-button border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-tg-hint mt-3">Loading transactions...</p>
      </div>
    );
  }

  return (
    <div className="px-4 pt-4">
      {/* Header */}
      <div className="mb-6">
        <h1 className="text-2xl font-semibold mb-2">Transactions</h1>
        {totalSpent > 0 && (
          <div className="mt-2 p-3 bg-tg-hint/10 rounded-lg">
            <span className="text-sm text-tg-hint">Total Spent: </span>
            <span className="text-xl font-semibold text-tg-button-text">
              {new Intl.NumberFormat('ru-RU', { style: 'currency', currency: 'RUB' }).format(totalSpent / 100)}
            </span>
          </div>
        )}
      </div>

      {error && (
        <div className="mb-4 p-3 rounded-lg bg-tg-danger/20 text-tg-danger">
          {error}
        </div>
      )}

      {/* Transactions List */}
      <div className="space-y-3">
        {transactions.length === 0 ? (
          <div className="text-center py-12">
            <span className="text-4xl">📭</span>
            <p className="text-sm text-tg-hint mt-2">No transactions yet</p>
          </div>
        ) : (
          transactions.map((tx) => (
            <div key={tx.id} className="bg-tg-hint/10 p-4 rounded-lg border border-tg-button/20">
              <div className="flex items-start justify-between mb-2">
                <div className="flex items-center gap-3">
                  <span className="text-2xl">{getTypeIcon(tx.type)}</span>
                  <div>
                    <p className="font-semibold">{tx.description}</p>
                    {tx.plan && (
                      <span className="text-sm text-tg-hint ml-2">({tx.plan})</span>
                    )}
                  </div>
                </div>
                <span className={`text-sm font-medium ${getStatusColor(tx.status)}`}>
                  {tx.status.charAt(0).toUpperCase() + tx.status.slice(1)}
                </span>
              </div>
              <div className="flex items-center justify-between text-sm text-tg-hint">
                <span>{new Date(tx.created_at).toLocaleString('ru-RU')}</span>
                <span className="font-semibold text-tg-button-text">
                  {new Intl.NumberFormat('ru-RU', { style: 'currency', currency: 'RUB' }).format(parseInt(tx.amount) / 100)}
                </span>
              </div>
            </div>
          ))
        )}
      </div>

      {/* Load More */}
      {hasMore && !loading && (
        <button
          onClick={loadMore}
          className="w-full mt-4 px-4 py-3 rounded-xl bg-tg-button text-tg-button-text font-medium"
        >
          Load More Transactions
        </button>
      )}
    </div>
  );
}
