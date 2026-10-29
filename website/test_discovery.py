"""Verify public discovery against generated routes, including project Pages."""
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch
from urllib.parse import unquote

import build


class DiscoveryTests(unittest.TestCase):
    def test_published_prompt_syntax_stays_in_the_bilingual_entry_page(self):
        japanese = (build.ROOT / "docs/use-cases.md").read_text(encoding="utf-8")
        english = (build.ROOT / "docs/en/use-cases.md").read_text(encoding="utf-8")
        example = (build.ROOT / "examples/tutorial/prompt-input.nagi").read_text(encoding="utf-8")
        self.assertIn('name = try read_line("名前 > ")', japanese)
        self.assertIn('name = try read_line("Name > ")', english)
        self.assertIn("return ok(print(name))", japanese)
        self.assertIn("return ok(print(name))", english)
        self.assertIn('name = try read_line("名前: ")', example)
        self.assertIn("return ok(print(name))", example)

    def test_capability_examples_are_source_backed_and_discoverable(self):
        japanese = (build.ROOT / "docs/use-cases.md").read_text(encoding="utf-8")
        english = (build.ROOT / "docs/en/use-cases.md").read_text(encoding="utf-8")
        task_api = (build.ROOT / "test-nagi-code/web-demo/tasks.nagi").read_text(encoding="utf-8")
        task_results = (build.ROOT / "test-nagi-code/library-examples/task-results/main.nagi").read_text(encoding="utf-8")
        supervisor = (build.ROOT / "test-nagi-code/library-examples/supervised-service/main.nagi").read_text(encoding="utf-8")
        rust_library = (build.ROOT / "test-nagi-code/rust-library/library-pricing.nagi").read_text(encoding="utf-8")
        for source, marker in [
            (task_api, "input = try json_decode[SaveTask](request.body)"),
            (task_results, "transferred = move(original)"),
            (supervisor, "counter = try actor.register[CounterState, Message, i64, CounterError]("),
            (rust_library, 'extern def quote(product: view[str], unit_price_cents: i64, quantity: i64)'),
        ]:
            self.assertIn(marker, source)
            self.assertIn(marker, japanese)
            self.assertIn(marker, english)
        for text in (japanese, english):
            self.assertIn("nagic run --project test-nagi-code/web-demo --profile dev", text)
            self.assertIn("PASS: 21 HTTP checks", text)
            self.assertIn("nagic run --project test-nagi-code/library-examples/task-results", text)
            self.assertIn("nagic run --project test-nagi-code/library-examples/supervised-service", text)
            self.assertIn("nagic run --project test-nagi-code/rust-library", text)
        self.assertEqual(japanese.count("単体fileとしては使いません"), 4)
        self.assertEqual(english.count("it is not a standalone file"), 4)

    def test_builtins_have_a_checked_first_program(self):
        japanese = (build.ROOT / "docs/builtins.md").read_text(encoding="utf-8")
        english = (build.ROOT / "docs/en/builtins.md").read_text(encoding="utf-8")
        for text in (japanese, english):
            self.assertIn('number = try parse_i64(view(text))', text)
            self.assertIn('nagic check builtins.nagi', text)
            self.assertIn('nagic run builtins.nagi --profile dev', text)
            self.assertIn('Nagi 0.2.2', text)

    def test_pages_rebuilds_when_the_canonical_changelog_changes(self):
        workflow = (build.ROOT / ".github/workflows/pages.yml").read_text(encoding="utf-8")
        self.assertIn("'CHANGELOG.md'", workflow)

    def test_generated_indexes_resolve_for_official_and_fork_sites(self):
        (build.ROOT / "build").mkdir(exist_ok=True)
        for repo, origin, base in [
            (build.DEFAULT_REPOSITORY, "https://nagi.disnana.com", "/"),
            ("https://github.com/example/Nagi-fork", "https://example.github.io", "/Nagi-fork/"),
        ]:
            with self.subTest(repo=repo), tempfile.TemporaryDirectory(dir=build.ROOT / "build") as parent:
                output = Path(parent) / "site"
                build.build(output, base, origin, repo, "feature/docs")
                index = (output / "llms.txt").read_text(encoding="utf-8")
                self.assertIn(f"{repo}/raw/feature%2Fdocs/ai/skills/nagi-development/SKILL.md", index)
                self.assertIn("Published compiler baseline: 0.2.2", index)
                self.assertIn("planned 0.3.0 Go target", index)
                self.assertIn(f"{origin}{base}en/docs/changelog/", index)
                self.assertNotIn("docs/internal/", index)
                self.assertNotIn("local-only/", index)
                urls = re.findall(r"\]\((https://[^)]+)\)", index)
                self.assertGreater(len(urls), 10)
                for url in urls:
                    if url.startswith(origin + base):
                        route = unquote(url[len(origin + base):])
                        self.assertTrue((output / route / "index.html").is_file(), url)
                    else:
                        self.assertTrue(url.startswith(repo + "/"), url)
                robots = (output / "robots.txt").read_text(encoding="utf-8")
                self.assertIn(f"Sitemap: {origin}{base}sitemap.xml", robots)
                for path in ["docs/developer-workflow/", "en/docs/comparison/", "docs/user-ci/",
                             "docs/changelog/", "en/docs/changelog/"]:
                    content = (output / path / "index.html").read_text(encoding="utf-8")
                    self.assertIn(f'href="{base}llms.txt"', content)
                    self.assertIn(f'href="{origin}{base}{path}"', content)
                    if path.endswith("changelog/"):
                        self.assertIn("Nagi 0.2.2", content)
                        self.assertIn("Unreleased", content)
                        self.assertIn("data-changelog-filter", content)
                        filter_position = content.index("data-changelog-filter")
                        self.assertLess(content.index("<h1"), filter_position)
                        self.assertLess(filter_position, content.index("<h2", filter_position))
                        self.assertNotIn("Supervisor shutdown stability", content)
                        self.assertLess(len(content.encode("utf-8")), 35_000)
                        self.assertIn(
                            f'{repo}/blob/feature%2Fdocs/CHANGELOG.md', content
                        )
                for prefix in ("", "en/"):
                    version = output / prefix / "docs/changelog/nagi-0-2-2/index.html"
                    self.assertTrue(version.is_file())
                    detail = version.read_text(encoding="utf-8")
                    self.assertIn("Build and dependency selection", detail)
                    self.assertIn(f'href="{base}{prefix}docs/changelog/"', detail)
                    self.assertIn('aria-current="page">Changelog</a>' if prefix else 'aria-current="page">変更履歴</a>', detail)
                    other_prefix = "" if prefix else "en/"
                    self.assertIn(
                        f'class="language-link" href="{base}{other_prefix}docs/changelog/nagi-0-2-2/"',
                        detail,
                    )
                japanese_index = (output / "docs/changelog/index.html").read_text(encoding="utf-8")
                self.assertIn('id="nagi-022-2026-10-27"', japanese_index)
                self.assertIn('id="未リリース-unreleased"', japanese_index)
                sitemap = (output / "sitemap.xml").read_text(encoding="utf-8")
                self.assertIn(f"{origin}{base}docs/changelog/nagi-0-2-2/", sitemap)
                self.assertIn(f"{origin}{base}en/docs/changelog/nagi-0-2-2/", sitemap)
                if repo != build.DEFAULT_REPOSITORY:
                    self.assertFalse((output / "CNAME").exists())
                    self.assertNotIn("nagi.disnana.com", index)

    def test_missing_agent_source_fails_instead_of_publishing_a_broken_index(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            with patch.object(build, "ROOT", output), self.assertRaisesRegex(ValueError, "Missing agent documentation"):
                build.discovery_files(output, "/", "https://example.com", build.DEFAULT_REPOSITORY, "main")
            self.assertFalse((output / "llms.txt").exists())

    def test_changelog_is_split_from_one_canonical_source(self):
        text = """# Changelog

## Unreleased

- Work in progress.

### Notes

Not released.

## Nagi 1.2.3 — 2026-01-02

- A [linked](https://example.com) feature with `code`.

### Details

Complete notes.
"""
        sections = build.changelog_sections(text)
        self.assertEqual([entry[1] for entry in sections], ["unreleased", "nagi-1-2-3"])
        index, routes = build.changelog_index(sections, "en")
        self.assertIn("linked feature with code", index)
        self.assertNotIn("Complete notes", index)
        self.assertEqual(routes["changelog/nagi-1-2-3/"],
                         "docs/changelog/nagi-1-2-3/")
        detail = build.changelog_detail(sections, 1, "en")
        self.assertIn("## Details", detail)
        self.assertIn("[← Newer: Unreleased]", detail)
        self.assertIn("[All versions]", detail)


if __name__ == "__main__":
    unittest.main()
