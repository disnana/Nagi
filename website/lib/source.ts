import { defineDocs } from 'fumadocs-mdx/macro';
import { loader } from 'fumadocs-core/source';
import { pageSchema } from 'fumadocs-core/source/schema';
import { z } from 'zod';
import { i18n } from '@/lib/i18n';

const docsSchema = pageSchema.extend({
  status: z.enum(['released', 'unreleased', 'planned', 'reference']),
  since: z.string().optional(),
  backends: z.array(z.enum(['rust', 'go'])),
});

const docs = defineDocs({
  dir: 'content/docs',
  docs: {
    schema: docsSchema,
  },
});

const legacyDocs = defineDocs({
  dir: '.generated/legacy-docs',
  docs: {
    schema: docsSchema,
  },
});

export const source = loader({
  baseUrl: '/docs',
  source: {
    curated: docs.toFumadocsSource(),
    legacy: legacyDocs.toFumadocsSource(),
  },
  i18n,
});
