"""Run and reconcile the original Windows nagic regression inventory in two shards.

The timing vector is only used to choose the checked-in assignment below.  At
runtime Cargo metadata, compiler artifacts, libtest listings, outer logfile
events, source/toolchain identity and receipts are all checked independently.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath, PureWindowsPath
import platform
import re
import shlex
import subprocess
import sys
import traceback
from typing import Any

from nextest_evaluation import parse_libtest_listing
import linux_critical_path as libtest_protocol


ROOT = Path(__file__).resolve().parents[2]
PACKAGE = "nagic"
SHARD_COUNT = 2
SCHEMA = 2
PROTOCOL = "windows-stable-libtest-logfile-v2"
MAX_PLAN_BYTES = 8 * 1024 * 1024
MAX_EVENT_BYTES = 16 * 1024 * 1024
MAX_COMMAND_OUTPUT_BYTES = 64 * 1024 * 1024
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
HEX40 = re.compile(r"[0-9a-f]{40}\Z")

# Frozen selectors from the two original #221 Cargo commands.  Keep them in
# their original command order separately from the LPT assignment.
ORIGINAL_COMMANDS = (
    "cargo test --locked -p nagic --lib --test cli --test installation --test codegen --test loop_ownership --test ownership_calls --test ownership_boundaries --test comparison_ownership --test owned_copy_codegen --test build_diagnostics --test rust_dependencies --test expression_contracts --test high_elif --test frontend_contracts --test source_read_retention --test literal_contracts --test http_route_conflicts --test symbols --test editor_assistance --test modules --test module_parser --test module_emission --test module_symbols --test enum_parser --test enum_modules --test enum_emission --test enum_patterns --test nullable_database --test nullable_roundtrip --test owned_database --test borrow_free_returns --test static_callback_views --test view_iteration --test view_parameter_rebinding --test sql_check --test typed_errors --test error_routes --test class_field_types --test resource_contract_characterization --test stdlib_imports --test option_match --test http_stdlib --test http_codegen --test actor_stdlib --test actor_codegen --test graph_types --test graph_relations --test graph_render --test graph_html --test map_cli --test low_multiline --test list_ownership_inference --test list_borrow_iteration --test result_stdlib",
    "cargo test --locked -p nagic --test output_safety --test integer_arithmetic --test integer_zero_division --test constant_validation --test auth_boundaries --test view_branch_rebinding --test view_container_rebinding --test view_container_drop --test view_flow_foundation --test view_flow_completion --test explicit_moves --test task_handles --test conformance --test scope_runtime_contract --test copy_capabilities --test shared_field_moves --test asset_calls --test shared_target --test build_generations --test project",
)

ORIGINAL_SELECTORS = (
    "lib:nagic",
    "test:cli", "test:installation", "test:codegen", "test:loop_ownership",
    "test:ownership_calls", "test:ownership_boundaries", "test:comparison_ownership",
    "test:owned_copy_codegen", "test:build_diagnostics", "test:rust_dependencies",
    "test:expression_contracts", "test:high_elif", "test:frontend_contracts",
    "test:source_read_retention", "test:literal_contracts", "test:http_route_conflicts",
    "test:symbols", "test:editor_assistance", "test:modules", "test:module_parser",
    "test:module_emission", "test:module_symbols", "test:enum_parser", "test:enum_modules",
    "test:enum_emission", "test:enum_patterns", "test:nullable_database",
    "test:nullable_roundtrip", "test:owned_database", "test:borrow_free_returns",
    "test:static_callback_views", "test:view_iteration", "test:view_parameter_rebinding",
    "test:sql_check", "test:typed_errors", "test:error_routes", "test:class_field_types",
    "test:resource_contract_characterization", "test:stdlib_imports", "test:option_match",
    "test:http_stdlib", "test:http_codegen", "test:actor_stdlib", "test:actor_codegen",
    "test:graph_types", "test:graph_relations", "test:graph_render", "test:graph_html",
    "test:map_cli", "test:low_multiline", "test:list_ownership_inference",
    "test:list_borrow_iteration", "test:result_stdlib",
    "test:output_safety", "test:integer_arithmetic", "test:integer_zero_division",
    "test:constant_validation", "test:auth_boundaries", "test:view_branch_rebinding",
    "test:view_container_rebinding", "test:view_container_drop", "test:view_flow_foundation",
    "test:view_flow_completion", "test:explicit_moves", "test:task_handles",
    "test:conformance", "test:scope_runtime_contract", "test:copy_capabilities",
    "test:shared_field_moves", "test:asset_calls", "test:shared_target",
    "test:build_generations", "test:project",
)

# These package integration targets existed in #221 but were intentionally
# outside its two regression commands (other workflow lanes own their scope).
# Keeping this closed set makes newly-added Cargo test targets require policy
# review instead of silently falling out of the sharded inventory.
KNOWN_UNSHARDED_TESTS = frozenset("""
async_value_types builtin_cast_precedence builtin_type_contracts console_io go_backend
http_client_contracts http_client_native http_client_registry http_entrypoint imports
iterator_borrows named_arguments native_run_summary operator_types output_types ownership
result_discard result_match scoped_tasks security_sf01 security_sf01_native security_sf02
security_sf03_b1 security_sf03_cors security_sf04 security_sf05 security_sf05_native
sqlite_contract_inputs sqlite_public type_layouts value_codegen view_origins
""".split())

# Longest-processing-time assignments from the checked-in #221 timing vector.
# Seconds are intentionally absent here: these assignments are coverage policy,
# not ETA promises.
SHARD_SELECTORS = {
    0: (
        "test:error_routes", "test:auth_boundaries", "test:nullable_database", "lib:nagic",
        "test:actor_codegen", "test:actor_stdlib", "test:frontend_contracts",
        "test:scope_runtime_contract", "test:owned_database", "test:editor_assistance",
        "test:codegen", "test:shared_target", "test:static_callback_views",
        "test:module_emission", "test:integer_zero_division", "test:view_flow_completion",
        "test:sql_check", "test:borrow_free_returns", "test:constant_validation",
        "test:view_branch_rebinding", "test:rust_dependencies", "test:installation",
        "test:output_safety", "test:asset_calls", "test:cli", "test:enum_emission",
        "test:comparison_ownership", "test:expression_contracts", "test:low_multiline",
        "test:ownership_boundaries", "test:module_symbols", "test:source_read_retention",
        "test:graph_relations", "test:graph_types", "test:module_parser", "test:enum_parser",
        "test:graph_html", "test:graph_render", "test:resource_contract_characterization",
    ),
    1: (
        "test:http_stdlib", "test:project", "test:task_handles", "test:build_generations",
        "test:typed_errors", "test:http_codegen", "test:http_route_conflicts",
        "test:build_diagnostics", "test:explicit_moves", "test:result_stdlib",
        "test:view_parameter_rebinding", "test:modules", "test:shared_field_moves",
        "test:owned_copy_codegen", "test:class_field_types", "test:view_container_drop",
        "test:view_container_rebinding", "test:view_flow_foundation",
        "test:list_borrow_iteration", "test:conformance", "test:symbols", "test:loop_ownership",
        "test:copy_capabilities", "test:view_iteration", "test:integer_arithmetic",
        "test:high_elif", "test:literal_contracts", "test:enum_patterns",
        "test:nullable_roundtrip", "test:map_cli", "test:enum_modules",
        "test:ownership_calls", "test:stdlib_imports", "test:list_ownership_inference",
        "test:option_match",
    ),
}

KNOWN_PACKAGE_TESTS = frozenset(
    [selector.removeprefix("test:") for selector in ORIGINAL_SELECTORS if selector.startswith("test:")]
    + list(KNOWN_UNSHARDED_TESTS)
)
CONFIG_ENV = (
    "CARGO_INCREMENTAL", "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_TEST_DEBUG",
    "CARGO_PROFILE_DEV_OPT_LEVEL", "CARGO_PROFILE_TEST_OPT_LEVEL",
    "CARGO_PROFILE_DEV_LTO", "CARGO_PROFILE_TEST_LTO", "CARGO_PROFILE_DEV_CODEGEN_UNITS",
    "CARGO_PROFILE_TEST_CODEGEN_UNITS", "CARGO_PROFILE_DEV_PANIC", "CARGO_PROFILE_TEST_PANIC",
    "CARGO_PROFILE_DEV_STRIP", "CARGO_PROFILE_TEST_STRIP", "CARGO_BUILD_TARGET",
    "CARGO_TARGET_DIR", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTDOCFLAGS",
    "CARGO_ENCODED_RUSTDOCFLAGS", "CARGO_HOME", "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTC_BOOTSTRAP",
    "RUSTCFLAGS", "RUSTUP_TOOLCHAIN",
)
EMPTY_CONFIG_ENV = frozenset(CONFIG_ENV) - {"CARGO_HOME"}
# Source: https://github.com/actions/runner-images/blob/win25-vs2026/20261030.294/images/windows/Windows2025-Readme.md
# Its Windows Server 2025 image README (image version 20261030.294.1) documents
# VCPKG_INSTALLATION_ROOT=C:\vcpkg; archived at
# /workspace/evidence/nagi-ci221-measurement/windows2025-vs2026-20261030.294-readme.md.
# Keep this stock path in the producer identity; reject every other VCPKG override.
STOCK_VCPKG_INSTALLATION_ROOT = r"C:\vcpkg"
OPTIONAL_HASHED_FILES = (".cargo/config", ".cargo/config.toml", "compiler/.cargo/config",
                         "compiler/.cargo/config.toml")
HASHED_FILES = (
    ".github/workflows/ci.yml", "Cargo.lock", "Cargo.toml", "compiler/Cargo.toml",
    "scripts/ci/windows_regression_sharding.py",
    "scripts/ci/windows_regression_sharding_projection.py",
    "scripts/ci/linux_critical_path.py", "scripts/ci/nextest_evaluation.py",
)


class RegressionError(ValueError):
    """The selected Cargo binaries/cases do not match the reviewed contract."""


class CommandError(RuntimeError):
    def __init__(self, argv: list[str], returncode: int, output: str):
        super().__init__(f"command exited {returncode}: {argv[0]}")
        self.argv, self.returncode, self.output = argv, returncode, output


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def digest(value: Any) -> str:
    return hashlib.sha256(canonical_json(value)).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise RegressionError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def load_json(path: Path, limit: int = MAX_PLAN_BYTES) -> Any:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise RegressionError(f"missing, linked or oversized JSON evidence: {path.name}")
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_unique_object)


def relative_file(root: Path, relative: str, *, limit: int = MAX_COMMAND_OUTPUT_BYTES) -> Path:
    if not isinstance(relative, str) or not relative or "\\" in relative:
        raise RegressionError("invalid evidence relative path")
    pure = PurePosixPath(relative)
    if pure.is_absolute() or any(part in ("", ".", "..") for part in pure.parts):
        raise RegressionError("evidence path escapes its artifact root")
    path = root
    for part in pure.parts:
        path = path / part
        if path.is_symlink():
            raise RegressionError(f"linked evidence path: {relative}")
    if not path.is_file() or path.stat().st_size > limit:
        raise RegressionError(f"missing, linked or oversized evidence file: {relative}")
    return path


def canonical_executable(value: str) -> str:
    """Normalize Cargo's executable path without ever shell-splitting it."""
    if not isinstance(value, str) or not value:
        raise RegressionError("empty test executable path")
    windows_path = PureWindowsPath(value)
    if windows_path.is_absolute():
        return str(windows_path).casefold()
    return os.path.normcase(os.path.realpath(os.path.abspath(value)))


def absolute_workspace(value: str) -> bool:
    if not isinstance(value, str) or not value:
        return False
    windows_path = PureWindowsPath(value)
    if windows_path.is_absolute():
        return str(windows_path) == value and not any(part in (".", "..") for part in windows_path.parts)
    posix_path = PurePosixPath(value)
    return (posix_path.is_absolute() and str(posix_path) == value and
            not any(part in (".", "..") for part in posix_path.parts))


def workspace_file(workspace: str, relative: str) -> str:
    pure = PurePosixPath(relative)
    if (not relative or "\\" in relative or pure.is_absolute() or
            any(part in ("", ".", "..") for part in pure.parts)):
        raise RegressionError("invalid producer-workspace evidence path")
    windows_root = PureWindowsPath(workspace)
    if windows_root.is_absolute():
        return str(windows_root.joinpath(*pure.parts))
    return str(Path(workspace).joinpath(*pure.parts))


def runner_argv(plan_path: str, workspace: str, python_executable: str) -> list[str]:
    """Build Cargo's runner array without invoking a shell or splitting paths."""
    script = workspace_file(workspace, "scripts/ci/windows_regression_sharding.py")
    if not absolute_workspace(python_executable):
        raise RegressionError("the Python runner executable path is not absolute")
    return [python_executable, "-B", script, "--record-binary", "--plan", plan_path]


def executable_under_target(value: str, workspace: str) -> bool:
    windows_path = PureWindowsPath(value)
    windows_root = PureWindowsPath(workspace)
    if windows_path.is_absolute() or windows_root.is_absolute():
        if not windows_path.is_absolute() or not windows_root.is_absolute():
            return False
        try:
            relative = windows_path.relative_to(windows_root / "target")
        except ValueError:
            return False
        return bool(relative.parts) and ".." not in relative.parts
    try:
        relative = Path(value).relative_to(Path(workspace) / "target")
    except ValueError:
        return False
    return Path(value).is_absolute() and bool(relative.parts) and ".." not in relative.parts


def run_command(
    argv: list[str], *, cwd: Path = ROOT, env: dict[str, str] | None = None,
    stdout_path: Path | None = None, stderr_path: Path | None = None,
    merged: bool = False, max_output: int = MAX_COMMAND_OUTPUT_BYTES,
) -> subprocess.CompletedProcess[str]:
    """Run an argv list and preserve bounded output, including paths with spaces."""
    result = subprocess.run(
        argv, cwd=cwd, env=env, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT if merged else subprocess.PIPE,
        text=True, encoding="utf-8", errors="replace", check=False,
    )
    out_bytes = result.stdout.encode("utf-8", errors="replace")
    err_bytes = b"" if merged else (result.stderr or "").encode("utf-8", errors="replace")
    if len(out_bytes) > max_output or len(err_bytes) > max_output:
        if stdout_path:
            stdout_path.parent.mkdir(parents=True, exist_ok=True)
            stdout_path.write_bytes(out_bytes[:max_output] + b"\n<output limit exceeded>\n")
        if stderr_path and err_bytes:
            stderr_path.parent.mkdir(parents=True, exist_ok=True)
            stderr_path.write_bytes(err_bytes[:max_output] + b"\n<output limit exceeded>\n")
        raise RegressionError(f"command output exceeded {max_output} bytes")
    if stdout_path:
        stdout_path.parent.mkdir(parents=True, exist_ok=True)
        stdout_path.write_bytes(out_bytes)
    if stderr_path:
        stderr_path.parent.mkdir(parents=True, exist_ok=True)
        stderr_path.write_bytes(err_bytes)
    return result


def checked_build_environment(environment: dict[str, str]) -> tuple[dict[str, str], ...]:
    """Reject ambient profile/compiler overrides and keep only the default Cargo home identity."""
    config = {key: environment.get(key, "") for key in CONFIG_ENV}
    if any(config[key] for key in EMPTY_CONFIG_ENV):
        raise RegressionError("unexpected Cargo profile, target or Rust flag override")
    profile = {key: value for key, value in sorted(environment.items())
               if key.startswith("CARGO_PROFILE_")}
    if any(profile.values()):
        raise RegressionError("unexpected Cargo profile environment override")
    target = {key: value for key, value in sorted(environment.items())
              if key.startswith("CARGO_TARGET_")}
    if any(target.values()):
        raise RegressionError("unexpected Cargo target-specific build override")
    build = {key: value for key, value in sorted(environment.items())
             if key.startswith("CARGO_BUILD_")}
    if any(build.values()):
        raise RegressionError("unexpected Cargo build environment override")
    rustc = {key: value for key, value in sorted(environment.items())
             if key.startswith("RUSTC_")}
    if any(rustc.values()):
        raise RegressionError("unexpected Rust compiler environment override")
    vcpkg_items = [(key.upper(), value) for key, value in environment.items()
                   if key.upper().startswith("VCPKG_")]
    if len({key for key, _ in vcpkg_items}) != len(vcpkg_items):
        raise RegressionError("duplicate case-insensitive VCPKG environment override")
    vcpkg_environment = dict(vcpkg_items)
    vcpkg_root = vcpkg_environment.get("VCPKG_INSTALLATION_ROOT", "")
    if vcpkg_root and vcpkg_root != STOCK_VCPKG_INSTALLATION_ROOT:
        raise RegressionError("VCPKG_INSTALLATION_ROOT differs from the documented stock Windows image")
    if any(key != "VCPKG_INSTALLATION_ROOT" for key in vcpkg_environment):
        raise RegressionError("unexpected VCPKG build environment override")
    native = {key: value for key, value in sorted(environment.items())
              if key in {"CC", "CXX", "AR", "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS",
                         "ASFLAGS", "ARFLAGS", "PKG_CONFIG", "LIBCLANG_PATH",
                         "BINDGEN_EXTRA_CLANG_ARGS", "LIB", "LIBPATH", "INCLUDE"} or
              key.startswith(("CC_", "CXX_", "AR_", "CFLAGS_", "CXXFLAGS_", "CPPFLAGS_",
                              "LDFLAGS_", "ASFLAGS_", "ARFLAGS_", "PKG_CONFIG_", "CMAKE_",
                              "BINDGEN_EXTRA_CLANG_ARGS", "OPENSSL_", "SQLITE3_",
                              "LIBSQLITE3_SYS_")) or
              (key.startswith(("HOST_", "TARGET_")) and
               key.endswith(("_CC", "_CXX", "_AR", "_CFLAGS", "_CXXFLAGS", "_CPPFLAGS",
                             "_LDFLAGS", "_ASFLAGS", "_ARFLAGS")))}
    if any(native.values()):
        raise RegressionError("unexpected native-child build environment override")
    if vcpkg_root:
        native["VCPKG_INSTALLATION_ROOT"] = vcpkg_root
    cargo_home = config["CARGO_HOME"]
    if cargo_home and os.path.normcase(os.path.realpath(cargo_home)) != os.path.normcase(
            os.path.realpath(str(Path.home() / ".cargo"))):
        raise RegressionError("CARGO_HOME differs from the cached default Cargo home")
    config["CARGO_HOME"] = "default"
    return config, profile, target, build, rustc, native


def _hash_inputs() -> dict[str, str]:
    result = {}
    for name in HASHED_FILES:
        path = ROOT / name
        if not path.is_file():
            raise RegressionError(f"identity input is missing: {name}")
        result[name] = sha256_file(path)
    for name in OPTIONAL_HASHED_FILES:
        path = ROOT / name
        if path.is_symlink():
            raise RegressionError(f"Cargo configuration must not be a symlink: {name}")
        result[name] = sha256_file(path) if path.is_file() else "absent"
    cargo_home = Path(os.environ.get("CARGO_HOME") or (Path.home() / ".cargo"))
    global_config = cargo_home / "config.toml"
    if global_config.is_symlink():
        raise RegressionError("global Cargo configuration must not be a symlink")
    result["<cargo-home>/config.toml"] = sha256_file(global_config) if global_config.is_file() else "absent"
    legacy_global_config = cargo_home / "config"
    if legacy_global_config.is_symlink():
        raise RegressionError("legacy global Cargo configuration must not be a symlink")
    result["<cargo-home>/config"] = sha256_file(legacy_global_config) if legacy_global_config.is_file() else "absent"
    return result


def expected_members() -> dict[str, dict[str, str]]:
    members: dict[str, dict[str, str]] = {}
    for selector in ORIGINAL_SELECTORS:
        if selector == "lib:nagic":
            kind, name, source = "lib", "nagic", "compiler/src/lib.rs"
        elif selector.startswith("test:"):
            kind, name = "test", selector.removeprefix("test:")
            source = f"compiler/tests/{name}.rs"
        else:
            raise RegressionError(f"invalid reviewed selector: {selector}")
        members[selector] = dict(package=PACKAGE, kind=kind, name=name, source=source)
    if len(members) != 74 or set(members) != set(ORIGINAL_SELECTORS):
        raise RegressionError("frozen original selector inventory is duplicated or incomplete")
    if set(SHARD_SELECTORS) != set(range(SHARD_COUNT)):
        raise RegressionError("wrong shard policy indices")
    flattened = [selector for shard in range(SHARD_COUNT) for selector in SHARD_SELECTORS[shard]]
    if len(flattened) != 74 or len(set(flattened)) != 74 or set(flattened) != set(members):
        raise RegressionError("shard assignments do not partition the original 74 selectors")
    commands = []
    for command in ORIGINAL_COMMANDS:
        words = shlex.split(command)
        if words[:5] != ["cargo", "test", "--locked", "-p", PACKAGE]:
            raise RegressionError("frozen original Cargo command prefix changed")
        selectors: list[str] = []
        rest = words[5:]
        cursor = 0
        while cursor < len(rest):
            word = rest[cursor]
            if word == "--lib":
                selectors.append("lib:nagic")
                cursor += 1
            elif word == "--test" and cursor + 1 < len(rest):
                selectors.append("test:" + rest[cursor + 1])
                cursor += 2
            else:
                raise RegressionError("frozen original Cargo command contains an unknown selector")
        if not selectors or len(selectors) != len(set(selectors)):
            raise RegressionError("frozen original Cargo command has duplicate or empty selectors")
        commands.extend(selectors)
    if len(commands) != 74 or len(commands) != len(set(commands)) or tuple(commands) != ORIGINAL_SELECTORS:
        raise RegressionError("frozen original Cargo commands differ from the reviewed selector inventory")
    return members


def cargo_selector_args(selectors: tuple[str, ...] | list[str]) -> list[str]:
    """Build Cargo arguments only from the frozen selector grammar."""
    expected_members()
    if not selectors or len(selectors) != len(set(selectors)) or not set(selectors) <= set(ORIGINAL_SELECTORS):
        raise RegressionError("empty, duplicate or unknown Cargo selector set")
    args: list[str] = []
    for selector in selectors:
        if selector == "lib:nagic":
            args.append("--lib")
        else:
            args.extend(("--test", selector.removeprefix("test:")))
    return args


def cargo_runner_config(host: str, runner_command: list[str]) -> str:
    if not re.fullmatch(r"[A-Za-z0-9_-]+", host):
        raise RegressionError("invalid Cargo target host for runner configuration")
    if (not isinstance(runner_command, list) or not runner_command or
            any(not isinstance(item, str) or not item for item in runner_command)):
        raise RegressionError("Cargo runner configuration must be an argv array")
    runner_json = json.dumps(runner_command, ensure_ascii=False, separators=(",", ":"))
    return f"target.{host}.runner={runner_json}"


def cargo_test_command(shard: int, *, no_run: bool = False, harness: bool = False,
                       runner_command: list[str] | None = None) -> list[str]:
    if type(shard) is not int or shard not in SHARD_SELECTORS:
        raise RegressionError("unknown Windows regression shard")
    args = ["cargo", "test", "--locked", "-p", PACKAGE,
            *cargo_selector_args(SHARD_SELECTORS[shard])]
    if no_run:
        args += ["--no-run", "--message-format=json"]
    if harness:
        if runner_command is None:
            raise RegressionError("Windows regression execution requires a plan-bound Cargo runner")
        args += ["--config", cargo_runner_config("x86_64-pc-windows-msvc", runner_command)]
        args += ["--", "--format=pretty"]
    return args


def package_inventory(metadata: dict[str, Any]) -> tuple[str, dict[str, dict[str, str]], dict[str, str], str]:
    """Require exact package/test target identity, including known non-shard tests."""
    try:
        root = Path(metadata["workspace_root"]).resolve(strict=True)
        package_ids = set(metadata["workspace_members"])
        packages = [p for p in metadata["packages"] if p.get("name") == PACKAGE and p.get("id") in package_ids]
    except (KeyError, TypeError, OSError) as error:
        raise RegressionError("invalid Cargo workspace metadata") from error
    if len(packages) != 1:
        raise RegressionError("expected exactly one workspace nagic package")
    package = packages[0]
    try:
        manifest_path = Path(package["manifest_path"]).resolve(strict=True)
        manifest_relative = manifest_path.relative_to(root).as_posix()
    except (KeyError, OSError, ValueError, TypeError) as error:
        raise RegressionError("Cargo package manifest is outside the workspace or missing") from error
    if manifest_relative != "compiler/Cargo.toml":
        raise RegressionError("Cargo nagic package manifest moved from its reviewed source root")
    actual_tests: dict[str, dict[str, str]] = {}
    binaries: dict[str, str] = {}
    seen_sources: set[tuple[str, str]] = set()
    for target in package.get("targets", []):
        kinds = target.get("kind")
        if not isinstance(kinds, list) or len(kinds) != 1:
            continue
        kind = kinds[0]
        name = target.get("name")
        try:
            source = Path(target["src_path"]).resolve(strict=True).relative_to(root).as_posix()
        except (KeyError, OSError, ValueError, TypeError) as error:
            if kind in ("test", "lib", "bin"):
                raise RegressionError("Cargo target source is outside the workspace or missing") from error
            continue
        if kind == "test" and target.get("test") is True:
            if target.get("required-features"):
                raise RegressionError(f"unreviewed feature-gated test target: {name}")
            if not isinstance(name, str) or name in actual_tests:
                raise RegressionError("duplicate or malformed Cargo integration target")
            identity = (kind, source)
            if identity in seen_sources:
                raise RegressionError("ambiguous Cargo target source identity")
            seen_sources.add(identity)
            actual_tests[name] = dict(package=PACKAGE, kind=kind, name=name, source=source)
        elif kind == "lib" and name == PACKAGE and target.get("test") is True:
            if "lib:nagic" in actual_tests:
                raise RegressionError("duplicate Cargo nagic library target")
            actual_tests["lib:nagic"] = dict(package=PACKAGE, kind="lib", name=name, source=source)
        elif kind == "bin" and name == PACKAGE:
            if name in binaries:
                raise RegressionError("duplicate Cargo nagic binary target")
            binaries[name] = source
    if set(actual_tests) != set(KNOWN_PACKAGE_TESTS) | {"lib:nagic"}:
        missing = sorted((set(KNOWN_PACKAGE_TESTS) | {"lib:nagic"}) - set(actual_tests))
        unknown = sorted(set(actual_tests) - set(KNOWN_PACKAGE_TESTS) - {"lib:nagic"})
        raise RegressionError(f"Cargo integration inventory drift: missing={missing[:8]} unknown={unknown[:8]}")
    if actual_tests.get("lib:nagic") != expected_members()["lib:nagic"]:
        raise RegressionError("Cargo nagic library identity changed")
    expected_tests = {selector.removeprefix("test:"): member
                      for selector, member in expected_members().items() if selector.startswith("test:")}
    if any(actual_tests[name] != member for name, member in expected_tests.items()):
        raise RegressionError("a reviewed Cargo test selector resolves to a different source identity")
    if set(binaries) != {PACKAGE} or binaries[PACKAGE] != "compiler/src/main.rs":
        raise RegressionError("Cargo nagic CLI binary identity changed")
    return package["id"], actual_tests, binaries, str(manifest_path.parent)


def _current_context(metadata: dict[str, Any] | None = None) -> tuple[dict[str, Any], dict[str, Any]]:
    if Path.cwd().resolve() != ROOT:
        raise RegressionError("Windows regression commands must run from the checkout root")
    if not sys.platform.startswith("win") or platform.system() != "Windows":
        raise RegressionError("the regression sharding runner is Windows-only")
    expected_head = os.environ.get("NAGI_REGRESSION_EXPECTED_HEAD", "")
    if not HEX40.fullmatch(expected_head):
        raise RegressionError("missing exact workflow-selected source SHA")
    head_result = run_command(["git", "rev-parse", "HEAD"])
    head = head_result.stdout.strip()
    if head != expected_head:
        raise RegressionError("checked out source differs from the workflow-selected SHA")
    status = run_command(["git", "status", "--porcelain", "--untracked-files=no"])
    if status.stdout.strip():
        raise RegressionError("tracked source tree is dirty")
    rust = run_command(["rustc", "-vV"])
    cargo = run_command(["cargo", "--version"])
    cfg = run_command(["rustc", "--print", "cfg"])
    host_rows = re.findall(r"(?m)^host: ([A-Za-z0-9_-]+)$", rust.stdout)
    if host_rows != ["x86_64-pc-windows-msvc"]:
        raise RegressionError("Windows regression requires x86_64-pc-windows-msvc")
    (config, profile_environment, target_environment, build_environment,
     rustc_environment, native_environment) = checked_build_environment(dict(os.environ))
    if metadata is None:
        meta = run_command(["cargo", "metadata", "--locked", "--no-deps", "--format-version=1"])
        try:
            metadata = json.loads(meta.stdout, object_pairs_hook=_unique_object)
        except (json.JSONDecodeError, RegressionError) as error:
            raise RegressionError("Cargo metadata output is not unambiguous JSON") from error
    package_id, package_targets, binary_targets, test_cwd = package_inventory(metadata)
    if Path(metadata["workspace_root"]).resolve(strict=True) != ROOT.resolve():
        raise RegressionError("Cargo workspace root differs from the checked-out source root")
    run_id = os.environ.get("GITHUB_RUN_ID", "")
    attempt = os.environ.get("GITHUB_RUN_ATTEMPT", "")
    if not run_id.isdecimal() or not attempt.isdecimal() or int(attempt) < 1:
        raise RegressionError("missing Actions run/attempt identity")
    identity = dict(
        schema=SCHEMA, protocol=PROTOCOL, run_id=run_id, attempt=attempt, head=head,
        tree=run_command(["git", "rev-parse", "HEAD^{tree}"]).stdout.strip(),
        package=PACKAGE, platform="windows-x86_64", runner_os=os.environ.get("RUNNER_OS", ""),
        runner_arch=os.environ.get("RUNNER_ARCH", ""), host=host_rows[0],
        rustc=rust.stdout, rustc_cfg=cfg.stdout, cargo=cargo.stdout, python=sys.version,
        config=config, profile_environment=profile_environment, target_environment=target_environment,
        build_environment=build_environment, rustc_environment=rustc_environment,
        native_environment=native_environment,
        cargo_lock_sha256=sha256_file(ROOT / "Cargo.lock"),
        package_manifest_sha256=sha256_file(ROOT / "compiler/Cargo.toml"),
        inventory_sha256=digest({"selected": expected_members(),
                                 "all_test_targets": sorted(KNOWN_PACKAGE_TESTS),
                                 "binary": {PACKAGE: binary_targets[PACKAGE]}}),
        inputs=_hash_inputs(),
    )
    python_executable = str(Path(sys.executable).resolve(strict=True))
    context = dict(identity=identity,
                   producer=dict(workspace=str(ROOT.resolve()), test_cwd=test_cwd,
                                 package_id=package_id, python_executable=python_executable,
                                 python_sha256=sha256_file(Path(python_executable))))
    return context, dict(metadata=metadata, package_targets=package_targets,
                         binary_targets=binary_targets)


def _relative_source(path: str, workspace: str) -> str:
    windows_path = PureWindowsPath(path)
    windows_root = PureWindowsPath(workspace)
    if windows_path.is_absolute() or windows_root.is_absolute():
        if not windows_path.is_absolute() or not windows_root.is_absolute():
            raise RegressionError("Cargo compiler-artifact uses mixed absolute path forms")
        if str(windows_path) != path or str(windows_root) != workspace:
            raise RegressionError("Cargo compiler-artifact Windows path is not normalized")
        try:
            relative = windows_path.relative_to(windows_root).as_posix()
        except ValueError as error:
            raise RegressionError("Cargo compiler-artifact source is outside its producer workspace") from error
        if any(part in ("", ".", "..") for part in PurePosixPath(relative).parts):
            raise RegressionError("Cargo compiler-artifact source path is not normalized")
    else:
        posix_path = PurePosixPath(path)
        posix_root = PurePosixPath(workspace)
        if ("\\" in path or not posix_path.is_absolute() or not posix_root.is_absolute()):
            raise RegressionError("Cargo compiler-artifact source uses a malformed POSIX path")
        try:
            relative = posix_path.relative_to(posix_root).as_posix()
        except ValueError as error:
            raise RegressionError("Cargo compiler-artifact source is outside its producer workspace") from error
        if any(part in ("", ".", "..") for part in PurePosixPath(relative).parts):
            raise RegressionError("Cargo compiler-artifact source path is not normalized")
    if not (ROOT / relative).is_file():
        raise RegressionError("Cargo compiler-artifact source is missing from the aggregate checkout")
    return relative


def _identity_for_artifact(artifact: dict[str, Any], package_id: str,
                           expected: dict[str, dict[str, str]], workspace: str) -> tuple[str, str] | None:
    if artifact.get("reason") != "compiler-artifact" or artifact.get("package_id") != package_id:
        return None
    target = artifact.get("target", {})
    kinds = target.get("kind")
    name = target.get("name")
    src = target.get("src_path")
    if not isinstance(kinds, list) or not isinstance(name, str) or not isinstance(src, str):
        raise RegressionError("malformed Cargo compiler-artifact identity")
    relative = _relative_source(src, workspace)
    profile = artifact.get("profile", {})
    is_test = profile.get("test") is True
    if kinds == ["bin"] and name == PACKAGE and relative == "compiler/src/main.rs":
        executable = artifact.get("executable")
        if profile.get("test") is not False or not isinstance(executable, str):
            raise RegressionError("Cargo omitted the non-test nagic CLI executable")
        return "bin:nagic", executable
    if is_test:
        matches = [selector for selector, member in expected.items()
                   if kinds == [member["kind"]] and name == member["name"] and relative == member["source"]]
        if len(matches) != 1:
            raise RegressionError("unexpected or ambiguous compiled test target")
        if not isinstance(artifact.get("executable"), str):
            raise RegressionError("Cargo omitted a selected test executable")
        return matches[0], artifact["executable"]
    return None


def compiled_artifacts(output: str, package_id: str,
                       selectors: tuple[str, ...], *, workspace: str | None = None,
                       require_files: bool = True) -> tuple[dict[str, str], str]:
    expected_all = expected_members()
    expected = {selector: expected_all[selector] for selector in selectors}
    found: dict[str, str] = {}
    binary: str | None = None
    for number, line in enumerate(output.splitlines(), 1):
        try:
            row = json.loads(line, object_pairs_hook=_unique_object)
        except (json.JSONDecodeError, RegressionError) as error:
            raise RegressionError(f"Cargo JSON build output line {number} is malformed") from error
        result = _identity_for_artifact(row, package_id, expected, workspace or str(ROOT.resolve()))
        if result is None:
            continue
        identity, executable = result
        if identity == "bin:nagic":
            if binary is not None:
                raise RegressionError("duplicate compiled nagic CLI artifact")
            binary = executable
            continue
        if identity in found:
            raise RegressionError("duplicate selected compiler-artifact")
        found[identity] = executable
    if set(found) != set(selectors):
        raise RegressionError(f"compiled executable inventory mismatch: missing={sorted(set(selectors)-set(found))}")
    if binary is None:
        raise RegressionError("Cargo did not report the nagic CLI binary required by integration tests")
    paths = [canonical_executable(path) for path in [*found.values(), binary]]
    if len(paths) != len(set(paths)):
        raise RegressionError("Cargo mapped multiple targets to one executable path")
    for path in [*found.values(), binary]:
        executable = Path(path)
        if not executable_under_target(path, workspace or str(ROOT.resolve())):
            raise RegressionError("compiled Windows test/CLI executable escapes its producer target directory")
        if executable.suffix.lower() != ".exe":
            raise RegressionError("compiled Windows test/CLI executable is not an .exe")
        if require_files and (not executable.is_file() or executable.is_symlink()):
            raise RegressionError("compiled Windows test/CLI executable is missing")
    return found, binary


def _list_cases(executable: str, selector: str, destination: Path,
                env: dict[str, str]) -> tuple[set[str], set[str], dict[str, str]]:
    if not HEX64.fullmatch(digest(selector)):
        raise RegressionError("selector digest failed internal validation")
    path = Path(executable)
    all_output = run_command([str(path), "--list", "--format=pretty"], env=env, max_output=MAX_EVENT_BYTES)
    ignored_output = run_command([str(path), "--list", "--ignored", "--format=pretty"], env=env,
                                 max_output=MAX_EVENT_BYTES)
    key = digest(selector)
    all_rel = f"lists/{key}.all.txt"
    ignored_rel = f"lists/{key}.ignored.txt"
    (destination / all_rel).parent.mkdir(parents=True, exist_ok=True)
    (destination / all_rel).write_text(all_output.stdout, encoding="utf-8", newline="\n")
    (destination / ignored_rel).write_text(ignored_output.stdout, encoding="utf-8", newline="\n")
    cases = parse_libtest_listing(all_output.stdout)
    ignored = parse_libtest_listing(ignored_output.stdout)
    if not cases or not ignored <= cases:
        raise RegressionError(f"empty or inconsistent libtest inventory for {selector}")
    files = {all_rel: sha256_file(destination / all_rel),
             ignored_rel: sha256_file(destination / ignored_rel)}
    return cases, ignored, files


def _write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def _write_json_exclusive(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, ensure_ascii=False, sort_keys=True, indent=2)
        stream.write("\n")


def _log_command(directory: Path, argv: list[str], label: str, result: subprocess.CompletedProcess[str],
                 *, environment: dict[str, str] | None = None, merged: bool = False) -> None:
    log = directory / "orchestration.log"
    with log.open("a", encoding="utf-8", newline="\n") as stream:
        stream.write("\nCOMMAND " + json.dumps(argv, ensure_ascii=False) + "\n")
        stream.write("LABEL " + label + "\n")
        if environment:
            stream.write("SCOPED_ENV " + json.dumps(environment, sort_keys=True) + "\n")
        stream.write("EXIT " + str(result.returncode) + "\n")
        if result.stdout:
            stream.write("OUTPUT\n" + result.stdout)
            if not result.stdout.endswith("\n"):
                stream.write("\n")
        if not merged and result.stderr:
            stream.write("STDERR\n" + result.stderr)
            if not result.stderr.endswith("\n"):
                stream.write("\n")


def _begin_run(directory: Path, shard: int) -> tuple[Path, Path]:
    directory.mkdir(parents=True, exist_ok=False)
    orchestration = directory / "orchestration.log"
    orchestration.write_text(f"Windows regression shard {shard}\n", encoding="utf-8")
    return directory, orchestration


def _run_shard(shard: int, destination: Path) -> None:
    if shard not in SHARD_SELECTORS:
        raise RegressionError("unknown Windows regression shard")
    directory, _ = _begin_run(destination, shard)
    try:
        context, inventory = _current_context()
        artifact_root = destination.resolve().relative_to(ROOT.resolve()).as_posix()
        if artifact_root != f"build/ci-evidence/windows-regression/shard-{shard}":
            raise RegressionError("shard evidence directory differs from the reviewed artifact path")
        selectors = SHARD_SELECTORS[shard]
        members = expected_members()
        selected = {selector: members[selector] for selector in selectors}
        _write_json(directory / "inventory.json", dict(context=context, shard=shard,
                   selector_set=sorted(selectors), targets=selected,
                   inventory_sha256=context["identity"]["inventory_sha256"]))

        compile_command = cargo_test_command(shard, no_run=True)
        compile_env = os.environ.copy()
        compile_env["CARGO_TERM_COLOR"] = "never"
        compiled = run_command(compile_command, env=compile_env,
                               stdout_path=directory / "compile.jsonl",
                               stderr_path=directory / "compile.stderr.log")
        _log_command(directory, compile_command, "compile selected test executables", compiled,
                     environment={"CARGO_TERM_COLOR": "never"})
        if compiled.returncode:
            raise CommandError(compile_command, compiled.returncode, compiled.stdout + (compiled.stderr or ""))
        binaries, compiler_binary = compiled_artifacts(
            compiled.stdout, context["producer"]["package_id"], selectors,
            workspace=context["producer"]["workspace"])
        binary_identity = {"path": canonical_executable(compiler_binary), "sha256": sha256_file(Path(compiler_binary))}
        binary_identities = {}
        cases_by_selector: dict[str, set[str]] = {}
        ignored_by_selector: dict[str, set[str]] = {}
        list_files: dict[str, str] = {}
        for selector in selectors:
            executable = binaries[selector]
            cases, ignored, files = _list_cases(executable, selector, directory, os.environ.copy())
            cases_by_selector[selector], ignored_by_selector[selector] = cases, ignored
            list_files.update(files)
            binary_identities[selector] = dict(path=canonical_executable(executable), sha256=sha256_file(Path(executable)))
        if not any(cases_by_selector[s] - ignored_by_selector[s] for s in selectors):
            raise RegressionError("selected shard contains no runnable tests")

        execution_dir = directory / "execution"
        execution_dir.mkdir(parents=True, exist_ok=False)
        manifest_members: dict[str, dict[str, str]] = {}
        rows_by_selector = {}
        for selector in selectors:
            identity = binary_identities[selector]
            marker = digest(dict(selector=selector, executable=identity["path"],
                                 sha256=identity["sha256"], context=context))
            event_rel = f"events/{marker}.events"
            started_rel = f"runner/{marker}.started.json"
            finished_rel = f"runner/{marker}.finished.json"
            workspace = context["producer"]["workspace"]
            manifest_members[selector] = dict(executable=identity["path"], sha256=identity["sha256"],
                                             marker=marker,
                                             events=workspace_file(workspace, f"{artifact_root}/{event_rel}"),
                                             started=workspace_file(workspace, f"{artifact_root}/{started_rel}"),
                                             finished=workspace_file(workspace, f"{artifact_root}/{finished_rel}"),
                                             event_file=event_rel, started_file=started_rel,
                                             finished_file=finished_rel)
            rows_by_selector[selector] = dict(
                target=selected[selector], executable=identity,
                cases=sorted(cases_by_selector[selector]),
                ignored=sorted(ignored_by_selector[selector]),
                list_files={relative: list_files[relative] for relative in
                            (f"lists/{digest(selector)}.all.txt", f"lists/{digest(selector)}.ignored.txt")},
                marker=marker, event_file=event_rel, started_file=started_rel,
                finished_file=finished_rel)
        plan_path = execution_dir / "execution.json"
        runner_command = runner_argv(str(plan_path.resolve()), context["producer"]["workspace"],
                                     context["producer"]["python_executable"])
        plan = dict(schema=SCHEMA, protocol=PROTOCOL, context=context, shard=shard,
                    requested_harness_arguments=["--format=pretty"],
                    runner_command=runner_command, cwd=context["producer"]["test_cwd"], artifact_root=artifact_root,
                    nagic_binary=binary_identity, members=manifest_members)
        _write_json(plan_path, plan)

        test_env = os.environ.copy()
        test_env["CARGO_TERM_COLOR"] = "never"
        run_command_args = cargo_test_command(shard, harness=True, runner_command=runner_command)
        execution = run_command(run_command_args, env=test_env,
                                stdout_path=directory / "cargo-test.log", merged=True)
        _log_command(directory, run_command_args, "execute and reconcile selected test cases", execution,
                     environment={"CARGO_TERM_COLOR": "never"}, merged=True)
        if execution.returncode:
            raise CommandError(run_command_args, execution.returncode, execution.stdout)
        if sha256_file(Path(compiler_binary)) != binary_identity["sha256"]:
            raise RegressionError("nagic CLI compiler artifact changed during the shard execution")
        markers = {selector: row["marker"] for selector, row in rows_by_selector.items()}
        blocks = libtest_protocol.execution_blocks(execution.stdout, markers)
        results = []
        files = {"compile.jsonl": sha256_file(directory / "compile.jsonl"),
                 "compile.stderr.log": sha256_file(directory / "compile.stderr.log"),
                 "cargo-test.log": sha256_file(directory / "cargo-test.log"),
                 "inventory.json": sha256_file(directory / "inventory.json"),
                 "execution/execution.json": sha256_file(plan_path),
                 "orchestration.log": sha256_file(directory / "orchestration.log")}
        files.update(list_files)
        for selector in selectors:
            row = rows_by_selector[selector]
            event = relative_file(directory, row["event_file"], limit=MAX_EVENT_BYTES)
            started = load_json(directory / row["started_file"])
            finished = load_json(directory / row["finished_file"])
            expected_args = ["--format=pretty"]
            actual_args = [*expected_args, "--logfile", str(event.resolve())]
            if started != dict(schema=SCHEMA, protocol=PROTOCOL, selector=selector,
                               executable=row["executable"]["path"], sha256=row["executable"]["sha256"],
                               cwd=context["producer"]["test_cwd"], requested_arguments=expected_args,
                               actual_arguments=actual_args):
                raise RegressionError("outer test runner start record differs from the execution plan")
            if finished != dict(schema=SCHEMA, protocol=PROTOCOL, selector=selector,
                                executable=row["executable"]["path"], sha256=row["executable"]["sha256"],
                                returncode=0):
                raise RegressionError("outer test runner did not finish successfully")
            checked = libtest_protocol.check_execution(
                blocks[selector], event.read_text(encoding="utf-8"),
                set(row["cases"]), set(row["ignored"]), dict(id=selector))
            if sha256_file(Path(row["executable"]["path"])) != row["executable"]["sha256"]:
                raise RegressionError("test executable changed while its cases were running")
            files[row["event_file"]] = sha256_file(event)
            files[row["started_file"]] = sha256_file(directory / row["started_file"])
            files[row["finished_file"]] = sha256_file(directory / row["finished_file"])
            results.append(dict(**row, **checked))
        if sha256_file(Path(compiler_binary)) != binary_identity["sha256"]:
            raise RegressionError("nagic CLI binary changed while tests were running")
        passed = sum(row["passed"] for row in results)
        if passed <= 0:
            raise RegressionError("shard executed zero passing cases")
        receipt = dict(schema=SCHEMA, shard=shard, context=context,
                       inventory_sha256=context["identity"]["inventory_sha256"],
                       selector_ids=list(selectors), command=run_command_args, exit=0,
                       nagic_binary=binary_identity, targets=results, passed=passed,
                       files=files)
        _write_json(directory / "receipt.json", receipt)
        print(f"Windows regression shard {shard}: {len(results)} targets, {passed} named cases passed.")
    except BaseException:
        with (directory / "orchestration.log").open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("\nORCHESTRATION FAILURE\n" + traceback.format_exc())
        raise


def _validate_receipt(root: Path, receipt: dict[str, Any], shard: int,
                      context: dict[str, Any]) -> int:
    selectors = SHARD_SELECTORS[shard]
    if set(receipt) != {"schema", "shard", "context", "inventory_sha256", "selector_ids",
                       "command", "exit", "nagic_binary", "targets", "passed", "files"}:
        raise RegressionError("shard receipt has missing or unexpected fields")
    if receipt.get("schema") != SCHEMA or receipt.get("shard") != shard:
        raise RegressionError("missing, wrong-schema or wrong-numbered shard receipt")
    producer_context = receipt.get("context")
    if (not isinstance(producer_context, dict) or set(producer_context) != {"identity", "producer"} or
            producer_context.get("identity") != context.get("identity")):
        raise RegressionError("shard receipt source, run, toolchain, config or inventory mismatch")
    producer = producer_context.get("producer")
    if (not isinstance(producer, dict) or set(producer) != {
                "workspace", "test_cwd", "package_id", "python_executable", "python_sha256"} or
            not isinstance(producer.get("workspace"), str) or
            not absolute_workspace(producer["workspace"]) or
            not isinstance(producer.get("test_cwd"), str) or
            producer["test_cwd"] != workspace_file(producer["workspace"], "compiler") or
            not isinstance(producer.get("package_id"), str) or not producer["package_id"] or
            not isinstance(producer.get("python_executable"), str) or
            not absolute_workspace(producer["python_executable"]) or
            not isinstance(producer.get("python_sha256"), str) or
            not HEX64.fullmatch(producer["python_sha256"])):
        raise RegressionError("shard producer workspace or Cargo package identity is malformed")
    workspace_name = producer["workspace"]
    package_id = producer["package_id"]
    inventory_sha256 = producer_context["identity"]["inventory_sha256"]
    if receipt.get("inventory_sha256") != inventory_sha256:
        raise RegressionError("shard receipt inventory hash differs from its source identity")
    if receipt.get("selector_ids") != list(selectors):
        raise RegressionError("shard receipt selector assignment mismatch")
    if type(receipt.get("exit")) is not int or receipt["exit"] != 0:
        raise RegressionError("shard Cargo test command did not exit successfully")
    files = receipt.get("files")
    if not isinstance(files, dict) or not files:
        raise RegressionError("shard receipt has no log/artifact digests")
    inventory = load_json(relative_file(root, "inventory.json"))
    if inventory != dict(context=producer_context, shard=shard, selector_set=sorted(selectors),
                         targets={s: expected_members()[s] for s in selectors},
                         inventory_sha256=inventory_sha256):
        raise RegressionError("shard inventory manifest mismatch")
    plan = load_json(relative_file(root, "execution/execution.json"))
    if set(plan) != {"schema", "protocol", "context", "shard", "requested_harness_arguments",
                     "runner_command", "cwd", "artifact_root", "nagic_binary", "members"}:
        raise RegressionError("outer runner plan has missing or unexpected fields")
    if (plan.get("schema") != SCHEMA or plan.get("protocol") != PROTOCOL or
            plan.get("context") != producer_context):
        raise RegressionError("outer runner execution plan identity mismatch")
    artifact_root = f"build/ci-evidence/windows-regression/shard-{shard}"
    expected_plan_path = workspace_file(workspace_name, f"{artifact_root}/execution/execution.json")
    _validate_runner_plan(plan, manifest_path=expected_plan_path)
    runner_command = plan["runner_command"]
    if receipt.get("command") != cargo_test_command(shard, harness=True, runner_command=runner_command):
        raise RegressionError("shard Cargo invocation differs from its plan-bound runner")
    if (plan.get("shard") != shard or plan.get("cwd") != producer["test_cwd"] or
            plan.get("artifact_root") != artifact_root or
            plan.get("requested_harness_arguments") != ["--format=pretty"] or
            set(plan.get("members", {})) != set(selectors)):
        raise RegressionError("outer runner plan has unexpected Windows command or members")
    compile_data = relative_file(root, "compile.jsonl").read_text(encoding="utf-8")
    binaries, cli = compiled_artifacts(compile_data, package_id, selectors,
                                      workspace=workspace_name, require_files=False)
    cli_expected = receipt.get("nagic_binary")
    if (not isinstance(cli_expected, dict) or set(cli_expected) != {"path", "sha256"} or
            cli_expected != plan.get("nagic_binary") or
            canonical_executable(cli_expected.get("path", "")) != canonical_executable(cli)):
        raise RegressionError("nagic CLI compiler-artifact path differs from receipt")
    if not HEX64.fullmatch(str(cli_expected.get("sha256", ""))):
        raise RegressionError("nagic CLI receipt is missing an executable digest")
    rows = receipt.get("targets")
    if not isinstance(rows, list) or len(rows) != len(selectors):
        raise RegressionError("missing or duplicate target receipt rows")
    by_selector = {}
    for row in rows:
        if not isinstance(row, dict):
            raise RegressionError("malformed target receipt")
        target = row.get("target")
        if not isinstance(target, dict):
            raise RegressionError("target receipt has no Cargo identity")
        selector = ("lib:nagic" if target.get("kind") == "lib" and target.get("name") == "nagic"
                    else "test:" + str(target.get("name")))
        if selector in by_selector:
            raise RegressionError("duplicate target receipt")
        by_selector[selector] = row
    if set(by_selector) != set(selectors):
        raise RegressionError("target receipt set differs from shard assignment")
    if set(plan["members"]) != set(by_selector):
        raise RegressionError("runner plan target set differs from receipt")
    cargo_output = relative_file(root, "cargo-test.log").read_text(encoding="utf-8")
    markers = {}
    cases_by_selector: dict[str, set[str]] = {}
    ignored_by_selector: dict[str, set[str]] = {}
    expected_file_paths = {"compile.jsonl", "compile.stderr.log", "cargo-test.log", "inventory.json",
                           "execution/execution.json", "orchestration.log"}
    test_cwd = producer["test_cwd"]
    for selector in selectors:
        row = by_selector[selector]
        if set(row) != {"target", "executable", "cases", "ignored", "ignored_reasons", "list_files",
                        "marker", "event_file", "started_file", "finished_file", "passed"}:
            raise RegressionError("target receipt has missing or unexpected fields")
        expected_target = expected_members()[selector]
        if row.get("target") != expected_target:
            raise RegressionError("receipt package/kind/name/source identity mismatch")
        exe = row.get("executable")
        if (not isinstance(exe, dict) or set(exe) != {"path", "sha256"} or
                canonical_executable(exe.get("path", "")) != canonical_executable(binaries[selector]) or
                not HEX64.fullmatch(str(exe.get("sha256", "")))):
            raise RegressionError("test executable path or SHA256 mismatch")
        plan_row = plan["members"].get(selector)
        marker = digest(dict(selector=selector, executable=canonical_executable(exe["path"]),
                             sha256=exe["sha256"], context=producer_context))
        event_rel = f"events/{marker}.events"
        started_rel = f"runner/{marker}.started.json"
        finished_rel = f"runner/{marker}.finished.json"
        expected_plan_row = dict(
            executable=canonical_executable(exe["path"]), sha256=exe["sha256"], marker=marker,
            events=workspace_file(workspace_name, f"{artifact_root}/{event_rel}"),
            started=workspace_file(workspace_name, f"{artifact_root}/{started_rel}"),
            finished=workspace_file(workspace_name, f"{artifact_root}/{finished_rel}"),
            event_file=event_rel, started_file=started_rel, finished_file=finished_rel)
        if (row.get("marker") != marker or row.get("event_file") != event_rel or
                row.get("started_file") != started_rel or row.get("finished_file") != finished_rel or
                plan_row != expected_plan_row):
            raise RegressionError("test executable does not match its outer runner entry")
        markers[selector] = marker
        cases = row.get("cases")
        ignored = row.get("ignored")
        reasons = row.get("ignored_reasons")
        if (not isinstance(cases, list) or not cases or any(not isinstance(n, str) or not n for n in cases) or
                len(cases) != len(set(cases)) or not isinstance(ignored, list) or
                any(not isinstance(n, str) or not n for n in ignored) or len(ignored) != len(set(ignored)) or
                not set(ignored) <= set(cases) or not isinstance(reasons, dict) or set(reasons) != set(ignored) or
                any(reason is not None and not isinstance(reason, str) for reason in reasons.values())):
            raise RegressionError("missing, duplicate or malformed named test/ignored inventory")
        all_path = f"lists/{digest(selector)}.all.txt"
        ignored_path = f"lists/{digest(selector)}.ignored.txt"
        if row.get("list_files") != {all_path: files.get(all_path), ignored_path: files.get(ignored_path)}:
            raise RegressionError("target receipt listing digests differ from the aggregate file map")
        expected_file_paths.update((all_path, ignored_path, event_rel, started_rel, finished_rel))
        listed_cases = parse_libtest_listing(relative_file(root, all_path, limit=MAX_EVENT_BYTES).read_text(encoding="utf-8"))
        listed_ignored = parse_libtest_listing(relative_file(root, ignored_path, limit=MAX_EVENT_BYTES).read_text(encoding="utf-8"))
        if listed_cases != set(cases) or listed_ignored != set(ignored):
            raise RegressionError("receipt named cases differ from saved independent libtest listings")
        event_path = relative_file(root, event_rel, limit=MAX_EVENT_BYTES)
        started = load_json(relative_file(root, started_rel))
        finished = load_json(relative_file(root, finished_rel))
        runner_row = plan_row
        expected_args = ["--format=pretty"]
        actual_args = [*expected_args, "--logfile", runner_row["events"]]
        if started != dict(schema=SCHEMA, protocol=PROTOCOL, selector=selector,
                           executable=exe["path"], sha256=exe["sha256"], cwd=test_cwd,
                           requested_arguments=expected_args, actual_arguments=actual_args):
            raise RegressionError("outer runner start record mismatch")
        if finished != dict(schema=SCHEMA, protocol=PROTOCOL, selector=selector,
                            executable=exe["path"], sha256=exe["sha256"], returncode=0):
            raise RegressionError("outer runner finish record mismatch")
        cases_by_selector[selector], ignored_by_selector[selector] = set(cases), set(ignored)
    if set(files) != expected_file_paths:
        raise RegressionError("shard artifact file set is missing or has unexpected entries")
    for relative, sha in files.items():
        path = relative_file(root, relative)
        if not isinstance(sha, str) or not HEX64.fullmatch(sha) or sha256_file(path) != sha:
            raise RegressionError(f"artifact digest mismatch for {relative}")
    blocks = libtest_protocol.execution_blocks(cargo_output, markers)
    total = 0
    for selector in selectors:
        row = by_selector[selector]
        checked = libtest_protocol.check_execution(
            blocks[selector], relative_file(root, row["event_file"], limit=MAX_EVENT_BYTES).read_text(encoding="utf-8"),
            cases_by_selector[selector], ignored_by_selector[selector], dict(id=selector))
        if checked.get("ignored_reasons") != row.get("ignored_reasons"):
            raise RegressionError("case event ignored reasons differ from receipt")
        if row.get("cases") != sorted(cases_by_selector[selector]) or row.get("ignored") != sorted(ignored_by_selector[selector]):
            raise RegressionError("target receipt case inventory is not canonical")
        if type(row.get("passed")) is not int or row["passed"] != checked["passed"]:
            raise RegressionError("target receipt passed count differs from validated events")
        total += checked["passed"]
    if type(receipt.get("passed")) is not int or receipt["passed"] != total or total <= 0:
        raise RegressionError("shard receipt count is incomplete or zero")
    return total


def reconcile(receipt_root: Path, context: dict[str, Any]) -> None:
    expected_artifacts = {f"nagi-regression-windows-shard-{i}" for i in range(SHARD_COUNT)}
    if not receipt_root.is_dir() or receipt_root.is_symlink():
        raise RegressionError("Windows regression artifact directory is missing")
    actual_artifacts = {path.name for path in receipt_root.iterdir() if path.is_dir() and not path.is_symlink()}
    if actual_artifacts != expected_artifacts:
        raise RegressionError(f"missing, duplicate or unexpected shard artifacts: {sorted(actual_artifacts)}")
    receipts = []
    total = 0
    all_selectors: list[str] = []
    for shard in range(SHARD_COUNT):
        root = receipt_root / f"nagi-regression-windows-shard-{shard}"
        receipt = load_json(relative_file(root, "receipt.json"))
        if not isinstance(receipt, dict):
            raise RegressionError("shard receipt must be a JSON object")
        total += _validate_receipt(root, receipt, shard, context)
        receipts.append(receipt)
        all_selectors.extend(receipt["selector_ids"])
    if len(all_selectors) != 74 or len(set(all_selectors)) != 74 or set(all_selectors) != set(ORIGINAL_SELECTORS):
        raise RegressionError("Windows shard receipts do not cover the exact original 74 selectors")
    if total <= 0:
        raise RegressionError("Windows regression aggregate executed zero passing cases")


def _aggregate(receipt_root: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=False)
    try:
        shard_result = os.environ.get("NAGI_REGRESSION_SHARDS_RESULT", "")
        context, _ = _current_context()
        reconcile(receipt_root, context)
        if shard_result != "success":
            raise RegressionError("selected Windows regression shard matrix result is " +
                                  (shard_result or "missing") + "; rerun both shards and aggregate together")
        print("Both Windows regression shards executed and reconciled every original target and named case.")
    except BaseException:
        with (destination / "aggregate.log").open("a", encoding="utf-8", newline="\n") as stream:
            stream.write("AGGREGATE FAILURE\n" + traceback.format_exc())
        raise


def _validate_runner_plan(plan: dict[str, Any], *, manifest_path: str | None = None,
                          live: bool = False) -> dict[str, Any]:
    required = {"schema", "protocol", "context", "shard", "requested_harness_arguments",
                "runner_command", "cwd", "artifact_root", "nagic_binary", "members"}
    if set(plan) != required or plan.get("schema") != SCHEMA or plan.get("protocol") != PROTOCOL:
        raise RegressionError("Cargo runner plan has missing or unexpected fields")
    context = plan.get("context")
    if not isinstance(context, dict) or set(context) != {"identity", "producer"}:
        raise RegressionError("Cargo runner plan has no exact source/producer context")
    identity = context["identity"]
    producer = context["producer"]
    if (not isinstance(identity, dict) or not isinstance(producer, dict) or
            set(producer) != {"workspace", "test_cwd", "package_id", "python_executable", "python_sha256"} or
            not isinstance(producer.get("workspace"), str) or
            not absolute_workspace(producer["workspace"]) or
            not isinstance(producer.get("test_cwd"), str) or
            producer["test_cwd"] != workspace_file(producer["workspace"], "compiler") or
            not isinstance(producer.get("package_id"), str) or not producer["package_id"] or
            not isinstance(producer.get("python_executable"), str) or
            not absolute_workspace(producer["python_executable"]) or
            not isinstance(producer.get("python_sha256"), str) or
            not HEX64.fullmatch(producer["python_sha256"])):
        raise RegressionError("Cargo runner plan source or workspace identity is malformed")
    workspace = producer["workspace"]
    test_cwd = producer["test_cwd"]
    shard = plan.get("shard")
    if type(shard) is not int or shard not in SHARD_SELECTORS:
        raise RegressionError("Cargo runner plan shard number is invalid")
    if (identity.get("host") != "x86_64-pc-windows-msvc" or
            plan.get("cwd") != test_cwd or
            plan.get("artifact_root") != f"build/ci-evidence/windows-regression/shard-{shard}" or
            plan.get("requested_harness_arguments") != ["--format=pretty"]):
        raise RegressionError("Cargo runner plan command, root, host or artifact policy differs")
    runner_command = plan.get("runner_command")
    expected_script = workspace_file(workspace, "scripts/ci/windows_regression_sharding.py")
    if (not isinstance(runner_command, list) or len(runner_command) != 6 or
            any(not isinstance(item, str) or not item for item in runner_command) or
            not absolute_workspace(runner_command[0]) or runner_command[1] != "-B" or
            canonical_executable(runner_command[0]) !=
            canonical_executable(producer["python_executable"]) or
            canonical_executable(runner_command[2]) != canonical_executable(expected_script) or
            runner_command[3:5] != ["--record-binary", "--plan"] or
            not absolute_workspace(runner_command[5])):
        raise RegressionError("Cargo runner command is not an absolute, shell-free Python argv array")
    if manifest_path is not None and canonical_executable(runner_command[5]) != canonical_executable(manifest_path):
        raise RegressionError("Cargo runner plan path differs from the producer's recorded plan")
    if live:
        if (canonical_executable(runner_command[0]) != canonical_executable(sys.executable) or
                not Path(runner_command[0]).is_file() or
                sha256_file(Path(runner_command[0])) != producer["python_sha256"] or
                canonical_executable(str(Path(expected_script).resolve(strict=True))) !=
                canonical_executable(expected_script)):
            raise RegressionError("Cargo runner Python/script path differs from the live checkout")
    cli = plan.get("nagic_binary")
    if (not isinstance(cli, dict) or set(cli) != {"path", "sha256"} or
            not isinstance(cli.get("path"), str) or canonical_executable(cli["path"]) != cli["path"] or
            not executable_under_target(cli["path"], workspace) or
            Path(cli["path"]).suffix.lower() != ".exe" or
            not isinstance(cli.get("sha256"), str) or not HEX64.fullmatch(cli["sha256"])):
        raise RegressionError("Cargo runner plan lacks the compiled CLI binary identity")
    if live and (not Path(cli["path"]).is_file() or sha256_file(Path(cli["path"])) != cli["sha256"]):
        raise RegressionError("Cargo runner plan CLI path or SHA256 differs from its compiled artifact")
    members = plan.get("members")
    if not isinstance(members, dict) or set(members) != set(SHARD_SELECTORS[shard]):
        raise RegressionError("Cargo runner plan target set differs from its reviewed shard")
    paths: set[str] = set()
    for selector in SHARD_SELECTORS[shard]:
        row = members.get(selector)
        if not isinstance(row, dict) or set(row) != {"executable", "sha256", "marker", "events", "started",
                                                    "finished", "event_file", "started_file", "finished_file"}:
            raise RegressionError("Cargo runner plan target entry is malformed")
        executable = row.get("executable")
        sha = row.get("sha256")
        if (not isinstance(executable, str) or canonical_executable(executable) != executable or
                not executable_under_target(executable, workspace) or
                not isinstance(sha, str) or not HEX64.fullmatch(sha)):
            raise RegressionError("Cargo runner plan executable is outside its exact target identity")
        canonical = canonical_executable(executable)
        if canonical in paths:
            raise RegressionError("Cargo runner plan maps multiple targets to one executable")
        paths.add(canonical)
        marker = digest(dict(selector=selector, executable=canonical, sha256=sha, context=context))
        event_rel = f"events/{marker}.events"
        started_rel = f"runner/{marker}.started.json"
        finished_rel = f"runner/{marker}.finished.json"
        expected = dict(executable=canonical, sha256=sha, marker=marker,
                        events=workspace_file(workspace, f"{plan['artifact_root']}/{event_rel}"),
                        started=workspace_file(workspace, f"{plan['artifact_root']}/{started_rel}"),
                        finished=workspace_file(workspace, f"{plan['artifact_root']}/{finished_rel}"),
                        event_file=event_rel, started_file=started_rel, finished_file=finished_rel)
        if row != expected:
            raise RegressionError("Cargo runner plan event path or marker is malformed")
    return context


def invoke_test_binary(executable: str, arguments: list[str], *, selector: str, marker: str,
                       sha256: str, cwd: str, events: str, started: str, finished: str,
                       expected_arguments: list[str]) -> int:
    """Portable outer runner mechanics shared by the Windows policy and protocol fixture."""
    if arguments != expected_arguments or not HEX64.fullmatch(sha256):
        raise RegressionError("outer test runner received unexpected harness arguments or binary identity")
    executable_path = Path(executable).resolve(strict=True)
    if executable_path.is_symlink() or sha256_file(executable_path) != sha256:
        raise RegressionError("outer test executable differs from its compiled identity")
    if Path.cwd().resolve() != Path(cwd).resolve():
        raise RegressionError("Cargo changed the test executable working directory")
    events_path, started_path, finished_path = (Path(path) for path in (events, started, finished))
    for path in (events_path, started_path, finished_path):
        if path.exists() or path.is_symlink():
            raise RegressionError("outer test runner evidence path already exists")
        path.parent.mkdir(parents=True, exist_ok=True)
    actual = [*arguments, "--logfile", str(events_path.resolve())]
    _write_json_exclusive(started_path, dict(schema=SCHEMA, protocol=PROTOCOL, selector=selector,
                                   executable=canonical_executable(executable), sha256=sha256,
                                   cwd=os.getcwd(), requested_arguments=arguments,
                                   actual_arguments=actual))
    print("NAGI_CI_OUTER_EXECUTION " + marker, flush=True)
    try:
        result = subprocess.run([str(executable_path), *actual], cwd=os.getcwd(),
                                env=os.environ.copy(), check=False)
    except OSError as error:
        _write_json_exclusive(finished_path, dict(schema=SCHEMA, protocol=PROTOCOL, selector=selector,
                                        executable=canonical_executable(executable), sha256=sha256,
                                        spawn_error=type(error).__name__))
        raise RegressionError("failed to start the outer test executable") from error
    if sha256_file(executable_path) != sha256:
        raise RegressionError("outer test executable changed during execution")
    _write_json_exclusive(finished_path, dict(schema=SCHEMA, protocol=PROTOCOL, selector=selector,
                                    executable=canonical_executable(executable), sha256=sha256,
                                    returncode=result.returncode))
    return result.returncode


def record_binary(arguments: list[str], manifest_name: str) -> int:
    """Cargo runner wrapper that applies Windows plan policy, then uses portable runner mechanics."""
    if not manifest_name:
        raise RegressionError("Cargo runner manifest path is missing")
    manifest_path = Path(manifest_name)
    plan = load_json(manifest_path)
    context = _validate_runner_plan(plan, manifest_path=str(manifest_path.resolve()), live=True)
    if len(arguments) != 2 or arguments[1:] != plan["requested_harness_arguments"]:
        raise RegressionError("Cargo passed unexpected test harness arguments")
    executable = canonical_executable(arguments[0])
    matches = [(selector, row) for selector, row in plan.get("members", {}).items()
               if isinstance(row, dict) and canonical_executable(row.get("executable", "")) == executable]
    if len(matches) != 1:
        raise RegressionError("unexpected or ambiguous Cargo test executable")
    selector, row = matches[0]
    if row.get("sha256") is None or not HEX64.fullmatch(str(row["sha256"])):
        raise RegressionError("runner entry lacks executable identity")
    if canonical_executable(str(Path(row.get("executable", "")).resolve(strict=True))) != executable:
        raise RegressionError("runner executable path changed")
    executable_path = Path(row["executable"]).resolve(strict=True)
    if executable_path.suffix.lower() != ".exe" or sha256_file(executable_path) != row["sha256"]:
        raise RegressionError("outer Windows test executable differs from its compiled identity")
    return invoke_test_binary(row["executable"], list(arguments[1:]), selector=selector,
                              marker=row["marker"], sha256=row["sha256"], cwd=plan["cwd"],
                              events=row["events"], started=row["started"], finished=row["finished"],
                              expected_arguments=plan["requested_harness_arguments"])


def main(argv: list[str] | None = None) -> None:
    args = list(sys.argv[1:] if argv is None else argv)
    if args and args[0] == "--record-binary":
        if len(args) < 4 or args[1] != "--plan":
            raise RegressionError("Cargo runner invocation lacks its exact execution plan")
        raise SystemExit(record_binary(args[3:], args[2]))
    parser = argparse.ArgumentParser()
    parser.add_argument("--shard", type=int, choices=range(SHARD_COUNT))
    parser.add_argument("--aggregate", action="store_true")
    parser.add_argument("--receipts", type=Path, default=Path("build/ci-evidence/windows-regression/artifacts"))
    parser.add_argument("--evidence", type=Path, default=Path("build/ci-evidence/windows-regression"))
    parsed = parser.parse_args(args)
    if parsed.shard is not None and not parsed.aggregate:
        _run_shard(parsed.shard, parsed.evidence / f"shard-{parsed.shard}")
    elif parsed.aggregate and parsed.shard is None:
        _aggregate(parsed.receipts, parsed.evidence / "aggregate")
    else:
        parser.error("choose exactly one of --shard or --aggregate")


if __name__ == "__main__":
    main()
