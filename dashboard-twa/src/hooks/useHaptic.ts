import { useCallback } from 'react';
import { haptic } from '../tg';

export function useHaptic() {
  const light = useCallback(() => haptic('light'), []);
  const medium = useCallback(() => haptic('medium'), []);
  const heavy = useCallback(() => haptic('heavy'), []);
  const success = useCallback(() => haptic('success'), []);
  const error = useCallback(() => haptic('error'), []);
  const warning = useCallback(() => haptic('warning'), []);
  return { light, medium, heavy, success, error, warning };
}
