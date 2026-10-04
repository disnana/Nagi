"""Find a built application's executable from its generated Cargo manifest."""
from __future__ import annotations

import os
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def native_executable(generated: Path, target: Path | None = None, *,
                      fallback_name: str | None = None) -> Path:
    manifest = generated / "Cargo.toml"
    if manifest.is_file():
        with manifest.open("rb") as source:
            name = tomllib.load(source)["package"]["name"]
    elif fallback_name is not None:
        name = fallback_name
    else:
        raise FileNotFoundError(f"Build the application first: {manifest}")
    if target is None:
        target = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target"))
    return target / "release" / (name + (".exe" if os.name == "nt" else ""))
