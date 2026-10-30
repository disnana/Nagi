'use client';

import type { ComponentProps } from 'react';
import { useI18n } from 'fumadocs-ui/contexts/i18n';
import {
  LanguageSelect,
  LanguageSelectText,
} from 'fumadocs-ui/layouts/shared/slots/language-select';

export function AccessibleLanguageSelect(props: ComponentProps<typeof LanguageSelect>) {
  const { locale, locales } = useI18n();
  const current = locales?.find((item) => item.locale === locale)?.name ?? locale ?? '';
  const prompt = locale === 'ja' ? '表示言語を選択' : 'Choose a language';

  return <LanguageSelect {...props} aria-label={`${prompt}: ${current}`} />;
}

export { LanguageSelectText };
