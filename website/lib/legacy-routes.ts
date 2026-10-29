import englishManifest from '@/content/docs/en/reference/legacy-routes.json';
import japaneseManifest from '@/content/docs/ja/reference/legacy-routes.json';
import { source } from '@/lib/source';

type Locale = 'ja' | 'en';
type LegacyRoute =
  | { mode: 'redirect'; target: string }
  | { mode: 'localized-import'; jaSource: string; enSource: string };

const manifests: Record<Locale, Record<string, LegacyRoute>> = {
  ja: japaneseManifest as Record<string, LegacyRoute>,
  en: englishManifest as Record<string, LegacyRoute>,
};

export function docsUrl(locale: Locale, slug: string) {
  return `${locale === 'en' ? '/en/docs' : '/docs'}/${slug.replace(/^\/+|\/+$/g, '')}/`;
}

export function getLegacyAlias(locale: Locale, slugs: string[] | undefined) {
  const slug = slugs?.join('/') ?? '';
  if (source.getPage(slugs, locale)) return undefined;
  if (!slug) return { mode: 'redirect' as const, target: 'writing' };
  const route = manifests[locale][slug];
  return route?.mode === 'redirect' ? route : undefined;
}

export function getAllStaticParams(locale: Locale) {
  const routes = new Set(['', ...source.getPages(locale).map((page) => page.slugs.join('/'))]);
  for (const slug of Object.keys(manifests[locale])) routes.add(slug);
  return [...routes].sort().map((slug) => ({ slug: slug ? slug.split('/') : [] }));
}
