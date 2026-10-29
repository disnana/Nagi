import Link from 'next/link';
import { DocumentationPage } from '@/components/docs-page';

export function LegacyAlias({ locale, href }: { locale: 'ja' | 'en'; href: string }) {
  const japanese = locale === 'ja';
  const title = japanese ? 'このページは移動しました' : 'This page has moved';
  const description = japanese
    ? '内容を整理し、より見つけやすいガイドへ統合しました。'
    : 'This content now lives in a clearer, consolidated guide.';
  return (
    <DocumentationPage title={title} description={description} toc={[]}>
      <p>
        <Link href={href} className="font-medium underline underline-offset-4">
          {japanese ? '新しいページを開く' : 'Open the new page'}
        </Link>
      </p>
    </DocumentationPage>
  );
}
