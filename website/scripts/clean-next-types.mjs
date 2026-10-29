import { rm } from 'node:fs/promises';
await Promise.all([
  rm(new URL('../.next/types/', import.meta.url), { recursive: true, force: true }),
  rm(new URL('../.next/dev/types/', import.meta.url), { recursive: true, force: true }),
]);
