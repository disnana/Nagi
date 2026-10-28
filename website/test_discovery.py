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
                        self.assertIn(
                            f'{repo}/blob/feature%2Fdocs/CHANGELOG.md', content
                        )
                if repo != build.DEFAULT_REPOSITORY:
                    self.assertFalse((output / "CNAME").exists())
                    self.assertNotIn("nagi.disnana.com", index)

    def test_missing_agent_source_fails_instead_of_publishing_a_broken_index(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            with patch.object(build, "ROOT", output), self.assertRaisesRegex(ValueError, "Missing agent documentation"):
                build.discovery_files(output, "/", "https://example.com", build.DEFAULT_REPOSITORY, "main")
            self.assertFalse((output / "llms.txt").exists())


if __name__ == "__main__":
    unittest.main()
