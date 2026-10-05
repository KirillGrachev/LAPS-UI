import { Search, ScrollText, Settings, Info, X, ShieldCheck, LogOut, FlaskConical } from 'lucide-react';
import * as Dialog from '@radix-ui/react-dialog';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';
import { useTranslation } from '../../i18n/index.tsx';
import { useConfig } from '../../context/ConfigContext';
import { useSession } from '../../context/SessionContext';
import { AboutTab } from '../tabs/AboutTab';
import { Logo } from '../ui/Logo';

interface SidebarProps {
  activeTab: string;
  setActiveTab: (id: string) => void;
  isMobileMenuOpen: boolean;
  closeMobileMenu: () => void;
}

export function Sidebar({ activeTab, setActiveTab, isMobileMenuOpen, closeMobileMenu }: SidebarProps) {
  const { t } = useTranslation();
  const { config } = useConfig();
  const { session, logout } = useSession();
  const [isAboutOpen, setIsAboutOpen] = useState(false);

  const NAV_ITEMS = [
    { id: 'search', label: t('app.searchTab'), icon: Search },
    { id: 'audit', label: t('app.auditTab'), icon: ScrollText },
    { id: 'settings', label: t('app.settingsTab'), icon: Settings },
  ];

  const navItemActive =
    'px-[20px] py-[12px] flex items-center gap-[10px] bg-primary/10 text-primary border-l-[3px] border-primary text-[0.85rem] font-bold uppercase tracking-normal cursor-pointer transition-all duration-300 w-full text-left';
  const navItemInactive =
    'px-[20px] py-[12px] flex items-center gap-[10px] text-foreground/50 hover:text-foreground hover:bg-foreground/5 text-[0.85rem] font-bold uppercase tracking-normal cursor-pointer transition-all duration-300 border-l-[3px] border-transparent w-full text-left';

  /** Краткое доменное имя из настроек — для статусной карточки. */
  const displayDomain = (() => {
    if (config.baseDn) {
      const match = config.baseDn.match(/DC=([^,]+)/i);
      if (match?.[1]) return match[1].toUpperCase();
    }
    if (config.ldapUrl) {
      const host = config.ldapUrl.replace(/^ldaps?:\/\//i, '').split('/')[0];
      const parts = host.split('.');
      if (parts.length > 2) return parts[1].toUpperCase();
      if (parts[0]) return parts[0].toUpperCase();
    }
    return '—';
  })();

  const identity = session?.actor ?? '—';

  return (
    <>
      <div
        className={`
          fixed lg:static top-[32px] bottom-0 left-0 lg:inset-y-0 w-[240px] bg-surface text-foreground flex flex-col shrink-0 z-50 border-r border-border
          transition-transform duration-500 ease-in-out lg:translate-x-0 min-h-0
          ${isMobileMenuOpen ? 'translate-x-0' : '-translate-x-full'}
        `}
      >
        <div className="p-6 border-b border-border flex flex-col items-center gap-3 shrink-0">
          <Logo size="md" />
          <span className="text-foreground font-black text-sm tracking-normal uppercase text-center opacity-80">
            {t('app.title')}
          </span>
        </div>

        <div className="flex-1 py-5 flex flex-col gap-1 min-h-0">
          {NAV_ITEMS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className={activeTab === id ? navItemActive : navItemInactive}
              onClick={() => {
                setActiveTab(id);
                closeMobileMenu();
              }}
              title={label}
            >
              <Icon
                className={
                  activeTab === id
                    ? 'w-[16px] h-[16px] shrink-0 text-primary'
                    : 'w-[16px] h-[16px] shrink-0 opacity-40'
                }
              />
              <span className="truncate">{label}</span>
            </button>
          ))}
        </div>

        <div className="mt-auto p-4 flex flex-col gap-2 shrink-0 border-t border-border bg-surface/50">
          {config.testMode && (
            <div className="flex items-center gap-2 rounded-lg bg-amber-500/10 px-3 py-2 text-amber-600 dark:text-amber-400">
              <FlaskConical size={12} className="shrink-0" aria-hidden />
              <span className="text-[0.62rem] font-black uppercase tracking-tight truncate">
                {t('app.testModeBadge')}
              </span>
            </div>
          )}
          <div className="bg-background/50 rounded-lg p-4 mb-1 flex flex-col gap-3 border border-border">
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-foreground/5 flex items-center justify-center border border-border shrink-0">
                <ShieldCheck size={14} className="text-primary" />
              </div>
              <div className="flex flex-col min-w-0">
                <span className="text-[0.7rem] font-black text-foreground leading-tight truncate">
                  {identity}
                </span>
                <span className="text-[0.6rem] text-foreground/40 uppercase tracking-tighter font-bold truncate">
                  {displayDomain}
                  {session?.integrated ? ' · SSO' : ' · account'}
                </span>
              </div>
            </div>
            <button
              onClick={() => void logout()}
              className="flex items-center justify-center gap-2 w-full py-2 px-3 bg-foreground/5 hover:bg-red-500/10 text-foreground/60 hover:text-red-500 rounded-lg transition-all duration-300 text-[0.65rem] font-black uppercase tracking-tight group"
            >
              <LogOut size={12} className="group-hover:scale-110 transition-transform shrink-0" />
              <span>{t('auth.logout')}</span>
            </button>
            {!config.ldapUrl && (
              <button
                onClick={() => {
                  setActiveTab('settings');
                  closeMobileMenu();
                }}
                className="text-[0.62rem] font-bold text-amber-500 hover:text-amber-400 transition-colors text-left leading-snug"
              >
                ⚠ {t('errors.CONFIG_ERROR')}
              </button>
            )}
          </div>

          <Dialog.Root open={isAboutOpen} onOpenChange={setIsAboutOpen}>
            <Dialog.Trigger asChild>
              <button className="flex items-center gap-3 px-4 py-3 rounded-lg hover:bg-foreground/5 transition-all duration-300 group text-foreground/50 hover:text-foreground w-full">
                <Info size={16} className="opacity-40 group-hover:opacity-100" />
                <span className="text-[0.7rem] font-bold uppercase tracking-tight">
                  {t('app.aboutTab')}
                </span>
              </button>
            </Dialog.Trigger>
            <AnimatePresence>
              {isAboutOpen && (
                <Dialog.Portal forceMount>
                  <Dialog.Overlay key="overlay" asChild>
                    <motion.div
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.2 }}
                      className="fixed inset-0 bg-black/60 backdrop-blur-sm z-[100]"
                    />
                  </Dialog.Overlay>
                  <Dialog.Content
                    key="content"
                    asChild
                    onOpenAutoFocus={(e) => e.preventDefault()}
                  >
                    <div className="fixed inset-0 z-[101] flex items-center justify-center p-4 outline-none pointer-events-none">
                      <motion.div
                        initial={{ opacity: 0, scale: 0.95 }}
                        animate={{ opacity: 1, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.95 }}
                        transition={{
                          type: 'spring',
                          damping: 30,
                          stiffness: 300,
                        }}
                        className="w-full max-w-lg outline-none pointer-events-auto"
                        style={{ willChange: 'transform', transform: 'translateZ(0)' }}
                      >
                        <div className="bg-surface rounded-xl p-6 sm:p-8 shadow-2xl border border-border overflow-y-auto scrollbar-hide relative group max-h-[85vh] flex flex-col">
                          <div className="sr-only">
                            <Dialog.Title>{t('app.aboutTab')}</Dialog.Title>
                            <Dialog.Description>{t('about.description')}</Dialog.Description>
                          </div>
                          <div className="absolute top-0 right-0 p-4 z-10">
                            <Dialog.Close asChild>
                              <button className="p-2 hover:bg-surface-hover rounded-xl transition-all text-muted-foreground active:scale-95">
                                <X size={18} />
                              </button>
                            </Dialog.Close>
                          </div>
                          <AboutTab />
                        </div>
                      </motion.div>
                    </div>
                  </Dialog.Content>
                </Dialog.Portal>
              )}
            </AnimatePresence>
          </Dialog.Root>

          <div className="px-5 py-3 text-[0.6rem] text-foreground/20 border-t border-border tracking-normal uppercase font-bold text-center">
            {t('app.copyright')}
          </div>
        </div>
      </div>

      <AnimatePresence>
        {isMobileMenuOpen && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={closeMobileMenu}
            className="lg:hidden fixed inset-0 bg-black/50 z-40 backdrop-blur-sm"
          />
        )}
      </AnimatePresence>
    </>
  );
}
