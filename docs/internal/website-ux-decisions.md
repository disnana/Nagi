# Website UX decisions

## 2026-10-28: separate canonical content from page weight

`CHANGELOG.md` remains the only release-note source, but a canonical source does not require one large public page. The site generator builds a small version index and one permalink per release. The index contains only titles and bounded summaries; full notes are loaded by navigating to a version. This keeps source ownership, release tooling, and public delivery separate.

The correction follows these rules:

- keep `/docs/changelog/` and `/en/docs/changelog/` as stable, lightweight indexes;
- generate version pages from `CHANGELOG.md` rather than copying release text;
- expose version filtering, section contents, newer/older links, and source links with ordinary HTML controls and links that work without JavaScript;
- verify desktop/mobile layout, keyboard navigation, focus, dark theme, copy controls, and contained code scrolling in an isolated browser;
- keep capability discovery code-first: supported features link to complete source and show syntax-highlighted excerpts, exact commands, expected output, release/backend status, and limits.

### Shadcn Admin comparison

[Shadcn Admin](https://github.com/satnaing/shadcn-admin) is a responsive, accessible Vite/React dashboard using Shadcn UI, TanStack Router, a collapsible sidebar, command search, theme controls, and many application dependencies. Its navigation hierarchy, compact cards, search, focus states, responsive sidebar, and theme behavior are useful references. It is explicitly an admin dashboard rather than a documentation engine or starter template.

Nagi's previous site used a custom static Python builder. Replacing it with Shadcn Admin would have introduced a dashboard-oriented dependency and navigation model without supplying a documentation source model, versioned pages, or search pipeline.

The adopted decision is to use Fumadocs with Next.js static export. It provides a documentation-specific sidebar, table of contents, code blocks, search integration, and responsive navigation while keeping GitHub Pages as plain static hosting. The migration retains explicit Japanese/English routes, physical legacy aliases, versioned changelog pages, local link validation, and browser QA. No runtime server, middleware, authenticated dashboard, or server action is required.

Fumadocs does not remove the need for product-specific content and validation. Nagi keeps its own locale-parity checks, canonical changelog import, legacy-route manifest, static search payload limits, tutorial evidence, and base-path tests. The framework choice is therefore an implementation detail behind a stable public URL and content contract, rather than permission to weaken those contracts.

## Review checklist for future public pages

Before publishing a new large page, check initial HTML size and whether detail content can have stable subpages. Check the primary user journey on desktop and a narrow viewport, keyboard access, visible focus, light/dark contrast, overflow containment, no-JavaScript navigation, and source/version/backend accuracy. HTTP 200 and successful link generation are necessary but do not establish usable visual layout.
