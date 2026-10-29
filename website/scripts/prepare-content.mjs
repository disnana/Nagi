import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, posix, resolve } from 'node:path';

const website = resolve(import.meta.dirname, '..');
const repository = resolve(website, '..');
const output = resolve(website, '.generated/legacy-docs');
const locales = ['ja', 'en'];

const manifests = Object.fromEntries(
  await Promise.all(
    locales.map(async (locale) => [
      locale,
      JSON.parse(
        await readFile(resolve(website, `content/docs/${locale}/reference/legacy-routes.json`), 'utf8'),
      ),
    ]),
  ),
);

const jaKeys = Object.keys(manifests.ja).sort();
const enKeys = Object.keys(manifests.en).sort();
if (JSON.stringify(jaKeys) !== JSON.stringify(enKeys)) {
  throw new Error('Japanese and English legacy route manifests must have identical route keys.');
}

function routeForMarkdownLink(locale, sourceRelative, href) {
  if (href.startsWith('#') || /^(?:https?:|mailto:|tel:)/.test(href)) return href;
  const [path, hash = ''] = href.split('#', 2);
  const absoluteDocs = path.match(/^\/(?:en\/)?docs\/([^/]+)\/?$/);
  const normalized = posix.normalize(path).replace(/^\.\//, '');
  const routeCandidate = absoluteDocs?.[1]
    ?? normalized.replace(/^(?:\.\.\/)*docs\/(?:en\/)?/, '').replace(/\.md$/, '');
  const sameDirectoryCandidate = path.endsWith('.md') ? posix.basename(path, '.md') : '';
  const slug = jaKeys.includes(routeCandidate)
    ? routeCandidate
    : jaKeys.includes(sameDirectoryCandidate) ? sameDirectoryCandidate : undefined;
  if (slug) {
    const entry = manifests[locale][slug];
    const target = entry.mode === 'redirect' ? entry.target : slug;
    const suffix = entry.mode === 'localized-import' && hash ? `#${hash}` : '';
    const prefix = locale === 'en' ? '/en/docs' : '/docs';
    return `${prefix}/${target}/${suffix}`;
  }
  if (path.startsWith('/')) return href;
  if (path.startsWith('../../../assets/')) return href;
  const repositoryPath = posix.normalize(posix.join(posix.dirname(sourceRelative), path));
  const mode = path.endsWith('/') ? 'tree' : 'blob';
  return `https://github.com/disnana/Nagi/${mode}/main/${repositoryPath}${hash ? `#${hash}` : ''}`;
}

function rewriteMarkdownLinks(locale, sourceRelative, body) {
  return body
    .replace(
      /!\[([^\]]*)\]\((?:\.\.\/)+website\/assets\/([^)]+)\)/g,
      (_, alt, asset) => `<LegacyAsset src=\"/assets/${asset}\" alt=\"${alt.replaceAll('\"', '&quot;')}\" />`,
    )
    .replace(/(?<!!)\[([^\]]+)\]\(([^)]+)\)/g, (_, label, href) => {
      const targetLocale = label === '日本語' ? 'ja' : label === 'English' ? 'en' : locale;
      return `[${label}](${routeForMarkdownLink(targetLocale, sourceRelative, href)})`;
    });
}

function plainText(markdown) {
  return markdown
    .replace(/!\[([^\]]*)\]\([^)]+\)/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/<[^>]+>/g, ' ')
    .replace(/[*_~#>]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

function truncateDescription(text, limit = 220) {
  if (text.length <= limit) return text;
  const prefix = text.slice(0, limit + 1);
  const punctuation = Math.max(prefix.lastIndexOf('。'), prefix.lastIndexOf('！'), prefix.lastIndexOf('？'), prefix.lastIndexOf('. '));
  const whitespace = prefix.lastIndexOf(' ');
  const boundary = punctuation >= Math.floor(limit * 0.55) ? punctuation + 1 : whitespace >= Math.floor(limit * 0.55) ? whitespace : limit;
  return `${prefix.slice(0, boundary).trim()}…`;
}

function firstParagraph(markdown) {
  const paragraph = markdown
    .split(/\n\s*\n/)
    .map((part) => part.trim())
    .find((part) => part && !part.startsWith('```') && !part.startsWith('- ') && !/^\[(?:日本語|English)\]\([^)]+\)$/.test(part));
  return truncateDescription(plainText(paragraph ?? ''));
}

await rm(output, { recursive: true, force: true });
const generated = [];

for (const locale of locales) {
  for (const [slug, entry] of Object.entries(manifests[locale])) {
    if (entry.mode !== 'localized-import') continue;
    const sourceRelative = locale === 'en' ? entry.enSource : entry.jaSource;
    if (typeof sourceRelative !== 'string') {
      throw new Error(`${locale}:${slug} is missing a localized source path.`);
    }
    const sourcePath = resolve(repository, sourceRelative);
    const raw = await readFile(sourcePath, 'utf8');
    const titleMatch = raw.match(/^#\s+(.+)$/m);
    if (!titleMatch) throw new Error(`${sourceRelative} is missing a level-one title.`);
    const title = titleMatch[1].trim();
    const withoutTitle = raw.replace(/^#\s+.+\n?/, '').trim();
    const description = firstParagraph(withoutTitle) || title;
    const body = rewriteMarkdownLinks(locale, sourceRelative, withoutTitle);
    // These imports can contain repository-owned presentation components such
    // as LegacyAsset. Keep them as MDX so those components render instead of
    // being discarded as raw HTML comments by the Markdown-only pipeline.
    const destination = resolve(output, locale, `${slug}.mdx`);
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(
      destination,
      [
        '---',
        `title: ${JSON.stringify(title)}`,
        `description: ${JSON.stringify(description)}`,
        'status: reference',
        'backends: []',
        `legacySource: ${JSON.stringify(sourceRelative)}`,
        '---',
        '',
        body,
        '',
      ].join('\n'),
      'utf8',
    );
    generated.push({ locale, slug, source: sourceRelative });
  }
}

await mkdir(resolve(website, '.generated'), { recursive: true });
await writeFile(
  resolve(website, '.generated/legacy-pages.json'),
  `${JSON.stringify(generated, null, 2)}\n`,
  'utf8',
);
console.log(`generated ${generated.length} localized legacy pages`);
