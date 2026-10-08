"""Release note source/range tests; no GitHub requests or publication occur."""
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import notes
import publish
from package import PLATFORMS, archive_name
from test_release import FakeGitHub


class NotesGitHub(FakeGitHub):
    repository = "owner/repo"

    def __init__(self, history=(), refs=None, release=None):
        super().__init__()
        self.history = list(history)
        self.refs = dict(refs or {})
        self.release = release
        self.generated = []
        self.events = []
        self.fail_history = False
        self.bad_readback = False

    def api(self, resource):
        self.api_calls.append(resource)
        if resource.startswith("git/ref/tags/"):
            sha = self.refs.get(resource.removeprefix("git/ref/tags/"))
            return {"object": {"type": "commit", "sha": sha}} if sha else None
        if resource.startswith("releases/tags/"):
            return self.release if self.release and not self.release["draft"] and resource.endswith(self.release["tag_name"]) else None
        if resource.startswith("releases?per_page=100&page="):
            if self.fail_history:
                return None
            page = int(resource.rsplit("=", 1)[1])
            values = self.history + ([self.release] if self.release else [])
            return values[(page - 1) * 100:page * 100]
        if resource == "releases/1":
            if self.bad_readback and any(call[0] == "notes" for call in self.calls):
                return dict(self.release, body="wrong readback")
            return self.release
        if resource == "releases/latest":
            return {"tag_name": "vscode-v99.0.0"}
        raise AssertionError(resource)

    def create_tag(self, tag, sha):
        self.events.append("tag")
        super().create_tag(tag, sha)
        self.refs[tag] = sha

    def generate_notes(self, tag, sha, previous_tag):
        self.events.append("generate")
        self.generated.append((tag, sha, previous_tag))
        return {"body": "## What's Changed\n* Add typed actors by @maintainer in #42\n\n## New Contributors\n* @newcomer made their first contribution in #42"}

    def update_notes(self, release_id, body):
        self.calls.append(("notes", release_id, body))
        self.release["body"] = body
        return dict(self.release)


def previous(tag, *, draft=False, prerelease=False):
    return {"tag_name": tag, "draft": draft, "prerelease": prerelease}


class ReleaseNotesTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.write_changelog("# Changelog\n\n## Unreleased\n\n- Future work.\n\n## Nagi 0.1.8 / VS Code 0.1.10\n\n- Committed typed actor changes.\n- 凪 extension navigation.\n\n## Nagi 0.1.7 / VS Code 0.1.9\n\n- Previous changes.\n")
        self.sha = self.commit()
        root_patch = patch.object(notes, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        self.assets = self.root / "assets"
        self.assets.mkdir()
        for filename in [archive_name("0.1.8", platform) for platform in PLATFORMS] + ["nagi-language-0.1.10.vsix"]:
            path = self.assets / filename
            path.write_bytes(b"tested package")
            (self.assets / (filename + ".sha256")).write_text(f"{publish.hashlib.sha256(path.read_bytes()).hexdigest()}  {filename}\n")

    def write_changelog(self, text):
        (self.root / "CHANGELOG.md").write_text(text, encoding="utf-8")

    def commit(self):
        subprocess.run(["git", "add", "CHANGELOG.md"], cwd=self.root, check=True)
        subprocess.run(["git", "-c", "user.name=Release Test", "-c", "user.email=test@example.invalid",
                        "commit", "-qm", "release fixture"], cwd=self.root, check=True)
        return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.root).decode().strip()

    def publish(self, client, component="nagi"):
        publish.publish(client, component, "0.1.8" if component == "nagi" else "0.1.10", self.sha, self.assets)

    def test_component_history_and_immutable_source_determine_notes_and_generator_range(self):
        history = [previous("vscode-v99.0.0"), previous("nagi-v0.1.6"), previous("nagi-v0.1.7"),
                   previous("nagi-v0.1.9", draft=True), previous("nagi-v0.1.7", prerelease=True),
                   previous("nagi-v0.1.8-beta.1"), previous("nagi-v0.2.0")]
        client = NotesGitHub(history, {"nagi-v0.1.7": "b" * 40})
        self.write_changelog("Uncommitted private notes must never be published")
        self.publish(client)
        body = client.release["body"]
        self.assertIn("Committed typed actor changes", body)
        self.assertNotIn("private notes", body)
        self.assertNotIn("Future work", body)
        self.assertNotIn("Previous changes", body)
        self.assertIn(f"/compare/nagi-v0.1.7...{self.sha}", body)
        self.assertIn(f"/compare/{'b' * 40}...{self.sha}", body)
        self.assertIn(f"/blob/{self.sha}/CHANGELOG.md", body)
        self.assertIn("@newcomer made their first contribution", body)
        self.assertEqual(client.generated, [("nagi-v0.1.8", self.sha, "nagi-v0.1.7")])
        self.assertEqual(client.events[:2], ["generate", "tag"])
        self.assertLess(body.index("## Installation"), body.index("## Changes"))
        self.assertLess(body.index("## Changes"), body.index("## Release comparison"))
        self.assertLess(body.index("## Release comparison"), body.index("## Pull requests and contributors"))

    def test_vscode_uses_its_own_previous_formal_release(self):
        client = NotesGitHub([previous("nagi-v0.1.7"), previous("vscode-v0.1.8"), previous("vscode-v0.1.9"),
                             previous("vscode-v0.1.11", draft=True)], {"vscode-v0.1.9": "c" * 40})
        self.publish(client, "vscode")
        self.assertEqual(client.generated, [("vscode-v0.1.10", self.sha, "vscode-v0.1.9")])
        self.assertIn(f"/compare/vscode-v0.1.9...{self.sha}", client.release["body"])
        self.assertNotIn("/compare/nagi-", client.release["body"])

    def test_jetbrains_notes_use_the_jetbrains_entry_and_previous_plugin_release(self):
        self.write_changelog(
            "# Changelog\n\n## Unreleased\n\n- Future work.\n\n"
            "## JetBrains 0.1.8 — 2026-10-07\n\n- Ship one common ZIP for IDEA and PyCharm.\n\n"
            "## JetBrains 0.1.7\n\n- Previous plugin changes.\n"
        )
        self.sha = self.commit()
        client = NotesGitHub(
            [previous("nagi-v0.1.7"), previous("vscode-v0.1.10"), previous("jetbrains-v0.1.7"),
             previous("jetbrains-v0.1.9", draft=True)],
            {"jetbrains-v0.1.7": "b" * 40},
        )
        body = notes.release_notes(client, "jetbrains", "0.1.8", self.sha, "Install the common JetBrains ZIP.")
        self.assertIn("Ship one common ZIP for IDEA and PyCharm", body)
        self.assertNotIn("Future work", body)
        self.assertIn("[Previous tag → released commit]", body)
        self.assertIn("/compare/jetbrains-v0.1.7...", body)
        self.assertNotIn("/compare/nagi-", body)
        self.assertNotIn("/compare/vscode-", body)
        self.assertEqual(client.generated, [("jetbrains-v0.1.8", self.sha, "jetbrains-v0.1.7")])
        self.assertIn("First JetBrains", notes.release_notes(NotesGitHub([previous("nagi-v0.1.7")]),
                                                             "jetbrains", "0.1.8", self.sha, "Install."))

    def test_first_component_release_skips_generator_even_when_another_component_exists(self):
        for component, foreign in [("nagi", "vscode-v99.0.0"), ("vscode", "nagi-v0.1.7")]:
            with self.subTest(component=component):
                client = NotesGitHub([previous(foreign)])
                self.publish(client, component)
                self.assertEqual(client.generated, [])
                self.assertIn("First ", client.release["body"])
                self.assertIn(f"/tree/{self.sha}", client.release["body"])
                self.assertNotIn("/compare/", client.release["body"])

    def test_previous_release_selection_reads_every_page(self):
        client = NotesGitHub([previous(f"other-v{i}") for i in range(100)] + [previous("nagi-v0.1.7")],
                             {"nagi-v0.1.7": "b" * 40})
        self.publish(client)
        self.assertIn("releases?per_page=100&page=2", client.api_calls)
        self.assertEqual(client.generated[0][2], "nagi-v0.1.7")

    def test_draft_prerelease_and_nonformal_tags_cannot_be_the_previous_baseline(self):
        client = NotesGitHub([previous("nagi-v0.1.5"), previous("nagi-v0.1.7", draft=True),
                             previous("nagi-v0.1.6", prerelease=True), previous("nagi-v0.1.7-rc.1"),
                             previous("vscode-v99.0.0")])
        self.assertEqual(notes.previous_release(client, "nagi", "0.1.8")["tag_name"], "nagi-v0.1.5")

    def test_missing_duplicate_empty_or_wrong_version_entry_fails_before_mutations(self):
        for text in ["# Changelog\n## Unreleased\n- Work\n", "## Nagi 0.1.80 / VS Code 0.1.10\n- Wrong version\n",
                     "## Nagi 0.1.8\n\n", "## Nagi 0.1.8\n- One\n## Nagi 0.1.8\n- Two\n"]:
            with self.subTest(text=text):
                self.write_changelog(text)
                self.sha = self.commit()
                client = NotesGitHub()
                with self.assertRaisesRegex(ValueError, "exactly one nonempty"):
                    self.publish(client)
                self.assertEqual(client.calls, [])
                self.assertEqual(client.generated, [])

    def test_missing_changelog_file_fails_before_mutations(self):
        (self.root / "CHANGELOG.md").unlink()
        self.sha = self.commit()
        client = NotesGitHub()
        with self.assertRaisesRegex(RuntimeError, "Could not read CHANGELOG"):
            self.publish(client)
        self.assertEqual(client.calls, [])

    def test_history_or_previous_tag_failure_stops_before_mutations(self):
        client = NotesGitHub()
        client.fail_history = True
        with self.assertRaisesRegex(RuntimeError, "Could not .*releases"):
            self.publish(client)
        self.assertEqual(client.calls, [])
        client = NotesGitHub([previous("nagi-v0.1.7")])
        with self.assertRaisesRegex(RuntimeError, "no commit tag"):
            self.publish(client)
        self.assertEqual(client.calls, [])
        self.assertEqual(client.generated, [])

    def test_generation_failure_stops_before_tag_or_draft_creation(self):
        for response in [None, {}, {"body": ""}]:
            with self.subTest(response=response):
                client = NotesGitHub([previous("nagi-v0.1.7")], {"nagi-v0.1.7": "b" * 40})
                with patch.object(client, "generate_notes", return_value=response):
                    with self.assertRaisesRegex(RuntimeError, "generated release notes"):
                        self.publish(client)
                self.assertEqual(client.calls, [])
        client = NotesGitHub([previous("nagi-v0.1.7")], {"nagi-v0.1.7": "b" * 40})
        with patch.object(client, "generate_notes", side_effect=RuntimeError("API failed")):
            with self.assertRaisesRegex(RuntimeError, "API failed"):
                self.publish(client)
        self.assertEqual(client.calls, [])

    def test_published_release_keeps_immutable_notes_without_source_or_history_reads(self):
        release = {"id": 1, "tag_name": "vscode-v0.1.10", "draft": False, "assets": [], "body": "Original reviewed notes"}
        client = NotesGitHub(refs={release["tag_name"]: self.sha}, release=release)
        for name in ["nagi-language-0.1.10.vsix", "nagi-language-0.1.10.vsix.sha256"]:
            client.add(name, (self.assets / name).read_bytes())
        client.fail_history = True
        with patch.object(notes, "changelog_entry", side_effect=AssertionError("source must not be read")):
            self.publish(client, "vscode")
        self.assertEqual(client.release["body"], "Original reviewed notes")
        self.assertEqual(client.generated, [])
        self.assertEqual(client.calls, [])
        self.assertFalse(any("per_page" in call for call in client.api_calls))

    def test_draft_notes_are_updated_and_read_back_while_matching_assets_are_kept(self):
        release = {"id": 1, "tag_name": "vscode-v0.1.10", "draft": True, "assets": [], "body": "Stale draft notes"}
        client = NotesGitHub(refs={release["tag_name"]: self.sha}, release=release)
        client.add("nagi-language-0.1.10.vsix", (self.assets / "nagi-language-0.1.10.vsix").read_bytes())
        self.publish(client, "vscode")
        self.assertEqual([call[0] for call in client.calls], ["notes", "upload", "edit"])
        self.assertIn("Committed typed actor changes", client.release["body"])
        self.assertFalse(client.release["draft"])

    def test_failed_draft_note_readback_prevents_upload_and_publication(self):
        release = {"id": 1, "tag_name": "vscode-v0.1.10", "draft": True, "assets": [], "body": "Stale notes"}
        client = NotesGitHub(refs={release["tag_name"]: self.sha}, release=release)
        client.bad_readback = True
        with self.assertRaisesRegex(RuntimeError, "Draft note readback"):
            self.publish(client, "vscode")
        self.assertEqual([call[0] for call in client.calls], ["notes"])
        self.assertTrue(client.release["draft"])

    def test_optional_date_and_fenced_heading_examples_preserve_the_exact_entry(self):
        self.write_changelog("## Nagi 0.1.8 / VS Code 0.1.10 — 2026-10-03\n\n- Dated entry.\n```markdown\n## Nagi 0.1.8\n```\n## Unreleased\n- Later work.\n")
        self.sha = self.commit()
        entry = notes.changelog_entry(self.sha, "nagi", "0.1.8")
        self.assertIn("Dated entry", entry)
        self.assertIn("## Nagi 0.1.8", entry)
        self.assertNotIn("Later work", entry)

    def test_fence_marker_with_info_is_code_content_and_not_a_closing_fence(self):
        for marker in ["```", "~~~"]:
            with self.subTest(marker=marker):
                entry = f"- Actual release.\n{marker}markdown\n{marker}python\n## Nagi 0.1.8\nThis heading is code, not another release.\n{marker} \t"
                self.write_changelog(f"# Changelog\n\n## Nagi 0.1.8\n\n{entry}\n\n## Nagi 0.1.7\n- Older release.\n")
                self.sha = self.commit()
                self.assertEqual(notes.changelog_entry(self.sha, "nagi", "0.1.8"), entry.rstrip())


class NoteProtocolTests(unittest.TestCase):
    def test_generator_and_draft_update_use_structured_json_stdin(self):
        client = publish.GitHub("owner/repo")
        body = "notes\n凪 `$(touch private)`\n"
        response = subprocess.CompletedProcess([], 0, stdout=json.dumps({"body": body}).encode(), stderr=b"")
        with patch.object(publish.subprocess, "run", return_value=response) as run:
            client.generate_notes("nagi-v0.1.8", "a" * 40, "nagi-v0.1.7")
            self.assertEqual(run.call_args.args[0][:5], ["gh", "api", "--method", "POST", "repos/owner/repo/releases/generate-notes"])
            self.assertEqual(json.loads(run.call_args.kwargs["input"]), {
                "tag_name": "nagi-v0.1.8", "target_commitish": "a" * 40, "previous_tag_name": "nagi-v0.1.7"})
            client.update_notes(42, body)
            self.assertEqual(run.call_args.args[0][:5], ["gh", "api", "--method", "PATCH", "repos/owner/repo/releases/42"])
            self.assertEqual(json.loads(run.call_args.kwargs["input"]), {"body": body})
            self.assertNotIn(body, run.call_args.args[0])
            client.create_tag("nagi-v0.1.8", "a" * 40)
            self.assertEqual(json.loads(run.call_args.kwargs["input"]), {"ref": "refs/tags/nagi-v0.1.8", "sha": "a" * 40})


if __name__ == "__main__":
    unittest.main()
