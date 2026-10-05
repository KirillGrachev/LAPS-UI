/**
 * Логика вкладки поиска: запрос пароля LAPS, сброс/продление срока,
 * копирование с автоочисткой буфера обмена.
 *
 * Таймер автоочистки буфера хранится в ref и корректно отменяется при
 * новом копировании, очистке результата и размонтировании.
 */
import { useCallback, useEffect, useRef } from 'react';
import { toast } from 'sonner';
import type { LapsInfo } from '../types';
import { tauriInvoke, toIpcError } from '../lib/ipc';
import { useTranslation } from '../i18n/index.tsx';
import { useConfig } from '../context/ConfigContext';
import { useSearchContext } from '../context/SearchContext';
import { useSession } from '../context/SessionContext';
import { useErrorHandler } from './useErrorHandler';
import { validateComputerQuery } from '../services/validationService';

export function useLapsSearch() {
  const {
    isSearching,
    setIsSearching,
    isRotating,
    setIsRotating,
    rotationKind,
    setRotationKind,
    result,
    setResult,
    setError,
    lastSearchQuery,
    setLastSearchQuery,
  } = useSearchContext();

  const { config } = useConfig();
  const { requireReauth } = useSession();
  const { t } = useTranslation();
  const { getMessage } = useErrorHandler();

  /** Сессия могла истечь (выход в другом окне): возвращаем ко входу. */
  const handleFailure = (err: unknown): string => {
    if (toIpcError(err).code === 'NOT_AUTHENTICATED') {
      requireReauth();
    }
    return getMessage(err);
  };

  const clipboardTimers = useRef<number[]>([]);

  const clearClipboardTimers = useCallback(() => {
    clipboardTimers.current.forEach((id) => window.clearTimeout(id));
    clipboardTimers.current = [];
  }, []);

  useEffect(() => clearClipboardTimers, [clearClipboardTimers]);

  const handleSearch = useCallback(
    async (rawQuery: string) => {
      const query = rawQuery.trim();
      setLastSearchQuery(query);

      const validation = validateComputerQuery(query);
      if (!validation.valid) {
        const msg = t(validation.errorKey ?? 'search.errors.pattern');
        setError(msg);
        toast.error(msg, { id: 'search-validation' });
        return;
      }

      setIsSearching(true);
      setError(null);
      try {
        const info = await tauriInvoke<LapsInfo>('search_computer', { query });
        setResult(info);
        toast.success(t('search.resultTitle') + ': ' + info.computerName, {
          id: 'search-result',
        });
      } catch (err) {
        setResult(null);
        const friendly = handleFailure(err);
        setError(friendly);
        toast.error(friendly, { id: 'search-error', duration: 8000 });
        console.error('search_computer failed:', toIpcError(err));
      } finally {
        setIsSearching(false);
      }
    },
    [handleFailure, setError, setIsSearching, setLastSearchQuery, setResult, t],
  );

  /** Сброс срока («сменить пароль сейчас») или запись конкретной даты. */
  const handleRotate = useCallback(
    async (mode: 'now' | 'at', localDateTime?: string) => {
      if (!result) return;
      if (mode === 'at' && !localDateTime) {
        toast.error(t('search.errors.empty'), { id: 'rotation-validation' });
        return;
      }

      setIsRotating(true);
      setRotationKind(mode);
      setError(null);
      try {
        const updated = await tauriInvoke<LapsInfo>('rotate_password', {
          dn: result.dn,
          mode,
          localDateTime: localDateTime ?? null,
        });
        setResult(updated);
        toast.success(mode === 'now' ? t('search.rotation.successNow') : t('search.rotation.successAt'), {
          id: 'rotation-result',
          duration: 6000,
        });
      } catch (err) {
        const friendly = handleFailure(err);
        setError(friendly);
        toast.error(friendly, { id: 'rotation-error', duration: 8000 });
      } finally {
        setIsRotating(false);
        setRotationKind(null);
      }
    },
    [handleFailure, result, setError, setIsRotating, setRotationKind, setResult, t],
  );

  /**
   * Копирование пароля + автоочистка буфера + событие аудита.
   *
   * Событие аудита пишет бэкенд в общий журнал. Автоочистка буфера
   * настраивается в настройках; перед очисткой буфер читается, чтобы
   * не затереть скопированное пользователем позже; нет фокуса или
   * разрешения — молча пропускаем (best effort).
   */
  const copyPassword = useCallback(async () => {
    if (!result?.password) return;
    const password = result.password;

    try {
      await navigator.clipboard.writeText(password);
    } catch {
      toast.error(t('errors.INTERNAL'), { id: 'copy-error' });
      return;
    }

    toast.success(t('search.copiedToast'), { id: 'copy-success' });

    tauriInvoke('note_password_copied', { computerName: result.computerName }).catch((err) => {
      console.warn('audit copy event failed:', err);
    });

    clearClipboardTimers();
    const seconds = config.clipboardClearSeconds;
    if (seconds > 0) {
      toast.info(t('search.clipboardClearToast', { seconds }), { id: 'copy-clear-info' });
      const timer = window.setTimeout(async () => {
        try {
          const current = await navigator.clipboard.readText();
          if (current === password) {
            await navigator.clipboard.writeText('');
            toast.info(t('search.clipboardClearedToast'), { id: 'copy-cleared' });
          }
        } catch {
        }
      }, seconds * 1000);
      clipboardTimers.current.push(timer);
    }
  }, [clearClipboardTimers, config.clipboardClearSeconds, result, t]);

  const clearResult = useCallback(() => {
    clearClipboardTimers();
    setResult(null);
    setError(null);
    setLastSearchQuery('');
    toast.dismiss('search-validation');
    toast.dismiss('search-error');
  }, [clearClipboardTimers, setError, setLastSearchQuery, setResult]);

  return {
    isSearching,
    isRotating,
    rotationKind,
    result,
    lastSearchQuery,
    setLastSearchQuery,
    handleSearch,
    handleRotate,
    copyPassword,
    clearResult,
  };
}
