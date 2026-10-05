import { useEffect, useState } from 'react';
import type { AppEnvironment } from '../types';
import { tauriInvoke } from '../lib/ipc';

/** Кэш на модуль: окружение не меняется в течение сеанса. */
let cached: AppEnvironment | null = null;
let inflight: Promise<AppEnvironment> | null = null;

function fetchEnvironment(): Promise<AppEnvironment> {
  if (cached) return Promise.resolve(cached);
  if (!inflight) {
    inflight = tauriInvoke<AppEnvironment>('app_environment')
      .then((env) => {
        cached = env;
        return env;
      })
      .catch((err) => {
        console.warn('app_environment недоступно:', err);
        inflight = null;
        throw err;
      });
  }
  return inflight;
}

/** Сведения об окружении: версия, платформа, текущая сессия Windows. */
export function useAppEnvironment(): AppEnvironment | null {
  const [env, setEnv] = useState<AppEnvironment | null>(cached);

  useEffect(() => {
    if (cached) {
      setEnv(cached);
      return;
    }
    let cancelled = false;
    fetchEnvironment()
      .then((value) => {
        if (!cancelled) setEnv(value);
      })
      .catch(() => {
        /* браузерный режим: моки уже вернули значения */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return env;
}
