"""Exact inverse for moving one unchanged Linux workspace command to its own runner."""
import re

WORKSPACE_STEP = "      - run: cargo test --locked\n"
ANCHOR = "      - name: Check a compiler without the SQL validation engine\n"
JOB = """  linux-workspace:
    name: Workspace regression (Linux)
    needs: [changes, ci-plan]
    if: needs.changes.result == 'success' && needs.ci-plan.result == 'success' && needs.ci-plan.outputs.comprehensive_checks == 'true'
    runs-on: ubuntu-latest
    env:
      NAGI_FAILURE_DIR: ${{ github.workspace }}/build/compiler-failures
      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # ci-pin:v4
        with:
          persist-credentials: false # ci-isolation
      - uses: dtolnay/rust-toolchain@b725eda525362e7cf34af9a7243cac1e3f01a7ec # ci-pin:stable
        id: rust
      - name: Cache Linux workspace regression builds
        uses: actions/cache/restore@0057852bfaa89a56745cba8c7296529d2fc39830 # ci-pin:v4
        id: nagi_rust_cache
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            native-target
          key: nagi-rust-v1-linux-workspace-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**') }}
          restore-keys: |
            nagi-rust-v1-linux-workspace-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-
            nagi-rust-v1-checks-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-
            nagi-rust-v1-checks-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-
      - uses: actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065 # ci-pin:v5
        with:
          python-version: '3.12'
      - run: cargo test --locked
      - name: Preserve workspace compiler and application failure evidence
        if: failure()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: compiler-failures-linux-workspace
          path: build/compiler-failures/
          if-no-files-found: ignore

      - name: Save validated Rust cache (main only) # ci-cache-save
        if: success() && github.event_name == 'push' && github.ref == 'refs/heads/main' && steps.nagi_rust_cache.outputs.cache-hit != 'true' && steps.nagi_rust_cache.outputs.cache-primary-key != ''
        uses: actions/cache/save@0057852bfaa89a56745cba8c7296529d2fc39830 # ci-pin:v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            native-target
          key: ${{ steps.nagi_rust_cache.outputs.cache-primary-key }}
"""
READY_OLD = "    needs: [changes, ci-plan, critical, linux, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression]\n"
PUBLISH_OLD = "    needs: [changes, ci-plan, critical, linux, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package, nagi-regression, ready]\n"
READY_NEW = READY_OLD.replace("linux,", "linux, linux-workspace,", 1)
PUBLISH_NEW = PUBLISH_OLD.replace("linux,", "linux, linux-workspace,", 1)


def inverse_workflow(source):
    if (len(re.findall(r"(?m)^  linux-workspace:$", source)) != 1
            or source.count(JOB + "  rust-1-88:\n") != 1
            or source.count(WORKSPACE_STEP) != 1):
        raise ValueError("Linux workspace job registration mismatch")
    source = source.replace(JOB, "", 1)
    linux = re.search(r"(?ms)^  linux:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)", source)
    if linux is None or linux.group().count(ANCHOR) != 1:
        raise ValueError("Linux workspace original placement mismatch")
    restored = linux.group().replace(ANCHOR, WORKSPACE_STEP + ANCHOR, 1)
    source = source[:linux.start()] + restored + source[linux.end():]
    for new, old in ((READY_NEW, READY_OLD), (PUBLISH_NEW, PUBLISH_OLD)):
        if source.count(new) != 1:
            raise ValueError("Linux workspace aggregate dependency mismatch")
        source = source.replace(new, old, 1)
    return source
