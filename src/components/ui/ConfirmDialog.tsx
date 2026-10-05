import { ReactNode } from 'react';
import { X, AlertTriangle } from 'lucide-react';
import { motion, AnimatePresence } from 'motion/react';
import { Button } from './Button';

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  message: ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  /** Опасное действие — красная кнопка подтверждения. */
  danger?: boolean;
  busy?: boolean;
}

/**
 * Лёгкий диалог подтверждения (hand-rolled, как в AboutTab первой версии):
 * Radix Dialog не нужен — сценарий простой, анимации — motion.
 */
export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
  danger = false,
  busy = false,
}: ConfirmDialogProps) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[150] flex items-center justify-center bg-black/60 backdrop-blur-sm p-[20px]"
          onClick={busy ? undefined : onCancel}
        >
          <motion.div
            initial={{ opacity: 0, scale: 0.92, y: 16 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 12 }}
            transition={{ type: 'spring', damping: 28, stiffness: 320 }}
            className="bg-surface w-full max-w-md rounded-2xl p-7 shadow-2xl flex flex-col gap-5"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-start justify-between gap-4">
              <div className="flex items-center gap-3 min-w-0">
                {danger && (
                  <div className="w-9 h-9 rounded-lg bg-red-500/10 border border-red-500/20 flex items-center justify-center shrink-0">
                    <AlertTriangle size={16} className="text-red-500" />
                  </div>
                )}
                <h3 className="text-[1.05rem] font-bold text-foreground tracking-tight">
                  {title}
                </h3>
              </div>
              <button
                onClick={onCancel}
                disabled={busy}
                className="p-1.5 rounded-lg hover:bg-surface-hover text-muted-foreground transition-colors disabled:opacity-40 shrink-0"
                aria-label={cancelLabel}
              >
                <X size={16} />
              </button>
            </div>

            <div className="text-[0.85rem] leading-relaxed text-muted-foreground font-medium">
              {message}
            </div>

            <div className="flex justify-end gap-3 mt-1">
              <Button variant="outline" onClick={onCancel} disabled={busy} className="px-5 py-2 text-[0.8rem]">
                {cancelLabel}
              </Button>
              <Button
                variant={danger ? 'danger' : 'primary'}
                onClick={onConfirm}
                disabled={busy}
                className={`px-5 py-2 text-[0.8rem] ${
                  danger
                    ? 'bg-red-600 hover:bg-red-700 border-transparent text-white enabled:hover:bg-red-700'
                    : ''
                }`}
              >
                {confirmLabel}
              </Button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
