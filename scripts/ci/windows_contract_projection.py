"""Exact inverse for the six archive-independent Windows contract steps."""
import re

SEND_STEP = r"""      - name: Check native buffered send deadline and idle Keep-Alive early on Windows
        if: runner.os == 'Windows'
        shell: bash
        run: |
          set -euo pipefail
          python - <<'PYSOURCE'
          import hashlib, pathlib
          expected = {
              'runtime/src/http_server.rs': 'e4af47720616a8f6818a47d2cb35408004cf35f3be1f4e6f71567e6318efca31',
              'runtime/src/http_server/tests.rs': '25ebad5039d20ec7109e40b44ac87a85a72bc4bd8c9c85f78fd97164c5b29de8',
              'runtime/src/http_server/send_deadline_tests.rs': '2abec513bff123840274641e6a84e4dd29989fafd686af2f10fc46fab42893ea',
              'runtime/src/http_server/windows_native_fixture.rs': '74701a0501fb4a080defc0fe299276a8c949881c210b2576881880c75cbbb282',
              'runtime/src/http_server/transport_lifecycle_tests.rs': 'bb45b069b1bd8584ad19c80ce29b622759df4e2a91e28848e330ee4fba86581b',
          }
          for name, digest in expected.items():
              source = pathlib.Path(name).read_bytes()
              if b"\r" in source.replace(b"\r\n", b"\n"):
                  raise AssertionError("unexpected lone CR in native fixture source")
              actual = hashlib.sha256(source.replace(b"\r\n", b"\n")).hexdigest()
              if actual != digest:
                  raise AssertionError("reviewed native send fixture source differs: " + name)
              print("native send fixture sha256=" + actual + " file=" + name)
          PYSOURCE
          cargo test --locked -p nagi-runtime --lib http_server::tests::send_deadline_tests:: 2>&1 | tee "$RUNNER_TEMP/http-send-deadline.log"
          python - <<'PYCODE'
          import os, pathlib, re
          text=(pathlib.Path(os.environ["RUNNER_TEMP"])/"http-send-deadline.log").read_text(encoding="utf-8")
          required={
              "header_flush_before_buffered_frame_keeps_original_send_end",
              "empty_response_completion_does_not_expire_idle_keep_alive",
              "buffered_response_completion_does_not_expire_idle_keep_alive",
          }
          summaries=re.findall(r"(?m)^test result: ok\. 3 passed; 0 failed; 0 ignored;.*$",text)
          if len(summaries)!=1:
              raise AssertionError("three actual native send-deadline cases required")
          for name in required:
              pattern=rf"(?m)^test http_server::tests::send_deadline_tests::{re.escape(name)} \.\.\. ok$"
              if len(re.findall(pattern,text))!=1:
                  raise AssertionError("native send-deadline case missing, duplicate or failed")
          PYCODE
"""
PRIVATE_STEP = r"""      - name: Diagnose private Session phase deadlines on Windows
        if: always() && runner.os == 'Windows'
        timeout-minutes: 30
        shell: bash
        run: |
          set +e
          if [ "${RUNNER_OS:-}" = "Windows" ]; then
            if [ -z "${RUNNER_TEMP:-}" ] || [ ! -d "${RUNNER_TEMP:-}" ]; then
              echo "RUNNER_TEMP must be a nonempty existing directory on Windows" >&2
              exit 2
            fi
            export TMP="$RUNNER_TEMP" TEMP="$RUNNER_TEMP"
          fi
          python -B scripts/tests/test_session_private_deadline_diagnostic.py
          fake_rc=$?
          python -B scripts/ci/session_private_deadline_diagnostic.py --run
          native_rc=$?
          if [ "$fake_rc" -ne 0 ] || [ "$native_rc" -ne 0 ]; then exit 1; fi
"""
OBSERVER_STEP = r"""      - name: Check bounded SF01 diagnostic observer
        env:
          RUSTFLAGS: --cfg nagi_sf01_diagnostic --cfg nagi_sf01_diag_oracle --check-cfg=cfg(nagi_sf01_diagnostic) --check-cfg=cfg(nagi_sf01_diag_oracle)
        run: cargo test --locked -p nagic --test security_sf01 sf01_diagnostic_observer_oracle -- --exact
"""
POLICY_STEP = r"""      - name: Check SF01 High, saved Low, handwritten Low and native policy contracts
        env:
          RUSTFLAGS: --cfg nagi_sf01_diagnostic --check-cfg=cfg(nagi_sf01_diagnostic) --check-cfg=cfg(nagi_sf01_diag_oracle)
        run: cargo test --locked -p nagic --test security_sf01 --test security_sf01_native
"""
LEASE_STEP = r"""      - name: Check SF01 request lease and permit lifecycle through the public runtime
        shell: bash
        run: |
          set -o pipefail
          cargo test --locked -p nagi-runtime --test security_sf01
          cargo test --locked -p nagi-runtime --lib auth::tests:: 2>&1 | tee "$RUNNER_TEMP/sf01-auth.log"
          cargo test --locked -p nagi-runtime --lib http_server::tests::security_tests:: 2>&1 | tee "$RUNNER_TEMP/sf01-http.log"
          python -c 'import pathlib,re,os; root=pathlib.Path(os.environ["RUNNER_TEMP"]); counts={"sf01-auth.log":9,"sf01-http.log":11}; [(None if re.search(r"test result: ok\. %d passed; 0 failed; 0 ignored;"%n,(root/f).read_text()) else (_ for _ in ()).throw(AssertionError(f))) for f,n in counts.items()]'
"""
HOST_STEP = r"""      - name: Check Host authority and HTTP entrypoint contracts on the target platform
        shell: bash
        run: |
          set -o pipefail
          cargo test --locked -p nagi-runtime --lib http_server::tests::authority_tests:: 2>&1 | tee "$RUNNER_TEMP/host-authority.log"
          cargo test --locked -p nagic --test http_entrypoint 2>&1 | tee "$RUNNER_TEMP/host-http-entrypoint.log"
          python - <<'PY'
          import os
          import pathlib
          import re
          root = pathlib.Path(os.environ["RUNNER_TEMP"])
          expected = {"host-authority.log": 5, "host-http-entrypoint.log": 4}
          for name, count in expected.items():
              text = (root / name).read_text(encoding="utf-8")
              pattern = rf"test result: ok\. {count} passed; 0 failed; 0 ignored;"
              if not re.search(pattern, text):
                  raise AssertionError(f"{name}: expected {count} passed, 0 failed, 0 ignored")
          PY
"""
HEADER = r"""  windows-contracts:
    name: Compiler and runtime contracts (Windows)
    needs: [changes, release-plan, ci-plan]
    if: >-
      always() && !cancelled() && needs.changes.result == 'success' && needs.release-plan.result == 'success' &&
      needs.ci-plan.result == 'success' && needs.ci-plan.outputs.package_checks == 'true'
    runs-on: windows-latest
    env:
      NAGI_FAILURE_DIR: ${{ github.workspace }}/build/compiler-failures
      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # ci-pin:v4
        with:
          persist-credentials: false # ci-isolation
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
      - uses: dtolnay/rust-toolchain@b725eda525362e7cf34af9a7243cac1e3f01a7ec # ci-pin:stable
        id: rust
      - name: Cache Windows contract builds
        uses: actions/cache/restore@0057852bfaa89a56745cba8c7296529d2fc39830 # ci-pin:v4
        id: nagi_rust_cache
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            native-target
          key: nagi-rust-v1-windows-contracts-windows-x86_64-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**') }}
          restore-keys: |
            nagi-rust-v1-windows-contracts-windows-x86_64-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-
            nagi-rust-v1-package-windows-x86_64-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-
            nagi-rust-v1-package-windows-x86_64-${{ steps.rust.outputs.cachekey }}-
      - uses: actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065 # ci-pin:v5
        with:
          python-version: '3.12'
      - name: Verify the compiler host architecture
        env:
          RELEASE_RUST_HOST: x86_64-pc-windows-msvc
        shell: bash
        run: |
          python - <<'PYHOST'
          import os, subprocess
          lines = subprocess.check_output(['rustc', '-vV'], text=True).splitlines()
          host = next(line.removeprefix('host: ') for line in lines if line.startswith('host: '))
          assert host == os.environ['RELEASE_RUST_HOST'], host
          PYHOST
      - name: Fetch locked runtime dependencies before offline tests
        env:
          RELEASE_RUST_HOST: x86_64-pc-windows-msvc
        shell: bash
        run: cargo fetch --locked --target "$RELEASE_RUST_HOST"
"""
FOOTER = r"""      - name: Preserve Windows contract failure evidence
        if: failure()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: compiler-failures-windows-contracts
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
JOB = HEADER + SEND_STEP + PRIVATE_STEP + OBSERVER_STEP + POLICY_STEP + LEASE_STEP + HOST_STEP + FOOTER

COMMON_STEPS = (OBSERVER_STEP, POLICY_STEP, LEASE_STEP, HOST_STEP)
READY_OLD = '    needs: [changes, ci-plan, critical, linux, linux-workspace, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression]\n'
READY_NEW = '    needs: [changes, ci-plan, critical, linux, linux-workspace, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression, windows-contracts]\n'
PUBLISH_OLD = '    needs: [changes, ci-plan, critical, linux, linux-workspace, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package, nagi-regression, ready]\n'
PUBLISH_NEW = '    needs: [changes, ci-plan, critical, linux, linux-workspace, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package, nagi-regression, windows-contracts, ready]\n'


def guarded(step):
    return step.replace("\n", "\n        if: runner.os != 'Windows'\n", 1)


def inverse_workflow(source):
    if (len(re.findall(r"(?m)^  windows-contracts:$", source)) != 1
            or source.count(JOB + "  nagi-regression:\n") != 1):
        raise ValueError("Windows contract job registration mismatch")
    source = source.replace(JOB, "", 1)
    package = re.search(r"(?ms)^  nagi-package:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)", source)
    if package is None:
        raise ValueError("Windows contract original package missing")
    restored = package.group()
    for step in COMMON_STEPS:
        current = guarded(step)
        if source.count(current) != 1 or restored.count(current) != 1 or step in source:
            raise ValueError("Windows contract platform coverage mismatch")
        restored = restored.replace(current, step, 1)
    for step, anchor in ((SEND_STEP, "      - name: Check stalled send permit release and graceful join early on Windows\n"),
                         (PRIVATE_STEP, "      - name: Capture bounded Session auth stages before broad checks\n")):
        if step in source or restored.count(anchor) != 1:
            raise ValueError("Windows contract original placement mismatch")
        restored = restored.replace(anchor, step + anchor, 1)
    source = source[:package.start()] + restored + source[package.end():]
    for new, old in ((READY_NEW, READY_OLD), (PUBLISH_NEW, PUBLISH_OLD)):
        if source.count(new) != 1:
            raise ValueError("Windows contract aggregate dependency mismatch")
        source = source.replace(new, old, 1)
    return source
