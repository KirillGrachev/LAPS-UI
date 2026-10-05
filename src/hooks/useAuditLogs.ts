/**
 * Журнал аудита: постраничная загрузка с бэкенда (`list_audit`),
 * обновление и «загрузить ещё».
 */
import { useCallback, useEffect, useState } from 'react';
import type { AuditEvent, AuditPage } from '../types';
import { tauriInvoke, toIpcError } from '../lib/ipc';
import { system } from '../lib/system';

const PAGE_SIZE = 50;

export function useAuditLogs() {
  const [logs, setLogs] = useState<AuditEvent[]>([]);
  const [total, setTotal] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchPage = useCallback(async (offset: number, append: boolean) => {
    if (append) {
      setIsLoadingMore(true);
    } else {
      setIsLoading(true);
    }
    setError(null);
    try {
      const page = await tauriInvoke<AuditPage>('list_audit', { limit: PAGE_SIZE, offset });
      setTotal(page.total);
      setLogs((prev) => (append ? [...prev, ...page.entries] : page.entries));
    } catch (err) {
      setError(toIpcError(err).message);
    } finally {
      setIsLoading(false);
      setIsLoadingMore(false);
    }
  }, []);

  const refresh = useCallback(() => fetchPage(0, false), [fetchPage]);
  const loadMore = useCallback(() => fetchPage(logs.length, true), [fetchPage, logs.length]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const openFolder = useCallback(async () => {
    try {
      await system.openAuditFolder();
    } catch (err) {
      setError(toIpcError(err).message);
    }
  }, []);

  return {
    logs,
    total,
    isLoading,
    isLoadingMore,
    error,
    refresh,
    loadMore,
    openFolder,
    hasMore: logs.length < total,
  };
}
