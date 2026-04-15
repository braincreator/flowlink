import { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { api } from '../api/client';

interface AuthContextType {
  token: string | null;
  user: { account_id: string; email: string } | null;
  loading: boolean;
  login: (email: string, code: string) => Promise<void>;
  logout: () => void;
  sendCode: (email: string) => Promise<void>;
}

const AuthContext = createContext<AuthContextType>({
  token: null,
  user: null,
  loading: true,
  login: async () => {},
  logout: () => {},
  sendCode: async () => {},
});

export function AuthProvider({ children }: { children: ReactNode }) {
  const [token, setToken] = useState<string | null>(api.getToken());
  const [user, setUser] = useState<{ account_id: string; email: string } | null>(null);
  const [loading, setLoading] = useState(true);

  // Check stored token validity on mount
  useEffect(() => {
    const stored = localStorage.getItem('flowlink_token');
    if (stored) {
      api.setToken(stored);
      // Try to fetch account info to validate token
      api.getAccountInfo()
        .then((info) => {
          setUser({ account_id: info.user?.id || '', email: info.user?.email || '' });
          setLoading(false);
        })
        .catch(() => {
          // Token invalid
          api.setToken(null);
          setToken(null);
          setUser(null);
          localStorage.removeItem('flowlink_user');
          setLoading(false);
        });
    } else {
      setLoading(false);
    }
  }, []);

  const sendCode = async (email: string) => {
    const res = await api.sendEmailCode(email);
    if (!res.ok) throw new Error(res.error || 'Failed to send code');
  };

  const login = async (email: string, code: string) => {
    const res = await api.verifyEmailCode(email, code);
    api.setToken(res.token);
    setToken(res.token);
    setUser(res.user);
    localStorage.setItem('flowlink_user', JSON.stringify(res.user));
  };

  const logout = () => {
    api.setToken(null);
    setToken(null);
    setUser(null);
    localStorage.removeItem('flowlink_token');
    localStorage.removeItem('flowlink_user');
  };

  return (
    <AuthContext.Provider value={{ token, user, loading, login, logout, sendCode }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  return useContext(AuthContext);
}
