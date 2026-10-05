import { useEffect, useState } from 'react';
import type { RefObject } from 'react';

export interface PopoverStyle {
  position: 'fixed';
  left: number;
  top: number;
  width: number;
}

/**
 * Позиционирование попапов (календарь, списки) как у нативных контролов:
 * `position: fixed` в координатах viewport с клампом по краям окна и
 * флипом вверх, когда снизу не хватает места. Попап рендерится через
 * portal вне скролл-контейнеров, поэтому не растягивает страницу и не
 * вылезает за пределы окна — поведение, отличающее приложение от веба.
 *
 * `width = 0` — растянуть попап по ширине якоря (списки).
 *
 * Дрифт при скролле/ресайзе недопустим: попап закрывается.
 */
export function usePopoverPosition(
  anchorRef: RefObject<HTMLElement | null>,
  open: boolean,
  width: number,
  height: number,
  margin = 8,
): PopoverStyle | null {
  const [style, setStyle] = useState<PopoverStyle | null>(null);

  useEffect(() => {
    if (!open) {
      setStyle(null);
      return;
    }
    const compute = () => {
      const anchor = anchorRef.current;
      if (!anchor) return;
      const rect = anchor.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;

      const popWidth = width > 0 ? width : rect.width;
      let left = rect.left + rect.width / 2 - popWidth / 2;
      left = Math.min(Math.max(margin, left), Math.max(margin, vw - popWidth - margin));

      let top = rect.bottom + 6;
      if (top + height + margin > vh) {
        const flipped = rect.top - height - 6;
        top = flipped >= margin ? flipped : Math.max(margin, vh - height - margin);
      }
      setStyle({ position: 'fixed', left, top, width: popWidth });
    };
    compute();

    const close = () => setStyle(null);
    window.addEventListener('resize', close);
    document.addEventListener('scroll', close, true);
    return () => {
      window.removeEventListener('resize', close);
      document.removeEventListener('scroll', close, true);
    };
  }, [open, anchorRef, width, height, margin]);

  return style;
}
