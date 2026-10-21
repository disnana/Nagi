import copy
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("verify_build_receipt", Path(__file__).resolve().parents[1] / "verify_build_receipt.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.app = "nagi-main-0123456789abcdef"
        self.generation = "g-1-2-3"
        self.root = Path(self.temporary.name) / self.app / "generations" / self.generation
        self.root.mkdir(parents=True)
        self.executable = "nagi-0123456789abcdef-g-1-2-3"
        self.compiler = Path(self.temporary.name) / "compiler"
        self.compiler.write_bytes(b"compiler")
        self.receipt = {"schema_version": 1, "app_id": self.app, "generation": self.generation,
                        "compiler": MODULE.identity(self.compiler), "artifacts": {}}
        for name in MODULE.FIXED | {self.executable}:
            path = self.root / name
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(name.encode())
            self.receipt["artifacts"][name] = MODULE.identity(path)
        self.path = self.root / "build-receipt.json"
        self.write()

    def write(self):
        self.path.write_text(json.dumps(self.receipt), encoding="utf-8")

    def configure_schema2(self):
        self.receipt["schema_version"] = 2
        body_manifest_bytes = b'[package]\nname = "nagi-body-test"\nversion = "0.1.0"\n'
        body_main_bytes = b"pub fn generated_body() {}\n"
        for name, contents in [("body-Cargo.toml", body_manifest_bytes),
                               ("src/entry.rs", b"fn main() {}\n"),
                               ("src/main.rs", body_main_bytes)]:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(contents)
            self.receipt["artifacts"][name] = MODULE.identity(path)

        framed = hashlib.sha256()
        framed.update(b"nagi-generated-body-v1\0")
        for contents in (body_manifest_bytes, body_main_bytes):
            framed.update(len(contents).to_bytes(8, "big"))
            framed.update(contents)
        digest = framed.hexdigest()
        self.receipt["generated_body_sha256"] = digest
        app_suffix = self.app.rsplit("-", 1)[-1]
        cargo = ("[[bin]]\nname = \"nagi-app-test\"\npath = \"src/entry.rs\"\n\n"
                 "[dependencies.nagi-runtime]\npath = \"../../code/nagi-runtime\"\n\n"
                 "[package]\nautobins = false\nedition = \"2021\"\n"
                 "name = \"nagi-app-test\"\nversion = \"0.1.0\"\n\n"
                 "[profile.release]\ncodegen-units = 1\nlto = false\n"
                 "opt-level = 3\npanic = \"unwind\"\n\n"
                 "[workspace]\nexclude = [\"../../code/ignored\"]\n\n"
                 "[dependencies.nagi-app-body]\n"
                 f'path = "../../code/{digest}"\n'
                 f'package = "nagi-body-{app_suffix}"\n').encode()
        cargo_path = self.root / "Cargo.toml"
        cargo_path.write_bytes(cargo)
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)

        cas_root = self.root.parent.parent / "code" / digest
        (cas_root / "src").mkdir(parents=True, exist_ok=True)
        (cas_root / "Cargo.toml").write_bytes(body_manifest_bytes)
        (cas_root / "src" / "lib.rs").write_bytes(body_main_bytes)
        self.write()
        return digest

    def schema3_node(self, unit, dependencies=(), *, edit=None, source=b"pub fn value() {}\n"):
        suffix = self.app.rsplit("-", 1)[-1]
        package = f"nagi-unit-{suffix}-{unit}" if unit else f"nagi-body-{suffix}"
        library = f"nagi_unit_{unit}" if unit else "nagi_app_body"
        runtime = str((Path(self.temporary.name) / 'runtime\\quote"\n\x7f雪').resolve())
        quoted = lambda value: json.dumps(value, ensure_ascii=False).replace("\x7f", "\\u007f")
        manifest = (f"[package]\nname = {quoted(package)}\nversion = \"0.1.0\"\n"
                    "edition = \"2021\"\nbuild = false\nautobins = false\n"
                    "autoexamples = false\nautotests = false\nautobenches = false\n"
                    f"[lib]\nname = {quoted(library)}\npath = \"src/lib.rs\"\n"
                    f"[workspace]\n[dependencies.nagi-runtime]\npath = {quoted(runtime)}\n")
        for dependency in dependencies:
            manifest += (f"[dependencies.nagi_unit_{dependency['unit']}]\n"
                         f"path = {quoted(str(dependency['path']))}\n"
                         f"package = {quoted(dependency['package'])}\n")
        if edit:
            manifest = edit(manifest)
        manifest = manifest.encode("utf-8")
        framed = hashlib.sha256(MODULE.GENERATED_BODY_DOMAIN)
        for contents in (manifest, source):
            framed.update(len(contents).to_bytes(8, "big"))
            framed.update(contents)
        digest = framed.hexdigest()
        directory = self.root.parent.parent / "code" / digest
        (directory / "src").mkdir(parents=True, exist_ok=True)
        # The compiler publishes dependency paths from its canonical output
        # root. Match that producer contract, including macOS /var aliases
        # and Windows short-name temporary directories.
        directory = directory.resolve(strict=True)
        (directory / "Cargo.toml").write_bytes(manifest)
        (directory / "src/lib.rs").write_bytes(source)
        return {"unit": unit, "path": directory, "digest": digest,
                "package": package, "runtime": runtime}

    def schema3_finish(self, dependencies):
        body = self.schema3_node(None, dependencies)
        self.receipt["schema_version"] = 3
        self.receipt["generated_body_sha256"] = body["digest"]
        for name, contents in [("body-Cargo.toml", (body["path"] / "Cargo.toml").read_bytes()),
                               ("src/main.rs", (body["path"] / "src/lib.rs").read_bytes()),
                               ("src/entry.rs", b"fn main() { ::nagi_app_body::main(); }\n")]:
            (self.root / name).write_bytes(contents)
            self.receipt["artifacts"][name] = MODULE.identity(self.root / name)
        quoted = lambda value: json.dumps(value, ensure_ascii=False).replace("\x7f", "\\u007f")
        exclusions = [f'../../code/{body["digest"]}'] + [f'../../code/{unit["digest"]}' for unit in dependencies]
        manifest = (f'[[bin]]\nname = "{self.executable}"\npath = "src/entry.rs"\n'
                    f'[package]\nname = "{self.app}"\nversion = "0.1.0"\nedition = "2021"\nautobins = false\n'
                    '[profile.release]\nopt-level = 3\nlto = false\ncodegen-units = 1\npanic = "unwind"\n'
                    f'[workspace]\nexclude = {json.dumps(exclusions)}\n'
                    f'[dependencies.nagi-runtime]\npath = {quoted(body["runtime"])}\n')
        for unit in dependencies:
            manifest += (f'[dependencies.nagi_unit_{unit["unit"]}]\n'
                         f'path = {quoted(str(unit["path"]))}\npackage = {quoted(unit["package"])}\n')
        manifest += ('[dependencies.nagi-app-body]\n'
                     f'path = "../../code/{body["digest"]}"\npackage = "{body["package"]}"\n')
        (self.root / "Cargo.toml").write_text(manifest, encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(self.root / "Cargo.toml")
        self.write()
        return body

    def configure_schema3(self, *, count=3, chain=False, edit=None):
        units = []
        for index in range(count):
            dependencies = units[-1:] if chain or index == 1 else []
            units.append(self.schema3_node(f"{index + 1:016x}", dependencies,
                                          edit=edit if index == 0 else None))
        return self.schema3_finish(units), units

    def schema3_edit_root(self, transform):
        path = self.root / "Cargo.toml"
        path.write_text(transform(path.read_text(encoding="utf-8")), encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(path)
        self.write()

    def test_schema3_valid_shared_dependency_and_eight_unit_boundary(self):
        self.configure_schema3()
        MODULE.verify(self.path, self.compiler)
        self.configure_schema3(count=8, chain=True)
        MODULE.verify(self.path)

    def test_schema3_preserves_legacy_schema_boundaries(self):
        self.configure_schema3()
        self.receipt["schema_version"] = 2
        self.write()
        with self.assertRaisesRegex(ValueError, "unexpected generated Cargo.toml table|malformed generated Cargo.toml value"):
            MODULE.verify(self.path)
        self.receipt["schema_version"] = 1
        self.write()
        with self.assertRaisesRegex(ValueError, "unexpected, missing"):
            MODULE.verify(self.path)

    def test_schema3_root_dependency_runtime_and_workspace_mismatch(self):
        _, units = self.configure_schema3()
        original = (self.root / "Cargo.toml").read_text(encoding="utf-8")
        first = units[0]
        changes = [
            lambda s: s.replace(f'[dependencies.nagi_unit_{first["unit"]}]', '[dependencies.untrusted]'),
            lambda s: s.replace(first["package"], first["package"] + "0"),
            lambda s: s.replace(str(first["path"]).replace("\\", "\\\\"), "/outside/" + first["digest"]),
            lambda s: s.replace('exclude = [', 'exclude = ["../../code/other", '),
            lambda s: s.replace('path = "src/entry.rs"', 'path = "src/main.rs"'),
            lambda s: s.replace('opt-level = 3', 'opt-level = 2'),
            lambda s: s.replace('opt-level = 3', 'opt-level = 03'),
            lambda s: s.replace('[dependencies.nagi-runtime]\npath = ',
                                '[dependencies.nagi-runtime]\npath = "/other"\nother = '),
            lambda s: s.replace('[dependencies.nagi-app-body]', '[dependencies.nagi-app-body]\nextra = "no"'),
            lambda s: s + '[workspace.metadata]\nkey = "no"\n',
        ]
        for change in changes:
            with self.subTest(change=change):
                self.schema3_edit_root(lambda _: change(original))
                with self.assertRaises(ValueError):
                    MODULE.verify(self.path)

    def test_schema3_explicit_workspace_exclusions_accept_multiline_arrays(self):
        body, units = self.configure_schema3()
        entries = [f'../../code/{body["digest"]}'] + [f'../../code/{unit["digest"]}' for unit in units]
        single = json.dumps(entries)
        multiline = '[\n' + ''.join(f'    {json.dumps(entry)},\n' for entry in entries) + ']'
        self.schema3_edit_root(lambda text: text.replace(single, multiline))
        MODULE.verify(self.path)
        self.receipt["schema_version"] = 2
        self.write()
        with self.assertRaises(ValueError):
            MODULE.verify(self.path)

    def test_schema3_workspace_exclusions_are_complete_ordered_and_unique(self):
        body, units = self.configure_schema3()
        entries = [f'../../code/{body["digest"]}'] + [f'../../code/{unit["digest"]}' for unit in units]
        original = (self.root / "Cargo.toml").read_text(encoding="utf-8")
        single = json.dumps(entries)
        malformed = [
            json.dumps(entries[:-1]),
            json.dumps(list(reversed(entries))),
            json.dumps(entries + entries[:1]),
            json.dumps(['../../code/*']),
            json.dumps(['../../outside/' + body['digest']] + entries[1:]),
            json.dumps(['../../code/../code/' + body['digest']] + entries[1:]),
            '[]',
            '[' + ' '.join(json.dumps(entry) for entry in entries) + ']',
            '[\n' + ''.join(f'    {json.dumps(entry)}\n' for entry in entries) + ']',
            '[\n' + ''.join(f'    {json.dumps(entry)},\n' for entry in entries + entries[:1]) + ']',
            '[\n    "../../code/anything",\n# comment\n]',
            '[\n    "../../code/anything",\n',
            '[\n    "../../code\\/anything",\n]',
            '[\n    ["nested"],\n]',
        ]
        for replacement in malformed:
            with self.subTest(replacement=replacement):
                self.schema3_edit_root(lambda _: original.replace(single, replacement))
                with self.assertRaises(ValueError):
                    MODULE.verify(self.path)

    def test_schema3_closed_child_manifest_grammar_and_identities(self):
        edits = [
            lambda s: s.replace('version = "0.1.0"', 'version = "9.0.0"'),
            lambda s: s.replace('build = false', 'build = true'),
            lambda s: s.replace('name = "nagi_unit_', 'name = "wrong_'),
            lambda s: s.replace('[workspace]', '[workspace]\nresolver = "2"'),
            lambda s: s + '[dependencies.foreign]\npath = "/outside"\n',
            lambda s: s + '[package]\nname = "duplicate"\n',
            lambda s: s.replace('autobins = false', 'autobins = false\nautobins = false'),
            lambda s: s.replace('path = "src/lib.rs"', "path = 'src/lib.rs'"),
            lambda s: s.replace('path = "src/lib.rs"', 'path = "src\\/lib.rs"'),
            lambda s: s.replace('path = "src/lib.rs"', 'path = "\\ud800"'),
            lambda s: s.replace('path = "src/lib.rs"', 'path = "\\uD83D\\uDE00"'),
            lambda s: s.replace('path = "src/lib.rs"', 'path = "src/lib.rs" # comment'),
            lambda s: s.replace('path = "src/lib.rs"', 'path = """src/lib.rs"""'),
        ]
        for edit in edits:
            with self.subTest(edit=edit):
                self.configure_schema3(edit=edit)
                with self.assertRaises(ValueError):
                    MODULE.verify(self.path)

    def test_schema3_child_runtime_must_match_facade(self):
        child = self.schema3_node("1" * 16, edit=lambda s: s.replace('雪', 'other'))
        self.schema3_finish([child])
        with self.assertRaisesRegex(ValueError, "runtime path mismatch"):
            MODULE.verify(self.path)

    def test_schema3_conflicting_logical_unit_versions_are_rejected(self):
        old = self.schema3_node("1" * 16, source=b"pub fn old() {}\n")
        caller = self.schema3_node("2" * 16, [old])
        new = self.schema3_node("1" * 16, source=b"pub fn new() {}\n")
        self.schema3_finish([new, caller])
        with self.assertRaisesRegex(ValueError, "multiple digests"):
            MODULE.verify(self.path)

    def test_schema3_facade_must_list_transitive_units(self):
        leaf = self.schema3_node("1" * 16)
        caller = self.schema3_node("2" * 16, [leaf])
        self.schema3_finish([caller])
        with self.assertRaisesRegex(ValueError, "reference every unit"):
            MODULE.verify(self.path)

    def test_schema3_dependency_path_is_not_followed_outside_namespace(self):
        leaf = self.schema3_node("1" * 16)
        outside = Path(self.temporary.name) / "outside" / leaf["digest"]
        for path in [outside, leaf["path"].parent / ".." / "code" / leaf["digest"]]:
            with self.subTest(path=path):
                forged = dict(leaf, path=path)
                self.schema3_finish([forged])
                with self.assertRaisesRegex(ValueError, "dependency path"):
                    MODULE.verify(self.path)
                self.assertFalse(outside.exists())

    def test_schema3_child_corruption_extra_entries_missing_and_cycles_rejected(self):
        for mutation in ["source", "manifest", "extra", "missing", "cycle"]:
            with self.subTest(mutation=mutation):
                _, units = self.configure_schema3()
                child = units[0]
                directory = child["path"]
                if mutation == "source":
                    (directory / "src/lib.rs").write_bytes(b"changed\n")
                elif mutation == "manifest":
                    with (directory / "Cargo.toml").open("ab") as target:
                        target.write(b"\n")
                elif mutation == "extra":
                    (directory / "build.rs").write_bytes(b"not allowed")
                elif mutation == "missing":
                    (directory / "src/lib.rs").unlink()
                else:
                    with (directory / "Cargo.toml").open("a", encoding="utf-8") as target:
                        target.write(f'[dependencies.nagi_unit_{child["unit"]}]\n'
                                     f'path = {json.dumps(str(directory))}\npackage = "{child["package"]}"\n')
                with self.assertRaises((OSError, ValueError)):
                    MODULE.verify(self.path)
                # Remove invalid fixture only; the verifier must not repair it.
                shutil.rmtree(directory)

    def test_schema3_snapshot_identity_and_runtime_root_are_checked(self):
        self.configure_schema3()
        snapshot = self.root / "src/main.rs"
        snapshot.write_bytes(b"changed snapshot\n")
        self.receipt["artifacts"]["src/main.rs"] = MODULE.identity(snapshot)
        self.write()
        with self.assertRaisesRegex(ValueError, "generated body digest mismatch"):
            MODULE.verify(self.path)
        body, _ = self.configure_schema3()
        runtime = json.dumps(body["runtime"], ensure_ascii=False).replace("\x7f", "\\u007f")
        self.schema3_edit_root(lambda text: text.replace(runtime, '"/different-runtime"'))
        with self.assertRaisesRegex(ValueError, "root runtime path mismatch"):
            MODULE.verify(self.path)

    def test_schema3_lexical_receipt_directory_alias_is_supported(self):
        self.configure_schema3()
        # This trusted alias lies above the protected app/generations/generation
        # tail, like macOS's /var -> /private/var system directory alias.
        parent_alias = Path(self.temporary.name).with_name(Path(self.temporary.name).name + "-alias")
        try:
            parent_alias.symlink_to(Path(self.temporary.name), target_is_directory=True)
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        self.addCleanup(parent_alias.unlink)
        aliased = parent_alias / self.app / "generations" / self.generation / "build-receipt.json"
        MODULE.verify(aliased)

    def test_schema3_alias_above_output_and_receipt_matches_canonical_producer(self):
        # Reproduce two distinct lexical aliases even on Linux, where the
        # default temp root usually has no OS-provided alias.
        original = Path(self.temporary.name)
        aliases = [original.with_name(original.name + suffix) for suffix in ("-output", "-receipt")]
        for alias in aliases:
            try:
                alias.symlink_to(original, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"OS does not permit test symlinks: {error}")
            self.addCleanup(alias.unlink)
        self.root = aliases[0] / self.app / "generations" / self.generation
        self.path = self.root / "build-receipt.json"
        self.configure_schema3()
        MODULE.verify(self.path)
        MODULE.verify(aliases[1] / self.app / "generations" / self.generation / "build-receipt.json")

    def test_schema3_node_depth_and_file_size_limits(self):
        self.configure_schema3(count=9)
        with self.assertRaisesRegex(ValueError, "dependency count"):
            MODULE.verify(self.path)
        _, units = self.configure_schema3(count=9, chain=True)
        self.schema3_finish(list(reversed(units)))
        with mock.patch.object(MODULE, "V3_NODE_LIMIT", 100):
            with self.assertRaisesRegex(ValueError, "dependency depth"):
                MODULE.verify(self.path)
        self.configure_schema3()
        for name, limit in [("body-Cargo.toml", MODULE.V3_MANIFEST_LIMIT),
                            ("src/main.rs", MODULE.V3_SOURCE_LIMIT)]:
            previous = self.receipt["artifacts"][name]["bytes"]
            self.receipt["artifacts"][name]["bytes"] = limit + 1
            self.write()
            with self.assertRaisesRegex(ValueError, "exceeds size limit"):
                MODULE.verify(self.path)
            self.receipt["artifacts"][name]["bytes"] = previous
        _, units = self.configure_schema3()
        for name, length in [("Cargo.toml", MODULE.V3_MANIFEST_LIMIT + 1),
                             ("src/lib.rs", MODULE.V3_SOURCE_LIMIT + 1)]:
            path = units[0]["path"] / name
            original = path.read_bytes()
            with path.open("wb") as target:
                target.truncate(length)
            with self.assertRaisesRegex(ValueError, "exceeds size limit"):
                MODULE.verify(self.path)
            path.write_bytes(original)

    def test_schema3_child_file_and_directory_links_are_rejected(self):
        for name in ["Cargo.toml", "src/lib.rs", "src"]:
            with self.subTest(name=name):
                _, units = self.configure_schema3()
                original = units[0]["path"] / name
                moved = original.with_name(original.name + "-moved")
                original.rename(moved)
                try:
                    original.symlink_to(moved, target_is_directory=moved.is_dir())
                except OSError as error:
                    self.skipTest(f"OS does not permit test symlinks: {error}")
                with self.assertRaises(ValueError):
                    MODULE.verify(self.path)
                original.unlink()
                moved.rename(original)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "Unix named pipes")
    def test_schema3_child_fifo_cannot_block_cli(self):
        _, units = self.configure_schema3()
        source = units[0]["path"] / "src/lib.rs"
        source.unlink()
        os.mkfifo(source)
        result = subprocess.run([sys.executable, str(SPEC.origin), str(self.path)],
                                capture_output=True, text=True, timeout=5)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not a regular file", result.stderr)

    def test_valid_and_corrupt_bytes(self):
        MODULE.verify(self.path, self.compiler)
        (self.root / "src/main.rs").write_bytes(b"wrong")
        with self.assertRaisesRegex(ValueError, "integrity mismatch"):
            MODULE.verify(self.path)

    def test_schema2_valid_and_final_blank_line_is_accepted(self):
        self.configure_schema2()
        MODULE.verify(self.path, self.compiler)

        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8")
        cargo = cargo.replace('path = "../../code/nagi-runtime"',
                              'path = "../../code/runtime-\\u2603"')
        cargo = cargo.replace('name = "nagi-app-test"', 'name = "nagi-app-Δ"')
        cargo_path.write_text(cargo, encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        MODULE.verify(self.path)

        cargo_path.write_text(cargo_path.read_text(encoding="utf-8") + "\n", encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        MODULE.verify(self.path)

    def test_schema2_accepts_conservative_literal_manifest_strings(self):
        self.configure_schema2()
        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8")
        cargo = cargo.replace(
            'path = "../../code/nagi-runtime"',
            r"path = '../../../../../../rust\root/runtime'",
        )
        cargo = cargo.replace(
            'exclude = ["../../code/ignored"]',
            "exclude = ['../../code/ignored-雪']",
        )
        cargo_path.write_text(cargo, encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        MODULE.verify(self.path)

        for value in ("'embedded'quote'", "'''multi-line'''", "'line\nbreak'", "'\x85'"):
            with self.subTest(value=value):
                self.assertFalse(MODULE._valid_literal_string(value))

    def test_schema2_snapshot_and_cache_tampering_are_rejected(self):
        self.configure_schema2()
        main = self.root / "src/main.rs"
        main.write_bytes(b"changed source\n")
        with self.assertRaisesRegex(ValueError, "integrity mismatch"):
            MODULE.verify(self.path)

        self.configure_schema2()
        main = self.root / "src/main.rs"
        main.write_bytes(b"x" * main.stat().st_size)
        self.receipt["artifacts"]["src/main.rs"] = MODULE.identity(main)
        self.write()
        with self.assertRaisesRegex(ValueError, "generated body digest mismatch"):
            MODULE.verify(self.path)

        self.configure_schema2()
        digest = self.receipt["generated_body_sha256"]
        cas_root = self.root.parent.parent / "code" / digest
        cas_manifest = cas_root / "Cargo.toml"
        cas_manifest.write_bytes(b"x" * cas_manifest.stat().st_size)
        with self.assertRaisesRegex(ValueError, "cache Cargo.toml mismatch"):
            MODULE.verify(self.path)

        self.configure_schema2()
        digest = self.receipt["generated_body_sha256"]
        cas_lib = self.root.parent.parent / "code" / digest / "src/lib.rs"
        cas_lib.write_bytes(b"x" * cas_lib.stat().st_size)
        with self.assertRaisesRegex(ValueError, "cache src/lib.rs mismatch"):
            MODULE.verify(self.path)

    def test_schema2_digest_and_dependency_path_are_strict(self):
        self.configure_schema2()
        self.receipt["generated_body_sha256"] = "A" * 64
        self.write()
        with self.assertRaisesRegex(ValueError, "invalid generated body digest"):
            MODULE.verify(self.path)

        digest = self.configure_schema2()
        self.receipt["generated_body_sha256"] = "0" * 64
        self.write()
        with self.assertRaisesRegex(ValueError, "generated body digest mismatch"):
            MODULE.verify(self.path)

        digest = self.configure_schema2()
        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8").replace(
            f'path = "../../code/{digest}"', 'path = "../../../../outside"')
        cargo_path.write_text(cargo, encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        with self.assertRaisesRegex(ValueError, "dependency table mismatch"):
            MODULE.verify(self.path)

    def test_schema2_rejects_extra_cargo_inputs(self):
        digest = self.configure_schema2()
        cas = self.root.parent.parent / "code" / digest
        for name, directory in [("build.rs", False), ("examples", True),
                                ("src/extra.rs", False), ("src/bin", True)]:
            with self.subTest(name=name):
                extra = cas / name
                if directory:
                    extra.mkdir()
                else:
                    extra.write_bytes(b"unexpected")
                with self.assertRaisesRegex(ValueError, "unexpected or missing entries"):
                    MODULE.verify(self.path)
                self.assertTrue(extra.exists())
                if directory:
                    extra.rmdir()
                else:
                    extra.unlink()
        MODULE.verify(self.path)

    def test_schema2_dependency_table_rejects_duplicates_and_extra_keys(self):
        self.configure_schema2()
        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8")
        cargo_path.write_text(cargo + "\n[dependencies.\"nagi-app-body\"]\n", encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        with self.assertRaisesRegex(ValueError, "unexpected generated Cargo.toml table|duplicate"):
            MODULE.verify(self.path)

        self.configure_schema2()
        cargo_path = self.root / "Cargo.toml"
        cargo = cargo_path.read_text(encoding="utf-8").replace(
            f'package = "nagi-body-{self.app.rsplit("-", 1)[-1]}"',
            f'package = "nagi-body-{self.app.rsplit("-", 1)[-1]}"\nother = "key"')
        cargo_path.write_text(cargo, encoding="utf-8")
        self.receipt["artifacts"]["Cargo.toml"] = MODULE.identity(cargo_path)
        self.write()
        with self.assertRaisesRegex(ValueError, "generated Cargo.toml key|dependency table mismatch"):
            MODULE.verify(self.path)

    def test_schema2_rejects_missing_and_extra_artifact_names(self):
        self.configure_schema2()
        del self.receipt["artifacts"]["src/entry.rs"]
        self.write()
        with self.assertRaisesRegex(ValueError, "unexpected, missing"):
            MODULE.verify(self.path)

        self.configure_schema2()
        extra = self.root / "extra.txt"
        extra.write_text("extra", encoding="utf-8")
        self.receipt["artifacts"]["extra.txt"] = MODULE.identity(extra)
        self.write()
        with self.assertRaisesRegex(ValueError, "unexpected, missing"):
            MODULE.verify(self.path)

    def test_incomplete_schema2_receipt_is_rejected(self):
        self.configure_schema2()
        del self.receipt["generated_body_sha256"]
        self.write()
        with self.assertRaisesRegex(ValueError, "invalid generated body digest"):
            MODULE.verify(self.path)

    def test_schema2_rejects_cache_file_links(self):
        digest = self.configure_schema2()
        cas_lib = self.root.parent.parent / "code" / digest / "src/lib.rs"
        cas_lib.unlink()
        try:
            cas_lib.symlink_to(self.root / "src/main.rs")
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        with self.assertRaisesRegex(ValueError, "symbolic link"):
            MODULE.verify(self.path)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "Unix named pipes")
    def test_schema2_rejects_cache_fifos_without_blocking(self):
        digest = self.configure_schema2()
        cas_lib = self.root.parent.parent / "code" / digest / "src/lib.rs"
        cas_lib.unlink()
        os.mkfifo(cas_lib)
        result = subprocess.run([sys.executable, str(SPEC.origin), str(self.path)],
                                capture_output=True, text=True, timeout=5)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not a regular file", result.stderr)

    def test_schema2_rejects_symlinked_code_parent(self):
        self.configure_schema2()
        code = self.root.parent.parent / "code"
        moved = code.with_name("code-moved")
        code.rename(moved)
        try:
            code.symlink_to(moved, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        with self.assertRaisesRegex(ValueError, "symbolic link"):
            MODULE.verify(self.path)

    def test_metadata_cannot_select_arbitrary_paths(self):
        initial = copy.deepcopy(self.receipt)
        for key, value in [("schema_version", True), ("schema_version", 2),
                           ("generation", "../g-1-2-3"), ("app_id", "other")]:
            self.receipt = copy.deepcopy(initial)
            self.receipt[key] = value
            self.write()
            with self.assertRaises(ValueError):
                MODULE.verify(self.path)
        self.receipt = initial
        self.receipt["artifacts"]["../../compiler"] = MODULE.identity(self.compiler)
        self.write()
        with self.assertRaises(ValueError):
            MODULE.verify(self.path)

    def test_missing_truncated_and_malformed(self):
        (self.root / self.executable).unlink()
        with self.assertRaises(OSError):
            MODULE.verify(self.path)
        for text in ["{", "[]", "null", "x" * (1024 * 1024 + 1)]:
            self.path.write_text(text)
            with self.assertRaises(ValueError):
                MODULE.verify(self.path)

    def test_invalid_digest_length_boolean_and_compiler(self):
        initial = copy.deepcopy(self.receipt)
        for field, value in [("sha256", "x" * 64), ("bytes", True), ("bytes", -1)]:
            self.receipt = copy.deepcopy(initial)
            self.receipt["artifacts"][self.executable][field] = value
            self.write()
            with self.assertRaises(ValueError):
                MODULE.verify(self.path)
        self.receipt = initial
        self.write()
        self.compiler.write_bytes(b"other compiler")
        with self.assertRaisesRegex(ValueError, "compiler"):
            MODULE.verify(self.path, self.compiler)

    def test_symlink_artifact_is_rejected(self):
        path = self.root / self.executable
        path.unlink()
        try:
            path.symlink_to(self.compiler)
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        with self.assertRaisesRegex(ValueError, "symbolic link"):
            MODULE.verify(self.path)

    def test_directory_artifact_is_rejected(self):
        path = self.root / self.executable
        path.unlink()
        path.mkdir()
        with self.assertRaisesRegex(ValueError, "not a regular file"):
            MODULE.verify(self.path)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "Unix named pipes")
    def test_named_pipe_receipt_or_artifact_cannot_block_cli(self):
        for path in (self.path, self.root / self.executable):
            original = path.read_bytes()
            path.unlink()
            os.mkfifo(path)
            result = subprocess.run([sys.executable, str(SPEC.origin), str(self.path)],
                                    capture_output=True, text=True, timeout=5)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("not a regular file", result.stderr)
            path.unlink()
            path.write_bytes(original)

    def test_compiler_command_link_is_supported_but_source_directory_link_is_rejected(self):
        compiler_link = self.compiler.with_name("compiler-command")
        try:
            compiler_link.symlink_to(self.compiler)
        except OSError as error:
            self.skipTest(f"OS does not permit test symlinks: {error}")
        MODULE.verify(self.path, compiler_link)
        source = self.root / "src"
        moved = self.root / "original-src"
        source.rename(moved)
        source.symlink_to(moved, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symbolic link"):
            MODULE.verify(self.path)

    @unittest.skipUnless(os.name == "nt", "Windows directory junctions")
    def test_windows_junction_is_rejected_without_reading_target(self):
        source = self.root / "src"
        moved = self.root / "original-src"
        source.rename(moved)
        result = subprocess.run(["cmd", "/c", "mklink", "/J", str(source), str(moved)],
                                capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        try:
            with self.assertRaisesRegex(ValueError, "reparse point"):
                MODULE.verify(self.path)
        finally:
            source.rmdir()


if __name__ == "__main__":
    unittest.main()
