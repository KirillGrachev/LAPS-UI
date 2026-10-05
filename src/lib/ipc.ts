/// <reference types="vite/client" />
import type { IpcError } from '../types';
import { system } from './system';
import { mockInvoke, mockResetSession } from './mockIpc';

/**
 * Универсальный адаптер команд Tauri IPC.
 *
 * В среде Tauri делегирует вызов функции `invoke`; в браузерной среде
 * разработки (`npm run dev` без Tauri) переключается на мок-реализацию,
 * чтобы интерфейс можно было разрабатывать и ревьюить без домена.
 *
 * Тестовый режим (настройка `testMode`) включает демо-данные и внутри
 * desktop-сборки: доменные команды обслуживает мок, а конфигурация и
 * системные команды продолжают ходить на бэкенд — флаг переживает
 * перезапуск и его всегда можно выключить обратно.
 */

/**
 * Команды, которые даже в тестовом режиме выполняются по-настоящему:
 * конфигурация (иначе флаг не сохранить), окружение и системные действия
 * (ссылки, папка журнала — это файловая система, а не каталог домена).
 */
const ALWAYS_REAL = new Set([
  'load_config',
  'save_config',
  'app_environment',
  'open_external',
  'open_audit_folder',
]);

/** Активен ли тестовый режим; синхронизируется из конфигурации. */
let testModeActive = false;

export function setTestModeActive(value: boolean): void {
  testModeActive = value;
}

export function isTestModeActive(): boolean {
  return testModeActive;
}

/**
 * Чистое правило маршрутизации команды: мок или настоящий бэкенд.
 * Вынесено для unit-тестов; вне Tauri (браузерная разработка) всё
 * обслуживает мок, как и раньше.
 */
export function shouldUseMock(
  cmd: string,
  env: { tauri: boolean; dev: boolean; testMode: boolean },
): boolean {
  if (!env.tauri) return env.dev;
  return env.testMode && !ALWAYS_REAL.has(cmd);
}

export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const isTauri = system.isTauri();
  if (shouldUseMock(cmd, { tauri: isTauri, dev: import.meta.env.DEV, testMode: testModeActive })) {
    return await mockInvoke<T>(cmd, args);
  }
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<T>(cmd, args);
  }
  throw new Error('Tauri API недоступен, и это не среда разработки.');
}

/**
 * Сброс аутентификации обоих транспортов при переключении тестового режима:
 * живая сессия бэкенда и демо-сессия стираются, приложение возвращает
 * пользователя ко входу под актуальный режим данных.
 */
export async function dropAllSessions(): Promise<void> {
  mockResetSession();
  if (system.isTauri()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('logout');
    } catch {
    }
  }
}

/** Признак структурированной ошибки бэкенда ({ code, message }). */
export function isIpcError(value: unknown): value is IpcError {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    'message' in value &&
    typeof (value as IpcError).code === 'string' &&
    typeof (value as IpcError).message === 'string'
  );
}

/**
 * Нормализует любое исключение invoke к `{ code, message }`.
 * Код неизвестен — `UNKNOWN`, сообщение — строковое представление.
 */
export function toIpcError(err: unknown): IpcError {
  if (isIpcError(err)) {
    return err;
  }
  if (err instanceof Error) {
    return { code: 'UNKNOWN', message: err.message };
  }
  return { code: 'UNKNOWN', message: String(err) };
}
