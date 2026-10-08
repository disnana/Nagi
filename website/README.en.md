# Nagi introduction and Docs site

Build the introduction and Japanese/English Docs as a static site for GitHub Pages. The home page describes Nagi's purpose, a runnable code example, the name, and the current limits. Docs content comes from repository Markdown.

Japanese Docs use existing Markdown such as `docs/*.md`; English pages use `docs/en/`. The public site is `https://nagi.disnana.com/`, with English pages under `/en/`. The builder sets language metadata, rewrites local links, and checks destinations, heading anchors, CSS, and JavaScript. Links to source code and example folders go to GitHub.

## Local preview

Use Python 3.12 or later. Run these commands from the repository root:

```bash
python -m venv build/website-venv
build/website-venv/bin/python -m pip install -r website/requirements.txt
build/website-venv/bin/python website/build.py --base-path / --out build/website-preview
python -m http.server 4173 --directory build/website-preview
```

On Windows, use `build\website-venv\Scripts\python.exe` for the virtual environment's Python.

Open `http://localhost:4173` in a browser. Review the home page, Docs index and articles, mobile contents navigation, and code-copy controls. Stop the server with Ctrl+C.

To build for publishing, run this from the repository root. Output goes to `build/website/`:

```bash
build/website-venv/bin/python website/build.py
```

The official repository uses `website/CNAME` and the `/` site root. The output includes `CNAME`. The Pages workflow and local build use the same settings.

For a fork, do not use the official `CNAME`. The builder derives the normal GitHub Pages URL and `/repository-name/` path from `--repository https://github.com/OWNER/REPOSITORY`. An `OWNER.github.io` repository uses `/`. Explicit `--base-path` and `--site-url` options override these defaults. To build the official repository for project Pages, use `--base-path /Nagi/ --site-url https://disnana.github.io`.

## Publish with GitHub Pages

Add `.github/workflows/pages.yml` to the default branch of the repository to publish, then:

1. In GitHub **Settings → Pages → Build and deployment**, set the source to **GitHub Actions**.
2. Open **Actions → Nagi website → Run workflow**.
3. Select the default branch and run the workflow.
4. After `deploy` succeeds, open the Pages URL shown by GitHub.

For a pull request, the workflow builds and checks the site, then saves the HTML as an Actions artifact. **When a site change reaches `main`, it deploys automatically after checks pass.** To publish the first time or redeploy, run the workflow from `main`. The workflow does not change the repository's visibility, custom domain, or other Pages settings.

You can serve the contents of `build/website/` elsewhere. It includes `.nojekyll` so GitHub Pages does not process the output with Jekyll.

## Update content

| Content | Files to edit |
|---|---|
| Home copy, name, and development status | Japanese: `website/templates/home.html`; English: `home.en.html` |
| Home code example | `website/examples/double.nagi`; run it and confirm the output |
| Docs body | Japanese: `docs/*.md` and sample READMEs; English: `docs/en/*.md` |
| Docs groups and page list | `GROUPS`, `ENGLISH_GROUPS`, and `EXTRA` in `website/build.py` |
| Typography, spacing, colors, responsive layout | `website/assets/site.css` |
| Shared navigation and metadata | `website/templates/page.html` |

When adding a Docs page, add the Japanese and English content and include both in the site navigation. If either body is missing or the page lists differ, the build stops. Keep translations in sync when editing a page. Pages must be added explicitly; the site does not publish every Markdown file automatically.

## Review explanations

- Describe Nagi as a backend language with Python-style syntax. Treat writing common operations without Rust knowledge as a goal, and link to runnable samples and references for implemented features.
- Current application builds use Rust/Cargo. Explain Low as a way to inspect code or replace a function; do not imply that unsupported pointers or `unsafe` are available.
- Distinguish a published feature from a repository change or a design proposal. Link performance statements to their conditions and results.
- For procedures, give the prerequisites, commands, and expected results in that order. Keep the home page brief; explain types, resource lifetimes, and detailed behavior in the relevant Docs.

The site build verifies generated HTML and local links. It does not establish that feature descriptions are correct or code samples run; check those against the implementation, tests, and execution results as well.
