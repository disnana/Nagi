import { createHash } from 'node:crypto';
import { glob, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const website = resolve(import.meta.dirname, '..');
const evidence = JSON.parse(await readFile(resolve(website, 'teaching-example-evidence.json'), 'utf8'));
const expected = new Map(
  evidence.compilerCheck.blocks.map((row) => [
    `${row.page}#${row.block}#${[...row.locales].sort().join(',')}`,
    row,
  ]),
);
const actual = new Map();

for (const locale of ['ja', 'en']) {
  for (const row of evidence.compilerCheck.blocks.filter((item) => item.locales.includes(locale))) {
    const path = resolve(website, 'content/docs', locale, row.page);
    const markdown = await readFile(path, 'utf8');
    const blocks = [...markdown.matchAll(/^```nagi[^\n]*\n(.*?)^```/gms)].map((match) => match[1].trim());
    if (row.block < 1 || row.block > blocks.length) throw new Error(`${locale}:${row.page} is missing Nagi block ${row.block}`);
    const source = blocks[row.block - 1];
    const sha256 = createHash('sha256').update(source).digest('hex');
    if (sha256 !== row.sha256) throw new Error(`${locale}:${row.page}#${row.block} changed without fresh compiler evidence`);
    const key = `${row.page}#${row.block}#${[...row.locales].sort().join(',')}`;
    actual.set(key, { row, source });
  }
}
if (actual.size !== expected.size || [...expected.keys()].some((key) => !actual.has(key))) {
  throw new Error('Teaching example evidence does not cover the current locale/code-block set');
}
for (const locale of ['ja', 'en']) {
  for await (const relative of glob('**/*.mdx', { cwd: resolve(website, 'content/docs', locale) })) {
    if (relative.startsWith('changelog/')) continue;
    const markdown = await readFile(resolve(website, 'content/docs', locale, relative), 'utf8');
    const count = [...markdown.matchAll(/^```nagi[^\n]*\n(.*?)^```/gms)].length;
    const covered = evidence.compilerCheck.blocks.filter((row) => row.page === relative && row.locales.includes(locale)).length;
    if (count !== covered) throw new Error(`${locale}:${relative} has ${count} Nagi blocks but ${covered} evidence entries`);
  }
}

const compiler = process.env.NAGI_RELEASE_COMPILER;
if (compiler) {
  const temporary = await mkdtemp(join(tmpdir(), 'nagi-fumadocs-check-'));
  try {
    for (const { row, source } of actual.values()) {
      const stem = `${basename(row.page, '.mdx')}-${row.block}-${row.sha256.slice(0, 12)}`;
      const input = join(temporary, `${stem}.nagi`);
      const output = join(temporary, `${stem}-out`);
      await writeFile(input, `${source}\n`, 'utf8');
      const result = spawnSync(compiler, ['check', input, '--no-project', '--out', output], {
        encoding: 'utf8',
        timeout: 60_000,
      });
      if (result.status !== 0) throw new Error(`${row.page}#${row.block}: ${result.stdout}${result.stderr}`);
    }
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

console.log(`verified evidence for ${actual.size} unique teaching blocks${compiler ? ' with the supplied compiler' : ''}`);
