import { useTranslation } from '../i18n/index.tsx';
import { toIpcError } from '../lib/ipc';

/**
 * Человекочитаемое сообщение об ошибке бэкенда.
 *
 * Стратегия: бэкенд отдаёт `{ code, message }`, где `message` — подробный
 * **русский** текст (продукт RU-first) с контекстом (имена, адреса, коды AD).
 * * В русской локали показываем `message` как есть — он богаче общего
 *   перевода кода;
 * * В английской локали берём перевод `errors.<CODE>`, а `message`
 *   используется как fallback для неизвестных кодов.
 * К переводу кода добавляются технические подробности, если бэкенд их передал.
 */
export function useErrorHandler() {
  const { t, language } = useTranslation();

  const getMessage = (error: unknown): string => {
    const ipcError = toIpcError(error);
    if (language !== 'ru') {
      const key = `errors.${ipcError.code}`;
      const translated = t(key);
      if (translated !== key) {
        return ipcError.details && ipcError.details !== ipcError.message
          ? `${translated} (${ipcError.details})`
          : translated;
      }
    }
    return ipcError.message || t('errors.UNKNOWN');
  };

  return { getMessage };
}
