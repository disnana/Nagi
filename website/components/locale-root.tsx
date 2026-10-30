import type { ReactNode } from 'react';
import { LocaleProvider } from '@/components/locale-provider';
import type { Locale } from '@/lib/i18n';
import '@/app/global.css';

export function LocaleRoot({ locale, children }: { locale: Locale; children: ReactNode }) {
  return (
    <html lang={locale} suppressHydrationWarning>
      <body className="flex min-h-screen flex-col">
        <LocaleProvider locale={locale}>{children}</LocaleProvider>
      </body>
    </html>
  );
}
