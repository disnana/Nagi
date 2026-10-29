import defaultMdxComponents from 'fumadocs-ui/mdx';
import type { MDXComponents } from 'mdx/types';

function LegacyAsset({ src, alt }: { src: string; alt: string }) {
  const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? '';
  return <img src={`${basePath}${src}`} alt={alt} loading="lazy" className="rounded-lg border" />;
}

export function getMDXComponents(components?: MDXComponents) {
  return {
    ...defaultMdxComponents,
    LegacyAsset,
    ...components,
  } satisfies MDXComponents;
}

export const useMDXComponents = getMDXComponents;

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>;
}
