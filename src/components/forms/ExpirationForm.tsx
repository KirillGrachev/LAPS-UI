import { useState } from 'react';
import { RefreshCcw, CalendarClock, Loader2, Info } from 'lucide-react';
import { Button } from '../ui/Button';
import { DateTimePicker } from '../ui/DateTimePicker';
import { ConfirmDialog } from '../ui/ConfirmDialog';
import type { LapsInfo } from '../../types';
import { useTranslation } from '../../i18n/index.tsx';

export interface ExpirationFormProps {
  result: LapsInfo | null;
  isRotating: boolean;
  rotationKind: 'now' | 'at' | null;
  onRotate: (mode: 'now' | 'at', localDateTime?: string) => void;
}

/** Значение по умолчанию: +7 дней, 12:00 (локальная строка YYYY-MM-DDTHH:mm). */
function defaultDateTime(): string {
  const d = new Date();
  d.setDate(d.getDate() + 7);
  d.setHours(12, 0, 0, 0);
  const pad = (n: number) => n.toString().padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/**
 * Управление сроком действия пароля LAPS:
 * * «Сменить пароль сейчас» — сброс срока (агент сгенерирует новый пароль
 *   при следующем применении политики), с диалогом подтверждения;
 * * «Изменить срок» — запись конкретной даты (продление).
 */
export function ExpirationForm({ result, isRotating, rotationKind, onRotate }: ExpirationFormProps) {
  const { t } = useTranslation();
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [dateTime, setDateTime] = useState(defaultDateTime);

  if (!result) return null;

  const disabled = isRotating || !result.rotationPossible;

  return (
    <div className="w-full bg-surface rounded-2xl p-5 flex flex-col gap-4">
      <div className="flex items-center gap-2 text-[0.72rem] font-black text-muted-foreground uppercase tracking-tight">
        <CalendarClock size={14} className="text-primary" />
        {t('search.rotation.title')}
      </div>

      <div className="flex flex-col sm:flex-row gap-3">
        <Button
          type="button"
          variant="outline"
          disabled={disabled}
          onClick={() => setConfirmOpen(true)}
          className="h-[42px] px-5 font-bold flex-1 gap-2.5 border-amber-500/30 text-amber-600 dark:text-amber-400 hover:bg-amber-500/10"
          title={t('search.rotation.resetHint')}
        >
          {rotationKind === 'now' ? (
            <Loader2 size={16} className="animate-spin" />
          ) : (
            <RefreshCcw size={16} />
          )}
          {t('search.rotation.resetNowBtn')}
        </Button>

        <div className="flex flex-1 gap-2 min-w-0 items-center">
          <DateTimePicker
            value={dateTime}
            onChange={setDateTime}
            disabled={disabled}
            ariaLabel={t('search.rotation.newExpirationLabel')}
            className="flex-1 min-w-0"
          />
          <Button
            type="button"
            variant="outline"
            disabled={disabled || !dateTime}
            onClick={() => onRotate('at', dateTime)}
            className="h-[42px] px-5 font-bold gap-2.5 shrink-0"
          >
            {rotationKind === 'at' ? (
              <Loader2 size={16} className="animate-spin" />
            ) : (
              <CalendarClock size={16} />
            )}
            {rotationKind === 'at' ? t('search.rotation.applyingBtn') : t('search.rotation.applyBtn')}
          </Button>
        </div>
      </div>

      {!result.rotationPossible && (
        <div className="flex items-start gap-2 text-[0.72rem] text-muted-foreground font-medium">
          <Info size={13} className="mt-0.5 shrink-0 opacity-60" />
          {t('search.rotation.notPossible')}
        </div>
      )}

      <ConfirmDialog
        open={confirmOpen}
        danger
        busy={isRotating && rotationKind === 'now'}
        title={t('search.rotation.resetNowConfirmTitle')}
        message={t('search.rotation.resetNowConfirmMsg', { computer: result.computerName })}
        confirmLabel={t('search.rotation.resetNowConfirmBtn')}
        cancelLabel={t('common.cancel')}
        onConfirm={() => {
          onRotate('now');
          setConfirmOpen(false);
        }}
        onCancel={() => setConfirmOpen(false)}
      />
    </div>
  );
}
