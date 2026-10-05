import { useEffect, useRef, useState } from 'react';
import {
  Sun,
  Moon,
  Monitor,
  PlugZap,
  Loader2,
  CheckCircle2,
  XCircle,
  ShieldCheck,
  FolderOpen,
  TriangleAlert,
  Database,
  Lock,
  Palette,
  FlaskConical,
  ArrowLeft,
  ChevronRight,
  Check,
} from 'lucide-react';
import { PageLayout } from '../ui/PageLayout';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import { Select } from '../ui/Select';
import { Switch } from '../ui/Switch';
import { useTheme } from '../../context/ThemeContext';
import { useConfig } from '../../context/ConfigContext';
import { useSession } from '../../context/SessionContext';
import { useTranslation } from '../../i18n/index.tsx';
import { tauriInvoke, toIpcError, dropAllSessions } from '../../lib/ipc';
import { system } from '../../lib/system';
import type { ConnectionTest, SaveConfigRequest, Theme } from '../../types';

const CLIPBOARD_OPTIONS = [0, 15, 30, 60, 120];

type SettingsView = 'menu' | 'ad' | 'security' | 'interface';

/** Строка меню настроек — стилистика hubs справочника. */
function MenuRow({
  icon: Icon,
  tint,
  title,
  onClick,
}: {
  icon: typeof Database;
  tint: string;
  title: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="w-full flex items-center gap-4 rounded-xl bg-surface px-5 py-4 hover:bg-surface-hover transition-colors text-left"
    >
      <span className={`w-10 h-10 rounded-xl flex items-center justify-center shrink-0 ${tint}`}>
        <Icon size={18} />
      </span>
      <span className="flex-1 text-[0.95rem] font-black text-foreground truncate">{title}</span>
      <ChevronRight size={16} className="text-muted-foreground/50 shrink-0" />
    </button>
  );
}

/** Строка-переключатель в стилистике справочника: текст слева, switch справа. */
function ToggleRow({
  id,
  checked,
  onChange,
  label,
  hint,
  warning,
  disabled,
}: {
  id: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  hint?: string;
  warning?: string;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-start justify-between gap-6 w-full">
      <div className="flex flex-col gap-1 min-w-0">
        <label
          htmlFor={id}
          className="text-[0.85rem] text-foreground font-bold cursor-pointer select-none leading-snug"
        >
          {label}
        </label>
        {hint && (
          <span className="text-[0.7rem] text-muted-foreground font-medium leading-snug">
            {hint}
          </span>
        )}
        {warning && checked && (
          <span className="inline-flex items-center gap-1.5 text-[0.7rem] text-amber-500 font-bold leading-snug">
            <TriangleAlert size={12} className="shrink-0" />
            {warning}
          </span>
        )}
      </div>
      <div className="pt-0.5">
        <Switch id={id} checked={checked} onChange={onChange} disabled={disabled} />
      </div>
    </div>
  );
}

/** Поле с подписью и подсказкой в стилистике справочника: всё слева. */
function Field({
  id,
  label,
  hint,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5 w-full">
      <label
        htmlFor={id}
        className="text-[0.68rem] font-black text-muted-foreground uppercase tracking-tight"
      >
        {label}
      </label>
      {children}
      {hint && (
        <span className="text-[0.68rem] text-muted-foreground/80 font-medium leading-snug">
          {hint}
        </span>
      )}
    </div>
  );
}

interface SettingsTabProps {
  /** Режим до входа: показывается кнопка возврата ко входу. */
  preAuth?: boolean;
  onBackToLogin?: () => void;
}

/**
 * Хаб настроек со страницами разделов.
 *
 * Автосохранение: черновик уехал от сохранённой конфигурации —
 * дебаунс 600 мс и тихий save; первый рендер не сохраняем.
 * my-auto: короткий контент (меню разделов) центрируется по
 * вертикали страницы, как в справочнике; длинный — скроллится
 * от верха без артефактов.
 */
export function SettingsTab({ preAuth = false, onBackToLogin }: SettingsTabProps) {
  const { theme } = useTheme();
  const { config, saveConfig } = useConfig();
  const { requireReauth } = useSession();
  const { t, language, setLanguage } = useTranslation();

  const [view, setView] = useState<SettingsView>('menu');
  const [draft, setDraft] = useState<SaveConfigRequest>({ ...config });
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<ConnectionTest | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [autoSavedAt, setAutoSavedAt] = useState<number | null>(null);
  const [testModeBusy, setTestModeBusy] = useState(false);
  const skipFirstSave = useRef(true);

  useEffect(() => {
    setDraft({ ...config });
  }, [config]);

  const patch = (partial: Partial<SaveConfigRequest>) => {
    setDraft((prev) => ({ ...prev, ...partial }));
  };

  useEffect(() => {
    if (skipFirstSave.current) {
      skipFirstSave.current = false;
      return;
    }
    const sameAsConfig =
      JSON.stringify(draft) ===
      JSON.stringify({
        theme: config.theme,
        language: config.language,
        ldapUrl: config.ldapUrl,
        baseDn: config.baseDn,
        useStartTls: config.useStartTls,
        allowInvalidTls: config.allowInvalidTls,
        auditEnabled: config.auditEnabled,
        clipboardClearSeconds: config.clipboardClearSeconds,
        testMode: config.testMode,
      });
    if (sameAsConfig) return;
    const timer = window.setTimeout(async () => {
      const saved = await saveConfig(draft, { silent: true });
      if (saved) setAutoSavedAt(Date.now());
    }, 600);
    return () => window.clearTimeout(timer);
  }, [draft, config, saveConfig]);

  /**
   * Тестовый режим применяется немедленно: конфигурация сохраняется,
   * сессии обоих транспортов (реальная и демо) сбрасываются — вход
   * выполняется заново уже под актуальный источник данных.
   * Сохранение не удалось — откатываем переключатель.
   */
  const handleTestMode = async (value: boolean) => {
    if (testModeBusy || value === config.testMode) return;
    setTestModeBusy(true);
    patch({ testMode: value });
    const saved = await saveConfig({ ...draft, testMode: value }, { silent: true });
    setTestModeBusy(false);
    if (!saved) {
      patch({ testMode: config.testMode });
      return;
    }
    await dropAllSessions();
    requireReauth();
  };

  const handleTest = async () => {
    setIsTesting(true);
    setTestResult(null);
    setTestError(null);
    try {
      const result = await tauriInvoke<ConnectionTest>('test_ldap_connection');
      setTestResult(result);
    } catch (err) {
      setTestError(toIpcError(err).message);
    } finally {
      setIsTesting(false);
    }
  };

  /** StartTLS неприменим к ldaps://: TLS уже встроен в соединение (порт 636). */
  const isLdaps = draft.ldapUrl.trim().toLowerCase().startsWith('ldaps://');

  const authModeLabel = (mode: ConnectionTest['authMode']) =>
    mode === 'integrated'
      ? t('settings.ad.authModeIntegrated')
      : mode === 'account'
        ? t('settings.ad.authModeAccount')
        : t('settings.ad.authModeAnonymous');

  const themeButtons: { value: Theme; icon: typeof Sun; label: string }[] = [
    { value: 'light', icon: Sun, label: t('settings.interface.themes.light') },
    { value: 'dark', icon: Moon, label: t('settings.interface.themes.dark') },
    { value: 'system', icon: Monitor, label: t('settings.interface.themes.system') },
  ];

  const viewTitles: Record<SettingsView, string> = {
    menu: t('settings.title'),
    ad: t('settings.ad.title'),
    security: t('settings.security.title'),
    interface: t('settings.interface.title'),
  };

  return (
    <PageLayout
      id="settings"
      title={viewTitles[view]}
      rightElement={
        preAuth ? (
          <Button
            variant="outline"
            onClick={onBackToLogin}
            className="h-[34px] px-4 text-[0.72rem] font-black gap-2 normal-case"
          >
            <ArrowLeft size={13} />
            {t('auth.title')}
          </Button>
        ) : autoSavedAt ? (
          <span className="inline-flex items-center gap-1.5 text-[0.68rem] font-black text-emerald-600 dark:text-emerald-400 uppercase tracking-tight normal-case">
            <Check size={12} />
            {t('settings.ad.autoSaved')}
          </span>
        ) : undefined
      }
      contentClassName="overflow-y-auto scrollbar-hide p-4 sm:p-6"
    >
      <div className="shrink-0 w-full max-w-2xl mx-auto my-auto flex flex-col gap-5 py-6">
        {view !== 'menu' && (
          <button
            type="button"
            onClick={() => setView('menu')}
            className="self-start inline-flex items-center gap-2 text-[0.75rem] font-black text-muted-foreground hover:text-primary transition-colors uppercase tracking-tight"
          >
            <ArrowLeft size={14} />
            {t('settings.back')}
          </button>
        )}

        {view === 'menu' && (
          <div className="flex flex-col gap-3">
            <MenuRow
              icon={Database}
              tint="bg-amber-500/10 text-amber-600 dark:text-amber-400"
              title={t('settings.ad.title')}
              onClick={() => setView('ad')}
            />
            <MenuRow
              icon={Lock}
              tint="bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
              title={t('settings.security.title')}
              onClick={() => setView('security')}
            />
            <MenuRow
              icon={Palette}
              tint="bg-primary/10 text-primary"
              title={t('settings.interface.title')}
              onClick={() => setView('interface')}
            />
          </div>
        )}

        {view === 'ad' && (
          <div className="flex flex-col gap-5">
            <div className="bg-surface rounded-2xl p-5 flex items-center justify-between gap-6">
              <div className="flex items-center gap-4 min-w-0">
                <span className="w-10 h-10 rounded-xl flex items-center justify-center shrink-0 bg-amber-500/10 text-amber-600 dark:text-amber-400">
                  <FlaskConical size={18} aria-hidden />
                </span>
                <div className="flex flex-col gap-1 min-w-0">
                  <span className="text-[0.9rem] font-black text-foreground leading-snug">
                    {t('settings.ad.testModeLabel')}
                  </span>
                  <span className="text-[0.72rem] text-muted-foreground font-medium leading-snug">
                    {t('settings.ad.testModeDesc')}
                  </span>
                </div>
              </div>
              <Switch
                id="testMode"
                checked={draft.testMode}
                onChange={(value) => void handleTestMode(value)}
                disabled={testModeBusy}
                ariaLabel={t('settings.ad.testModeLabel')}
              />
            </div>

            <div className="flex flex-col gap-5 bg-surface rounded-2xl p-6">
              <p className="text-[0.72rem] text-muted-foreground font-medium leading-relaxed">
                {t('settings.ad.hint')}
              </p>

              <Field id="ldapUrl" label={t('settings.ad.ldapUrlLabel')}>
                <Input
                  id="ldapUrl"
                  type="text"
                  value={draft.ldapUrl}
                  onChange={(e) => patch({ ldapUrl: e.target.value })}
                  placeholder={t('settings.ad.ldapUrlPlaceholder')}
                  className="w-full h-[42px] text-[0.85rem]"
                  autoComplete="off"
                  spellCheck={false}
                />
              </Field>

              <Field id="baseDn" label={t('settings.ad.baseDnLabel')} hint={t('settings.ad.baseDnHint')}>
                <Input
                  id="baseDn"
                  type="text"
                  value={draft.baseDn}
                  onChange={(e) => patch({ baseDn: e.target.value })}
                  placeholder={t('settings.ad.baseDnPlaceholder')}
                  className="w-full h-[42px] text-[0.85rem]"
                  autoComplete="off"
                  spellCheck={false}
                />
              </Field>

              <div className="flex flex-col gap-4 pt-3 border-t border-border/60">
                <ToggleRow
                  id="startTls"
                  checked={isLdaps ? false : draft.useStartTls}
                  onChange={(value) => patch({ useStartTls: value })}
                  label={t('settings.ad.startTlsLabel')}
                  hint={isLdaps ? t('settings.ad.startTlsLdapsNote') : t('settings.ad.startTlsHint')}
                  disabled={isLdaps}
                />
                <ToggleRow
                  id="invalidTls"
                  checked={draft.allowInvalidTls}
                  onChange={(value) => patch({ allowInvalidTls: value })}
                  label={t('settings.ad.invalidTlsLabel')}
                  hint={t('settings.ad.invalidTlsHint')}
                  warning={t('settings.ad.invalidTlsWarning')}
                />
              </div>

              <div className="flex flex-col items-center gap-3 pt-3 border-t border-border/60">
                <Button
                  type="button"
                  variant="primary"
                  onClick={() => void handleTest()}
                  disabled={isTesting || !draft.ldapUrl}
                  className="h-[40px] px-6 text-[0.8rem] font-black gap-2"
                >
                  {isTesting ? <Loader2 size={14} className="animate-spin" /> : <PlugZap size={14} />}
                  {isTesting ? t('settings.ad.testingBtn') : t('settings.ad.testBtn')}
                </Button>

                {testError && (
                  <div className="w-full rounded-lg bg-red-500/[0.08] px-4 py-3 flex items-start gap-2.5">
                    <XCircle size={15} className="text-red-500 mt-0.5 shrink-0" />
                    <div className="flex flex-col gap-1 min-w-0">
                      <span className="text-[0.78rem] font-black text-red-500">
                        {t('settings.ad.testFailedTitle')}
                      </span>
                      <span className="text-[0.73rem] text-muted-foreground font-medium break-words">
                        {testError}
                      </span>
                    </div>
                  </div>
                )}

                {testResult && (
                  <div className="w-full rounded-lg bg-emerald-500/[0.08] px-4 py-3 flex flex-col items-center gap-1.5 text-center">
                    <div className="flex items-center gap-2">
                      <CheckCircle2 size={15} className="text-emerald-500 shrink-0" />
                      <span className="text-[0.78rem] font-black text-emerald-600 dark:text-emerald-400">
                        {t('settings.ad.testOkTitle')}
                      </span>
                    </div>
                    <span className="text-[0.72rem] text-muted-foreground font-bold">
                      {t('settings.ad.testOkText', {
                        ms: testResult.bindMs,
                        server: testResult.serverDnsName ?? '?',
                      })}
                      {' · '}
                      {authModeLabel(testResult.authMode)}
                    </span>
                    {testResult.defaultNamingContext && !draft.baseDn && (
                      <button
                        type="button"
                        onClick={() => patch({ baseDn: testResult.defaultNamingContext ?? '' })}
                        className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-primary/10 text-primary text-[0.68rem] font-black hover:bg-primary/15 transition-colors"
                      >
                        <ShieldCheck size={11} />
                        {t('settings.ad.useAsBaseDnBtn')}: {testResult.defaultNamingContext}
                      </button>
                    )}
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {view === 'security' && (
          <div className="flex flex-col gap-5 bg-surface rounded-2xl p-6">
            <ToggleRow
              id="audit"
              checked={draft.auditEnabled}
              onChange={(value) => patch({ auditEnabled: value })}
              label={t('settings.security.auditLabel')}
              hint={t('settings.security.auditHint')}
            />

            <div className="flex items-start justify-between gap-6 w-full pt-3 border-t border-border/60">
              <div className="flex flex-col gap-1 min-w-0">
                <label
                  htmlFor="clipboardClear"
                  className="text-[0.85rem] text-foreground font-bold leading-snug"
                >
                  {t('settings.security.clipboardLabel')}
                </label>
                <span className="text-[0.7rem] text-muted-foreground font-medium leading-snug">
                  {t('settings.security.clipboardHint')}
                </span>
              </div>
              <Select
                id="clipboardClear"
                className="w-[180px] shrink-0"
                value={draft.clipboardClearSeconds}
                options={CLIPBOARD_OPTIONS.map((seconds) => ({
                  value: seconds,
                  label:
                    seconds === 0
                      ? t('settings.security.clipboardOff')
                      : t('settings.security.clipboardSeconds', { seconds }),
                }))}
                onChange={(value) => patch({ clipboardClearSeconds: value })}
              />
            </div>

            <div className="w-full flex justify-center pt-3 border-t border-border/60">
              <button
                type="button"
                onClick={() => void system.openAuditFolder()}
                className="inline-flex items-center gap-2 px-4 py-2 text-[0.68rem] font-bold uppercase tracking-tight text-muted-foreground hover:text-primary transition-colors bg-[#e6ebf2] dark:bg-white/[0.08] rounded-lg"
              >
                <FolderOpen size={13} />
                {t('settings.security.openAuditFolderBtn')}
              </button>
            </div>
          </div>
        )}

        {view === 'interface' && (
          <div className="flex flex-col gap-5 bg-surface rounded-2xl p-6">
            <div className="flex flex-col gap-3">
              <span className="text-[0.68rem] font-black text-muted-foreground uppercase tracking-tight">
                {t('settings.interface.themeLabel')}
              </span>
              <div className="flex items-center justify-center gap-3">
                {themeButtons.map(({ value, icon: Icon, label }) => (
                  <button
                    key={value}
                    type="button"
                    onClick={() => patch({ theme: value })}
                    className={`w-[92px] p-3 rounded-xl flex flex-col items-center gap-2 transition-all border-none ${
                      theme === value
                        ? 'bg-primary text-primary-foreground scale-105'
                        : 'bg-[#e6ebf2] dark:bg-white/[0.08] text-muted-foreground hover:bg-[#dde4ed] dark:hover:bg-white/[0.13]'
                    }`}
                  >
                    <Icon className="w-5 h-5 shrink-0" />
                    <span className="text-[0.6rem] font-bold uppercase tracking-tight">{label}</span>
                  </button>
                ))}
              </div>
            </div>

            <div className="flex flex-col gap-3 pt-3 border-t border-border/60">
              <span className="text-[0.68rem] font-black text-muted-foreground uppercase tracking-tight">
                {t('settings.interface.languageLabel')}
              </span>
              <div className="flex items-center justify-center gap-3">
                {(['ru', 'en'] as const).map((lang) => (
                  <button
                    key={lang}
                    type="button"
                    onClick={() => {
                      void setLanguage(lang);
                      patch({ language: lang });
                    }}
                    className={`w-[92px] p-3 rounded-xl flex flex-col items-center gap-1.5 transition-all border-none ${
                      language === lang
                        ? 'bg-primary text-primary-foreground scale-105'
                        : 'bg-[#e6ebf2] dark:bg-white/[0.08] text-muted-foreground hover:bg-[#dde4ed] dark:hover:bg-white/[0.13]'
                    }`}
                  >
                    <span className="text-[0.8rem] font-black">{lang.toUpperCase()}</span>
                    <span className="text-[0.6rem] font-bold uppercase tracking-tight">
                      {t(`settings.interface.${lang}`)}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>
    </PageLayout>
  );
}
