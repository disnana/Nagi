import { createHash } from 'node:crypto';
import { readdir, readFile, stat } from 'node:fs/promises';
import { extname, relative, resolve, sep } from 'node:path';

const website = resolve(import.meta.dirname, '..');
const repository = resolve(website, '..');
const locales = ['ja', 'en'];
const failures = [];

async function filesUnder(root) {
  const found = [];
  async function walk(directory) {
    for (const name of await readdir(directory)) {
      const path = resolve(directory, name);
      if ((await stat(path)).isDirectory()) await walk(path);
      else found.push(path);
    }
  }
  await walk(root);
  return found;
}

function frontmatter(markdown, file) {
  const match = markdown.match(/^---\n([\s\S]*?)\n---\n/);
  if (!match) {
    failures.push(`${file}: missing frontmatter`);
    return {};
  }
  return Object.fromEntries(
    match[1].split('\n').flatMap((line) => {
      const separator = line.indexOf(':');
      if (separator === -1) return [];
      const key = line.slice(0, separator).trim();
      const raw = line.slice(separator + 1).trim();
      if (/^-?\d+$/.test(raw)) return [[key, Number(raw)]];
      if (raw.startsWith('"')) {
        try { return [[key, JSON.parse(raw)]]; } catch { return [[key, raw]]; }
      }
      return [[key, raw]];
    }),
  );
}

const localeFiles = {};
for (const locale of locales) {
  const root = resolve(website, `content/docs/${locale}`);
  localeFiles[locale] = (await filesUnder(root))
    .map((file) => relative(root, file).split(sep).join('/'))
    .sort();
}
if (JSON.stringify(localeFiles.ja) !== JSON.stringify(localeFiles.en)) {
  failures.push('Japanese and English curated content paths differ.');
}

const changelog = await readFile(resolve(repository, 'CHANGELOG.md'), 'utf8');
const sections = new Map();
const headings = [...changelog.matchAll(/^##\s+(.+)$/gm)];
for (let index = 0; index < headings.length; index += 1) {
  const heading = headings[index][1].trim();
  const start = headings[index].index + headings[index][0].length;
  const end = headings[index + 1]?.index ?? changelog.length;
  sections.set(heading, changelog.slice(start, end).trim());
}

const localizedChangelog = {};
for (const locale of locales) {
  const directory = resolve(website, `content/docs/${locale}/changelog`);
  localizedChangelog[locale] = new Map();
  for (const file of (await filesUnder(directory)).filter((path) => extname(path) === '.mdx')) {
    if (file.endsWith(`${sep}index.mdx`)) continue;
    const markdown = await readFile(file, 'utf8');
    if (locale === 'ja' && /リポジトリ 直下|Content-型|ハンドラー ラッパー|引数 Info|支援 protocol|highlight、indentation|compatibility check/.test(markdown)) {
      failures.push(`${relative(repository, file)}: Japanese changelog contains a known untranslated fragment`);
    }
    const data = frontmatter(markdown, relative(repository, file));
    const id = data.canonicalId;
    if (typeof id !== 'string') {
      failures.push(`${relative(repository, file)}: missing canonicalId`);
      continue;
    }
    const matchingHeading = [...sections.keys()].find(
      (heading) => heading === data.canonicalTitle || heading.startsWith(`${data.canonicalTitle} —`),
    );
    if (!matchingHeading) {
      failures.push(`${relative(repository, file)}: canonicalTitle does not match CHANGELOG heading`);
      continue;
    }
    const body = sections.get(matchingHeading);
    const hash = `sha256:${createHash('sha256').update(body, 'utf8').digest('hex')}`;
    const rawBulletCount = body.split('\n').filter((line) => /^-\s/.test(line)).length;
    const items = [...markdown.matchAll(/(?:<!--|\{\/\*)\s*canonical-item:\s*(item-\d{3})\s*(?:-->|\*\/\})/g)].map((item) => item[1]);
    const expectedItems = Array.from({ length: items.length }, (_, i) => `item-${String(i + 1).padStart(3, '0')}`);
    if (data.canonicalSource !== 'CHANGELOG.md') failures.push(`${id}/${locale}: canonicalSource must be CHANGELOG.md`);
    if (data.canonicalSourceHash !== hash) failures.push(`${id}/${locale}: CHANGELOG source hash mismatch`);
    if (data.canonicalRawBulletCount !== rawBulletCount) failures.push(`${id}/${locale}: raw bullet count mismatch`);
    if (data.canonicalItemCount !== items.length) failures.push(`${id}/${locale}: canonical item count mismatch`);
    if (JSON.stringify(items) !== JSON.stringify(expectedItems)) failures.push(`${id}/${locale}: item IDs are not sequential`);
    localizedChangelog[locale].set(id, { data, items });
  }
}

const jaChangelogIds = [...localizedChangelog.ja.keys()].sort();
const enChangelogIds = [...localizedChangelog.en.keys()].sort();
if (JSON.stringify(jaChangelogIds) !== JSON.stringify(enChangelogIds)) {
  failures.push('Japanese and English changelog IDs differ.');
}
for (const id of jaChangelogIds) {
  const ja = localizedChangelog.ja.get(id);
  const en = localizedChangelog.en.get(id);
  if (!en) continue;
  for (const field of ['canonicalTitle', 'canonicalSource', 'canonicalDate', 'canonicalSourceHash', 'canonicalRawBulletCount', 'canonicalItemCount']) {
    if (ja.data[field] !== en.data[field]) failures.push(`${id}: ${field} differs between locales`);
  }
  if (JSON.stringify(ja.items) !== JSON.stringify(en.items)) failures.push(`${id}: canonical item IDs differ between locales`);
}

const manifests = {};
for (const locale of locales) {
  manifests[locale] = JSON.parse(
    await readFile(resolve(website, `content/docs/${locale}/reference/legacy-routes.json`), 'utf8'),
  );
}
const jaRoutes = Object.keys(manifests.ja).sort();
const enRoutes = Object.keys(manifests.en).sort();
if (JSON.stringify(jaRoutes) !== JSON.stringify(enRoutes)) failures.push('Legacy route keys differ between locales.');
const curatedSlugs = new Set(
  localeFiles.ja
    .filter((file) => file.endsWith('.mdx'))
    .map((file) => file.replace(/(?:^|\/)index\.mdx$/, '').replace(/\.mdx$/, '').replace(/\/$/, '')),
);
for (const slug of jaRoutes) {
  const ja = manifests.ja[slug];
  const en = manifests.en[slug];
  if (JSON.stringify(ja) !== JSON.stringify(en)) failures.push(`${slug}: legacy route entries differ between locales`);
  if (ja.mode === 'redirect') {
    if (!curatedSlugs.has(ja.target)) failures.push(`${slug}: redirect target ${ja.target} is not curated content`);
  } else if (ja.mode === 'localized-import') {
    for (const source of [ja.jaSource, ja.enSource]) {
      try { await stat(resolve(repository, source)); } catch { failures.push(`${slug}: missing legacy source ${source}`); }
    }
  } else failures.push(`${slug}: unknown legacy route mode`);
}

const generatedPages = JSON.parse(
  await readFile(resolve(website, '.generated/legacy-pages.json'), 'utf8'),
);
for (const page of generatedPages) {
  const file = resolve(website, `.generated/legacy-docs/${page.locale}/${page.slug}.mdx`);
  const markdown = await readFile(file, 'utf8');
  const data = frontmatter(markdown, relative(repository, file));
  if (data.status !== 'reference') failures.push(`${page.locale}:${page.slug}: legacy import must be reference content`);
  if ('since' in data) failures.push(`${page.locale}:${page.slug}: legacy import must not claim a release version`);
  if (data.backends !== '[]') failures.push(`${page.locale}:${page.slug}: legacy import must not infer backend support`);
  if (typeof data.description !== 'string' || /\[[^\]]*\]\(|https?:\/\//.test(data.description)) {
    failures.push(`${page.locale}:${page.slug}: legacy description must be plain text without URLs`);
  }
  for (const match of markdown.matchAll(/\[(日本語|English)\]\(([^)]+)\)/g)) {
    const expectedPrefix = match[1] === '日本語' ? '/docs/' : '/en/docs/';
    if (!match[2].startsWith(expectedPrefix)) {
      failures.push(`${page.locale}:${page.slug}: ${match[1]} link must target ${expectedPrefix}`);
    }
  }
}

if (failures.length > 0) {
  console.error(failures.map((failure) => `- ${failure}`).join('\n'));
  process.exit(1);
}
console.log(`content verified: ${localeFiles.ja.length} files/locale, ${jaChangelogIds.length} changelog versions, ${jaRoutes.length} legacy routes`);
