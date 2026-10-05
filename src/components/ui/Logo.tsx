interface LogoProps {
  className?: string;
  size?: 'sm' | 'md' | 'lg' | 'xl';
}

/**
 * Логотип приложения — бренд-иконка KMAruda LAPS (public/app-icon.png),
 * единая во всех местах: сайдбар, «О программе», окно, установщик.
 */
export function Logo({ className = '', size = 'md' }: LogoProps) {
  const sizeClasses = {
    sm: 'w-8 h-8 rounded-lg',
    md: 'w-16 h-16 rounded-2xl',
    lg: 'w-24 h-24 rounded-3xl',
    xl: 'w-32 h-32 rounded-3xl',
  };

  return (
    <div
      className={`relative overflow-hidden flex items-center justify-center shadow-lg shrink-0 ${sizeClasses[size]} ${className}`}
    >
      <img
        src="/app-icon.png"
        alt="KMAruda LAPS"
        className="w-full h-full object-contain select-none"
        draggable={false}
      />
    </div>
  );
}
