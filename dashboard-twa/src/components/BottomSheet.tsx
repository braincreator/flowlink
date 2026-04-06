import { useEffect, useRef } from 'react';
import { useHaptic } from '../hooks/useHaptic';

interface Props {
  open: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
}

export default function BottomSheet({ open, onClose, title, children }: Props) {
  const { light } = useHaptic();
  const sheetRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }
    return () => { document.body.style.overflow = ''; };
  }, [open]);

  if (!open) return null;

  return (
    <>
      <div className="bottom-sheet-overlay" onClick={() => { light(); onClose(); }} />
      <div ref={sheetRef} className="bottom-sheet">
        <div className="bottom-sheet-handle" />
        {title && (
          <div className="px-5 pb-3 flex items-center justify-between">
            <h2 className="font-semibold text-base">{title}</h2>
            <button onClick={() => { light(); onClose(); }} className="w-8 h-8 flex items-center justify-center rounded-full bg-tg-surface text-tg-hint">✕</button>
          </div>
        )}
        <div className="px-5 pb-6">{children}</div>
      </div>
    </>
  );
}
