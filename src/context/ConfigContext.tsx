import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';
import { toast } from 'sonner';
import type { AppConfig, SaveConfigRequest } from '../types';
import { tauriInvoke, toIpcError, setTestModeActive } from '../lib/ipc';
import { t } from '../i18n/index.tsx';

const DEFAULT_CONFIG: AppConfig = {
  theme: 'system',
  language: 'ru',
  ldapUrl: '',
  baseDn: '',
  useStartTls: false,
  allowInvalidTls: false,
  auditEnabled: true,
  clipboardClearSeconds: 30,
  testMode: false,
};

interface ConfigContextType {
  config: AppConfig;
  /** Полное сохранение конфигурации (кнопка «Сохранить» в настройках). */
  saveConfig: (next: SaveConfigRequest, options?: { silent?: boolean }) => Promise<AppConfig | null>;
  /** Перечитать конфигурацию с бэкенда. */
  refresh: () => Promise<void>;
  isLoaded: boolean;
}

const ConfigContext = createContext<ConfigContextType | undefined>(undefined);

export function ConfigProvider({ children }: { children: React.ReactNode }) {
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [isLoaded, setIsLoaded] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const loaded = await tauriInvoke<AppConfig>('load_config');
      const merged = { ...DEFAULT_CONFIG, ...loaded };
      setTestModeActive(merged.testMode === true);
      setConfig(merged);
    } catch (err) {
      console.error('Не удалось загрузить конфигурацию:', err);
      toast.error(toIpcError(err).message, { id: 'config-load-error' });
    } finally {
      setIsLoaded(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const saveConfig = useCallback(
    async (next: SaveConfigRequest, options?: { silent?: boolean }) => {
      try {
        const saved = await tauriInvoke<AppConfig>('save_config', { config: next });
        const merged = { ...DEFAULT_CONFIG, ...saved };
        setTestModeActive(merged.testMode === true);
        setConfig(merged);
        if (!options?.silent) {
          toast.success(t('settings.saveSuccess'), { id: 'config-save' });
        }
        return merged;
      } catch (err) {
        const ipcError = toIpcError(err);
        toast.error(ipcError.message || t('settings.saveError'), { id: 'config-save' });
        return null;
      }
    },
    [],
  );

  return (
    <ConfigContext.Provider value={{ config, saveConfig, refresh, isLoaded }}>
      {children}
    </ConfigContext.Provider>
  );
}

export function useConfig() {
  const context = useContext(ConfigContext);
  if (context === undefined) {
    throw new Error('useConfig must be used within a ConfigProvider');
  }
  return context;
}
