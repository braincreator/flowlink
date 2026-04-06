import { jsx as _jsx } from "react/jsx-runtime";
import { createContext, useContext } from 'react';
import { api } from '../api/client';
const AuthCtx = createContext({
    token: api.getToken(),
    login: (t) => api.setToken(t),
    logout: () => api.setToken(null),
});
export function AuthProvider({ children }) {
    return (_jsx(AuthCtx.Provider, { value: {
            token: api.getToken(),
            login: (t) => api.setToken(t),
            logout: () => api.setToken(null),
        }, children: children }));
}
export const useAuth = () => useContext(AuthCtx);
