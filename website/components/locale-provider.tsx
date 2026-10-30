'use client';

import type { ReactNode } from 'react';
import { useCallback } from 'react';
import { usePathname, useRouter } from 'next/navigation';
import { RootProvider } from 'fumadocs-ui/provider/next';
import { i18nProvider } from 'fumadocs-ui/i18n';
import { StaticSearchDialog } from '@/components/search';
import type { Locale } from '@/lib/i18n';
import { translations } from '@/lib/layout';

export function localePath(pathname: string, locale: Locale): string {
  const japanesePath = pathname.replace(/^\/en(?=\/|$)/, '') || '/';
  if (locale === 'ja') return japanesePath;
  return japanesePath === '/' ? '/en' : `/en${japanesePath}`;
}

export function LocaleProvider({ locale, children }: { locale: Locale; children: ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const onLocaleChange = useCallback((next: string) => {
    if (next !== 'ja' && next !== 'en') return;
    const hash = typeof window === 'undefined' ? '' : window.location.hash;
    router.push(`${localePath(pathname, next)}${hash}`);
  }, [pathname, router]);

  return (
    <RootProvider
      i18n={{ ...i18nProvider(translations, locale), onLocaleChange }}
      search={{ SearchDialog: StaticSearchDialog }}
    >
      {children}
    </RootProvider>
  );
}
