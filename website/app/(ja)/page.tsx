import { Home } from '@/components/home';
import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Nagi — 安全で高速なバックエンド言語',
  alternates: { canonical: '/', languages: { ja: '/', en: '/en/' } },
};

export default function JapaneseHome() {
  return <Home locale="ja" />;
}
