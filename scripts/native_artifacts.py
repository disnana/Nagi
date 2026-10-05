"""Select a successful generation; use the fixed Cargo cache for legacy outputs."""
from __future__ import annotations

import json
import os
from pathlib import Path, PureWindowsPath
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def native_executable(generated: Path, target: Path | None = None, *,
                      fallback_name: str | None = None) -> Path:
    manifest = generated / "Cargo.toml"
    if manifest.is_file():
        with manifest.open("rb") as source:
            name = tomllib.load(source)["package"]["name"]
    elif fallback_name is not None and not (generated / ".nagi/apps").exists():
        name = fallback_name
    else:
        raise FileNotFoundError(f"Build the application first: {manifest}")
    app = generated / ".nagi/apps" / name
    if app.exists():
        # A namespace without latest is a failed first Phase2 build, never a
        # reason to execute an unrelated fixed-name binary in a shared cache.
        with (app / "latest.json").open(encoding="utf-8") as source:
            latest = json.load(source)
        if (not isinstance(latest, dict)
                or type(latest.get("schema_version")) is not int
                or latest["schema_version"] != 1
                or latest.get("app_id") != name):
            raise ValueError(f"Invalid successful generation metadata: {app}")
        generation = latest.get("generation")
        relative = latest.get("executable")
        if not isinstance(generation, str) or not isinstance(relative, str):
            raise ValueError(f"Incomplete successful generation metadata: {app}")
        if generation in ("", ".", "..") or Path(generation).name != generation or PureWindowsPath(generation).name != generation:
            raise ValueError(f"Invalid generation component: {generation}")
        if Path(relative).is_absolute() or PureWindowsPath(relative).drive or PureWindowsPath(relative).root:
            raise ValueError(f"Generation executable must be relative: {relative}")
        base = (app / "generations").resolve()
        namespace = (base / generation).resolve()
        executable = (app / relative).resolve()
        if not base.is_relative_to(app.resolve()) or not namespace.is_relative_to(base) or not executable.is_relative_to(namespace):
            raise ValueError(f"Generation executable escapes its namespace: {relative}")
        if not executable.is_file():
            raise FileNotFoundError(f"Published generation executable is missing: {executable}")
        return executable
    # Pre-Phase2 build folders have no per-app namespace. Preserve the explicit
    # legacy fallback for existing benchmark/distribution workflows.
    if target is None:
        target = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target"))
    return target / "release" / (name + (".exe" if os.name == "nt" else ""))


def comparison_files(generated: Path, name: str) -> dict[str, str]:
    """Copy a generated project's dependency/profile contract to a probe.

    A probe has one explicit stable bin matching its renamed root package.
    Relative runtime references remain valid only for sibling output projects.
    """
    manifest = (generated / "Cargo.toml").read_text(encoding="utf-8")
    parsed = tomllib.loads(manifest)
    package = parsed["package"]["name"]
    original = "name = " + json.dumps(package, ensure_ascii=False)
    replacement = "name = " + json.dumps(name, ensure_ascii=False)
    files = {}
    for filename in ("Cargo.toml", "Cargo.lock"):
        text = (generated / filename).read_text(encoding="utf-8")
        if text.count(original) != 1:
            raise ValueError(f"Unexpected generated root package in {filename}")
        text = text.replace(original, replacement, 1)
        if filename == "Cargo.toml" and "bin" in parsed:
            bins = parsed["bin"]
            if len(bins) != 1:
                raise ValueError("Expected one generated executable bin")
            bin_name = "name = " + json.dumps(bins[0]["name"], ensure_ascii=False)
            if bins[0]["name"] != package:
                if text.count(bin_name) != 1:
                    raise ValueError("Unexpected generated executable bin")
                text = text.replace(bin_name, replacement, 1)
        files[filename] = text
    return files
