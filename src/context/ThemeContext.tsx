import React, { createContext, useContext, useEffect, useRef, useState } from 'react';
import { useConfig } from './ConfigContext';
import type { Theme } from '../types';

interface ThemeContextType {
  theme: Theme;
  resolvedTheme: 'light' | 'dark';
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

/**
 * Тема полностью управляется конфигурацией бэкенда (`config.theme`):
 * смена темы в настройках сохраняет конфиг и применяется здесь.
 */
export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const { config, isLoaded } = useConfig();
  const theme = config.theme;
  const [resolvedTheme, setResolvedTheme] = useState<'light' | 'dark'>('light');
  const isInitialMount = useRef(true);

  useEffect(() => {
    if (!isLoaded) return;

    const root = window.document.documentElement;
    const updateTheme = () => {
      let currentResolved: 'light' | 'dark' = 'light';

      if (theme === 'system') {
        currentResolved = window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light';
      } else {
        currentResolved = theme === 'dark' ? 'dark' : 'light';
      }

      setResolvedTheme(currentResolved);

      const isChanging = (currentResolved === 'dark') !== root.classList.contains('dark');
      if (isChanging && !isInitialMount.current) {
        root.classList.add('theme-transitioning');
        setTimeout(() => root.classList.remove('theme-transitioning'), 500);
      }

      if (currentResolved === 'dark') {
        root.classList.add('dark');
      } else {
        root.classList.remove('dark');
      }
    };

    updateTheme();
    isInitialMount.current = false;

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = () => {
      if (theme === 'system') updateTheme();
    };
    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  }, [theme, isLoaded]);

  return (
    <ThemeContext.Provider value={{ theme, resolvedTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (context === undefined) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }
  return context;
}
