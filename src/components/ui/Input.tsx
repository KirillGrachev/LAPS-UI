import { InputHTMLAttributes, forwardRef } from 'react';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {}

/**
 * Поле ввода без обводки: слой обозначается только фоном.
 * Фокус — лёгкое осветление фона, без ring/outline.
 */
export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className = '', readOnly, ...props }, ref) => {
    const baseClasses =
      'bg-[#e6ebf2] dark:bg-white/[0.08] px-[14px] py-[10px] border-none rounded-[8px] text-[0.95rem] font-medium outline-none transition-colors duration-200 focus:bg-white dark:focus:bg-white/[0.13] disabled:opacity-60 disabled:cursor-not-allowed text-[#1e293b] dark:text-white placeholder:font-bold placeholder:uppercase placeholder:text-muted-foreground/50';

    return (
      <input ref={ref} readOnly={readOnly} className={`${baseClasses} ${className}`} {...props} />
    );
  },
);

Input.displayName = 'Input';
