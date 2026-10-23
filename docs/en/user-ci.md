# GitHub Actions for a Nagi application

Use the project CI generator to create an application workflow. Python 3.10 or
later is required. Published 0.2.1 distributions include
`tools/generate_project_ci.py`; released 0.2.0 does not include this tool.
Use a 0.2.1 archive or this source checkout. Run it from the
application repository root, where `nagi.toml` is located:

```bash
# Point to the extracted 0.2.1 distribution
python /path/to/nagi/tools/generate_project_ci.py \
  --nagi-version 0.2.1 --output .github/workflows/nagi.yml
```

With a source checkout:

```bash
python /path/to/Nagi/scripts/generate_project_ci.py \
  --nagi-version 0.2.1 --output .github/workflows/nagi.yml
```

The generator uses only Python's standard library. It writes the workflow at
the requested path, refuses to replace an existing file, and creates missing
parent directories. Add `--force` only when you intend to replace that file.
The default output can also be reviewed before saving:

```bash
python /path/to/Nagi/scripts/generate_project_ci.py
```

The generator in this source checkout defaults to Nagi `0.2.1`. The generator
bundled in published 0.2.1 archives still defaults to `0.2.0`, so the examples
explicitly pin `--nagi-version 0.2.1` for both. Native builds use stable Rust.
Change these settings when generating the file:

```bash
python /path/to/Nagi/scripts/generate_project_ci.py \
  --nagi-version 0.2.1 --toolchain stable \
  --output .github/workflows/nagi.yml
```

The version must be a complete `X.Y.Z` release version. Builds use the release
profile by default. Use `--os-matrix` to build on Ubuntu,
macOS, and Windows; without it, the workflow builds on Ubuntu only.

The generator in this source checkout accepts `--build-profile dev` for
development CI. This is a next-version generator feature; the
`tools/generate_project_ci.py` bundled in published 0.2.1 archives does not
accept it. The generated workflow requires a released Nagi CLI version of
0.2.1 or later:

```bash
python /path/to/Nagi/scripts/generate_project_ci.py \
  --nagi-version 0.2.1 --build-profile dev --os-matrix \
  --output .github/workflows/nagi.yml
```

The selected profile applies to both Windows and Unix on every event in the
generated workflow. Dev retains `nagic check`, complete checks during the
build, and Cargo dependency validation. Dev and release differ in behavior,
including overflow checks; see [development builds](development-builds.md).
Keep production release builds and application tests in your validation
process. If you create a separate release workflow, give it a different
`concurrency.group` so the workflows do not cancel each other on the same event.

Explicit `--build-profile release` produces the default workflow. For Nagi
0.2.1 or later, the generator emits `nagic build --profile release|dev`; for
0.2.0, it retains the existing `nagic build` command. Selecting dev with 0.2.0
fails before any output file is changed.

The workflow runs `nagic check` in a separate job before the native build. That
check job does not install Rust or use Cargo, so type and project errors surface
without waiting for native compilation. The build job installs the pinned Nagi
release through Nagi's official installer, installs the selected Rust
toolchain, and runs the released CLI's `nagic build` command. The step summary
records the measured check or build duration and
exit code. These durations describe that runner execution; the generator does
not include sample or benchmark timings.

Nagi does not provide a universal application `test` command. Add your own
checks after the build step, such as running the generated executable with
fixtures, invoking an existing test harness, or checking generated API output.
The generated file has a comment marking this extension point. Do not label
the compiler check or build as application tests.

The build job caches Cargo's registry and Git downloads plus the directory
selected by `NAGI_NATIVE_TARGET_DIR`. The exact cache key includes Nagi, build profile,
operating system, CPU architecture, Rust toolchain, Cargo lockfiles and
manifests, and `nagi.toml`; there are no broad fallback keys. This source
checkout's generator uses profile-specific `v2` keys, so existing `v1` caches
are not restored and the first run after migration creates a new cache. Pull requests,
including forked pull requests, may restore an exact matching cache, but the
workflow saves cache entries only after a successful push to the repository's
default branch. This keeps untrusted pull-request builds from writing cache
content that trusted builds later restore. The workflow requests only
`contents: read`, disables checkout credential persistence, uses no secrets,
and does not use `pull_request_target`.

The default workflow does not install Python, `uv`, or `sccache`. If an
application-specific test hook has declared Python dependencies, add Python
setup and install from that project's existing requirements or lock file. Add
`uv` only when the project already uses it. `sccache` is an optional tuning
choice for projects whose native builds justify its setup and cache policy; it
is not required for correctness.

See the [project guide](projects.md) for `nagi.toml`, command behavior, and the
native target cache.

Push builds are restricted to the default branch (`main` by default); feature branches use PR events, avoiding duplicate builds. Set `--default-branch master` when appropriate. New runs cancel previous runs for the same PR. Adjust cancellation separately for release workflows that must process every commit. The Rust action is commit-pinned and receives the toolchain through its input; verify your chosen Rust version against dependency requirements.

Generated `build/*/Cargo.lock` files are restored and saved with the native target so fresh runners inherit the previous dependency resolution. Adjust that path for custom output locations. Generation snapshots are excluded. Caching does not replace reviewed dependency updates or lockfile management.

GitHub base/default-branch caches are readable by fork PRs. Skipping restore with a workflow `if` is not a confidentiality boundary. Native artifacts, lockfiles, and Cargo registry/Git downloads can contain source, private dependencies, or values embedded through `env!`. Cache only data that is safe to disclose to forks. Remove affected cache paths for confidential data; if nothing is shareable, omit the cache steps or use a separate access-controlled backend. Isolate untrusted PRs in temporary workspaces on self-hosted runners, away from production credentials. Check both read and write permissions for any optional shared sccache backend.
