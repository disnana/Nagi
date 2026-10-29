import { createSearchAPI, type AdvancedIndex } from 'fumadocs-core/search/server';
import type { Locale } from '@/lib/i18n';
import { source } from '@/lib/source';

async function indexPage(page: ReturnType<typeof source.getPages>[number]): Promise<AdvancedIndex> {
  const structured = page.data.structuredData as
    | AdvancedIndex['structuredData']
    | (() => Promise<AdvancedIndex['structuredData']>);
  const fullData = typeof structured === 'function' ? await structured() : structured;
  const changelogDetail = page.slugs[0] === 'changelog' && page.slugs.length > 1;
  return {
    id: page.url,
    title: page.data.title,
    description: page.data.description,
    locale: page.locale,
    url: page.url,
    structuredData: changelogDetail
      ? { headings: [], contents: [{ heading: undefined, content: `${page.data.title} ${page.data.description ?? ''}` }] }
      : fullData,
  };
}

export function staticLocaleSearch(locale: Locale) {
  return createSearchAPI('advanced', {
    localeFilter: true,
    indexes: async () => Promise.all(source.getPages(locale).map(indexPage)),
  }).staticGET;
}
