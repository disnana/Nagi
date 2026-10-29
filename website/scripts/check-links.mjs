import { access, readFile } from 'node:fs/promises';
import { glob } from 'node:fs/promises';
import { relative, resolve } from 'node:path';

const output = resolve(import.meta.dirname, '../out');
const basePath = process.env.PAGES_BASE_PATH ?? '';
const failures = [];
const html = new Map();

for await (const file of glob('**/*.html', { cwd: output })) {
  html.set(`/${file}`, await readFile(resolve(output, file), 'utf8'));
}

function targetFile(pathname) {
  let route = decodeURIComponent(pathname);
  if (basePath && (route === basePath || route.startsWith(`${basePath}/`))) {
    route = route.slice(basePath.length) || '/';
  }
  route = route.replace(/^\//, '');
  if (!route) return 'index.html';
  if (route.endsWith('/')) return `${route}index.html`;
  return route.includes('.') ? route : `${route}/index.html`;
}

for (const [file, body] of html) {
  const sourceRoute = file.replace(/index\.html$/, '').replace(/\.html$/, '');
  const ids = new Set([...body.matchAll(/\s(?:id|name)="([^"]+)"/g)].map((match) => match[1]));
  for (const match of body.matchAll(/<(a|link|script|img)\b[^>]+?(?:href|src)="([^"]+)"/g)) {
    const value = match[2];
    if (/^(?:https?:|mailto:|tel:|data:|javascript:)/.test(value)) continue;
    const url = new URL(value, `https://nagi.invalid${sourceRoute || '/'}`);
    const path = targetFile(url.pathname);
    try {
      await access(resolve(output, path));
    } catch {
      failures.push(`${relative(output, resolve(output, file.slice(1)))} -> ${value} (missing ${path})`);
      continue;
    }
    if (url.hash && path.endsWith('.html')) {
      const target = html.get(`/${path}`) ?? await readFile(resolve(output, path), 'utf8');
      const targetIds = path === file.slice(1)
        ? ids
        : new Set([...target.matchAll(/\s(?:id|name)="([^"]+)"/g)].map((item) => item[1]));
      const anchor = decodeURIComponent(url.hash.slice(1));
      if (anchor && !targetIds.has(anchor)) failures.push(`${file} -> ${value} (missing anchor ${anchor})`);
    }
  }
}

if (failures.length) {
  throw new Error(`Broken internal links:\n${failures.slice(0, 100).join('\n')}`);
}
console.log(`verified links in ${html.size} HTML files`);
