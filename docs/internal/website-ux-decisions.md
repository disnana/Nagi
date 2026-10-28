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

Nagi's current site is a static Python build: Markdown is rendered at build time, all local links and anchors are checked, pages need no client framework, and GitHub Pages deploys plain files with a restrictive same-origin CSP. Replacing it with Shadcn Admin would add Node, React, Vite, Tailwind, TanStack, Radix and component update work; rewrite the bilingual route/source-link generator and link validator; and increase dependency, cache, security-audit, browser-test, and hydration responsibilities without improving the compiler or documentation content by itself.

The adopted decision is to apply the useful interaction patterns to the current static site first. A framework migration should be reconsidered only if Nagi needs substantial client-side state, authenticated dashboard workflows, or a component system whose measured maintenance benefit exceeds the new dependency and migration cost. If a docs-specific framework is evaluated later, compare a dedicated option such as Fumadocs with the static builder rather than treating an admin dashboard as a drop-in docs engine.

## Review checklist for future public pages

Before publishing a new large page, check initial HTML size and whether detail content can have stable subpages. Check the primary user journey on desktop and a narrow viewport, keyboard access, visible focus, light/dark contrast, overflow containment, no-JavaScript navigation, and source/version/backend accuracy. HTTP 200 and successful link generation are necessary but do not establish usable visual layout.
