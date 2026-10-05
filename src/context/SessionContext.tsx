import { createContext, useCallback, useContext, useEffect, useState, ReactNode } from 'react';
import type { SessionInfo } from '../types';
import { tauriInvoke } from '../lib/ipc';

interface SessionContextType {
  session: SessionInfo | null;
  /** true, пока статус сессии не получен при старте. */
  checking: boolean;
  setSession: (session: SessionInfo | null) => void;
  /** Выход: стирает сессию на бэкенде (пароль затирается в памяти). */
  logout: () => Promise<void>;
  /** Команды вернули NOT_AUTHENTICATED — принудительно возвращаем ко входу. */
  requireReauth: () => void;
}

const SessionContext = createContext<SessionContextType | undefined>(undefined);

export function SessionProvider({ children }: { children: ReactNode }) {
  const [session, setSessionState] = useState<SessionInfo | null>(null);
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    let cancelled = false;
    tauriInvoke<SessionInfo | null>('session_status')
      .then((value) => {
        if (!cancelled) setSessionState(value);
      })
      .catch(() => {
        if (!cancelled) setSessionState(null);
      })
      .finally(() => {
        if (!cancelled) setChecking(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const setSession = useCallback((value: SessionInfo | null) => {
    setSessionState(value);
  }, []);

  const logout = useCallback(async () => {
    try {
      await tauriInvoke('logout');
    } finally {
      setSessionState(null);
    }
  }, []);

  const requireReauth = useCallback(() => {
    setSessionState(null);
  }, []);

  return (
    <SessionContext.Provider value={{ session, checking, setSession, logout, requireReauth }}>
      {children}
    </SessionContext.Provider>
  );
}

export function useSession() {
  const context = useContext(SessionContext);
  if (context === undefined) {
    throw new Error('useSession must be used within a SessionProvider');
  }
  return context;
}
