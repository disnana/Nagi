"""Build and run the JSON/future probe using the generated HTTP app's lock.

First build benchmarks/http_stdlib.nagi into build/http_stdlib. This probe
measures calling-thread JSON work and concrete future layouts, not network I/O.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess

from native_artifacts import comparison_files

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path,
        default=ROOT / "benchmarks/results/http-stdlib-matched/json-probe.jsonl",
    )
    parser.add_argument("--cpu", type=int, help="pin the probe to an allowed Linux CPU")
    parser.add_argument("--iterations", type=int, help="iterations per timed sample")
    args = parser.parse_args()
    if args.iterations is not None and args.iterations < 1:
        parser.error("iterations must be positive")
    if args.cpu is not None:
        if not hasattr(os, "sched_getaffinity") or args.cpu not in os.sched_getaffinity(0):
            parser.error("CPU affinity requires an allowed Linux CPU")

    generated = ROOT / "build/http_stdlib"
    project = ROOT / "build/http_json_probe"
    manifest = generated / "Cargo.toml"
    if not manifest.is_file():
        parser.error(f"missing {manifest}; build benchmarks/http_stdlib.nagi first")
    files = comparison_files(generated, "nagi-http-json-probe")
    source = (generated / "src/main.rs").read_text()
    entry = r"(?m)^fn main\(\)"
    if len(re.findall(entry, source)) != 1:
        parser.error("expected one generated executable entry")
    source = re.sub(entry, "fn generated_unused_main()", source, count=1)
    source += "\n" + (ROOT / "benchmarks/http_json_probe.rs").read_text()

    (project / "src").mkdir(parents=True, exist_ok=True)
    for filename, text in files.items():
        (project / filename).write_text(text)
    (project / "src/main.rs").write_text(source)
    # Both generated projects are siblings, so copied relative runtime paths
    # resolve identically. Reuse the native cache selected by the compiler.
    native = Path(os.environ.get("NAGI_NATIVE_TARGET_DIR", ROOT / "native-target")).resolve()
    environment = dict(os.environ, CARGO_TARGET_DIR=str(native))
    if args.iterations is not None:
        environment["JSON_PROBE_ITERATIONS"] = str(args.iterations)
    subprocess.run(
        [os.environ.get("CARGO", "cargo"), "build", "--release", "--locked", "--offline",
         "--manifest-path", str(project / "Cargo.toml")],
        env=environment, check=True,
    )
    binary = native / "release" / ("nagi-http-json-probe" + (".exe" if os.name == "nt" else ""))
    run_options = {}
    if args.cpu is not None:
        run_options["preexec_fn"] = lambda: os.sched_setaffinity(0, {args.cpu})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w") as output:
        subprocess.run([str(binary)], env=environment, stdout=output, check=True, **run_options)
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
