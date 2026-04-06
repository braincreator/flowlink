import { useState, useCallback } from 'react';
let toastId = 0;
export function useToast() {
    const [toasts, setToasts] = useState([]);
    const addToast = useCallback((toast) => {
        const id = `toast-${++toastId}`;
        const t = { ...toast, id };
        setToasts(prev => [...prev, t]);
        setTimeout(() => {
            setToasts(prev => prev.filter(x => x.id !== id));
        }, 4000);
    }, []);
    const removeToast = useCallback((id) => {
        setToasts(prev => prev.filter(x => x.id !== id));
    }, []);
    return { toasts, addToast, removeToast };
}
