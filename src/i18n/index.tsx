import { ReactNode } from 'react';
import i18n from 'i18next';
import { initReactI18next, useTranslation as useI18nTranslation } from 'react-i18next';
import { ru } from './ru';
import { en } from './en';

i18n
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      ru: { translation: ru },
    },
    lng: 'ru',
    fallbackLng: 'en',
    interpolation: {
      escapeValue: false,
    },
  });

export type Language = 'ru' | 'en';

/** Опции интерполяции i18next ({{seconds}}, {{date}} и т.п.). */
export type TranslateOptions = Record<string, unknown>;

type Join<K, P> = K extends string | number
  ? P extends string | number
    ? `${K}${'' extends P ? '' : '.'}${P}`
    : never
  : never;

type Prev = [never, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

type Leaves<T, D extends number = 10> = [D] extends [never]
  ? never
  : T extends object
    ? {
        [K in keyof T]-?: K extends string | number
          ? T[K] extends object
            ? Join<K, Leaves<T[K], Prev[D]>>
            : `${K}`
          : never;
      }[keyof T]
    : '';

export type TranslationKeys = Leaves<typeof en>;

export function I18nProvider({ children }: { children: ReactNode }) {
  return <>{children}</>;
}

export const t = (key: TranslationKeys | string, options?: TranslateOptions) => {
  return i18n.t(key as string, options);
};

export function useTranslation() {
  const { t, i18n: i18nInstance } = useI18nTranslation();

  return {
    t: (key: TranslationKeys | string, options?: TranslateOptions) =>
      t(key as string, options) as string,
    language: i18nInstance.language as Language,
    setLanguage: (lang: Language) => i18nInstance.changeLanguage(lang),
  };
}
