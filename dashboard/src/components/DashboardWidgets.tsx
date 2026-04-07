import { useState, useEffect, useCallback, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { GripVertical } from 'lucide-react';

export interface WidgetDef {
  id: string;
  title: string;
  render: () => ReactNode;
  defaultOrder: number;
  colSpan?: 1 | 2;
}

interface DashboardWidgetsProps {
  widgets: WidgetDef[];
}

const STORAGE_KEY = 'flowlink_widget_layout';

function getStoredOrder(): string[] | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch { return null; }
}

function saveOrder(order: string[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(order));
}

export function DashboardWidgets({ widgets }: DashboardWidgetsProps) {
  const { t } = useTranslation();
  const [customizing, setCustomizing] = useState(false);
  const [order, setOrder] = useState<string[]>(() => {
    const stored = getStoredOrder();
    if (stored) {
      // Merge with any new widgets not in stored order
      const ids = widgets.map(w => w.id);
      const merged = [...stored];
      ids.forEach(id => { if (!merged.includes(id)) merged.push(id); });
      return merged.filter(id => ids.includes(id));
    }
    return widgets.map(w => w.id);
  });

  const [dragId, setDragId] = useState<string | null>(null);
  const [overId, setOverId] = useState<string | null>(null);

  const widgetMap = Object.fromEntries(widgets.map(w => [w.id, w]));

  const resetLayout = useCallback(() => {
    const defaultOrder = widgets.map(w => w.id);
    setOrder(defaultOrder);
    localStorage.removeItem(STORAGE_KEY);
  }, [widgets]);

  useEffect(() => {
    if (!customizing) return;
    const handler = () => {
      setDragId(null);
      setOverId(null);
      saveOrder(order);
    };
    window.addEventListener('dragend', handler);
    return () => window.removeEventListener('dragend', handler);
  }, [customizing, order]);

  const handleDragStart = (e: React.DragEvent, id: string) => {
    setDragId(id);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', id);
  };

  const handleDragOver = (e: React.DragEvent, id: string) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    if (dragId && dragId !== id) setOverId(id);
  };

  const handleDrop = (e: React.DragEvent, targetId: string) => {
    e.preventDefault();
    if (!dragId || dragId === targetId) return;
    const newOrder = [...order];
    const fromIdx = newOrder.indexOf(dragId);
    const toIdx = newOrder.indexOf(targetId);
    newOrder.splice(fromIdx, 1);
    newOrder.splice(toIdx, 0, dragId);
    setOrder(newOrder);
    setDragId(null);
    setOverId(null);
    saveOrder(newOrder);
  };

  return (
    <div>
      <div className="flex items-center gap-3 mb-4">
        <button
          onClick={() => { if (customizing) saveOrder(order); setCustomizing(!customizing); }}
          className={`flex items-center gap-2 rounded-xl px-4 py-2 text-sm font-medium transition-all ${
            customizing
              ? 'bg-[var(--color-accent)] text-white'
              : 'border border-[var(--color-border)] hover:bg-[var(--color-surface2)]'
          }`}
        >
          <GripVertical size={16} />
          {customizing ? t('common.done') : t('dashboard.customize')}
        </button>
        {customizing && (
          <button onClick={resetLayout} className="text-xs text-[var(--color-dim)] hover:text-[var(--color-text)] transition-colors">
            {t('dashboard.reset_layout')}
          </button>
        )}
      </div>

      <div
        className="grid gap-4"
        style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))' }}
      >
        {order.map(id => {
          const widget = widgetMap[id];
          if (!widget) return null;
          const isDragging = dragId === id;
          const isOver = overId === id;
          return (
            <div
              key={id}
              draggable={customizing}
              onDragStart={e => handleDragStart(e, id)}
              onDragOver={e => handleDragOver(e, id)}
              onDrop={e => handleDrop(e, id)}
              onDragEnd={() => { setDragId(null); setOverId(null); }}
              className={`rounded-xl border transition-all duration-200 ${
                widget.colSpan === 2 ? 'col-span-2' : ''
              } ${
                customizing
                  ? `cursor-grab active:cursor-grabbing ${
                      isDragging ? 'opacity-40 scale-95' : isOver ? 'border-[var(--color-accent)] ring-2 ring-[var(--color-accent)]/30' : 'border-[var(--color-border)] bg-[var(--color-surface)]'
                    }`
                  : 'border-[var(--color-border)] bg-[var(--color-surface)]'
              }`}
              style={widget.colSpan === 2 ? { gridColumn: 'span 2' } : undefined}
            >
              {customizing && (
                <div className="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-2.5">
                  <GripVertical size={14} className="text-[var(--color-dim)]" />
                  <span className="text-xs font-medium text-[var(--color-dim)]">{widget.title}</span>
                </div>
              )}
              <div className={customizing ? 'opacity-70 pointer-events-none p-5' : ''}>
                {widget.render()}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
