import { access, glob, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';

const root = new URL('../out/', import.meta.url);
const expected = [
  'index.html',
  'en/index.html',
  'docs/index.html',
  'en/docs/index.html',
  'api/search/ja',
  'api/search/en',
];

for (const path of expected) {
  await access(new URL(path, root));
}

const legacyRoutes = JSON.parse(
  await readFile(new URL('../content/docs/ja/reference/legacy-routes.json', import.meta.url), 'utf8'),
);
for (const slug of Object.keys(legacyRoutes)) {
  await access(new URL(`docs/${slug}/index.html`, root));
  await access(new URL(`en/docs/${slug}/index.html`, root));
}

const japanese = await readFile(new URL('docs/index.html', root), 'utf8');
const english = await readFile(new URL('en/docs/index.html', root), 'utf8');
const japaneseWriting = await readFile(new URL('docs/writing/index.html', root), 'utf8');
const englishWriting = await readFile(new URL('en/docs/writing/index.html', root), 'utf8');
if (!japanese.includes('<html lang="ja"')) throw new Error('Japanese docs must render lang="ja"');
if (!english.includes('<html lang="en"')) throw new Error('English docs must render lang="en"');
if (!japaneseWriting.includes('こんにちは') || !englishWriting.includes('Hello')) {
  throw new Error('Both locales must render the runnable example');
}

const japaneseSearch = await readFile(new URL('api/search/ja', root), 'utf8');
const englishSearch = await readFile(new URL('api/search/en', root), 'utf8');
if (!japaneseSearch.includes('タスク管理JSON APIをゼロから動かす')) {
  throw new Error('Japanese search index is missing imported Japanese legacy content');
}
if (japaneseSearch.includes('Build a task JSON API end to end')) {
  throw new Error('Japanese search index contains English legacy content');
}
if (!englishSearch.includes('Build a task JSON API end to end')) {
  throw new Error('English search index is missing imported English legacy content');
}
if (englishSearch.includes('タスク管理JSON APIをゼロから動かす')) {
  throw new Error('English search index contains Japanese legacy content');
}
for (const [locale, payload] of [['ja', japaneseSearch], ['en', englishSearch]]) {
  if (Buffer.byteLength(payload) > 3_000_000) {
    throw new Error(`${locale} search payload exceeds the 3 MB raw budget`);
  }
  if (gzipSync(payload).byteLength > 750 * 1024) {
    throw new Error(`${locale} search payload exceeds the 750 KiB gzip budget`);
  }
}

const htmlFiles = [];
for await (const file of glob('**/*.html', { cwd: root.pathname })) htmlFiles.push(file);
for (const file of htmlFiles) {
  const body = await readFile(new URL(file, root), 'utf8');
  const h1Count = (body.match(/<h1(?:\s|>)/g) ?? []).length;
  if (h1Count !== 1) throw new Error(`${file} must contain exactly one h1, found ${h1Count}`);
  if (process.env.PAGES_BASE_PATH) {
    for (const match of body.matchAll(/(?:href|src)=\"(\/[^\"]*)\"/g)) {
      if (match[1] !== process.env.PAGES_BASE_PATH && !match[1].startsWith(`${process.env.PAGES_BASE_PATH}/`)) {
        throw new Error(`${file} has a root asset or route without the Pages base path: ${match[1]}`);
      }
    }
  }
}

console.log(`verified static export and ${Object.keys(legacyRoutes).length * 2} legacy locale routes in ${join(root.pathname)}`);
