import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronDown } from 'lucide-react';
import { usePopoverPosition } from './usePopoverPosition';

export interface SelectOption<T extends string | number> {
  value: T;
  label: string;
}

interface SelectProps<T extends string | number> {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  id?: string;
  ariaLabel?: string;
  className?: string;
  disabled?: boolean;
}

const ROW_HEIGHT = 42;
const MAX_ROWS = 5;

/**
 * Кастомный выпадающий список в стиле приложения.
 *
 * Нативный `<select>` в WebView2 рисует ОС-попап, чуждый интерфейсу
 * продукта; здесь список — popover с токенами темы. Рендерится через
 * portal с `position: fixed`, шириной по якорю, клампом по краям окна
 * и флипом вверх — не растягивает скролл и не выходит за форму приложения.
 * Шеврон отстоит от края поля на 12px, значение и пункты центрированы.
 */
export function Select<T extends string | number>({
  value,
  options,
  onChange,
  id,
  ariaLabel,
  className = '',
  disabled,
}: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const popRef = useRef<HTMLDivElement>(null);

  const panelHeight = Math.min(options.length, MAX_ROWS) * ROW_HEIGHT + 12;
  const position = usePopoverPosition(rootRef, open, 0, panelHeight);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (rootRef.current?.contains(target) || popRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [open]);

  const selected = options.find((option) => option.value === value);

  return (
    <div ref={rootRef} className={`relative ${className}`}>
      <button
        type="button"
        id={id}
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
        className="relative w-full h-[42px] rounded-lg border-none bg-[#e6ebf2] dark:bg-white/[0.08] pl-4 pr-10 text-center text-[0.82rem] font-black text-foreground transition-colors hover:bg-[#dde4ed] dark:hover:bg-white/[0.13] focus:outline-none disabled:opacity-60 disabled:cursor-not-allowed"
      >
        <span className="block truncate">{selected?.label ?? '—'}</span>
        <ChevronDown
          size={15}
          className={`absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground opacity-70 pointer-events-none transition-transform ${
            open ? 'rotate-180' : ''
          }`}
        />
      </button>

      {open &&
        position &&
        createPortal(
          <div
            ref={popRef}
            style={{
              position: position.position,
              left: position.left,
              top: position.top,
              width: position.width,
            }}
            role="listbox"
            className="z-[95] rounded-lg bg-surface shadow-2xl py-1.5 max-h-60 overflow-y-auto scrollbar-hide"
          >
            {options.map((option) => {
              const isSelected = option.value === value;
              return (
                <button
                  key={String(option.value)}
                  type="button"
                  role="option"
                  aria-selected={isSelected}
                  onClick={() => {
                    onChange(option.value);
                    setOpen(false);
                  }}
                  className={`w-full px-4 py-2.5 text-center text-[0.8rem] font-bold transition-colors flex items-center justify-center gap-2 ${
                    isSelected
                      ? 'text-primary bg-primary/5'
                      : 'text-foreground hover:bg-surface-hover'
                  }`}
                >
                  {isSelected && <Check size={13} className="shrink-0" />}
                  <span className="truncate">{option.label}</span>
                </button>
              );
            })}
          </div>,
          document.body,
        )}
    </div>
  );
}
