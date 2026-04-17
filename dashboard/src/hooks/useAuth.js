import { jsx as _jsx } from "react/jsx-runtime";
import { createContext, useContext } from 'react';
import { api, isAdmin as checkIsAdmin } from '../api/client';
const AuthCtx = createContext({
    token: api.getToken(),
    isAdmin: checkIsAdmin(),
    login: (t) => api.setToken(t),
    logout: () => api.clearTokens(),
});
export function AuthProvider({ children }) {
    return (_jsx(AuthCtx.Provider, { value: {
            token: api.getToken(),
            isAdmin: checkIsAdmin(),
            login: (t) => api.setToken(t),
            logout: () => api.clearTokens(),
        }, children: children }));
}
export const useAuth = () => useContext(AuthCtx);
