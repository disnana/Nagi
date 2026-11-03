"""Local protocol, gate and exact-inverse tests for Windows regression sharding."""
from __future__ import annotations

import contextlib
import copy
import hashlib
import io
import json
import os
from pathlib import Path, PureWindowsPath
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import windows_regression_sharding as policy
import windows_regression_sharding_projection as projection
import gate
import levels
import test_levels


ROOT = Path(__file__).resolve().parents[2]


def artifact_row(package_id: str, selector: str, executable: str, workspace: str) -> dict:
    member = policy.expected_members()[selector]
    return dict(reason="compiler-artifact", package_id=package_id,
                profile={"test": True}, target={"kind": [member["kind"]], "name": member["name"],
                                                "src_path": policy.workspace_file(workspace, member["source"])},
                executable=executable)


def test_context(root: Path, workspace: str | None = None, package_id: str = "nagic local package id",
                 vcpkg_root: str | None = None, cargo_incremental: str | None = None) -> dict:
    workspace = workspace or str(ROOT)
    python_executable = (policy.workspace_file(workspace, ".toolchain/Python with spaces/python.exe")
                         if PureWindowsPath(workspace).is_absolute() else
                         f"/toolchain/{package_id.replace(' ', '-')}/python.exe")
    inventory_hash = policy.digest({"selected": policy.expected_members(),
                                    "all_test_targets": sorted(policy.KNOWN_PACKAGE_TESTS),
                                    "binary": {policy.PACKAGE: "compiler/src/main.rs"}})
    python_root = policy.python_executable_parent(python_executable)
    native_environment = ({"VCPKG_INSTALLATION_ROOT": vcpkg_root} if vcpkg_root else {})
    native_environment.update({
        "PKG_CONFIG_PATH": policy.python_pkg_config_path(python_executable),
        "pythonLocation": python_root,
        "Python_ROOT_DIR": python_root,
        "Python2_ROOT_DIR": python_root,
        "Python3_ROOT_DIR": python_root,
    })
    return dict(
        identity=dict(schema=policy.SCHEMA, protocol=policy.PROTOCOL, run_id="99112233", attempt="2",
                      head="a" * 40, tree="b" * 40, package=policy.PACKAGE,
                      platform="windows-x86_64", runner_os="Windows", runner_arch="X64",
                      host="x86_64-pc-windows-msvc", rustc="rustc 1.99.1\n", rustc_cfg="debug_assertions\n",
                      cargo="cargo 1.99.1\n", python="3.12.7\n",
                      config=({"CARGO_INCREMENTAL": cargo_incremental} if cargo_incremental is not None else {}),
                      profile_environment={},
                      target_environment={}, build_environment={}, rustc_environment={},
                      native_environment=native_environment,
                      cargo_lock_sha256="c" * 64, package_manifest_sha256="d" * 64,
                      inventory_sha256=inventory_hash, inputs={"Cargo.lock": "e" * 64}),
        producer=dict(workspace=workspace, test_cwd=policy.workspace_file(workspace, "compiler"),
                      package_id=package_id,
                      python_executable=python_executable,
                      python_sha256=hashlib.sha256(package_id.encode()).hexdigest()))


def write_shard_artifact(receipt_root: Path, shard: int, context: dict, temporary: Path) -> dict:
    selectors = policy.SHARD_SELECTORS[shard]
    members = policy.expected_members()
    artifact_root = f"build/ci-evidence/windows-regression/shard-{shard}"
    workspace = context["producer"]["workspace"]
    package_id = context["producer"]["package_id"]
    shard_root = receipt_root / f"nagi-regression-windows-shard-{shard}"
    shard_root.mkdir(parents=True)
    (shard_root / "lists").mkdir()
    (shard_root / "events").mkdir()
    (shard_root / "runner").mkdir()
    (shard_root / "execution").mkdir()
    for profile in policy.PREFETCH_PROFILES:
        (shard_root / "prefetch" / profile / "src").mkdir(parents=True)

    inventory = dict(context=context, shard=shard, selector_set=sorted(selectors),
                     targets={selector: members[selector] for selector in selectors},
                     inventory_sha256=context["identity"]["inventory_sha256"])
    (shard_root / "inventory.json").write_text(json.dumps(inventory), encoding="utf-8")
    orchestration_lines = [f"fixture shard {shard}\n"]
    compile_rows = []
    targets = []
    plan_members = {}
    markers = {}
    cases_by_selector = {}
    ignored_by_selector = {}
    files = {}

    cli_path = policy.workspace_file(workspace, f"target/debug/nagic CLI {shard}.exe")
    cli_sha = hashlib.sha256(f"cli-{shard}".encode()).hexdigest()
    compile_rows.append(dict(reason="compiler-artifact", package_id=package_id, profile={"test": False},
                            target={"kind": ["bin"], "name": "nagic",
                                    "src_path": policy.workspace_file(workspace, "compiler/src/main.rs")}, executable=cli_path))

    for index, selector in enumerate(selectors):
        executable = policy.workspace_file(workspace, f"target/debug/deps/test binary {shard}-{index}.exe")
        exe_sha = hashlib.sha256(f"binary-{shard}-{index}".encode()).hexdigest()
        compile_rows.append(artifact_row(package_id, selector, executable, workspace))
        active = selector.replace(":", "_") + "::active"
        ignored = selector.replace(":", "_") + "::ignored"
        cases = {active, ignored}
        ignored_cases = {ignored}
        cases_by_selector[selector] = cases
        ignored_by_selector[selector] = ignored_cases
        all_listing = f"{active}: test\n{ignored}: test\n2 tests, 0 benchmarks\n"
        ignored_listing = f"{ignored}: test\n1 test, 0 benchmarks\n"
        all_rel = f"lists/{policy.digest(selector)}.all.txt"
        ignored_rel = f"lists/{policy.digest(selector)}.ignored.txt"
        (shard_root / all_rel).write_text(all_listing, encoding="utf-8")
        (shard_root / ignored_rel).write_text(ignored_listing, encoding="utf-8")
        files[all_rel] = policy.sha256_file(shard_root / all_rel)
        files[ignored_rel] = policy.sha256_file(shard_root / ignored_rel)

        marker = policy.digest(dict(selector=selector, executable=policy.canonical_executable(executable),
                                    sha256=exe_sha, context=context))
        markers[selector] = marker
        event_rel = f"events/{marker}.events"
        started_rel = f"runner/{marker}.started.json"
        finished_rel = f"runner/{marker}.finished.json"
        event_abs = policy.workspace_file(workspace, f"{artifact_root}/{event_rel}")
        started_abs = policy.workspace_file(workspace, f"{artifact_root}/{started_rel}")
        finished_abs = policy.workspace_file(workspace, f"{artifact_root}/{finished_rel}")
        plan_members[selector] = dict(executable=policy.canonical_executable(executable), sha256=exe_sha,
                                      marker=marker, events=event_abs, started=started_abs,
                                      finished=finished_abs, event_file=event_rel,
                                      started_file=started_rel, finished_file=finished_rel)

        event_text = f"ok {active}\nignored: fixture reason {ignored}\n"
        test_output = (
            "running 2 tests\n"
            "test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; "
            "0 filtered out; finished in 0.00s\n"
        )
        checked = policy.libtest_protocol.check_execution(
            test_output, event_text, cases, ignored_cases, dict(id=selector))
        (shard_root / event_rel).write_text(event_text, encoding="utf-8")
        started = dict(schema=policy.SCHEMA, protocol=policy.PROTOCOL, selector=selector,
                       executable=policy.canonical_executable(executable), sha256=exe_sha,
                       cwd=context["producer"]["test_cwd"], requested_arguments=["--format=pretty"],
                       actual_arguments=["--format=pretty", "--logfile", event_abs])
        finished = dict(schema=policy.SCHEMA, protocol=policy.PROTOCOL, selector=selector,
                        executable=policy.canonical_executable(executable), sha256=exe_sha, returncode=0)
        (shard_root / started_rel).write_text(json.dumps(started), encoding="utf-8")
        (shard_root / finished_rel).write_text(json.dumps(finished), encoding="utf-8")
        files[event_rel] = policy.sha256_file(shard_root / event_rel)
        files[started_rel] = policy.sha256_file(shard_root / started_rel)
        files[finished_rel] = policy.sha256_file(shard_root / finished_rel)
        orchestration_lines.append(f"NAGI_CI_OUTER_EXECUTION {marker}\n" + test_output)
        target_row = dict(
            target=members[selector],
            executable=dict(path=policy.canonical_executable(executable), sha256=exe_sha),
            cases=sorted(cases),
            ignored=sorted(ignored_cases),
            list_files={all_rel: files[all_rel], ignored_rel: files[ignored_rel]},
            marker=marker,
            event_file=event_rel,
            started_file=started_rel,
            finished_file=finished_rel,
        )
        targets.append(policy.compose_target_result(target_row, checked))

    compile_text = "\n".join(json.dumps(row) for row in compile_rows) + "\n"
    (shard_root / "compile.jsonl").write_text(compile_text, encoding="utf-8")
    (shard_root / "compile.stderr.log").write_text("", encoding="utf-8")
    cargo_test = "".join(orchestration_lines[1:])
    (shard_root / "cargo-test.log").write_text(cargo_test, encoding="utf-8")
    (shard_root / "orchestration.log").write_text("fixture command logs\n", encoding="utf-8")
    plan_path = policy.workspace_file(workspace, f"{artifact_root}/execution/execution.json")
    runner = [context["producer"]["python_executable"], "-B",
              policy.workspace_file(workspace, "scripts/ci/windows_regression_sharding.py"),
              "--record-binary", "--plan", plan_path]
    plan = dict(schema=policy.SCHEMA, protocol=policy.PROTOCOL, context=context, shard=shard,
                requested_harness_arguments=["--format=pretty"],
                runner_command=runner, cwd=context["producer"]["test_cwd"],
                artifact_root=artifact_root,
                nagic_binary=dict(path=policy.canonical_executable(cli_path), sha256=cli_sha),
                members=plan_members)
    (shard_root / "execution/execution.json").write_text(json.dumps(plan), encoding="utf-8")
    prefetch_profiles = {}
    for profile, (package, runtime_package, _) in policy.PREFETCH_PROFILES.items():
        manifest_rel = f"prefetch/{profile}/Cargo.toml"
        source_rel = f"prefetch/{profile}/src/main.rs"
        lock_rel = f"prefetch/{profile}/Cargo.lock"
        profile_cwd, manifest_arg, _ = policy._prefetch_profile_paths(workspace, artifact_root, profile)
        packages = [
            f'[[package]]\nname = "{package}"\nversion = "0.1.0"\n',
            f'[[package]]\nname = "{runtime_package}"\nversion = "0.2.2"\n',
            '[[package]]\nname = "bumpalo"\nversion = "3.20.3"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
            'checksum = "' + "1" * 64 + '"\n',
        ]
        if profile == "full":
            packages.extend((
                '[[package]]\nname = "serde"\nversion = "1.0.228"\n',
                '[[package]]\nname = "serde_json"\nversion = "1.0.145"\n',
                '[[package]]\nname = "tokio"\nversion = "1.54.1"\n',
            ))
            packages.append(
                '[[package]]\nname = "async-trait"\nversion = "0.1.92"\n'
                'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
                'checksum = "' + "2" * 64 + '"\n')
        lock_text = "version = 4\n\n" + "\n".join(packages)
        manifest_file = shard_root / manifest_rel
        source_file = shard_root / source_rel
        lock_file = shard_root / lock_rel
        manifest_file.write_text(policy._prefetch_manifest(profile, workspace), encoding="utf-8", newline="\n")
        source_file.write_text("fn main() {}\n", encoding="utf-8", newline="\n")
        lock_file.write_text(lock_text, encoding="utf-8", newline="\n")
        prefetch_profiles[profile] = dict(
            cwd=profile_cwd,
            command_environment={"CARGO_NET_OFFLINE": "false"},
            manifest=manifest_rel,
            manifest_sha256=policy.sha256_file(manifest_file),
            source=source_rel,
            source_sha256=policy.sha256_file(source_file),
            lock=lock_rel,
            lock_sha256=policy.sha256_file(lock_file),
            package_count=len(packages),
            generate_command=["cargo", "generate-lockfile", "--manifest-path", manifest_arg],
            fetch_command=["cargo", "fetch", "--locked", "--manifest-path", manifest_arg,
                           "--target", context["identity"]["host"]],
        )
    (shard_root / "prefetch/report.json").write_text(json.dumps(dict(
        schema=policy.PREFETCH_SCHEMA,
        protocol=policy.PREFETCH_PROTOCOL,
        host=context["identity"]["host"],
        workspace_fetch=dict(cwd=workspace,
                             command=["cargo", "fetch", "--locked", "--target",
                                      context["identity"]["host"]],
                             command_environment={"CARGO_NET_OFFLINE": "false"},
                             cargo_lock_sha256=context["identity"]["cargo_lock_sha256"]),
        profiles=prefetch_profiles,
    )), encoding="utf-8")
    files.update({relative: policy.sha256_file(shard_root / relative) for relative in (
        "compile.jsonl", "compile.stderr.log", "cargo-test.log", "inventory.json",
        "execution/execution.json", "orchestration.log", *sorted(policy.PREFETCH_FILE_PATHS))})
    receipt = dict(schema=policy.SCHEMA, shard=shard, context=context,
                   inventory_sha256=context["identity"]["inventory_sha256"],
                   selector_ids=list(selectors), command=policy.cargo_test_command(
                       shard, harness=True, runner_command=runner),
                   exit=0, nagic_binary=dict(path=policy.canonical_executable(cli_path), sha256=cli_sha),
                   targets=targets, passed=len(selectors), files=files)
    (shard_root / "receipt.json").write_text(json.dumps(receipt), encoding="utf-8")
    return receipt


class WindowsRegressionPolicyTests(unittest.TestCase):
    def test_original_commands_and_two_shards_are_exact_and_complete(self):
        members = policy.expected_members()
        self.assertEqual(len(members), 74)
        self.assertEqual([len(policy.SHARD_SELECTORS[index]) for index in range(2)], [39, 35])
        self.assertEqual(set().union(*(set(policy.SHARD_SELECTORS[index]) for index in range(2))), set(members))
        self.assertEqual(sum(len(policy.cargo_selector_args(policy.SHARD_SELECTORS[index]))
                             for index in range(2)), 147)
        runner = [r"C:\Python with spaces\python.exe", "-B",
                  r"C:\checkout with spaces\scripts\ci\windows_regression_sharding.py",
                  "--record-binary", "--plan", r"C:\checkout with spaces\execution.json"]
        command = policy.cargo_test_command(0, harness=True, runner_command=runner)
        self.assertEqual(command[:5],
                         ["cargo", "test", "--locked", "-p", "nagic"])
        config = command[command.index("--config") + 1]
        self.assertEqual(config, "target.x86_64-pc-windows-msvc.runner=" +
                         json.dumps(runner, ensure_ascii=False, separators=(",", ":")))
        self.assertTrue(policy.cargo_test_command(0, no_run=True)[-2:] == ["--no-run", "--message-format=json"])
        with patch.object(policy, "ORIGINAL_COMMANDS", policy.ORIGINAL_COMMANDS + (policy.ORIGINAL_COMMANDS[0],)):
            with self.assertRaises(policy.RegressionError):
                policy.expected_members()

    def test_metadata_inventory_is_closed_over_selected_known_and_cli_targets(self):
        targets = [dict(kind=["lib"], name="nagic", test=True,
                        src_path=str(ROOT / "compiler/src/lib.rs")),
                   dict(kind=["bin"], name="nagic", test=True,
                        src_path=str(ROOT / "compiler/src/main.rs"))]
        for name in sorted(policy.KNOWN_PACKAGE_TESTS):
            source = ROOT / "compiler/tests" / f"{name}.rs"
            self.assertTrue(source.is_file(), str(source))
            targets.append(dict(kind=["test"], name=name, test=True, src_path=str(source)))
        metadata = dict(workspace_root=str(ROOT), workspace_members=["nagic-id"],
                        packages=[dict(id="nagic-id", name="nagic", manifest_path=str(ROOT / "compiler/Cargo.toml"),
                                       targets=targets)])
        package_id, observed, binary, test_cwd = policy.package_inventory(metadata)
        self.assertEqual(package_id, "nagic-id")
        self.assertEqual(set(observed), set(policy.KNOWN_PACKAGE_TESTS) | {"lib:nagic"})
        self.assertEqual(binary, {"nagic": "compiler/src/main.rs"})
        self.assertEqual(test_cwd, str(ROOT / "compiler"))
        for mutate in (
            lambda value: value["packages"][0]["targets"].append(
                dict(kind=["test"], name="unreviewed", test=True,
                     src_path=str(ROOT / "compiler/tests/cli.rs"))),
            lambda value: value["packages"][0]["targets"][2].update(src_path=str(ROOT / "compiler/tests/cli.rs")),
        ):
            changed = copy.deepcopy(metadata)
            mutate(changed)
            with self.assertRaises(policy.RegressionError):
                policy.package_inventory(changed)

    def test_profile_toolchain_target_and_native_overrides_fail_closed(self):
        config, profile, target, build, rustc, native = policy.checked_build_environment({})
        self.assertEqual(config["CARGO_HOME"], "default")
        self.assertEqual(config["CARGO_INCREMENTAL"], "")
        self.assertEqual((profile, target, build, rustc, native), ({}, {}, {}, {}, {}))
        python_root = r"C:\hostedtoolcache\windows\Python\3.12.10\x64"
        python_executable = python_root + r"\python.exe"
        post_setup_environment = {
            "CARGO_INCREMENTAL": "0",
            "VCPKG_INSTALLATION_ROOT": r"C:\vcpkg",
            "pythonLocation": python_root,
            "Python_ROOT_DIR": python_root,
            "Python2_ROOT_DIR": python_root,
            "Python3_ROOT_DIR": python_root,
            "PKG_CONFIG_PATH": python_root + "/lib/pkgconfig",
        }
        stock_dtolnay = policy.checked_build_environment(
            post_setup_environment, python_executable=python_executable)
        self.assertEqual(stock_dtolnay[0]["CARGO_INCREMENTAL"], "0")
        self.assertEqual(stock_dtolnay[5], {
            "VCPKG_INSTALLATION_ROOT": r"C:\vcpkg",
            "PKG_CONFIG_PATH": python_root + "/lib/pkgconfig",
            "pythonLocation": python_root,
            "Python_ROOT_DIR": python_root,
            "Python2_ROOT_DIR": python_root,
            "Python3_ROOT_DIR": python_root,
        })
        accepted = policy.checked_build_environment({"VCPKG_INSTALLATION_ROOT": r"C:\vcpkg"})
        self.assertEqual(accepted[5], {"VCPKG_INSTALLATION_ROOT": r"C:\vcpkg"})
        normalized_name = policy.checked_build_environment({"vcpkg_installation_root": r"C:\vcpkg"})
        self.assertEqual(normalized_name[5], {"VCPKG_INSTALLATION_ROOT": r"C:\vcpkg"})

        def assert_rejected_environment(environment, expected_names):
            with self.assertRaises(policy.RegressionError) as caught:
                policy.checked_build_environment(environment)
            diagnostic = str(caught.exception)
            for name in expected_names:
                self.assertIn(name, diagnostic)
            for value in environment.values():
                if value:
                    self.assertNotIn(value, diagnostic)

        def assert_rejected_python_environment(environment, expected_names, executable):
            with self.assertRaises(policy.RegressionError) as caught:
                policy.checked_build_environment(environment, python_executable=executable)
            diagnostic = str(caught.exception)
            for name in expected_names:
                self.assertIn(name, diagnostic)
            for value in environment.values():
                if value:
                    self.assertNotIn(value, diagnostic)

        for key in (
            "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS", "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS",
            "CARGO_BUILD_BUILD_DIR", "RUSTC_WORKSPACE_WRAPPER", "RUSTFLAGS",
            "CFLAGS_x86_64_pc_windows_msvc", "PKG_CONFIG_PATH", "LIB",
        ):
            with self.subTest(variable=key):
                assert_rejected_environment({key: "secret-path-or-flag"}, [key])
        for value in ("1", "false"):
            with self.subTest(cargo_incremental=value):
                assert_rejected_environment({"CARGO_INCREMENTAL": value}, ["CARGO_INCREMENTAL"])
        for path, expected_names in (
            (python_root + "/lib/pkgconfig;D:\\unexpected\\pkgconfig", ["PKG_CONFIG_PATH"]),
            (r"D:\other-python\lib/pkgconfig", ["PKG_CONFIG_PATH"]),
            ("", ["PKG_CONFIG_PATH"]),
        ):
            bad_python_environment = dict(post_setup_environment, PKG_CONFIG_PATH=path)
            with self.subTest(pkg_config_path=path):
                assert_rejected_python_environment(bad_python_environment, expected_names, python_executable)
        wrong_python_root = dict(post_setup_environment, Python_ROOT_DIR=r"D:\other-python")
        assert_rejected_python_environment(wrong_python_root, ["Python_ROOT_DIR"], python_executable)
        empty_python_root = dict(post_setup_environment, Python_ROOT_DIR="")
        assert_rejected_python_environment(empty_python_root, ["Python_ROOT_DIR"], python_executable)
        wrong_interpreter = r"D:\other-python\python.exe"
        assert_rejected_python_environment(post_setup_environment, ["pythonLocation"], wrong_interpreter)
        other_pkg_config_override = dict(post_setup_environment, PKG_CONFIG_LIBDIR=r"C:\secret\pkgconfig")
        assert_rejected_python_environment(other_pkg_config_override, ["PKG_CONFIG_LIBDIR"], python_executable)
        empty_pkg_config_override = dict(post_setup_environment, PKG_CONFIG_LIBDIR="")
        assert_rejected_python_environment(empty_pkg_config_override, ["PKG_CONFIG_LIBDIR"], python_executable)
        duplicate_pkg_config_path = dict(post_setup_environment,
                                         pkg_config_path=post_setup_environment["PKG_CONFIG_PATH"])
        assert_rejected_python_environment(duplicate_pkg_config_path, ["PKG_CONFIG_PATH"], python_executable)
        self.assertEqual(policy.checked_build_environment(
            {"CARGO_HOME": str(Path.home() / ".cargo")})[0]["CARGO_HOME"], "default")
        assert_rejected_environment({"CARGO_HOME": "/secret/unrelated/cargo"}, ["CARGO_HOME"])
        for environment, expected_names in (
            ({"VCPKG_INSTALLATION_ROOT": r"D:\vcpkg"}, ["VCPKG_INSTALLATION_ROOT"]),
            ({"VCPKG_INSTALLATION_ROOT": r"C:\vcpkg\other"}, ["VCPKG_INSTALLATION_ROOT"]),
            ({"VCPKG_FEATURE_FLAGS": "manifests"}, ["VCPKG_FEATURE_FLAGS"]),
            ({"VCPKG_FEATURE_FLAGS": ""}, ["VCPKG_FEATURE_FLAGS"]),
            ({"VCPKG_INSTALLATION_ROOT": r"C:\vcpkg", "VCPKG_FEATURE_FLAGS": "manifests"},
             ["VCPKG_FEATURE_FLAGS"]),
            ({"VCPKG_INSTALLATION_ROOT": r"C:\vcpkg", "vcpkg_installation_root": r"C:\vcpkg"},
             ["VCPKG_INSTALLATION_ROOT"]),
        ):
            with self.subTest(vcpkg_environment=environment):
                assert_rejected_environment(environment, expected_names)

    def test_two_attempt_bound_receipts_reconcile_exact_all_target_and_case_unions(self):
        with tempfile.TemporaryDirectory(prefix="windows regression fixture with spaces ") as temporary:
            root = Path(temporary)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            shard_contexts = [
                test_context(root, r"C:\runner-one\_work\nagi", "nagic package-id runner-one", r"C:\vcpkg"),
                test_context(root, r"D:\runner two\_work\nagi", "nagic package-id runner-two", r"C:\vcpkg"),
            ]
            aggregate_context = test_context(root, r"E:\aggregate\_work\nagi", "nagic aggregate package-id",
                                             r"C:\vcpkg")
            self.assertNotEqual(
                shard_contexts[0]["identity"]["native_environment"]["PKG_CONFIG_PATH"],
                aggregate_context["identity"]["native_environment"]["PKG_CONFIG_PATH"])
            write_shard_artifact(artifacts, 0, shard_contexts[0], root)
            write_shard_artifact(artifacts, 1, shard_contexts[1], root)
            policy.reconcile(artifacts, aggregate_context)
            receipt_root = artifacts / "nagi-regression-windows-shard-0"
            receipt = policy.load_json(receipt_root / "receipt.json")
            self.assertEqual(
                receipt["context"]["identity"]["native_environment"]["PKG_CONFIG_PATH"],
                policy.python_pkg_config_path(receipt["context"]["producer"]["python_executable"]))
            tampered_python_path = copy.deepcopy(receipt)
            tampered_python_path["context"]["identity"]["native_environment"]["PKG_CONFIG_PATH"] += \
                ";D:\\untrusted\\pkgconfig"
            with self.assertRaises(policy.RegressionError):
                policy._validate_receipt(receipt_root, tampered_python_path, 0, aggregate_context)
            missing_vcpkg_context = test_context(root, r"E:\aggregate\_work\nagi",
                                                 "nagic aggregate package-id")
            with self.assertRaises(policy.RegressionError):
                policy._validate_receipt(receipt_root, receipt, 0, missing_vcpkg_context)
            changed_incremental_context = test_context(root, r"E:\aggregate\_work\nagi",
                                                       "nagic aggregate package-id", cargo_incremental="0")
            with self.assertRaises(policy.RegressionError):
                policy._validate_receipt(receipt_root, receipt, 0, changed_incremental_context)

            stale = root / "stale"
            stale.mkdir()
            write_shard_artifact(stale, 0, shard_contexts[0], root)
            receipt1 = write_shard_artifact(stale, 1, shard_contexts[1], root)
            receipt1["context"]["identity"]["attempt"] = "1"
            (stale / "nagi-regression-windows-shard-1" / "receipt.json").write_text(
                json.dumps(receipt1), encoding="utf-8")
            with self.assertRaises(policy.RegressionError):
                policy.reconcile(stale, aggregate_context)

            extra = stale / "unexpected-artifact"
            extra.mkdir()
            with self.assertRaises(policy.RegressionError):
                policy.reconcile(stale, aggregate_context)

            drift = root / "drift"
            drift.mkdir()
            write_shard_artifact(drift, 0, shard_contexts[0], root)
            write_shard_artifact(drift, 1, shard_contexts[1], root)
            shard_one = drift / "nagi-regression-windows-shard-1"
            changed_lock = shard_one / "prefetch/core/Cargo.lock"
            changed_lock.write_text(changed_lock.read_text(encoding="utf-8").replace(
                "3.20.3", "3.20.4"), encoding="utf-8")
            changed_report_path = shard_one / "prefetch/report.json"
            changed_report = json.loads(changed_report_path.read_text(encoding="utf-8"))
            changed_report["profiles"]["core"]["lock_sha256"] = policy.sha256_file(changed_lock)
            changed_report_path.write_text(json.dumps(changed_report), encoding="utf-8")
            changed_receipt_path = shard_one / "receipt.json"
            changed_receipt = policy.load_json(changed_receipt_path)
            changed_receipt["files"]["prefetch/core/Cargo.lock"] = policy.sha256_file(changed_lock)
            changed_receipt["files"]["prefetch/report.json"] = policy.sha256_file(changed_report_path)
            changed_receipt_path.write_text(json.dumps(changed_receipt), encoding="utf-8")
            with self.assertRaises(policy.RegressionError):
                policy.reconcile(drift, aggregate_context)

    def test_receipt_rejects_malformed_commands_targets_cases_hashes_and_paths(self):
        with tempfile.TemporaryDirectory(prefix="windows malformed receipt fixture ") as temporary:
            temporary = Path(temporary)
            artifacts = temporary / "artifacts"
            artifacts.mkdir()
            shard_context = test_context(temporary, r"C:\runner-one\_work\nagi",
                                         "nagic package-id runner-one")
            aggregate_context = test_context(temporary, r"E:\aggregate\_work\nagi",
                                             "nagic aggregate package-id")
            write_shard_artifact(artifacts, 0, shard_context, temporary)
            root = artifacts / "nagi-regression-windows-shard-0"
            receipt = policy.load_json(root / "receipt.json")
            plan = policy.load_json(root / "execution/execution.json")
            plan_path = policy.workspace_file(shard_context["producer"]["workspace"],
                                              "build/ci-evidence/windows-regression/shard-0/execution/execution.json")
            policy._validate_runner_plan(plan, manifest_path=plan_path)
            for name, mutate in (
                ("wrong package cwd", lambda value: value.update(cwd=value["context"]["producer"]["workspace"])),
                ("relative Python path", lambda value: value["runner_command"].__setitem__(0, "python.exe")),
                ("wrong producer interpreter", lambda value: value["runner_command"].__setitem__(0, r"Z:\python.exe")),
                ("script outside checkout", lambda value: value["runner_command"].__setitem__(2, r"Z:\script.py")),
                ("CLI outside target", lambda value: value["nagic_binary"].update(path=r"Z:\other\nagic.exe")),
            ):
                changed_plan = copy.deepcopy(plan)
                mutate(changed_plan)
                with self.subTest(runner_plan=name), self.assertRaises(policy.RegressionError):
                    policy._validate_runner_plan(changed_plan, manifest_path=plan_path)
            mutations = (
                ("unexpected receipt field", lambda value: value.update(extra="forbidden")),
                ("failed cargo result", lambda value: value.update(exit=1)),
                ("wrong cargo invocation", lambda value: value.update(command=["cargo", "test"])),
                ("missing selector", lambda value: value["selector_ids"].pop()),
                ("missing compiled target", lambda value: value["targets"].pop()),
                ("duplicate compiled target", lambda value: value["targets"].append(copy.deepcopy(value["targets"][0]))),
                ("changed test binary SHA", lambda value: value["targets"][0]["executable"].update(sha256="0" * 64)),
                ("changed source identity", lambda value: value["targets"][0]["target"].update(source="compiler/tests/cli.rs")),
                ("duplicate named case", lambda value: value["targets"][0]["cases"].append(value["targets"][0]["cases"][0])),
                ("unknown named case", lambda value: value["targets"][0]["cases"].append("unexpected::case")),
                ("changed ignored reason", lambda value: value["targets"][0]["ignored_reasons"].update(
                    {next(iter(value["targets"][0]["ignored_reasons"])): "wrong reason"})),
                ("changed CLI binary SHA", lambda value: value["nagic_binary"].update(sha256="0" * 64)),
                ("changed log SHA", lambda value: value["files"].update({"cargo-test.log": "0" * 64})),
                ("unexpected artifact", lambda value: value["files"].update({"extra.log": "f" * 64})),
                ("producer root mismatch", lambda value: value["context"]["producer"].update(
                    workspace=r"C:\\outside\\checkout")),
            )
            for name, mutate in mutations:
                changed = copy.deepcopy(receipt)
                mutate(changed)
                with self.subTest(mutation=name), self.assertRaises(policy.RegressionError):
                    policy._validate_receipt(root, changed, 0, aggregate_context)
            first_target = receipt["targets"][0]
            event_path = root / first_target["event_file"]
            original_event = event_path.read_text(encoding="utf-8")
            broken_event = original_event.replace("ok ", "FAILED ", 1)
            event_path.write_text(broken_event, encoding="utf-8")
            changed = copy.deepcopy(receipt)
            changed["files"][first_target["event_file"]] = policy.sha256_file(event_path)
            try:
                with self.assertRaises((policy.RegressionError, ValueError)):
                    policy._validate_receipt(root, changed, 0, aggregate_context)
            finally:
                event_path.write_text(original_event, encoding="utf-8")

    def test_target_result_composition_is_explicit_and_fails_closed(self):
        workspace = r"C:\runner with spaces\_work\nagi"
        selector = "test:cli"
        marker = "a" * 64
        active = "cli::active"
        ignored = "cli::ignored"
        all_path = f"lists/{policy.digest(selector)}.all.txt"
        ignored_path = f"lists/{policy.digest(selector)}.ignored.txt"
        row = dict(
            target=policy.expected_members()[selector],
            executable=dict(
                path=policy.canonical_executable(policy.workspace_file(
                    workspace, "target/debug/deps/cli.exe")),
                sha256="b" * 64,
            ),
            cases=[active, ignored],
            ignored=[ignored],
            list_files={all_path: "c" * 64, ignored_path: "d" * 64},
            marker=marker,
            event_file=f"events/{marker}.events",
            started_file=f"runner/{marker}.started.json",
            finished_file=f"runner/{marker}.finished.json",
        )
        checked = dict(cases=[active, ignored], ignored=[ignored],
                       ignored_reasons={ignored: "fixture reason"}, passed=1)
        result = policy.compose_target_result(row, checked)
        self.assertEqual(result, dict(
            target=row["target"], executable=row["executable"], cases=[active, ignored],
            ignored=[ignored], ignored_reasons={ignored: "fixture reason"},
            list_files=row["list_files"], marker=marker, event_file=f"events/{marker}.events",
            started_file=f"runner/{marker}.started.json", finished_file=f"runner/{marker}.finished.json",
            passed=1))

        mutations = (
            ("duplicate producer case", lambda source, outcome: source["cases"].append(active)),
            ("producer and validated cases differ", lambda source, outcome: outcome["cases"].pop()),
            ("producer and validated ignored cases differ", lambda source, outcome: outcome["ignored"].clear()),
            ("missing ignored reason", lambda source, outcome: outcome["ignored_reasons"].clear()),
            ("wrong passed count", lambda source, outcome: outcome.update(passed=2)),
            ("boolean passed count", lambda source, outcome: outcome.update(passed=True)),
            ("missing checked count", lambda source, outcome: outcome.pop("passed")),
            ("unexpected checked field", lambda source, outcome: outcome.update(extra=0)),
            ("unexpected source field", lambda source, outcome: source.update(ignored_reasons={})),
        )
        for name, mutate in mutations:
            changed_row = copy.deepcopy(row)
            changed_checked = copy.deepcopy(checked)
            mutate(changed_row, changed_checked)
            with self.subTest(composition=name), self.assertRaises(policy.RegressionError):
                policy.compose_target_result(changed_row, changed_checked)

    def test_generated_runtime_prefetch_covers_empty_cache_without_build_commands(self):
        with tempfile.TemporaryDirectory(prefix="Windows cold prefetch workspace with spaces ") as temporary:
            base = Path(temporary)
            workspace = (base / "checkout path with spaces").resolve()
            workspace.mkdir()
            (workspace / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
            for package in ("runtime", "runtime-core"):
                crate = workspace / package
                crate.mkdir()
                (crate / "Cargo.toml").write_text("[package]\nname='fixture'\n", encoding="utf-8")
            artifact_root = "build/ci-evidence/windows-regression/shard-0"
            evidence = workspace / artifact_root
            evidence.mkdir(parents=True)
            (evidence / "orchestration.log").write_text("cold-cache fixture\n", encoding="utf-8")
            cargo_home = workspace / "empty Cargo home"
            cargo_home.mkdir()
            environment = dict(os.environ)
            environment["CARGO_HOME"] = str(cargo_home)
            environment.pop("CARGO_TARGET_DIR", None)
            commands = []

            def fake_cargo(argv, *, cwd, env, **kwargs):
                self.assertEqual(env["CARGO_NET_OFFLINE"], "false")
                self.assertEqual(env["CARGO_HOME"], str(cargo_home))
                self.assertNotIn("CARGO_TARGET_DIR", env)
                commands.append((list(argv), Path(cwd)))
                if argv == ["cargo", "fetch", "--locked", "--target", "x86_64-pc-windows-msvc"]:
                    self.assertEqual(Path(cwd), workspace)
                    return subprocess.CompletedProcess(argv, 0, "workspace dependencies fetched\n", "")
                manifest = Path(argv[argv.index("--manifest-path") + 1])
                profile = manifest.parent.name
                self.assertIn(profile, policy.PREFETCH_PROFILES)
                self.assertEqual(Path(cwd), manifest.parent)
                self.assertTrue(manifest.is_file())
                if argv[1] == "generate-lockfile":
                    package, runtime_package, _ = policy.PREFETCH_PROFILES[profile]
                    package_rows = [
                        f'[[package]]\nname = "{package}"\nversion = "0.1.0"\n',
                        f'[[package]]\nname = "{runtime_package}"\nversion = "0.2.2"\n',
                        '[[package]]\nname = "bumpalo"\nversion = "3.20.3"\n'
                        'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
                        'checksum = "' + "1" * 64 + '"\n',
                    ]
                    if profile == "full":
                        package_rows.extend((
                            '[[package]]\nname = "serde"\nversion = "1.0.228"\n',
                            '[[package]]\nname = "serde_json"\nversion = "1.0.145"\n',
                            '[[package]]\nname = "tokio"\nversion = "1.54.1"\n',
                        ))
                        package_rows.append(
                            '[[package]]\nname = "async-trait"\nversion = "0.1.92"\n'
                            'source = "registry+https://github.com/rust-lang/crates.io-index"\n'
                            'checksum = "' + "2" * 64 + '"\n')
                    (manifest.parent / "Cargo.lock").write_text(
                        "version = 4\n\n" + "\n".join(package_rows), encoding="utf-8")
                    return subprocess.CompletedProcess(argv, 0, "generated isolated lockfile\n", "")
                self.assertEqual(argv[1], "fetch")
                self.assertIn("--locked", argv)
                self.assertIn("--target", argv)
                self.assertEqual(argv[argv.index("--target") + 1], "x86_64-pc-windows-msvc")
                return subprocess.CompletedProcess(argv, 0, "registry packages fetched\n", "")

            with patch.dict(os.environ, environment, clear=True), \
                    patch.object(policy, "run_command", side_effect=fake_cargo):
                hashes = policy._prefetch_generated_runtime_dependencies(
                    evidence, str(workspace), artifact_root, "x86_64-pc-windows-msvc",
                    policy.sha256_file(workspace / "Cargo.lock"))
            self.assertEqual(len(commands), 5)
            self.assertEqual(
                [argv[1] for argv, _ in commands],
                ["fetch", "generate-lockfile", "fetch", "generate-lockfile", "fetch"])
            self.assertTrue(all(argv[1] not in {"build", "check", "run", "test"}
                                for argv, _ in commands))
            self.assertEqual(set(hashes), set(policy.PREFETCH_FILE_PATHS))
            report = policy.load_json(evidence / "prefetch/report.json")
            self.assertIn("async-trait", policy._prefetch_lock_packages(
                evidence / "prefetch/full/Cargo.lock")[1])
            full_packages = policy._prefetch_lock_packages(evidence / "prefetch/full/Cargo.lock")[1]
            self.assertTrue({"serde", "serde_json", "tokio"} <= full_packages)
            self.assertIn("bumpalo", policy._prefetch_lock_packages(
                evidence / "prefetch/core/Cargo.lock")[1])
            self.assertEqual(
                policy._validate_prefetch_evidence(
                    evidence, hashes, str(workspace), artifact_root,
                    "x86_64-pc-windows-msvc", policy.sha256_file(workspace / "Cargo.lock")),
                {profile: row["lock_sha256"] for profile, row in report["profiles"].items()})
            self.assertEqual(list(cargo_home.iterdir()), [])
            self.assertFalse((evidence / "prefetch" / "target").exists())

            report_path = evidence / "prefetch/report.json"
            original_report = report_path.read_text(encoding="utf-8")
            changed = json.loads(original_report)
            changed["profiles"]["full"]["fetch_command"][-1] = "x86_64-unknown-linux-gnu"
            report_path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaises(policy.RegressionError):
                policy._validate_prefetch_evidence(
                    evidence, hashes, str(workspace), artifact_root,
                    "x86_64-pc-windows-msvc", policy.sha256_file(workspace / "Cargo.lock"))
            report_path.write_text(original_report, encoding="utf-8")
            (evidence / "prefetch/full/target").mkdir()
            with self.assertRaises(policy.RegressionError):
                policy._validate_prefetch_evidence(
                    evidence, hashes, str(workspace), artifact_root,
                    "x86_64-pc-windows-msvc", policy.sha256_file(workspace / "Cargo.lock"))

    def test_cargo_runner_uses_argv_list_and_preserves_paths_with_spaces(self):
        with tempfile.TemporaryDirectory(prefix="Windows runner path with spaces ") as temporary:
            root = Path(temporary)
            workspace = (root / "checkout path with spaces").resolve()
            package_root = workspace / "compiler"
            package_root.mkdir(parents=True)
            executable = workspace / "target" / "debug" / "deps" / "outer test binary.exe"
            executable.parent.mkdir(parents=True)
            report = root / "child report.json"
            executable.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, pathlib, sys\n"
                "pathlib.Path(sys.argv[2]).write_text('outer event log\\n', encoding='utf-8')\n"
                "pathlib.Path(os.environ['REPORT_PATH']).write_text(json.dumps({"
                "'argv':sys.argv[1:],'cwd':os.getcwd(),"
                "'keep':os.getenv('KEEP_TEST_ENV')}), encoding='utf-8')\n",
                encoding="utf-8")
            executable.chmod(0o755)
            context = test_context(root, str(workspace))
            artifact_root = "build/ci-evidence/windows-regression/shard-0"
            exe_sha = policy.sha256_file(executable)
            members = {}
            selected = policy.SHARD_SELECTORS[0][0]
            for index, selector in enumerate(policy.SHARD_SELECTORS[0]):
                path = executable if selector == selected else (
                    workspace / "target" / "debug" / "deps" / f"other binary {index}.exe")
                sha = exe_sha if selector == selected else hashlib.sha256(f"other-{index}".encode()).hexdigest()
                marker = policy.digest(dict(selector=selector, executable=policy.canonical_executable(str(path)),
                                            sha256=sha, context=context))
                event_rel = f"events/{marker}.events"
                started_rel = f"runner/{marker}.started.json"
                finished_rel = f"runner/{marker}.finished.json"
                members[selector] = dict(executable=policy.canonical_executable(str(path)), sha256=sha,
                    marker=marker,
                    events=policy.workspace_file(str(workspace), f"{artifact_root}/{event_rel}"),
                    started=policy.workspace_file(str(workspace), f"{artifact_root}/{started_rel}"),
                    finished=policy.workspace_file(str(workspace), f"{artifact_root}/{finished_rel}"),
                    event_file=event_rel, started_file=started_rel, finished_file=finished_rel)
            selected_row = members[selected]
            events = Path(selected_row["events"])
            started = Path(selected_row["started"])
            finished = Path(selected_row["finished"])
            plan_path = workspace / artifact_root / "execution" / "execution plan with spaces.json"
            plan_path.parent.mkdir(parents=True)
            runner = policy.runner_argv(str(plan_path.resolve()), str(workspace),
                                        context["producer"]["python_executable"])
            plan = dict(schema=policy.SCHEMA, protocol=policy.PROTOCOL, context=context, shard=0,
                        requested_harness_arguments=["--format=pretty"],
                        runner_command=runner, cwd=str(package_root),
                        artifact_root=artifact_root,
                        nagic_binary=dict(path=policy.canonical_executable(str(workspace / "target" / "debug" / "nagic.exe")),
                                          sha256=hashlib.sha256(b"cli").hexdigest()),
                        members=members)
            plan_path.write_text(json.dumps(plan), encoding="utf-8")
            environment = {"REPORT_PATH": str(report), "KEEP_TEST_ENV": "preserved"}
            cargo_command = policy.cargo_test_command(0, harness=True, runner_command=runner)
            config = cargo_command[cargo_command.index("--config") + 1].split("=", 1)[1]
            self.assertEqual(json.loads(config), runner)
            with patch.object(policy, "_validate_runner_plan", return_value=context), \
                    patch.dict(os.environ, environment), patch("os.getcwd", return_value=str(package_root)), \
                    contextlib.redirect_stdout(io.StringIO()):
                result = policy.record_binary([str(executable), "--format=pretty"], str(plan_path))
                self.assertEqual(result, 0)
                with self.assertRaises(policy.RegressionError):
                    policy.record_binary([str(executable), "--format=pretty"], str(plan_path))
            with patch.object(policy, "record_binary", return_value=0) as record:
                with self.assertRaises(SystemExit) as exit_result:
                    policy.main(["--record-binary", "--plan", str(plan_path),
                                 str(executable), "--format=pretty"])
                self.assertEqual(exit_result.exception.code, 0)
                record.assert_called_once_with([str(executable), "--format=pretty"], str(plan_path))
            observed = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(observed["argv"], ["--format=pretty", "--logfile", str(events)])
            self.assertEqual(observed["cwd"], str(package_root))
            self.assertEqual(observed["keep"], "preserved")
            self.assertEqual(json.loads(started.read_text(encoding="utf-8"))["actual_arguments"],
                             observed["argv"])
            self.assertEqual(json.loads(finished.read_text(encoding="utf-8"))["returncode"], 0)

    def test_selected_windows_gate_requires_shards_and_always_aggregate(self):
        selected = test_levels.needs("2", "true")
        self.assertEqual(gate.errors(selected), [])
        for name in ("nagi-regression-windows-shards", "nagi-regression-windows-aggregate"):
            for result in ("failure", "cancelled", "skipped", "missing"):
                changed = copy.deepcopy(selected)
                if result == "missing":
                    del changed[name]
                else:
                    changed[name]["result"] = result
                self.assertTrue(gate.errors(changed), (name, result))
        unselected = test_levels.needs("1", "false", test_levels.release())
        self.assertEqual(gate.errors(unselected), [])
        unselected["nagi-regression-windows-aggregate"]["result"] = "success"
        self.assertTrue(gate.errors(unselected))

    def test_workflow_projection_restores_exact_221_and_rejects_gate_runner_cache_drift(self):
        workflow_path = ROOT / ".github/workflows/ci.yml"
        current = workflow_path.read_text(encoding="utf-8")
        restored = projection.inverse_workflow(current)
        self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), projection.BASE_WORKFLOW_SHA256)
        self.assertEqual(projection.apply_workflow(restored), current)
        self.assertEqual(restored, subprocess.check_output(
            ["git", "show", "14eae12253a7caa91b0dda35093e8b5c4b7476c0:.github/workflows/ci.yml"],
            cwd=ROOT, text=True))
        legacy = projection._job(current, "nagi-regression").group()
        self.assertEqual(legacy.count("matrix.platform != 'windows-x86_64'"), 11)
        for command in policy.ORIGINAL_COMMANDS:
            self.assertEqual(current.count("run: " + command), 1)
        self.assertEqual(current.count("shard: [0, 1]"), 1)
        self.assertIn("nagi-regression-windows-aggregate", projection._job(current, "ready").group())
        self.assertIn("nagi-regression-windows-aggregate", projection._job(current, "publish-release").group())
        for before, after in (
            (projection.SHARDS_JOB, projection.SHARDS_JOB.replace("fail-fast: false", "fail-fast: true", 1)),
            (projection.AGGREGATE_JOB, projection.AGGREGATE_JOB.replace("always() && needs.changes.result", "success() && needs.changes.result", 1)),
            (legacy, legacy.replace(policy.ORIGINAL_COMMANDS[0], "cargo test --locked -p nagic --lib", 1)),
        ):
            mutant = current.replace(before, after, 1)
            with self.assertRaises(ValueError):
                projection.inverse_workflow(mutant)


if __name__ == "__main__":
    unittest.main()
