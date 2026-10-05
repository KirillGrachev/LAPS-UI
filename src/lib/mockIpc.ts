/**
 * Мок-реализация IPC для разработки интерфейса в браузере (`npm run dev`
 * вне Tauri). Повторяет контракты Rust-команд и хранит состояние в
 * localStorage, чтобы настройки и журнал переживали перезагрузку страницы.
 *
 * Демо-компьютеры покрывают все сценарии доменной логики:
 * * `WS01`      — Windows LAPS, открытое хранение (основной сценарий);
 * * `SRV-LEG01` — legacy LAPS (ms-Mcs-AdmPwd);
 * * `WS03`      — Windows LAPS, только шифрованный пароль (диагностика);
 * * `WS-NOLAPS` — компьютер без данных LAPS (ошибка NO_LAPS_DATA);
 * * всё прочее  — COMPUTER_NOT_FOUND.
 */
import type {
  AppConfig,
  AuditEvent,
  AuditPage,
  AppEnvironment,
  ConnectionTest,
  LapsInfo,
  IpcError,
} from '../types';

const LS_CONFIG = 'kmaruda-laps-mock-config';
const LS_AUDIT = 'kmaruda-laps-mock-audit';

/** Сессия входа живёт только в памяти вкладки (как на бэкенде). */
let mockSession: { actor: string; integrated: boolean } | null = null;

const DEFAULT_CONFIG: AppConfig = {
  theme: 'system',
  language: 'ru',
  ldapUrl: 'ldap://dc1.company.ru',
  baseDn: 'DC=company,DC=ru',
  useStartTls: false,
  allowInvalidTls: false,
  auditEnabled: true,
  clipboardClearSeconds: 30,
  testMode: false,
};

/** Сброс демо-сессии (переключение тестового режима, выход). */
export function mockResetSession(): void {
  mockSession = null;
}

function loadConfig(): AppConfig {
  try {
    const raw = localStorage.getItem(LS_CONFIG);
    if (!raw) return { ...DEFAULT_CONFIG };
    return { ...DEFAULT_CONFIG, ...JSON.parse(raw) };
  } catch {
    return { ...DEFAULT_CONFIG };
  }
}

function saveConfig(config: AppConfig) {
  localStorage.setItem(LS_CONFIG, JSON.stringify(config));
}

function loadAudit(): AuditEvent[] {
  try {
    const raw = localStorage.getItem(LS_AUDIT);
    return raw ? (JSON.parse(raw) as AuditEvent[]) : [];
  } catch {
    return [];
  }
}

function pushAudit(event: Omit<AuditEvent, 'ts' | 'actor'>) {
  const config = loadConfig();
  if (!config.auditEnabled) return;
  const entry: AuditEvent = {
    ts: new Date().toISOString(),
    actor: mockSession?.actor ?? 'MOCK\\engineer@DEV-PC',
    ...event,
  };
  const events = loadAudit();
  events.unshift(entry);
  localStorage.setItem(LS_AUDIT, JSON.stringify(events.slice(0, 500)));
}

function fail(code: string, message: string): never {
  const error: IpcError = { code, message };
  throw error;
}

function futureDate(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  return d.toISOString();
}

function demoComputer(query: string): LapsInfo {
  const name = query.trim().toUpperCase();
  if (name === 'WS01' || name === 'WS01.COMPANY.RU') {
    return {
      dn: 'CN=WS01,OU=Workstations,DC=company,DC=ru',
      computerName: 'WS01',
      dnsHostName: 'ws01.company.ru',
      operatingSystem: 'Windows 11 Pro',
      flavor: 'windowsPlain',
      accountName: 'Administrator',
      password: 'A6a3#7%eb!57be4a4B',
      updatedAt: new Date(Date.now() - 86400000 * 12).toISOString(),
      expiresAt: futureDate(18),
      decrypted: false,
      legacyPresent: false,
      windowsPlainPresent: true,
      encryptedPresent: false,
      rotationPossible: true,
      decryptMethod: null,
    };
  }
  if (name === 'SRV-LEG01') {
    return {
      dn: 'CN=SRV-LEG01,OU=Servers,DC=company,DC=ru',
      computerName: 'SRV-LEG01',
      dnsHostName: 'srv-leg01.company.ru',
      operatingSystem: 'Windows Server 2016 Standard',
      flavor: 'legacy',
      accountName: null,
      password: 'lVP;H3{.l99x0+',
      updatedAt: null,
      expiresAt: futureDate(3),
      decrypted: false,
      legacyPresent: true,
      windowsPlainPresent: false,
      encryptedPresent: false,
      rotationPossible: true,
      decryptMethod: null,
    };
  }
  if (name === 'WS03') {
    return {
      dn: 'CN=WS03,OU=Workstations,DC=company,DC=ru',
      computerName: 'WS03',
      dnsHostName: 'ws03.company.ru',
      operatingSystem: 'Windows 11 Enterprise',
      flavor: 'windowsEncrypted',
      accountName: null,
      password: null,
      updatedAt: null,
      expiresAt: futureDate(9),
      decrypted: false,
      legacyPresent: false,
      windowsPlainPresent: false,
      encryptedPresent: true,
      rotationPossible: true,
      decryptMethod: null,
    };
  }
  if (name === 'WS04') {
    return {
      dn: 'CN=WS04,OU=Workstations,DC=company,DC=ru',
      computerName: 'WS04',
      dnsHostName: 'ws04.company.ru',
      operatingSystem: 'Windows 11 Pro',
      flavor: 'windowsEncrypted',
      accountName: 'Administrator',
      password: 'D3crypt3d!ViaModule',
      updatedAt: new Date(Date.now() - 86400000 * 5).toISOString(),
      expiresAt: futureDate(25),
      legacyPresent: false,
      windowsPlainPresent: false,
      encryptedPresent: true,
      rotationPossible: true,
      decrypted: true,
      decryptMethod: 'laps-module',
    };
  }
  if (name === 'WS-NOLAPS') {
    fail(
      'NO_LAPS_DATA',
      'Компьютер WS-NOLAPS найден, но пароля LAPS не видно: машина не управляется LAPS, пароль ещё не сгенерирован, либо вашей учётной записи не делегировано право чтения атрибутов LAPS.',
    );
  }
  fail('COMPUTER_NOT_FOUND', `Компьютер не найден в Active Directory: «${query}» не найден в DC=company,DC=ru.`);
}

/** Имитация IPC бэкенда для тестового режима, включая сетевую задержку. */
export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((resolve) => setTimeout(resolve, 350));

  switch (cmd) {
    case 'load_config': {
      return loadConfig() as T;
    }
    case 'session_status':
      return mockSession as T;
    case 'login': {
      const config = loadConfig();
      if (!config.ldapUrl) fail('CONFIG_ERROR', 'Не указан адрес контроллера домена.');
      if (args?.sso) {
        mockSession = { actor: 'MOCK\\engineer@DEV-PC', integrated: true };
      } else {
        const account = String(args?.account ?? '').trim();
        const password = String(args?.password ?? '');
        if (!account) fail('VALIDATION', 'Укажите учётную запись');
        if (!password) fail('VALIDATION', 'Укажите пароль');
        if (password === 'bad') fail('LDAP_AUTH', 'неверный логин или пароль');
        mockSession = { actor: `${account.toUpperCase()}@DEV-PC`, integrated: false };
      }
      pushAudit({ action: 'login', target: mockSession?.actor ?? '-', code: 'OK' });
      return mockSession as T;
    }
    case 'logout': {
      if (mockSession) {
        pushAudit({ action: 'logout', target: mockSession.actor, code: 'OK' });
      }
      mockSession = null;
      return undefined as T;
    }
    case 'save_config': {
      const request = (args?.config ?? {}) as Partial<AppConfig>;
      const config: AppConfig = { ...loadConfig(), ...request };
      saveConfig(config);
      pushAudit({ action: 'config_saved', target: config.ldapUrl || '-', code: 'OK' });
      return config as T;
    }
    case 'test_ldap_connection': {
      const config = loadConfig();
      if (!config.ldapUrl) {
        fail('CONFIG_ERROR', 'Не указан адрес контроллера домена.');
      }
      const result: ConnectionTest = {
        bindMs: 42,
        serverDnsName: 'dc1.company.ru',
        defaultNamingContext: 'DC=company,DC=ru',
        authMode: mockSession ? (mockSession.integrated ? 'integrated' : 'account') : 'anonymous',
      };
      pushAudit({ action: 'connection_test', target: config.ldapUrl, code: 'OK', details: 'bind 42 мс' });
      return result as T;
    }
    case 'search_computer': {
      if (!mockSession) fail('NOT_AUTHENTICATED', 'Требуется вход: выполните аутентификацию, чтобы продолжить');
      const query = String(args?.query ?? '');
      if (!query.trim()) fail('VALIDATION', 'Введите имя компьютера, FQDN или IP-адрес');
      const info = demoComputer(query);
      pushAudit({ action: 'search', target: info.computerName, flavor: info.flavor, code: 'OK' });
      return info as T;
    }
    case 'rotate_password': {
      if (!mockSession) fail('NOT_AUTHENTICATED', 'Требуется вход: выполните аутентификацию, чтобы продолжить');
      const dn = String(args?.dn ?? '');
      const mode = String(args?.mode ?? '');
      const info = demoComputer(dn.replace(/^CN=/, '').split(',')[0]);
      const rotated: LapsInfo = {
        ...info,
        expiresAt: mode === 'now' ? null : new Date(String(args?.localDateTime ?? '')).toISOString(),
      };
      pushAudit({
        action: 'rotate',
        target: info.computerName,
        flavor: info.flavor,
        code: 'OK',
        details: mode === 'now' ? 'reset-now' : `set:${args?.localDateTime ?? ''}`,
      });
      return rotated as T;
    }
    case 'note_password_copied': {
      if (!mockSession) fail('NOT_AUTHENTICATED', 'Требуется вход');
      pushAudit({ action: 'copy', target: String(args?.computerName ?? '-'), code: 'OK' });
      return undefined as T;
    }
    case 'list_audit': {
      const limit = Number(args?.limit ?? 50);
      const offset = Number(args?.offset ?? 0);
      const events = loadAudit();
      const page: AuditPage = {
        entries: events.slice(offset, offset + limit),
        total: events.length,
      };
      return page as T;
    }
    case 'open_audit_folder':
    case 'open_external':
      return undefined as T;
    case 'app_environment': {
      const env: AppEnvironment = {
        version: typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '2.0.0-mock',
        os: 'browser-mock',
        arch: 'web',
        integratedAuthAvailable: false,
        windowsIdentity: 'MOCK\\engineer@DEV-PC',
      };
      return env as T;
    }
    default:
      throw new Error(`mockInvoke: неизвестная команда ${cmd}`);
  }
}
