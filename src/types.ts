/**
 * Контракты IPC между фронтендом и Rust-бэкендом.
 *
 * Имена полей — camelCase (Rust-структуры сериализуются с
 * `#[serde(rename_all = "camelCase")]`). Держим синхронно с
 * `src-tauri/src/services/**` — любое изменение контракта на бэкенде
 * обязано отражаться здесь.
 */

/** Тема интерфейса. */
export type Theme = "light" | "dark" | "system";

/** Язык интерфейса. */
export type Language = "ru" | "en";

/** Конфигурация приложения (ответ `load_config` / `save_config`). */
export interface AppConfig {
  theme: Theme;
  language: Language;
  /** Адрес контроллера домена: `ldap://dc1.company.ru` / `ldaps://...`. */
  ldapUrl: string;
  /** Корень поиска; пусто — берётся `defaultNamingContext` из rootDSE. */
  baseDn: string;
  useStartTls: boolean;
  /** Не проверять сертификат TLS (внутренние CA). */
  allowInvalidTls: boolean;
  auditEnabled: boolean;
  /** Автоочистка буфера обмена после копирования пароля, сек (0 — выключена). */
  clipboardClearSeconds: number;
  /** Тестовый режим: демо-данные вместо запросов к Active Directory. */
  testMode: boolean;
}

/** Данные для `save_config` (без вычисляемого `hasPassword`). */
export type SaveConfigRequest = Omit<AppConfig, "hasPassword">;

/** Вариант хранения пароля LAPS (сериализация `LapsFlavor`). */
export type LapsFlavor = "legacy" | "windowsPlain" | "windowsEncrypted" | "none";

/** Карточка LAPS компьютера (ответ `search_computer` / `rotate_password`). */
export interface LapsInfo {
  dn: string;
  computerName: string;
  dnsHostName: string | null;
  operatingSystem: string | null;
  flavor: LapsFlavor;
  /** Имя управляемой локальной учётки (из JSON Windows LAPS). */
  accountName: string | null;
  /** Пароль; `null` для шифрованного хранения (`windowsEncrypted`). */
  password: string | null;
  /** Момент последнего обновления пароля, RFC 3339 (локальное время). */
  updatedAt: string | null;
  /** Срок истечения пароля, RFC 3339 (локальное время); `null` — сброшен/не задан. */
  expiresAt: string | null;
  /** Пароль получен расшифровкой msLAPS-EncryptedPassword (KDS/DPAPI-NG). */
  decrypted: boolean;
  /** Метод дешифровки: `kds` (нативно) или `laps-module` (модуль Windows LAPS). */
  decryptMethod?: string | null;
  legacyPresent: boolean;
  windowsPlainPresent: boolean;
  encryptedPresent: boolean;
  /** Доступна ли операция сброса/продления срока. */
  rotationPossible: boolean;
}

/** Результат `test_ldap_connection`. */
export interface ConnectionTest {
  bindMs: number;
  serverDnsName: string | null;
  defaultNamingContext: string | null;
  /** Режим проверки: `integrated` | `account` | `anonymous`. */
  authMode: 'integrated' | 'account' | 'anonymous';
}

/** Сессия входа (ответ `session_status` / `login`). Секретов не содержит. */
export interface SessionInfo {
  /** `DOMAIN\user@MACHINE` (SSO) или учётная запись явного входа. */
  actor: string;
  integrated: boolean;
}

/** Тип журналируемого действия (сериализация `AuditAction`). */
export type AuditAction =
  | "search"
  | "search_failed"
  | "rotate"
  | "rotate_failed"
  | "copy"
  | "config_saved"
  | "login"
  | "logout"
  | "connection_test";

/** Запись журнала аудита. */
export interface AuditEvent {
  /** RFC 3339 с локальным смещением. */
  ts: string;
  /** Субъект: `DOMAIN\user@MACHINE` или DN сервисной учётки. */
  actor: string;
  action: AuditAction;
  target: string;
  flavor?: string;
  /** `OK` или стабильный код ошибки. */
  code: string;
  details?: string;
}

/** Страница журнала (ответ `list_audit`). */
export interface AuditPage {
  entries: AuditEvent[];
  total: number;
}

/** Сведения об окружении (ответ `app_environment`). */
export interface AppEnvironment {
  version: string;
  os: string;
  arch: string;
  /** Интегрированная аутентификация доступна (Windows-сборки). */
  integratedAuthAvailable: boolean;
  /** Текущая сессия Windows: `DOMAIN\user@MACHINE`. */
  windowsIdentity: string;
}

/** Единый формат ошибки бэкенда (`AppError` → JSON). */
export interface IpcError {
  code: string;
  message: string;
  details?: string;
}
