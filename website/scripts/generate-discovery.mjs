import { glob, readFile, writeFile } from 'node:fs/promises';

const output = new URL('../out/', import.meta.url);
const canonicalOrigin = 'https://nagi.disnana.com';
const pages = [];
const files = new Set(['index.html', 'en/index.html']);

for await (const file of glob('**/index.html', { cwd: output.pathname })) files.add(file);
for (const file of files) {
  const html = await readFile(new URL(file, output), 'utf8');
  const canonical = html.match(/<link rel="canonical" href="([^"]+)"/i)?.[1];
  const title = html.match(/<title>([^<]+)<\/title>/i)?.[1]?.replace(/\s*[|｜]\s*Nagi.*$/u, '')?.trim();
  if (!canonical || !canonical.startsWith(`${canonicalOrigin}/`) || !title) continue;
  pages.push({ canonical, title });
}

pages.sort((a, b) => a.canonical.localeCompare(b.canonical));
const unique = [...new Map(pages.map((page) => [page.canonical, page])).values()];
const xml = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${unique.map(({ canonical }) => `  <url><loc>${canonical.replaceAll('&', '&amp;')}</loc></url>`).join('\n')}\n</urlset>\n`;
const robots = `User-agent: *\nAllow: /\nSitemap: ${canonicalOrigin}/sitemap.xml\n`;
const publicDocs = unique.filter(({ canonical }) => canonical.includes('/docs/'));
const llms = [
  '# Nagi documentation',
  '',
  '> Public, versioned documentation for the Nagi backend language. Internal plans and audit logs are intentionally excluded.',
  '',
  ...publicDocs.map(({ title, canonical }) => `- [${title}](${canonical})`),
  '',
].join('\n');

await writeFile(new URL('sitemap.xml', output), xml);
await writeFile(new URL('robots.txt', output), robots);
await writeFile(new URL('llms.txt', output), llms);
console.log(`generated discovery files for ${unique.length} public routes (${publicDocs.length} Docs routes)`);
