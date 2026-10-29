# Nagi documentation site

Nagi's Japanese and English documentation is statically exported with Next.js 16 and Fumadocs. Japanese pages live under `/docs/`; English pages live under `/en/docs/`. Production uses `https://nagi.disnana.com/`, and the checks also cover a `/Nagi` GitHub Pages project path.

## Local preview

Use Node.js 24 or later. Run these commands from the repository root:

```bash
cd website
npm ci --ignore-scripts
npx playwright install chromium
npm run prepare:content
npm run dev
```

Open `http://localhost:3000/`. Development mode is convenient for editing; use the complete check below to verify the static deployment form.

```bash
cd website
npm test
```

`npm test` runs type checking, content generation and consistency checks, the static build, route/link/search-index checks, and real-browser checks for mobile layout, keyboard use, search, copy controls, and accessibility. The static output is `website/out/`. Individual commands are `npm run typecheck`, `npm run build`, `npm run verify:static`, `npm run verify:links`, and `npm run test:browser`.

To exercise the same path shape as project Pages, use the same build-time base path throughout:

```bash
cd website
PAGES_BASE_PATH=/Nagi npm run build
PAGES_BASE_PATH=/Nagi npm run verify:static
PAGES_BASE_PATH=/Nagi npm run verify:links
PAGES_BASE_PATH=/Nagi npm run test:browser
```

## Update content

| Content | Location |
|---|---|
| Japanese tutorials and reference | `website/content/docs/ja/` |
| English tutorials and reference | `website/content/docs/en/` |
| Sidebar ordering | `meta.json` in each locale |
| Home page and shared navigation | `website/components/` and `website/app/` |
| Changelog | Root `CHANGELOG.md` is canonical; generation creates a small index and per-version pages |
| Legacy URL mapping | `website/legacy-routes.json` and content preparation |
| Content, route, and link checks | `website/scripts/` |
| Browser QA | `website/tests/` |

Keep the Japanese and English page sets aligned. There is no Japanese fallback for missing English content, so one-sided additions fail validation. Separate released behavior from repository-only changes and design plans, and label runnable code with its supported version and backend.

`website/scripts/prepare-content.mjs` imports selected legacy documents and `CHANGELOG.md` into generated content. Do not edit generated files directly; change the canonical source or the import rule.

## GitHub Pages

`.github/workflows/pages.yml` installs the lockfile with Node.js 24, checks tutorial sources with Python, then runs type checking, static build, route/link validation, and browser QA. Pull requests retain the built artifact. Deployment happens only after a site change reaches `main` and all checks pass. The workflow does not change the custom domain, DNS, or repository visibility.

`website/public/CNAME` is the single source for the static custom-domain file. Do not keep a duplicate CNAME elsewhere.

The official domain has an empty base path. Forks and project Pages pass the path returned by `actions/configure-pages` as `PAGES_BASE_PATH`. Internal links, search requests, images, canonical URLs, and hreflang alternates are checked in both forms.

## Review documentation

Lead with a code block, then explain the syntax, exact invocation and result, followed by a practical example. A feature matrix or source link does not replace teaching how to write the code. Keep Japanese routes fully Japanese and English routes fully English, while preserving API identifiers.

The build emits canonical URLs, hreflang, sitemap, robots, and a scoped `llms.txt` for discovery. They do not guarantee recommendation or ranking. Support performance and security claims with measured conditions or implementation/test evidence.
