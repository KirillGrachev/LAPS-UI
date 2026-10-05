import { useState } from 'react';
import { Eye, EyeOff, CheckCircle2, Copy, Monitor, Globe, UserCog, Clock, Sparkles } from 'lucide-react';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import { motion, AnimatePresence } from 'motion/react';
import type { LapsInfo } from '../../types';
import { useTranslation } from '../../i18n/index.tsx';
import { expirationStatus, formatDateTime } from '../../lib/format';

export interface PasswordResultProps {
  result: LapsInfo | null;
  copyPassword: () => void;
}

/** Строка статуса срока истечения (цвет подсказывает срочность). */
function ExpirationStatusLine({ result }: { result: LapsInfo }) {
  const { t, language } = useTranslation();
  const status = expirationStatus(result.expiresAt);

  if (status.kind === 'notSet') {
    return (
      <span className="text-[0.75rem] font-bold text-muted-foreground">
        {t('search.expirationStatus.resetPending')}
      </span>
    );
  }

  const date = formatDateTime(result.expiresAt, language);
  let color = 'text-emerald-600 dark:text-emerald-400';
  let text = t('search.expirationStatus.expiresAt', { date });
  if (status.kind === 'expired') {
    color = 'text-red-500';
    text = t('search.expirationStatus.expired', { date });
  } else if (status.kind === 'today') {
    color = 'text-amber-500';
    text = t('search.expirationStatus.expiresSoon');
  } else if (status.kind === 'soon') {
    color = 'text-amber-500';
    text = `${t('search.expirationStatus.expiresIn', { days: status.days })} · ${date}`;
  }

  return (
    <span className={`text-[0.75rem] font-bold ${color}`}>
      <Clock size={12} className="inline mr-1.5 -mt-0.5" />
      {text}
    </span>
  );
}

/**
 * Карточка результата: метаинформация о компьютере, пароль
 * (маска/показ/копирование) и диагностика для шифрованного хранения.
 */
export function PasswordResult({ result, copyPassword }: PasswordResultProps) {
  const { t, language } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [copied, setCopied] = useState(false);

  if (!result) return null;

  const handleCopy = () => {
    void copyPassword();
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.25, ease: 'easeOut' }}
      className="w-full bg-surface rounded-2xl p-5 sm:p-6 flex flex-col gap-5"
    >
      <div className="flex items-start justify-between gap-4 flex-wrap">
        <div className="flex flex-col gap-1 min-w-0">
          <div className="flex items-center gap-2.5 flex-wrap">
            <Monitor size={16} className="text-primary shrink-0" />
            <span className="text-[1.05rem] font-black text-foreground tracking-tight uppercase truncate">
              {result.computerName}
            </span>
          </div>
          <div className="flex items-center gap-4 text-[0.72rem] text-muted-foreground font-medium flex-wrap">
            {result.dnsHostName && (
              <span className="inline-flex items-center gap-1">
                <Globe size={11} className="opacity-60" />
                {result.dnsHostName}
              </span>
            )}
            {result.operatingSystem && <span>{result.operatingSystem}</span>}
          </div>
        </div>
        <ExpirationStatusLine result={result} />
      </div>

      {result.decrypted && result.password !== null && (
        <div className="flex items-center gap-2 text-[0.72rem] font-bold text-primary">
          <Sparkles size={13} className="shrink-0" />
          {t(result.decryptMethod === 'kds' ? 'search.encrypted.decryptedNoteKds' : 'search.encrypted.decryptedNoteModule')}
        </div>
      )}

      {(result.password !== null || result.flavor === 'windowsEncrypted') && (
        <div className="flex flex-col gap-[8px]">
          <div className="flex items-center justify-between">
            <label
              htmlFor="currentPassword"
              className="text-[0.72rem] font-bold text-muted-foreground uppercase tracking-tight transition-colors"
            >
              {t('search.passwordLabel')}
            </label>
            {result.accountName && (
              <span className="inline-flex items-center gap-1.5 text-[0.7rem] font-bold text-muted-foreground">
                <UserCog size={12} className="opacity-60" />
                {t('search.labels.account')}: {result.accountName}
              </span>
            )}
          </div>
          <div className="flex flex-col sm:flex-row gap-[8px] sm:gap-[10px]">
            <div className="relative flex-1">
              <Input
                id="currentPassword"
                type="text"
                className={`w-full h-[40px] sm:h-[44px] text-[1rem] md:text-[1.05rem] font-mono font-bold tracking-wide text-center ${
                  result.password === null
                    ? 'pr-4 text-muted-foreground/70 font-sans text-[0.85rem] select-none'
                    : 'pr-12'
                }`}
                value={
                  result.password === null
                    ? t('search.encrypted.notDecrypted')
                    : isVisible
                      ? result.password
                      : '•'.repeat(Math.min(result.password.length, 24))
                }
                readOnly
                tabIndex={0}
              />
              {result.password !== null && (
                <button
                  type="button"
                  onClick={() => setIsVisible(!isVisible)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 p-2 text-muted-foreground hover:text-foreground transition-colors"
                  title={isVisible ? t('app.hide') : t('app.show')}
                >
                  {isVisible ? <EyeOff size={18} strokeWidth={2.2} /> : <Eye size={18} strokeWidth={2.2} />}
                </button>
              )}
            </div>
            <Button
              variant={copied ? 'primary' : 'outline'}
              type="button"
              onClick={handleCopy}
              disabled={!result.password}
              className={`relative font-black h-[40px] sm:h-[44px] px-8 transition-all w-full sm:w-[160px] sm:min-w-[160px] flex-shrink-0 ${
                copied ? 'bg-green-600 hover:bg-green-700 border-transparent text-white shadow-lg shadow-green-600/20' : ''
              }`}
            >
              <AnimatePresence mode="wait" initial={false}>
                {copied ? (
                  <motion.div
                    key="copied"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    transition={{ duration: 0.2, ease: 'easeOut' }}
                    className="flex items-center gap-2 justify-center"
                  >
                    <CheckCircle2 size={16} strokeWidth={3} />
                    <span className="whitespace-nowrap">{t('app.ready')}</span>
                  </motion.div>
                ) : (
                  <motion.div
                    key="copy"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    transition={{ duration: 0.2, ease: 'easeOut' }}
                    className="flex items-center gap-2 justify-center"
                  >
                    <Copy size={16} strokeWidth={3} />
                    <span className="whitespace-nowrap">{t('search.copyBtn')}</span>
                  </motion.div>
                )}
              </AnimatePresence>
            </Button>
          </div>
          {result.updatedAt && (
            <span className="text-[0.68rem] text-muted-foreground/70 font-medium text-center sm:text-left">
              {t('search.labels.updated')}: {formatDateTime(result.updatedAt, language)}
            </span>
          )}
        </div>
      )}
    </motion.div>
  );
}
