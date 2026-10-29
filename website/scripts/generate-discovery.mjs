import { access, glob, readFile, unlink, writeFile } from 'node:fs/promises';

const output = new URL('../out/', import.meta.url);
const canonicalOrigin = 'https://nagi.disnana.com';
const repository = new URL('../../', import.meta.url);
const publicAgentSources = [
  ['AI and agent entry point', 'ai/README.md'],
  ['Nagi development Skill', 'ai/skills/nagi-development/SKILL.md'],
  ['Debugging guide for agents', 'ai/debugging.md'],
  ['Language guide for agents', 'ai/language.md'],
  ['Rust interop guide for agents', 'ai/rust-interop.md'],
];
for (const [, path] of publicAgentSources) await access(new URL(path, repository));
const namedEntities = new Map([
  ['amp', '&'], ['quot', '"'], ['apos', "'"], ['lt', '<'], ['gt', '>'],
]);
const decodeTitle = (value) => value.replace(
  /&(?:#x[0-9a-f]+|#\d+|amp|quot|apos|lt|gt);/gi,
  (entity) => {
    const body = entity.slice(1, -1);
    if (body.startsWith('#x') || body.startsWith('#X')) return String.fromCodePoint(Number.parseInt(body.slice(2), 16));
    if (body.startsWith('#')) return String.fromCodePoint(Number.parseInt(body.slice(1), 10));
    return namedEntities.get(body.toLowerCase()) ?? entity;
  },
);
for (const [encoded, expected] of [
  ['&amp;lt;', '&lt;'], ['&amp;amp;', '&amp;'], ['Nagi&#x27;s', "Nagi's"],
  ['Nagi&#39;s', "Nagi's"], ['&unknown;', '&unknown;'],
]) {
  if (decodeTitle(encoded) !== expected) throw new Error(`unsafe title entity decoding for ${encoded}`);
}
const pages = [];
const files = new Set(['index.html', 'en/index.html']);

for await (const file of glob('**/index.html', { cwd: output.pathname })) files.add(file);
for (const file of files) {
  const html = await readFile(new URL(file, output), 'utf8');
  const canonical = html.match(/<link rel="canonical" href="([^"]+)"/i)?.[1];
  const rawTitle = html.match(/<title>([^<]+)<\/title>/i)?.[1];
  const title = rawTitle ? decodeTitle(rawTitle).replace(/\s*[|｜]\s*Nagi.*$/u, '').trim() : undefined;
  if (!canonical || !canonical.startsWith(`${canonicalOrigin}/`) || !title) continue;
  pages.push({ canonical, title });
}

pages.sort((a, b) => a.canonical.localeCompare(b.canonical));
const byCanonical = new Map();
const aliasTitle = /^(?:このページは移動しました|This page has moved)$/;
for (const page of pages) {
  const current = byCanonical.get(page.canonical);
  if (!current || (aliasTitle.test(current.title) && !aliasTitle.test(page.title))) {
    byCanonical.set(page.canonical, page);
  }
}
const unique = [...byCanonical.values()];
const xml = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${unique.map(({ canonical }) => `  <url><loc>${canonical.replaceAll('&', '&amp;')}</loc></url>`).join('\n')}\n</urlset>\n`;
const robots = `User-agent: *\nAllow: /\nSitemap: ${canonicalOrigin}/sitemap.xml\n`;
const publicDocs = unique.filter(({ canonical }) => canonical.includes('/docs/'));
const llms = [
  '# Nagi documentation',
  '',
  '> Public, versioned documentation for the Nagi backend language. Internal plans and audit logs are intentionally excluded.',
  '> Nagi 0.2.2 is the current released Rust target. Go backend work is planned and must not be treated as published support.',
  '',
  ...publicDocs.map(({ title, canonical }) => `- [${title}](${canonical})`),
  '',
  '## Public AI and agent resources',
  '',
  ...publicAgentSources.map(([title, path]) => `- [${title}](https://github.com/disnana/Nagi/blob/main/${path})`),
  '',
].join('\n');

await writeFile(new URL('sitemap.xml', output), xml);
await writeFile(new URL('robots.txt', output), robots);
await writeFile(new URL('llms.txt', output), llms);
if (process.env.PAGES_BASE_PATH) {
  await unlink(new URL('CNAME', output)).catch((error) => {
    if (error.code !== 'ENOENT') throw error;
  });
} else {
  await access(new URL('CNAME', output));
}
console.log(`generated discovery files for ${unique.length} public routes (${publicDocs.length} Docs routes)`);
