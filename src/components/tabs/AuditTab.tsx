import {
  RefreshCw,
  FolderOpen,
  Eye,
  EyeOff,
  RotateCcw,
  Clipboard,
  Save,
  LogIn,
  LogOut,
  PlugZap,
  CheckCircle2,
  XCircle,
  ScrollText,
  Loader2,
} from 'lucide-react';
import { PageLayout } from '../ui/PageLayout';
import { Button } from '../ui/Button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '../ui/Table';
import { useTranslation } from '../../i18n/index.tsx';
import { useAuditLogs } from '../../hooks/useAuditLogs';
import { formatLogTime } from '../../lib/format';
import type { AuditAction } from '../../types';

const ACTION_ICONS: Record<AuditAction, typeof Eye> = {
  search: Eye,
  search_failed: EyeOff,
  rotate: RotateCcw,
  rotate_failed: RotateCcw,
  copy: Clipboard,
  config_saved: Save,
  login: LogIn,
  logout: LogOut,
  connection_test: PlugZap,
};

/** Цветовой код действия: успех — нейтральный, ошибка — красный, копирование — янтарный. */
function actionClasses(action: AuditAction, code: string): string {
  if (code !== 'OK') return 'text-red-500';
  switch (action) {
    case 'copy':
      return 'text-amber-500';
    case 'rotate':
      return 'text-blue-500';
    case 'login':
    case 'logout':
    case 'config_saved':
    case 'connection_test':
      return 'text-muted-foreground';
    default:
      return 'text-emerald-600 dark:text-emerald-400';
  }
}

export function AuditTab() {
  const { t, language } = useTranslation();
  const { logs, total, isLoading, isLoadingMore, error, refresh, loadMore, openFolder, hasMore } =
    useAuditLogs();

  return (
    <PageLayout
      id="audit"
      title={t('audit.title')}
      subtitle={t('audit.subtitle')}
      rightElement={
        <div className="flex items-center gap-2 normal-case">
          <Button
            variant="outline"
            onClick={() => void refresh()}
            className="h-[34px] px-3 text-[0.72rem] font-bold gap-2"
          >
            <RefreshCw size={13} className={isLoading ? 'animate-spin' : ''} />
            {t('audit.refreshBtn')}
          </Button>
          <Button
            variant="outline"
            onClick={() => void openFolder()}
            className="h-[34px] px-3 text-[0.72rem] font-bold gap-2"
          >
            <FolderOpen size={13} />
            {t('audit.openFolderBtn')}
          </Button>
        </div>
      }
      contentClassName="overflow-y-auto p-4 sm:p-6"
    >
      <div className="w-full max-w-6xl mx-auto flex flex-col gap-4">
        {error && (
          <div className="rounded-lg border border-red-500/25 bg-red-500/5 px-4 py-3 text-[0.8rem] font-bold text-red-500">
            {error}
          </div>
        )}

        {isLoading ? (
          <div className="flex items-center justify-center gap-3 py-20 text-muted-foreground">
            <Loader2 size={18} className="animate-spin" />
            <span className="text-[0.85rem] font-bold">{t('app.loading')}</span>
          </div>
        ) : logs.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-3 py-20 text-muted-foreground">
            <ScrollText size={36} className="opacity-25" />
            <span className="text-[0.95rem] font-black uppercase tracking-tight">
              {t('audit.emptyTitle')}
            </span>
            <span className="text-[0.8rem] font-medium">{t('audit.emptyText')}</span>
          </div>
        ) : (
          <>
            <div className="rounded-xl border border-border overflow-hidden bg-surface">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t('audit.columns.time')}</TableHead>
                    <TableHead>{t('audit.columns.actor')}</TableHead>
                    <TableHead>{t('audit.columns.action')}</TableHead>
                    <TableHead>{t('audit.columns.target')}</TableHead>
                    <TableHead>{t('audit.columns.flavor')}</TableHead>
                    <TableHead>{t('audit.columns.result')}</TableHead>
                    <TableHead>{t('audit.columns.details')}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {logs.map((event, index) => {
                    const Icon = ACTION_ICONS[event.action] ?? Eye;
                    const ok = event.code === 'OK';
                    return (
                      <TableRow key={`${event.ts}-${index}`}>
                        <TableCell className="whitespace-nowrap font-mono text-[0.78rem] text-muted-foreground">
                          {formatLogTime(event.ts, language)}
                        </TableCell>
                        <TableCell className="font-bold text-[0.78rem] whitespace-nowrap">
                          {event.actor}
                        </TableCell>
                        <TableCell className="whitespace-nowrap">
                          <span
                            className={`inline-flex items-center gap-1.5 text-[0.78rem] font-bold whitespace-nowrap ${actionClasses(event.action, event.code)}`}
                          >
                            <Icon size={13} />
                            {t(`audit.actions.${event.action}`)}
                          </span>
                        </TableCell>
                        <TableCell className="font-mono font-bold text-[0.78rem] uppercase whitespace-nowrap">
                          {event.target}
                        </TableCell>
                        <TableCell className="text-[0.75rem] text-muted-foreground whitespace-nowrap">
                          {event.flavor ?? '—'}
                        </TableCell>
                        <TableCell className="whitespace-nowrap">
                          {ok ? (
                            <CheckCircle2 size={15} className="text-emerald-500" />
                          ) : (
                            <span className="inline-flex items-center gap-1.5 text-red-500">
                              <XCircle size={15} />
                              <span className="font-mono text-[0.68rem] font-bold">
                                {event.code}
                              </span>
                            </span>
                          )}
                        </TableCell>
                        <TableCell className="text-[0.72rem] text-muted-foreground min-w-[220px] max-w-[360px] break-words">
                          {event.details ?? '—'}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>

            <div className="flex items-center justify-between gap-4 px-1">
              <span className="text-[0.72rem] font-bold text-muted-foreground">
                {t('audit.shownOf', { shown: logs.length, total })}
              </span>
              {hasMore && (
                <Button
                  variant="outline"
                  onClick={() => void loadMore()}
                  disabled={isLoadingMore}
                  className="h-[34px] px-4 text-[0.72rem] font-bold gap-2"
                >
                  {isLoadingMore && <Loader2 size={13} className="animate-spin" />}
                  {t('audit.loadMoreBtn')}
                </Button>
              )}
            </div>
          </>
        )}
      </div>
    </PageLayout>
  );
}
