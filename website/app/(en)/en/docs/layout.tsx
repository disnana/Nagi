import type { ReactNode } from 'react';
import { DocsLayout } from 'fumadocs-ui/layouts/docs';
import { source } from '@/lib/source';
import { baseOptions } from '@/lib/layout';

export default function EnglishDocsLayout({ children }: { children: ReactNode }) {
  return (
    <DocsLayout tree={source.getPageTree('en')} {...baseOptions('en')}>
      {children}
    </DocsLayout>
  );
}
