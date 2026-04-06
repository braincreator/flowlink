import { useRef, useCallback } from 'react';

interface SwipeHandlers {
  onSwipeLeft?: () => void;
  onSwipeRight?: () => void;
  threshold?: number;
}

export function useSwipe({ onSwipeLeft, onSwipeRight, threshold = 80 }: SwipeHandlers) {
  const startX = useRef(0);
  const currentX = useRef(0);
  const dragging = useRef(false);

  const onTouchStart = useCallback((e: React.TouchEvent) => {
    startX.current = e.touches[0].clientX;
    currentX.current = startX.current;
    dragging.current = true;
  }, []);

  const onTouchMove = useCallback((e: React.TouchEvent) => {
    if (!dragging.current) return;
    currentX.current = e.touches[0].clientX;
    const diff = currentX.current - startX.current;
    if (Math.abs(diff) > 10) {
      (e.currentTarget as HTMLElement).style.transform = `translateX(${diff}px)`;
      (e.currentTarget as HTMLElement).style.transition = 'none';
    }
  }, []);

  const onTouchEnd = useCallback((e: React.TouchEvent) => {
    if (!dragging.current) return;
    dragging.current = false;
    const el = e.currentTarget as HTMLElement;
    el.style.transition = 'transform 200ms ease';
    el.style.transform = '';
    const diff = currentX.current - startX.current;
    if (diff > threshold) onSwipeRight?.();
    else if (diff < -threshold) onSwipeLeft?.();
    startX.current = 0;
    currentX.current = 0;
  }, [onSwipeLeft, onSwipeRight, threshold]);

  return { onTouchStart, onTouchMove, onTouchEnd };
}
