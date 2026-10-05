/**
 * Нормализует системные операции так, чтобы они бесшовно работали как в
 * Tauri (десктоп), так и в браузере (веб-режим разработки).
 *
 * Привилегированные действия (открытие ссылок, папки журнала) идут через
 * Rust-команды бэкенда: у фронтенда нет прямых разрешений плагинов —
 * capability главного окна ограничен управлением окном.
 */
import { tauriInvoke } from './ipc';

export const system = {
  isTauri: () => {
    /** @ts-ignore */
    return typeof window !== 'undefined' && !!(window.__TAURI_INTERNALS__ || window.__TAURI__);
  },

  /** Показать главное окно (вызывается фронтендом после гидрации). */
  showMainWindow: async () => {
    if (!system.isTauri()) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const win = getCurrentWindow();
      await win.show();
      await win.setFocus();
    } catch (e) {
      console.warn('Window show failed', e);
    }
  },

  minimizeWindow: async () => {
    try {
      if (system.isTauri()) {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().minimize();
      }
    } catch (e) {
      console.warn('Window minimize failed', e);
    }
  },

  toggleMaximizeWindow: async () => {
    try {
      if (system.isTauri()) {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const win = getCurrentWindow();
        const isMaximized = await win.isMaximized();
        if (isMaximized) {
          await win.unmaximize();
        } else {
          await win.maximize();
        }
      }
    } catch (e) {
      console.warn('Window toggleMaximize failed', e);
    }
  },

  closeWindow: async () => {
    try {
      if (system.isTauri()) {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await getCurrentWindow().close();
      }
    } catch (e) {
      console.warn('Window close failed', e);
    }
  },

  /** Открытие внешней ссылки — через бэкенд (валидация схемы, без shell-инъекций). */
  openExternalUrl: async (url: string) => {
    if (system.isTauri()) {
      await tauriInvoke('open_external', { url });
    } else {
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  },

  /** Показать файл журнала аудита в системном проводнике. */
  openAuditFolder: async () => {
    if (system.isTauri()) {
      await tauriInvoke('open_audit_folder');
    }
  },
};
