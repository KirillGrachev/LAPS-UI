import { useState, useEffect, lazy, Suspense } from 'react';
import { Toaster } from 'sonner';
import { ThemeProvider, useTheme } from './context/ThemeContext';
import { ConfigProvider } from './context/ConfigContext';
import { SearchProvider } from './context/SearchContext';
import { SessionProvider, useSession } from './context/SessionContext';
import { I18nProvider, useTranslation } from './i18n/index.tsx';
import { TitleBar } from './components/layout/TitleBar';
import { Sidebar } from './components/layout/Sidebar';
import { ErrorBoundary } from './components/ErrorBoundary';
import { LoginScreen } from './components/auth/LoginScreen';
import { Logo } from './components/ui/Logo';
import { system } from './lib/system';

const SearchTab = lazy(() =>
  import('./components/tabs/SearchTab').then((m) => ({ default: m.SearchTab })),
);
const AuditTab = lazy(() =>
  import('./components/tabs/AuditTab').then((m) => ({ default: m.AuditTab })),
);
const SettingsTab = lazy(() =>
  import('./components/tabs/SettingsTab').then((m) => ({ default: m.SettingsTab })),
);

function MainContent() {
  const [activeTab, setActiveTab] = useState('search');
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const { t } = useTranslation();

  const toggleMobileMenu = () => setIsMobileMenuOpen(!isMobileMenuOpen);
  const closeMobileMenu = () => setIsMobileMenuOpen(false);

  return (
    <>
      <TitleBar
        title={t('app.title')}
        isMobileMenuOpen={isMobileMenuOpen}
        toggleMobileMenu={toggleMobileMenu}
      />
      <div className="flex flex-1 lg:flex-row overflow-hidden relative">
        <Sidebar
          activeTab={activeTab}
          setActiveTab={setActiveTab}
          isMobileMenuOpen={isMobileMenuOpen}
          closeMobileMenu={closeMobileMenu}
        />
        <div className="flex-1 flex flex-col min-w-0 bg-background overflow-hidden relative z-0 border-t border-border">
          <main className="flex-1 h-full overflow-auto scrollbar-hide flex flex-col">
            <Suspense fallback={<div className="flex-1" />}>
              {activeTab === 'search' && <SearchTab />}
              {activeTab === 'audit' && <AuditTab />}
              {activeTab === 'settings' && <SettingsTab />}
            </Suspense>
          </main>
        </div>
      </div>
    </>
  );
}

/**
 * Корневой контент: до входа показывается экран аутентификации
 * (или настройки подключения до входа), после входа — основной интерфейс.
 * После успешного входа пред-авторизационный режим настроек сбрасывается.
 */
function AppContent() {
  const { session, checking } = useSession();
  const { t } = useTranslation();
  const [preAuthSettings, setPreAuthSettings] = useState(false);

  useEffect(() => {
    if (session) setPreAuthSettings(false);
  }, [session]);

  if (checking) {
    return (
      <div className="fixed inset-0 bg-background flex flex-col items-center justify-center gap-4 select-none">
        <Logo size="md" />
        <span className="text-[0.7rem] font-black text-muted-foreground uppercase tracking-tight">
          {t('app.loading')}
        </span>
      </div>
    );
  }

  if (!session) {
    return (
      <div className="fixed inset-0 bg-background text-foreground font-sans flex flex-col overflow-hidden select-none">
        <TitleBar title={t('app.title')} isMobileMenuOpen={false} toggleMobileMenu={() => {}} />
        {preAuthSettings ? (
          <div className="flex-1 flex flex-col min-h-0">
            <SettingsTab preAuth onBackToLogin={() => setPreAuthSettings(false)} />
          </div>
        ) : (
          <LoginScreen onOpenSettings={() => setPreAuthSettings(true)} />
        )}
      </div>
    );
  }

  return (
    <div className="fixed inset-0 bg-background text-foreground font-sans flex flex-col overflow-hidden select-none">
      <MainContent />
    </div>
  );
}

function MainApp() {
  const { resolvedTheme } = useTheme();
  return (
    <>
      <AppContent />
      <Toaster
        position="bottom-right"
        theme={resolvedTheme}
        duration={4000}
        closeButton={true}
        richColors={true}
        visibleToasts={3}
        toastOptions={{
          className:
            'shadow-2xl rounded-lg bg-surface text-foreground text-[0.85rem] font-bold p-4 z-[99999]',
          style: { fontFamily: 'inherit' },
        }}
        expand={false}
      />
    </>
  );
}

/**
 * Обёртка приложения: отключение браузерных поведений webview (продукт,
 * а не сайт: Ctrl+R/F5/зум, перетаскивание, контекстное меню, автопрокрутка
 * средней кнопкой мыши) и показ окна после гидрации (окно стартует невидимым;
 * на бэкенде есть страховочный таймер на случай сбоя JS).
 */
export default function App() {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const key = e.key.toLowerCase();
      if ((e.ctrlKey || e.metaKey) && ['r', 'p', 's', 'u'].includes(key)) e.preventDefault();
      if (e.key === 'F5') e.preventDefault();
      if ((e.ctrlKey || e.metaKey) && ['+', '=', '-', '0'].includes(e.key)) e.preventDefault();
    };
    const handleWheel = (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) e.preventDefault();
    };
    const handleDragStart = (e: DragEvent) => e.preventDefault();
    const handleContextMenu = (e: MouseEvent) => e.preventDefault();
    const handleMouseDown = (e: MouseEvent) => {
      if (e.button === 1) e.preventDefault();
    };

    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('wheel', handleWheel, { passive: false });
    document.addEventListener('dragstart', handleDragStart);
    document.addEventListener('contextmenu', handleContextMenu, { capture: true });
    document.addEventListener('mousedown', handleMouseDown);

    const showTimer = setTimeout(() => {
      void system.showMainWindow();
    }, 250);

    return () => {
      clearTimeout(showTimer);
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('wheel', handleWheel);
      document.removeEventListener('dragstart', handleDragStart);
      document.removeEventListener('contextmenu', handleContextMenu, { capture: true });
      document.removeEventListener('mousedown', handleMouseDown);
    };
  }, []);

  return (
    <ErrorBoundary>
      <I18nProvider>
        <ConfigProvider>
          <ThemeProvider>
            <SessionProvider>
              <SearchProvider>
                <MainApp />
              </SearchProvider>
            </SessionProvider>
          </ThemeProvider>
        </ConfigProvider>
      </I18nProvider>
    </ErrorBoundary>
  );
}
