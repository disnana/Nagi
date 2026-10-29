import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { DocumentationPage } from '@/components/docs-page';
import { LegacyAlias } from '@/components/legacy-alias';
import { getMDXComponents } from '@/components/mdx';
import { docsUrl, getAllStaticParams, getLegacyAlias } from '@/lib/legacy-routes';
import { source } from '@/lib/source';

type Props = { params: Promise<{ slug?: string[] }> };

export default async function JapaneseDocsPage({ params }: Props) {
  const { slug } = await params;
  const page = source.getPage(slug, 'ja');
  if (!page) {
    const alias = getLegacyAlias('ja', slug);
    if (!alias) notFound();
    return <LegacyAlias locale="ja" href={docsUrl('ja', alias.target)} />;
  }
  const MDX = page.data.body;

  return (
    <DocumentationPage title={page.data.title} description={page.data.description} toc={page.data.toc}>
      <MDX components={getMDXComponents()} />
    </DocumentationPage>
  );
}

export function generateStaticParams() {
  return getAllStaticParams('ja');
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { slug } = await params;
  const page = source.getPage(slug, 'ja');
  if (page) {
    const path = page.slugs.join('/');
    return {
      title: page.data.title,
      description: page.data.description,
      alternates: {
        canonical: docsUrl('ja', path),
        languages: { ja: docsUrl('ja', path), en: docsUrl('en', path) },
      },
    };
  }
  const alias = getLegacyAlias('ja', slug);
  if (!alias) notFound();
  return {
    title: 'このページは移動しました',
    alternates: {
      canonical: docsUrl('ja', alias.target),
      languages: { ja: docsUrl('ja', alias.target), en: docsUrl('en', alias.target) },
    },
  };
}
