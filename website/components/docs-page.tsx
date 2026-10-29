import type { ReactNode } from 'react';
import { DocsBody, DocsDescription, DocsPage, DocsTitle } from 'fumadocs-ui/layouts/docs/page';

export function DocumentationPage({
  title,
  description,
  toc,
  children,
}: {
  title: string;
  description?: string;
  toc: Parameters<typeof DocsPage>[0]['toc'];
  children: ReactNode;
}) {
  return (
    <DocsPage toc={toc}>
      <DocsTitle>{title}</DocsTitle>
      {description ? <DocsDescription>{description}</DocsDescription> : null}
      <DocsBody>{children}</DocsBody>
    </DocsPage>
  );
}
