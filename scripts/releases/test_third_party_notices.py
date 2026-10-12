"""Tests for lock-bound third-party notice generation and validation."""
from __future__ import annotations

import copy
import gzip
import hashlib
import io
import json
import os
import tarfile
import tempfile
import unittest
from pathlib import Path

import third_party_notices as notices


REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
TARGET = "x86_64-unknown-linux-gnu"
COMMIT = "a" * 40
BINARY_SHA256 = "b" * 64


class NoticeGraphFixture(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.registry_root = self.root / "cargo" / "registry" / "src" / "index.crates.io-test"
        self.registry_root.mkdir(parents=True)
        self.workspace_root = self.root / "workspace"
        (self.workspace_root / "compiler").mkdir(parents=True)
        (self.workspace_root / "runtime").mkdir(parents=True)
        (self.workspace_root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["compiler", "runtime"]\n'
            '[workspace.package]\nversion = "0.1.0"\nedition = "2021"\nlicense = "MIT"\n',
            encoding="utf-8")
        (self.workspace_root / "compiler/Cargo.toml").write_text(
            '[package]\nname = "nagic"\nversion = "0.1.0"\nedition = "2021"\n',
            encoding="utf-8")
        (self.workspace_root / "runtime/Cargo.toml").write_text(
            '[package]\nname = "nagi-runtime"\nversion = "0.1.0"\nedition = "2021"\n',
            encoding="utf-8")

        self.source_data = {}
        for name, version, expression, files in (
            ("compiler-dep", "1.0.0", "Apache-2.0 OR MIT", {
                "LICENSE-APACHE": b"Apache fixture text\n", "LICENSE-MIT": b"MIT fixture text\n"}),
            ("build-tool", "2.0.0", "MIT", {"LICENSE": b"Build fixture text\n"}),
            ("runtime-dep", "3.0.0", "ISC", {"LICENSE": b"Runtime fixture text\n"}),
            ("dev-only", "4.0.0", "LicenseRef-Unknown", {"LICENSE": b"Dev fixture text\n"}),
        ):
            package_root = self.registry_root / f"{name}-{version}"
            package_root.mkdir()
            (package_root / "Cargo.toml").write_text(
                f'[package]\nname = "{name}"\nversion = "{version}"\nlicense = "{expression}"\n',
                encoding="utf-8")
            for filename, data in files.items():
                (package_root / filename).write_bytes(data)
            self.source_data[(name, version)] = (package_root, files)

        self.archive_checksums = {}
        for name, version in self.source_data:
            package_root, files = self.source_data[(name, version)]
            members = {"Cargo.toml": (package_root / "Cargo.toml").read_bytes(), **files}
            archive_bytes = self.make_crate_archive(name, version, members)
            archive_path = self.crate_archive_path(name, version)
            archive_path.parent.mkdir(parents=True, exist_ok=True)
            archive_path.write_bytes(archive_bytes)
            self.archive_checksums[(name, version)] = hashlib.sha256(archive_bytes).hexdigest()

        self.lock_data = self.lock_bytes()
        self.metadata = self.metadata_fixture()
        self.graphs = {
            "nagic": [("nagic", "0.1.0"), ("compiler-dep", "1.0.0"), ("build-tool", "2.0.0")],
            "nagi-runtime": [("nagi-runtime", "0.1.0"), ("compiler-dep", "1.0.0"), ("runtime-dep", "3.0.0")],
        }

    def tearDown(self):
        self.temporary.cleanup()

    def crate_archive_path(self, name, version):
        registry_id = self.registry_root.name
        return self.registry_root.parent.parent / "cache" / registry_id / f"{name}-{version}.crate"

    @staticmethod
    def make_crate_archive(name, version, members):
        output = io.BytesIO()
        root = f"{name}-{version}"
        with tarfile.open(fileobj=output, mode="w:gz") as archive:
            directory = tarfile.TarInfo(root + "/")
            directory.type = tarfile.DIRTYPE
            archive.addfile(directory)
            member_rows = members.items() if isinstance(members, dict) else members
            for member_name, value in member_rows:
                if isinstance(value, tarfile.TarInfo):
                    archive.addfile(value)
                else:
                    data = value
                    member = tarfile.TarInfo(f"{root}/{member_name}")
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))
        return output.getvalue()

    def replace_crate_archive(self, name, version, archive_bytes):
        path = self.crate_archive_path(name, version)
        previous = self.archive_checksums[(name, version)]
        checksum = hashlib.sha256(archive_bytes).hexdigest()
        path.write_bytes(archive_bytes)
        marker = (f'name = "{name}"\nversion = "{version}"\nsource = "{REGISTRY}"\n'
                  f'checksum = "{previous}"\n').encode()
        replacement = (f'name = "{name}"\nversion = "{version}"\nsource = "{REGISTRY}"\n'
                       f'checksum = "{checksum}"\n').encode()
        if marker not in self.lock_data:
            raise AssertionError("fixture lock row was not found")
        self.lock_data = self.lock_data.replace(marker, replacement, 1)
        self.archive_checksums[(name, version)] = checksum

    def lock_bytes(self):
        rows = [
            ('nagic', '0.1.0', None, None),
            ('nagi-runtime', '0.1.0', None, None),
            ('compiler-dep', '1.0.0', REGISTRY, self.archive_checksums[("compiler-dep", "1.0.0")]),
            ('build-tool', '2.0.0', REGISTRY, self.archive_checksums[("build-tool", "2.0.0")]),
            ('runtime-dep', '3.0.0', REGISTRY, self.archive_checksums[("runtime-dep", "3.0.0")]),
            ('dev-only', '4.0.0', REGISTRY, self.archive_checksums[("dev-only", "4.0.0")]),
        ]
        chunks = ['version = 3\n']
        for name, version, source, checksum in rows:
            chunks.extend(['\n[[package]]\n', f'name = "{name}"\n', f'version = "{version}"\n'])
            if source:
                chunks.extend([f'source = "{source}"\n', f'checksum = "{checksum}"\n'])
        return ''.join(chunks).encode()

    def metadata_fixture(self):
        nagic_id = f"path+file://{self.workspace_root / 'compiler'}#nagic@0.1.0"
        runtime_id = f"path+file://{self.workspace_root / 'runtime'}#nagi-runtime@0.1.0"
        ids = {
            "compiler-dep": f"{REGISTRY}#compiler-dep@1.0.0",
            "build-tool": f"{REGISTRY}#build-tool@2.0.0",
            "runtime-dep": f"{REGISTRY}#runtime-dep@3.0.0",
            "dev-only": f"{REGISTRY}#dev-only@4.0.0",
        }
        packages = [
            {"id": nagic_id, "name": "nagic", "version": "0.1.0", "source": None,
             "manifest_path": str(self.workspace_root / "compiler/Cargo.toml"), "license": "MIT", "license_file": None},
            {"id": runtime_id, "name": "nagi-runtime", "version": "0.1.0", "source": None,
             "manifest_path": str(self.workspace_root / "runtime/Cargo.toml"), "license": "MIT", "license_file": None},
        ]
        license_by_name = {
            "compiler-dep": "Apache-2.0 OR MIT",
            "build-tool": "MIT",
            "runtime-dep": "ISC",
            "dev-only": "LicenseRef-Unknown",
        }
        for name, version in (("compiler-dep", "1.0.0"), ("build-tool", "2.0.0"),
                              ("runtime-dep", "3.0.0"), ("dev-only", "4.0.0")):
            package_root, _ = self.source_data[(name, version)]
            packages.append({"id": ids[name], "name": name, "version": version,
                             "source": REGISTRY, "manifest_path": str(package_root / "Cargo.toml"),
                             "license": license_by_name[name], "license_file": None})
        nodes = [
            {"id": nagic_id, "deps": [
                {"pkg": ids["compiler-dep"], "dep_kinds": [{"kind": None, "target": None}]},
                {"pkg": ids["build-tool"], "dep_kinds": [{"kind": "build", "target": None}]},
                {"pkg": ids["dev-only"], "dep_kinds": [{"kind": "dev", "target": None}]},
            ]},
            {"id": runtime_id, "deps": [
                {"pkg": ids["compiler-dep"], "dep_kinds": [{"kind": None, "target": None}]},
                {"pkg": ids["runtime-dep"], "dep_kinds": [{"kind": None, "target": None}]},
            ]},
            *[{"id": pid, "deps": []} for pid in ids.values()],
        ]
        return {"workspace_root": str(self.workspace_root), "workspace_members": [nagic_id, runtime_id],
                "packages": packages, "resolve": {"nodes": nodes}}


class ThirdPartyNoticeTests(NoticeGraphFixture):
    def test_cargo_tree_parser_accepts_workspace_paths_and_shared_rows(self):
        rows = notices.parse_cargo_tree(
            "nagi-runtime v0.1.11 (/workspace/nagi/runtime)\n"
            "aws-lc-rs v1.18.1\n"
            "aws-lc-rs v1.18.1 (*)\n")
        self.assertEqual(rows, [("nagi-runtime", "0.1.11"), ("aws-lc-rs", "1.18.1")])

    def test_graph_bound_notice_payload_tracks_binary_and_runtime_roles(self):
        payload = notices.build_notice_payload(
            self.lock_data, self.metadata, self.graphs, TARGET, COMMIT, BINARY_SHA256)
        manifest = notices.validate_notice_payload(
            payload, self.lock_data, TARGET, COMMIT, BINARY_SHA256)

        self.assertEqual(manifest["snapshot"]["cargo_lock_sha256"], hashlib.sha256(self.lock_data).hexdigest())
        self.assertEqual(manifest["snapshot"]["target"], TARGET)
        self.assertEqual(manifest["snapshot"]["source_commit"], COMMIT)
        self.assertEqual(manifest["snapshot"]["compiler_binary_sha256"], BINARY_SHA256)
        packages = {(row["name"], row["version"]): row for row in manifest["packages"]}
        self.assertEqual(set(packages), {("compiler-dep", "1.0.0"), ("build-tool", "2.0.0"),
                                         ("runtime-dep", "3.0.0")})
        self.assertEqual(packages[("compiler-dep", "1.0.0")]["distribution_roles"],
                         ["nagi-runtime-source", "nagic-binary"])
        self.assertEqual(packages[("build-tool", "2.0.0")]["distribution_roles"], ["nagic-binary"])
        self.assertEqual(packages[("runtime-dep", "3.0.0")]["distribution_roles"], ["nagi-runtime-source"])
        self.assertEqual(packages[("runtime-dep", "3.0.0")]["cargo_source"], REGISTRY)
        archive_path = self.crate_archive_path("runtime-dep", "3.0.0")
        self.assertEqual(packages[("runtime-dep", "3.0.0")]["lock_checksum"],
                         hashlib.sha256(archive_path.read_bytes()).hexdigest())
        serialized = payload[notices.MANIFEST_PATH]
        self.assertNotIn(str(self.root).encode(), serialized)
        self.assertNotIn(b"manifest_path", serialized)
        self.assertEqual(manifest["legal_review"], "not performed")

    def test_pre_generation_license_mutation_is_rejected_by_locked_crate(self):
        package_root, _ = self.source_data[("compiler-dep", "1.0.0")]
        (package_root / "LICENSE-MIT").write_bytes(b"mutated before notice generation\n")
        with self.assertRaisesRegex(ValueError, "source bytes differ from lock-verified crate"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_local_cargo_manifest_bytes_are_also_pinned(self):
        package_root, _ = self.source_data[("compiler-dep", "1.0.0")]
        (package_root / "Cargo.toml").write_bytes(
            (package_root / "Cargo.toml").read_bytes() + b"# local-only edit\n")
        with self.assertRaisesRegex(ValueError, "source bytes differ from lock-verified crate"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_crate_archive_checksum_must_match_cargo_lock(self):
        archive_path = self.crate_archive_path("compiler-dep", "1.0.0")
        archive_path.write_bytes(archive_path.read_bytes() + b"tamper")
        with self.assertRaisesRegex(ValueError, "crate archive checksum differs from Cargo.lock"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def _compiler_archive_rows(self, extra=()):
        name, version = "compiler-dep", "1.0.0"
        package_root, files = self.source_data[(name, version)]
        rows = [("Cargo.toml", (package_root / "Cargo.toml").read_bytes()), *files.items(), *extra]
        return rows

    def _compiler_archive_with_end_padding(self, nonzero):
        archive_bytes = self.make_crate_archive(
            "compiler-dep", "1.0.0", self._compiler_archive_rows())
        tar_bytes = bytearray(gzip.decompress(archive_bytes))
        with tarfile.open(fileobj=io.BytesIO(tar_bytes), mode="r:") as archive:
            list(archive)
            end_marker_offset = archive.offset
        self.assertEqual(len(tar_bytes), tarfile.RECORDSIZE)
        self.assertEqual(tar_bytes[end_marker_offset:end_marker_offset + 1024], bytes(1024))
        padding_offset = end_marker_offset + 1024
        self.assertTrue(tar_bytes[padding_offset:])
        self.assertFalse(any(tar_bytes[padding_offset:]))
        if nonzero:
            tar_bytes[-1] = 1
        return gzip.compress(bytes(tar_bytes), mtime=0)

    def _replace_compiler_archive_with_rows(self, rows):
        archive_bytes = self.make_crate_archive("compiler-dep", "1.0.0", rows)
        self.replace_crate_archive("compiler-dep", "1.0.0", archive_bytes)

    def test_crate_archive_accepts_zero_end_padding(self):
        archive_bytes = self._compiler_archive_with_end_padding(nonzero=False)
        self.replace_crate_archive("compiler-dep", "1.0.0", archive_bytes)
        payload = notices.build_notice_payload(
            self.lock_data, self.metadata, self.graphs, TARGET, COMMIT, BINARY_SHA256)
        manifest = notices.validate_notice_payload(
            payload, self.lock_data, TARGET, COMMIT, BINARY_SHA256)
        self.assertIn(("compiler-dep", "1.0.0"), {
            (row["name"], row["version"]) for row in manifest["packages"]})

    def test_crate_archive_rejects_nonzero_tar_end_padding(self):
        archive_bytes = self._compiler_archive_with_end_padding(nonzero=True)
        self.replace_crate_archive("compiler-dep", "1.0.0", archive_bytes)
        with self.assertRaisesRegex(ValueError, "non-zero trailing data"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_crate_archive_rejects_path_traversal(self):
        self._replace_compiler_archive_with_rows(self._compiler_archive_rows((("../escape", b"x"),)))
        with self.assertRaisesRegex(ValueError, "Unsafe crate archive member path"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_crate_archive_rejects_symlink_members(self):
        link = tarfile.TarInfo("compiler-dep-1.0.0/LICENSE-MIT")
        link.type = tarfile.SYMTYPE
        link.linkname = "LICENSE-APACHE"
        rows = self._compiler_archive_rows()
        # Replace the normal regular member to make the selected path a symlink.
        rows = [(name, value) for name, value in rows if name != "LICENSE-MIT"]
        rows.append(("LICENSE-MIT", link))
        self._replace_compiler_archive_with_rows(rows)
        with self.assertRaisesRegex(ValueError, "links or unsupported member types"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_crate_archive_rejects_ambiguous_duplicate_members(self):
        rows = self._compiler_archive_rows((("LICENSE-MIT", b"different duplicate\n"),))
        self._replace_compiler_archive_with_rows(rows)
        with self.assertRaisesRegex(ValueError, "duplicate crate archive member"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_crate_archive_rejects_case_insensitive_duplicate_member(self):
        rows = self._compiler_archive_rows((("license-mit", b"ambiguous on Windows\n"),))
        self._replace_compiler_archive_with_rows(rows)
        with self.assertRaisesRegex(ValueError, "ambiguous duplicate crate archive member"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_crate_archive_rejects_oversized_selected_member(self):
        large = b"x" * (notices.MAX_LICENSE_SOURCE_BYTES + 1)
        rows = self._compiler_archive_rows((("LICENSE-MIT", large),))
        rows = [(name, value) for name, value in rows if name != "LICENSE-MIT"]
        rows.append(("LICENSE-MIT", large))
        self._replace_compiler_archive_with_rows(rows)
        with self.assertRaisesRegex(ValueError, "crate source member exceeds size limit"):
            notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_nested_license_file_member_from_normal_crate_archive_is_accepted(self):
        package_root, files = self.source_data[("runtime-dep", "3.0.0")]
        license_directory = package_root / "licenses"
        license_directory.mkdir()
        nested_text = b"Nested selected license text\n"
        (license_directory / "MIT.txt").write_bytes(nested_text)
        manifest_bytes = (package_root / "Cargo.toml").read_bytes() + b'license-file = "licenses/MIT.txt"\n'
        (package_root / "Cargo.toml").write_bytes(manifest_bytes)
        package = next(item for item in self.metadata["packages"] if item["name"] == "runtime-dep")
        package["license_file"] = str(license_directory / "MIT.txt")
        archive = self.make_crate_archive(
            "runtime-dep", "3.0.0",
            {"Cargo.toml": manifest_bytes, **files, "licenses/MIT.txt": nested_text})
        self.replace_crate_archive("runtime-dep", "3.0.0", archive)
        payload = notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                              TARGET, COMMIT, BINARY_SHA256)
        manifest = json.loads(payload[notices.MANIFEST_PATH])
        row = next(item for item in manifest["packages"] if item["name"] == "runtime-dep")
        self.assertIn({"source_path": "licenses/MIT.txt", "role": "license",
                       "sha256": hashlib.sha256(nested_text).hexdigest(),
                       "bytes": len(nested_text),
                       "text_path": notices.TEXT_ROOT + hashlib.sha256(nested_text).hexdigest() + ".txt"},
                      row["notice_texts"])

    def test_unknown_spdx_blocks_only_when_in_selected_graph(self):
        self.assertNotIn(("dev-only", "4.0.0"), {
            (row["name"], row["version"]) for row in notices.validate_notice_payload(
                notices.build_notice_payload(self.lock_data, self.metadata, self.graphs,
                                             TARGET, COMMIT, BINARY_SHA256), self.lock_data,
                TARGET, COMMIT, BINARY_SHA256)["packages"]})
        broken_graphs = copy.deepcopy(self.graphs)
        broken_graphs["nagi-runtime"].append(("dev-only", "4.0.0"))
        with self.assertRaisesRegex(ValueError, "SPDX"):
            notices.build_notice_payload(
                self.lock_data, self.metadata, broken_graphs, TARGET, COMMIT, BINARY_SHA256)

    def test_missing_license_text_is_a_blocker(self):
        package_root, files = self.source_data[("runtime-dep", "3.0.0")]
        for filename in files:
            (package_root / filename).unlink()
        with self.assertRaisesRegex(ValueError, "license text source"):
            notices.build_notice_payload(
                self.lock_data, self.metadata, self.graphs, TARGET, COMMIT, BINARY_SHA256)

    def test_stale_lock_mapping_and_untrusted_source_are_blockers(self):
        payload = notices.build_notice_payload(
            self.lock_data, self.metadata, self.graphs, TARGET, COMMIT, BINARY_SHA256)
        old_checksum = self.archive_checksums[("compiler-dep", "1.0.0")].encode()
        stale = self.lock_data.replace(b'checksum = "' + old_checksum + b'"',
                                       b'checksum = "' + b'f' * 64 + b'"', 1)
        self.assertNotEqual(stale, self.lock_data)
        with self.assertRaisesRegex(ValueError, "Cargo.lock"):
            notices.validate_notice_payload(payload, stale, TARGET, COMMIT, BINARY_SHA256)
        changed_source = copy.deepcopy(payload)
        manifest = json.loads(changed_source[notices.MANIFEST_PATH])
        for package in manifest["packages"]:
            if package["name"] == "runtime-dep":
                package["cargo_source"] = "registry+https://example.invalid/other-index"
        for root in manifest["scope"]["roots"]:
            for package in root["registry_packages"]:
                if package["name"] == "runtime-dep":
                    package["cargo_source"] = "registry+https://example.invalid/other-index"
        changed_source[notices.MANIFEST_PATH] = (
            json.dumps(manifest, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode()
        with self.assertRaisesRegex(ValueError, "does not match Cargo.lock"):
            notices.validate_notice_payload(changed_source, self.lock_data, TARGET, COMMIT, BINARY_SHA256)
        metadata = copy.deepcopy(self.metadata)
        row = next(p for p in metadata["packages"] if p["name"] == "runtime-dep")
        row["source"] = "git+https://example.invalid/repo#deadbeef"
        with self.assertRaisesRegex(ValueError, "Unsupported dependency source"):
            notices.build_notice_payload(self.lock_data, metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_altered_payload_text_fails_hash_validation(self):
        payload = notices.build_notice_payload(
            self.lock_data, self.metadata, self.graphs, TARGET, COMMIT, BINARY_SHA256)
        path = next(p for p in payload if p.endswith(".txt"))
        payload[path] += b"tampered\n"
        with self.assertRaisesRegex(ValueError, "hash"):
            notices.validate_notice_payload(payload, self.lock_data, TARGET, COMMIT, BINARY_SHA256)

    def test_unknown_expression_and_absolute_license_file_block(self):
        metadata = copy.deepcopy(self.metadata)
        row = next(p for p in metadata["packages"] if p["name"] == "runtime-dep")
        row["license"] = "MIT OR LicenseRef-Private"
        with self.assertRaisesRegex(ValueError, "SPDX"):
            notices.build_notice_payload(self.lock_data, metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)
        row["license"] = "ISC"
        (self.root / "outside.txt").write_text("must not escape package root\n", encoding="utf-8")
        package_root, _ = self.source_data[("runtime-dep", "3.0.0")]
        outside_relative = Path(os.path.relpath(self.root / "outside.txt", package_root))
        row["license_file"] = str(self.root / "outside.txt")
        (package_root / "Cargo.toml").write_text(
            '[package]\nname = "runtime-dep"\nversion = "3.0.0"\nlicense = "ISC"\n'
            f'license-file = "{outside_relative.as_posix()}"\n', encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "escapes or is missing from the package source"):
            notices.build_notice_payload(self.lock_data, metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)

    def test_missing_spdx_expression_blocks_even_with_license_file(self):
        metadata = copy.deepcopy(self.metadata)
        row = next(p for p in metadata["packages"] if p["name"] == "runtime-dep")
        row["license"] = None
        row["license_file"] = str(self.source_data[("runtime-dep", "3.0.0")][0] / "LICENSE")
        package_root = self.source_data[("runtime-dep", "3.0.0")][0]
        (package_root / "Cargo.toml").write_text(
            '[package]\nname = "runtime-dep"\nversion = "3.0.0"\nlicense-file = "LICENSE"\n',
            encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "No SPDX expression"):
            notices.build_notice_payload(self.lock_data, metadata, self.graphs,
                                         TARGET, COMMIT, BINARY_SHA256)


class CargoTreeCommandTests(unittest.TestCase):
    def test_cargo_tree_disables_terminal_color_for_machine_output(self):
        from types import SimpleNamespace
        from unittest.mock import patch

        output = ("nagic v0.1.11 (/workspace/Nagi/compiler)\n"
                  "aws-lc-rs v1.18.1\n"
                  "aws-lc-rs v1.18.1 (*)\n")
        result = SimpleNamespace(returncode=0, stdout=output)
        with patch.dict(os.environ, {"CARGO_TERM_COLOR": "always"}):
            with patch.object(notices.subprocess, "run", return_value=result) as run:
                packages = notices.cargo_tree(Path("/workspace/Nagi"), "nagic", TARGET)

        command = run.call_args.args[0]
        self.assertEqual(command[1:4], ["--color", "never", "tree"])
        self.assertEqual(packages, [("nagic", "0.1.11"), ("aws-lc-rs", "1.18.1")])

    def test_parser_rejects_ansi_styled_shared_marker(self):
        colored_marker = "aws-lc-rs v1.18.1 \x1b[33m(*)\x1b[0m"
        with self.assertRaisesRegex(ValueError, "unrecognized package row"):
            notices.parse_cargo_tree("nagic v0.1.11\n" + colored_marker + "\n")


class WorkspaceInputDiagnosticTests(unittest.TestCase):
    def test_mismatch_diagnostic_reports_only_size_hash_and_difference_kind(self):
        current = b"token=example\n"
        committed = b"token=example\r\n"

        details = notices._byte_mismatch_details("worktree", current, "commit", committed)

        self.assertIn("worktree_size=14", details)
        self.assertIn("commit_size=15", details)
        self.assertIn(f"worktree_sha256={hashlib.sha256(current).hexdigest()}", details)
        self.assertIn(f"commit_sha256={hashlib.sha256(committed).hexdigest()}", details)
        self.assertIn("first_difference_kind=line-ending-byte", details)
        self.assertNotIn("token=example", details)


if __name__ == "__main__":
    unittest.main()
