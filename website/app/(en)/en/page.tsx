import { Home } from '@/components/home';
import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Nagi — a safe, fast backend language',
  alternates: { canonical: '/en/', languages: { ja: '/', en: '/en/' } },
};

export default function EnglishHome() {
  return <Home locale="en" />;
}
