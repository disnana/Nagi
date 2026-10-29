import type { Metadata } from 'next';
import type { ReactNode } from 'react';
import { LocaleRoot } from '@/components/locale-root';

export const metadata: Metadata = { metadataBase: new URL('https://nagi.disnana.com') };

export default function EnglishRootLayout({ children }: { children: ReactNode }) {
  return <LocaleRoot locale="en">{children}</LocaleRoot>;
}
