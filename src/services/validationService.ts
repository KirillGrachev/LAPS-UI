/**
 * Клиентская валидация поискового запроса — зеркало серверных правил
 * (`services/laps/query.rs`). Сервер валидирует повторно и строже:
 * клиентская проверка нужна только для мгновенного отклика UI.
 */
import type { TranslationKeys } from '../i18n/index.tsx';

interface ValidationResult {
  valid: boolean;
  errorKey?: TranslationKeys;
}

const NETBIOS_RE = /^[A-Za-z0-9-_]{1,15}\$?$/;
const FQDN_LABEL_RE = /^[A-Za-z0-9-]{1,63}$/;
const IPV4_RE = /^(\d{1,3})(\.\d{1,3}){3}$/;
const IPV6_RE = /^[0-9A-Fa-f:]+$/;

/**
 * FQDN: метки из букв/цифр/дефисов; четыре полностью числовых лейбла —
 * вырожденный IPv4 (в т.ч. невалидный, вроде 10.0.0.256): такие значения
 * проверяются как IP, а не как имя.
 */
function isValidFqdn(value: string): boolean {
  const normalized = value.replace(/\.$/, '');
  const labels = normalized.split('.');
  if (labels.length < 2) return false;
  if (labels.length === 4 && labels.every((label) => /^\d+$/.test(label))) return false;
  return labels.every(
    (label) => FQDN_LABEL_RE.test(label) && !label.startsWith('-') && !label.endsWith('-'),
  );
}

function isValidIp(value: string): boolean {
  if (IPV4_RE.test(value)) {
    return value.split('.').every((part) => Number(part) <= 255);
  }
  return value.includes(':') && IPV6_RE.test(value);
}

export function validateComputerQuery(raw: string): ValidationResult {
  const query = raw.trim();
  if (!query) {
    return { valid: false, errorKey: 'search.errors.empty' as TranslationKeys };
  }
  if (query.length > 255) {
    return { valid: false, errorKey: 'search.errors.max' as TranslationKeys };
  }
  if (NETBIOS_RE.test(query) || isValidFqdn(query) || isValidIp(query)) {
    return { valid: true };
  }
  return { valid: false, errorKey: 'search.errors.pattern' as TranslationKeys };
}

/** Локальная валидация значения datetime-local перед отправкой. */
export function validateLocalDateTime(value: string): ValidationResult {
  if (!value) {
    return { valid: false, errorKey: 'search.errors.empty' as TranslationKeys };
  }
  const date = new Date(value);
  if (isNaN(date.getTime())) {
    return { valid: false, errorKey: 'search.errors.pattern' as TranslationKeys };
  }
  return { valid: true };
}
