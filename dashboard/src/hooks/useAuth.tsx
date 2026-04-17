import { createContext, useContext, type ReactNode } from 'react';
import { api, isAdmin as checkIsAdmin } from '../api/client';

interface AuthContext {
  token: string | null;
  isAdmin: boolean;
  login: (token: string) => void;
  logout: () => void;
}

const AuthCtx = createContext<AuthContext>({
  token: api.getToken(),
  isAdmin: checkIsAdmin(),
  login: (t: string) => api.setToken(t),
  logout: () => api.clearTokens(),
});

export function AuthProvider({ children }: { children: ReactNode }) {
  return (
    <AuthCtx.Provider value={{
      token: api.getToken(),
      isAdmin: checkIsAdmin(),
      login: (t: string) => api.setToken(t),
      logout: () => api.clearTokens(),
    }}>
      {children}
    </AuthCtx.Provider>
  );
}

export const useAuth = () => useContext(AuthCtx);
