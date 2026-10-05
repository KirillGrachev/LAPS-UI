/**
 * Форматирование дат и статусов срока действия пароля LAPS.
 *
 * Бэкенд отдаёт время в RFC 3339 с локальным смещением
 * (`2026-10-07T12:00:00+03:00`) — `parseISO` из date-fns читает его как есть.
 */
import { differenceInCalendarDays, format, isValid, parseISO } from 'date-fns';
import { enUS, ru } from 'date-fns/locale';
import type { Language } from '../i18n';

export function dateLocale(language: Language) {
  return language === 'ru' ? ru : enUS;
}

/** Абсолютная дата/время: `07.10.2026 12:00`. Некорректный ввод → as-is. */
export function formatDateTime(value: string | null | undefined, language: Language): string {
  if (!value) return '';
  try {
    const date = parseISO(value);
    if (!isValid(date)) return value;
    return format(date, 'dd.MM.yyyy HH:mm', { locale: dateLocale(language) });
  } catch {
    return value;
  }
}

/** Дата/время для журнала: `05.10.2026 14:03:02` — привычный деловой вид. */
export function formatLogTime(value: string, language: Language): string {
  try {
    const date = parseISO(value);
    if (!isValid(date)) return value;
    return format(date, 'dd.MM.yyyy HH:mm:ss', { locale: dateLocale(language) });
  } catch {
    return value;
  }
}

/** Статус срока действия пароля для карточки результата. */
export type ExpirationStatus =
  | { kind: 'notSet' }
  | { kind: 'expired'; daysAgo: number }
  | { kind: 'today' }
  | { kind: 'soon'; days: number }
  | { kind: 'future'; days: number };

export function expirationStatus(expiresAt: string | null | undefined, now = new Date()): ExpirationStatus {
  if (!expiresAt) return { kind: 'notSet' };
  try {
    const date = parseISO(expiresAt);
    if (!isValid(date)) return { kind: 'notSet' };
    if (date.getTime() <= now.getTime()) {
      return { kind: 'expired', daysAgo: Math.max(1, -differenceInCalendarDays(date, now)) };
    }
    const days = differenceInCalendarDays(date, now);
    if (days <= 0) return { kind: 'today' };
    if (days <= 3) return { kind: 'soon', days };
    return { kind: 'future', days };
  } catch {
    return { kind: 'notSet' };
  }
}
