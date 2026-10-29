import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { defineConfig } from 'fumadocs-mdx/config';

const nagiTextMateGrammar = JSON.parse(
  readFileSync(resolve(process.cwd(), '../editors/vscode-nagi/syntaxes/nagi.tmLanguage.json'), 'utf8'),
);
const nagiGrammar = {
  ...nagiTextMateGrammar,
  name: 'nagi',
};
const lowGrammar = {
  ...nagiTextMateGrammar,
  name: 'low',
};

export default defineConfig({
  mdxOptions: {
    rehypeCodeOptions: {
      langs: [nagiGrammar, lowGrammar],
      themes: {
        light: 'github-light-high-contrast',
        dark: 'github-dark-high-contrast',
      },
    },
  },
});
