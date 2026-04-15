import { useState, useEffect } from 'react';
import { api } from '../api/client';

export default function Settings() {
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');
  const [messageType, setMessageType] = useState<'success' | 'error'>('success');

  const loadSettings = async () => {
    try {
      const data = await api.getAccountSettings();
      setName(data.name || '');
      setEmail(data.email || '');
    } catch (e: any) {
      setMessage(e.message || 'Failed to load settings');
      setMessageType('error');
    }
  };

  const saveSettings = async () => {
    if (!name) {
      setMessage('Name is required');
      setMessageType('error');
      return;
    }

    setLoading(true);
    try {
      await api.updateAccountSettings({ name });
      setMessage('Settings saved');
      setMessageType('success');
    } catch (e: any) {
      setMessage(e.message || 'Failed to save settings');
      setMessageType('error');
    } finally {
      setLoading(false);
    }
  };

  // Load settings on mount
  useEffect(() => {
    loadSettings();
  }, []);

  return (
    <div className="px-4 pt-4">
      <h1 className="text-2xl font-semibold mb-6">Account Settings</h1>

      {message && (
        <div className={`mb-4 p-3 rounded-lg ${messageType === 'success' ? 'bg-tg-success/20 text-tg-success' : 'bg-tg-danger/20 text-tg-danger'}`}>
          {message}
        </div>
      )}

      <div className="space-y-4">
        {/* Name */}
        <div>
          <label className="block text-sm font-medium mb-2">Display Name</label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="w-full px-4 py-3 rounded-lg bg-tg-hint/10 border-tg-button/30 text-tg-button-text placeholder:text-tg-hint/60 focus:outline-none focus:ring-2 focus:ring-tg-button"
            placeholder="Enter your display name"
          />
        </div>

        {/* Email (read-only) */}
        <div>
          <label className="block text-sm font-medium mb-2">Email</label>
          <input
            type="email"
            value={email}
            readOnly
            className="w-full px-4 py-3 rounded-lg bg-tg-hint/10 border-tg-button/30 text-tg-button-text opacity-60 cursor-not-allowed"
          />
          <p className="text-xs text-tg-hint mt-1">Email cannot be changed. Contact support to update.</p>
        </div>

        {/* Save Button */}
        <button
          onClick={saveSettings}
          disabled={loading}
          className="w-full px-4 py-3 rounded-xl bg-tg-button text-tg-button-text font-medium disabled:opacity-60"
        >
          {loading ? 'Saving...' : 'Save Settings'}
        </button>
      </div>
    </div>
  );
}
