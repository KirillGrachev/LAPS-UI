import { ButtonHTMLAttributes, forwardRef } from 'react';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'outline' | 'ghost' | 'danger';
}

/**
 * Минималистичные кнопки без обводок и теней: разделение слоёв —
 * только фоном (требование дизайна продукта).
 */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className = '', variant = 'primary', ...props }, ref) => {
    const baseClasses =
      'sm:w-auto w-full px-[24px] py-[10px] rounded-[8px] font-semibold text-[0.9rem] flex items-center justify-center gap-2 transition-colors duration-150 shrink-0 select-none enabled:cursor-pointer disabled:cursor-not-allowed disabled:opacity-40 outline-none border-none';

    const variants = {
      primary: 'bg-primary text-primary-foreground enabled:hover:bg-primary-hover',
      outline:
        'bg-[#eef2f7] dark:bg-white/[0.07] text-foreground enabled:hover:bg-[#e2e8f0] dark:enabled:hover:bg-white/[0.12]',
      ghost:
        'bg-transparent text-foreground enabled:hover:bg-[#eef2f7] dark:enabled:hover:bg-white/[0.07]',
      danger:
        'bg-red-500/10 text-red-500 enabled:hover:bg-red-500/[0.16]',
    };

    return (
      <button
        ref={ref}
        className={`${baseClasses} ${variants[variant]} ${className}`}
        {...props}
      />
    );
  },
);

Button.displayName = 'Button';
