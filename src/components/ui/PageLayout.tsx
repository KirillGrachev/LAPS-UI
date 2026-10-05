import React from 'react';
import { TabWrapper } from './TabWrapper';

interface PageLayoutProps {
  id: string;
  title: string;
  subtitle?: string;
  rightElement?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  contentClassName?: string;
}

export function PageLayout({
  id,
  title,
  subtitle,
  rightElement,
  children,
  className = '',
  contentClassName = '',
}: PageLayoutProps) {
  return (
    <TabWrapper id={id} className={`w-full h-full flex flex-col overflow-hidden ${className}`}>
      <div className="bg-surface p-[20px] pl-[56px] lg:pl-[28px] text-foreground flex items-center justify-between shrink-0 transition-colors border-b border-border">
        <div className="flex flex-col gap-1 min-w-0">
          <h1 className="text-[1.3rem] font-bold tracking-tight text-foreground truncate">{title}</h1>
          {subtitle && (
            <p className="text-[0.75rem] font-medium text-muted-foreground truncate">{subtitle}</p>
          )}
        </div>
        {rightElement && (
          <div className="hidden sm:flex items-center gap-[10px] uppercase tracking-normal text-[0.85rem] font-bold text-muted-foreground">
            {rightElement}
          </div>
        )}
      </div>

      <div className={`flex-1 flex flex-col relative min-h-0 bg-background ${contentClassName}`}>
        {children}
      </div>
    </TabWrapper>
  );
}
