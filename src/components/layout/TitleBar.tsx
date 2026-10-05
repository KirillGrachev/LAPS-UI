import { useEffect, useState } from 'react';
import { Minus, Menu, Square, X } from 'lucide-react';
import { system } from '../../lib/system';
import { useTranslation } from '../../i18n/index.tsx';

interface TitleBarProps {
  title: string;
  isMobileMenuOpen: boolean;
  toggleMobileMenu: () => void;
}

/**
 * Заголовок окна: drag-регион, название раздела, мобильное меню
 * и кнопки управления окном.
 *
 * Окно изменяемого размера — состояние «развёрнуто» отслеживается
 * для корректной иконки кнопки (развернуть/восстановить).
 * В браузере кнопки управления окном скрываются, в Tauri — показываются.
 */
export function TitleBar({ title, isMobileMenuOpen, toggleMobileMenu }: TitleBarProps) {
  const { t } = useTranslation();
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    if (!system.isTauri()) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;

    (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const win = getCurrentWindow();
        setIsMaximized(await win.isMaximized());
        const stop = await win.onResized(async () => {
          setIsMaximized(await win.isMaximized());
        });
        if (disposed) {
          stop();
        } else {
          unlisten = stop;
        }
      } catch (e) {
        console.warn('TitleBar maximize tracking failed', e);
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <div
      data-tauri-drag-region
      className="relative h-[32px] bg-surface shrink-0 z-[60] flex items-center justify-between select-none border-b border-border"
    >
      <div className="flex items-center h-full flex-1">
        <div data-tauri-drag-region="false" className="lg:hidden h-full flex items-center">
          <button
            onClick={toggleMobileMenu}
            className="w-[46px] h-full flex items-center justify-center hover:bg-foreground/5 text-foreground/40 hover:text-foreground transition-colors"
            title={isMobileMenuOpen ? t('app.close') : t('app.menu')}
          >
            {isMobileMenuOpen ? <X size={16} /> : <Menu size={16} />}
          </button>
        </div>

        <div
          data-tauri-drag-region
          style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
          className="pl-3 flex items-center flex-1 h-full cursor-default"
          onDoubleClick={() => void system.toggleMaximizeWindow()}
        >
          <span className="text-[0.65rem] font-bold text-foreground/40 uppercase tracking-normal">
            {title}
          </span>
        </div>
      </div>

      {system.isTauri() && (
        <div
          data-tauri-drag-region="false"
          style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}
          className="flex items-center h-full"
        >
          <button
            id="win-minimize"
            onClick={() => void system.minimizeWindow()}
            className="w-[46px] h-full flex items-center justify-center hover:bg-foreground/5 text-foreground/40 hover:text-foreground transition-colors"
            title={t('app.minimize')}
          >
            <Minus size={14} />
          </button>
          <button
            id="win-maximize"
            onClick={() => void system.toggleMaximizeWindow()}
            className="w-[46px] h-full flex items-center justify-center hover:bg-foreground/5 text-foreground/40 hover:text-foreground transition-colors"
            title={isMaximized ? t('app.restore') : t('app.maximize')}
          >
            {isMaximized ? (
              /* Иконка «восстановить»: два наложенных контура */
              <svg width="11" height="11" viewBox="0 0 11 11" fill="none" stroke="currentColor" strokeWidth="1.1">
                <rect x="0.5" y="2.5" width="8" height="8" />
                <path d="M2.5 2.5 v-2 h8 v8 h-2" />
              </svg>
            ) : (
              <Square size={11} strokeWidth={1.8} />
            )}
          </button>
          <button
            id="win-close"
            onClick={() => void system.closeWindow()}
            className="w-[46px] h-full flex items-center justify-center hover:bg-[#e81123] text-foreground/40 hover:text-white transition-colors"
            title={t('app.close')}
          >
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
