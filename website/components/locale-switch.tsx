'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import type { Locale } from '@/lib/i18n';

export function LocaleSwitch({ locale }: { locale: Locale }) {
  const pathname = usePathname();
  const targetLocale = locale === 'ja' ? 'en' : 'ja';
  const target = locale === 'ja'
    ? pathname === '/' ? '/en' : `/en${pathname}`
    : pathname.replace(/^\/en(?=\/|$)/, '') || '/';
  const label = targetLocale === 'ja' ? '日本語' : 'English';

  return (
    <Link href={target} hrefLang={targetLocale} lang={targetLocale}>
      {label}
    </Link>
  );
}
