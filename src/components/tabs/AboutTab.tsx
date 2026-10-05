import { useState } from 'react';
import { motion, AnimatePresence } from 'motion/react';
import { Button } from '../ui/Button';
import { Logo } from '../ui/Logo';
import { useTranslation } from '../../i18n/index.tsx';
import { useAppEnvironment } from '../../hooks/useAppEnvironment';
import { system } from '../../lib/system';

/**
 * Компактная карточка «О программе»: логотип, название, назначение,
 * версия и разработчик. Без панелей и технических деталей — содержимое
 * помещается в диалог без прокрутки.
 */
export function AboutTab() {
  const [showConfirm, setShowConfirm] = useState(false);
  const { t } = useTranslation();
  const env = useAppEnvironment();

  const handleDeveloperClick = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setShowConfirm(true);
  };

  const confirmNavigation = async () => {
    setShowConfirm(false);
    await system.openExternalUrl('https://github.com/KirillGrachev');
  };

  return (
    <motion.div
      initial={false}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: 'easeOut' }}
      className="flex flex-col items-center w-full"
    >
      <div className="flex flex-col items-center text-center gap-3 mb-5 w-full">
        <h2 className="text-[1.15rem] font-bold text-foreground tracking-tight">
          {t('about.title')}
        </h2>
        <div className="w-full h-px bg-border transition-colors" />
      </div>

      <div className="flex flex-col items-center gap-4 text-center w-full">
        <Logo size="lg" />
        <h3 className="font-black text-foreground text-xl uppercase tracking-tight leading-none">
          {t('about.appName')}
        </h3>
        <p className="text-[0.82rem] leading-relaxed text-muted-foreground font-medium max-w-[380px]">
          {t('about.description')}
        </p>

        <div className="flex items-center gap-8 pt-3 border-t border-border w-full justify-center">
          <div className="flex flex-col items-center gap-1">
            <span className="font-bold text-muted-foreground opacity-60 uppercase tracking-normal text-[0.6rem] leading-none">
              {t('about.versionLabel')}
            </span>
            <span className="font-black text-foreground text-[0.9rem] leading-none">
              {env?.version ?? __APP_VERSION__}
            </span>
          </div>
          <div className="w-[1px] h-8 bg-border" />
          <div className="flex flex-col items-center gap-1">
            <span className="font-bold text-muted-foreground opacity-60 uppercase tracking-normal text-[0.6rem] leading-none">
              {t('about.developer')}
            </span>
            <a
              href="https://github.com/KirillGrachev"
              onClick={handleDeveloperClick}
              className="text-foreground hover:text-primary transition-colors font-bold text-[0.85rem] cursor-pointer leading-none"
            >
              {t('about.devName')}
            </a>
            <span className="text-[0.58rem] font-medium text-muted-foreground/60 leading-tight max-w-[190px]">
              {t('about.department')}
            </span>
          </div>
        </div>
      </div>

      <AnimatePresence>
        {showConfirm && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-[200] flex items-center justify-center bg-black/60 p-[20px]"
          >
            <motion.div
              initial={{ opacity: 0, scale: 0.9, y: 20 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.9, y: 20 }}
              className="bg-surface w-full max-w-md rounded-[12px] p-8 shadow-2xl border border-border flex flex-col gap-6"
            >
              <h3 className="text-xl font-bold text-foreground uppercase tracking-tight">
                {t('about.confirmTitle')}
              </h3>
              <p className="text-muted-foreground leading-relaxed text-[0.85rem]">
                {t('about.confirmMsg')}
              </p>
              <div className="flex justify-end gap-3 mt-2">
                <Button variant="outline" onClick={() => setShowConfirm(false)} className="px-6">
                  {t('about.cancelBtn')}
                </Button>
                <Button variant="primary" onClick={() => void confirmNavigation()} className="px-6">
                  {t('about.goBtn')}
                </Button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
