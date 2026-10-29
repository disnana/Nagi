import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import { DocumentationPage } from '@/components/docs-page';
import { LegacyAlias } from '@/components/legacy-alias';
import { getMDXComponents } from '@/components/mdx';
import { docsUrl, getAllStaticParams, getLegacyAlias } from '@/lib/legacy-routes';
import { source } from '@/lib/source';

type Props = { params: Promise<{ slug?: string[] }> };

export default async function EnglishDocsPage({ params }: Props) {
  const { slug } = await params;
  const page = source.getPage(slug, 'en');
  if (!page) {
    const alias = getLegacyAlias('en', slug);
    if (!alias) notFound();
    return <LegacyAlias locale="en" href={docsUrl('en', alias.target)} />;
  }
  const MDX = page.data.body;

  return (
    <DocumentationPage title={page.data.title} description={page.data.description} toc={page.data.toc}>
      <MDX components={getMDXComponents()} />
    </DocumentationPage>
  );
}

export function generateStaticParams() {
  return getAllStaticParams('en');
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { slug } = await params;
  const page = source.getPage(slug, 'en');
  if (page) {
    const path = page.slugs.join('/');
    return {
      title: page.data.title,
      description: page.data.description,
      alternates: {
        canonical: docsUrl('en', path),
        languages: { ja: docsUrl('ja', path), en: docsUrl('en', path) },
      },
    };
  }
  const alias = getLegacyAlias('en', slug);
  if (!alias) notFound();
  return {
    title: 'This page has moved',
    alternates: {
      canonical: docsUrl('en', alias.target),
      languages: { ja: docsUrl('ja', alias.target), en: docsUrl('en', alias.target) },
    },
  };
}
