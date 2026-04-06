import { createContext, useContext, type ReactNode } from 'react';
import { api } from '../api/client';

interface AuthContext {
  token: string | null;
  login: (token: string) => void;
  logout: () => void;
}

const AuthCtx = createContext<AuthContext>({
  token: api.getToken(),
  login: (t: string) => api.setToken(t),
  logout: () => api.setToken(null),
});

export function AuthProvider({ children }: { children: ReactNode }) {
  return (
    <AuthCtx.Provider value={{
      token: api.getToken(),
      login: (t: string) => api.setToken(t),
      logout: () => api.setToken(null),
    }}>
      {children}
    </AuthCtx.Provider>
  );
}

export const useAuth = () => useContext(AuthCtx);
