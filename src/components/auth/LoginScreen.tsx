import { FormEvent, useState } from 'react';
import { toast } from 'sonner';
import { Loader2, Eye, EyeOff, ArrowBigUp, FlaskConical } from 'lucide-react';
import { motion } from 'motion/react';
import { Logo } from '../ui/Logo';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import { useTranslation } from '../../i18n/index.tsx';
import { useSession } from '../../context/SessionContext';
import { useConfig } from '../../context/ConfigContext';
import { useAppEnvironment } from '../../hooks/useAppEnvironment';
import { useErrorHandler } from '../../hooks/useErrorHandler';
import { tauriInvoke } from '../../lib/ipc';
import type { SessionInfo } from '../../types';

interface LoginScreenProps {
  onOpenSettings: () => void;
}

/**
 * Экран входа: минимализм старой школы — логин, пароль, «Войти».
 *
 * Безопасность: аутентификация при каждом запуске (настоящий LDAP-bind),
 * поля без подсказок и автозаполнения, учётные данные не сохраняются.
 * SSO под сессией Windows — опциональная маленькая ссылка ниже формы
 * (только в Windows-сборке).
 * В тестовом режиме вход демонстрационный — выводится подсказка, чтобы
 * не принимать демо-сессию за настоящую.
 */
export function LoginScreen({ onOpenSettings }: LoginScreenProps) {
  const { t } = useTranslation();
  const { setSession } = useSession();
  const { config } = useConfig();
  const env = useAppEnvironment();
  const { getMessage } = useErrorHandler();

  const [account, setAccount] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [capsOn, setCapsOn] = useState(false);
  const [isLoggingIn, setIsLoggingIn] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const trackCaps = (e: React.KeyboardEvent<HTMLInputElement>) => {
    setCapsOn(e.getModifierState('CapsLock'));
  };

  const enter = async (sso: boolean, e?: FormEvent) => {
    e?.preventDefault();
    if (isLoggingIn) return;
    setIsLoggingIn(true);
    setError(null);
    try {
      const session = await tauriInvoke<SessionInfo>('login', {
        account: sso ? null : account,
        password: sso ? null : password,
        sso,
      });
      setSession(session);
    } catch (err) {
      const message = getMessage(err);
      setError(message);
      toast.error(message, { id: 'login-error', duration: 8000 });
    } finally {
      setIsLoggingIn(false);
      setPassword('');
    }
  };

  return (
    <div className="flex-1 flex items-center justify-center bg-background overflow-y-auto scrollbar-hide p-6">
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.25, ease: 'easeOut' }}
        className="w-full max-w-[360px] flex flex-col items-center gap-6"
      >
        <Logo size="lg" />

        {config.testMode && (
          <span className="-mt-2 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-amber-500/10 text-amber-600 dark:text-amber-400 text-[0.68rem] font-black text-center">
            <FlaskConical size={12} className="shrink-0" aria-hidden />
            {t('auth.testModeNote')}
          </span>
        )}

        <form onSubmit={(e) => void enter(false, e)} className="w-full flex flex-col gap-4">
          <div className="flex flex-col gap-1.5">
            <label
              htmlFor="login-account"
              className="text-[0.68rem] font-black text-muted-foreground uppercase tracking-tight text-center"
            >
              {t('auth.accountLabel')}
            </label>
            <Input
              id="login-account"
              type="text"
              value={account}
              onChange={(e) => setAccount(e.target.value)}
              disabled={isLoggingIn}
              className="w-full h-[42px] text-center font-bold placeholder:normal-case placeholder:font-medium"
              placeholder={t('auth.accountPlaceholder')}
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              spellCheck={false}
              name="laps-login-account"
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <label
              htmlFor="login-password"
              className="text-[0.68rem] font-black text-muted-foreground uppercase tracking-tight text-center"
            >
              {t('auth.passwordLabel')}
            </label>
            <div className="relative">
              <Input
                id="login-password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                onKeyUp={trackCaps}
                onKeyDown={trackCaps}
                disabled={isLoggingIn}
                className="w-full h-[42px] text-center font-bold px-11 placeholder:font-medium"
                placeholder={t('auth.passwordPlaceholder')}
                autoComplete="new-password"
                autoCorrect="off"
                spellCheck={false}
                name="laps-login-password"
              />
              <button
                type="button"
                tabIndex={-1}
                onClick={() => setShowPassword((v) => !v)}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 p-1.5 text-muted-foreground hover:text-foreground transition-colors"
                title={showPassword ? t('app.hide') : t('app.show')}
              >
                {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
            {capsOn && (
              <span className="inline-flex items-center justify-center gap-1.5 text-[0.66rem] font-bold text-amber-500">
                <ArrowBigUp size={12} className="shrink-0" />
                {t('auth.capsLock')}
              </span>
            )}
          </div>
          <Button
            type="submit"
            disabled={isLoggingIn || !account.trim() || !password}
            className="w-full h-[42px] font-black"
          >
            {isLoggingIn ? <Loader2 size={15} className="animate-spin" /> : null}
            {isLoggingIn ? t('auth.loggingIn') : t('auth.loginButton')}
          </Button>
        </form>

        {error && (
          <p className="text-[0.72rem] font-bold text-red-500 text-center leading-snug break-words -mt-2">
            {error}
          </p>
        )}

        <div className="flex flex-col items-center gap-2 -mt-2">
          {env?.integratedAuthAvailable && (
            <button
              type="button"
              onClick={() => void enter(true)}
              disabled={isLoggingIn}
              className="text-[0.68rem] font-bold text-muted-foreground hover:text-primary transition-colors disabled:opacity-50"
            >
              {t('auth.ssoLink')}
            </button>
          )}
          <button
            type="button"
            onClick={onOpenSettings}
            className="text-[0.68rem] font-bold text-muted-foreground/60 hover:text-primary transition-colors"
          >
            {t('auth.settingsLink')}
          </button>
        </div>
      </motion.div>
    </div>
  );
}
