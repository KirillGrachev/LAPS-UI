import { describe, expect, it } from 'vitest';
import { shouldUseMock, setTestModeActive, isTestModeActive } from '../ipc';

const desktop = { tauri: true, dev: false };

describe('shouldUseMock — маршрутизация команд IPC', () => {
  it('в браузерной разработке обслуживает моком все команды', () => {
    const env = { tauri: false, dev: true, testMode: false };
    expect(shouldUseMock('login', env)).toBe(true);
    expect(shouldUseMock('load_config', env)).toBe(true);
    expect(shouldUseMock('search_computer', env)).toBe(true);
  });

  it('вне Tauri и вне разработки мок выключен', () => {
    expect(shouldUseMock('login', { tauri: false, dev: false, testMode: false })).toBe(false);
  });

  it('в desktop-сборке без тестового режима всё идёт на бэкенд', () => {
    expect(shouldUseMock('login', { ...desktop, testMode: false })).toBe(false);
    expect(shouldUseMock('search_computer', { ...desktop, testMode: false })).toBe(false);
  });

  it('в тестовом режиме доменные команды уходят в демо-данные', () => {
    const env = { ...desktop, testMode: true };
    for (const cmd of [
      'login',
      'logout',
      'session_status',
      'search_computer',
      'rotate_password',
      'note_password_copied',
      'list_audit',
      'test_ldap_connection',
    ]) {
      expect(shouldUseMock(cmd, env), cmd).toBe(true);
    }
  });

  it('в тестовом режиме конфигурация и системные команды остаются настоящими', () => {
    const env = { ...desktop, testMode: true };
    for (const cmd of [
      'load_config',
      'save_config',
      'app_environment',
      'open_external',
      'open_audit_folder',
    ]) {
      expect(shouldUseMock(cmd, env), cmd).toBe(false);
    }
  });
});

describe('флаг тестового режима', () => {
  it('устанавливается и читается', () => {
    setTestModeActive(true);
    expect(isTestModeActive()).toBe(true);
    setTestModeActive(false);
    expect(isTestModeActive()).toBe(false);
  });
});
