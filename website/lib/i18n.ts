import { defineI18n } from 'fumadocs-core/i18n';

export const i18n = defineI18n({
  languages: ['ja', 'en'],
  defaultLanguage: 'ja',
  fallbackLanguage: null,
  parser: 'dir',
  hideLocale: 'default-locale',
});

export type Locale = 'ja' | 'en';
