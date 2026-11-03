"""Exact outer inverse of the archive-independent platform contract relocation.

Base is main 2cb7c33aff2ccc3763fa80e6d0f07240ad09570f. Original command
bodies and old Windows wiring below are immutable evidence, not new expectations.
"""
import re

BASE_COMMIT = '2cb7c33aff2ccc3763fa80e6d0f07240ad09570f'
BASE_WORKFLOW_SHA256 = '127de03b03d887ea04265454409568f39d3ab8a754c1a511db293bd0388b62f0'

OLD_WINDOWS = r"""  windows-contracts:
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
          key: nagi-rust-v1-package-windows-x86_64-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**') }}
          restore-keys: |
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
      - name: Check native buffered send deadline and idle Keep-Alive early on Windows
        if: runner.os == 'Windows'
        shell: bash
        run: |
          set -euo pipefail
          python - <<'PYSOURCE'
          import hashlib, pathlib
          expected = {
              'runtime/src/http_server.rs': '397652b0f412e012c207c39288e2bda8771c486ea24f6d1f3e76da69e9204d54',
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
      - name: Diagnose private Session phase deadlines on Windows
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
      - name: Check bounded SF01 diagnostic observer
        env:
          RUSTFLAGS: --cfg nagi_sf01_diagnostic --cfg nagi_sf01_diag_oracle --check-cfg=cfg(nagi_sf01_diagnostic) --check-cfg=cfg(nagi_sf01_diag_oracle)
        run: cargo test --locked -p nagic --test security_sf01 sf01_diagnostic_observer_oracle -- --exact
      - name: Check SF01 High, saved Low, handwritten Low and native policy contracts
        env:
          RUSTFLAGS: --cfg nagi_sf01_diagnostic --check-cfg=cfg(nagi_sf01_diagnostic) --check-cfg=cfg(nagi_sf01_diag_oracle)
        run: cargo test --locked -p nagic --test security_sf01 --test security_sf01_native
      - name: Check SF01 request lease and permit lifecycle through the public runtime
        shell: bash
        run: |
          set -o pipefail
          cargo test --locked -p nagi-runtime --test security_sf01
          cargo test --locked -p nagi-runtime --lib auth::tests:: 2>&1 | tee "$RUNNER_TEMP/sf01-auth.log"
          cargo test --locked -p nagi-runtime --lib http_server::tests::security_tests:: 2>&1 | tee "$RUNNER_TEMP/sf01-http.log"
          python -c 'import pathlib,re,os; root=pathlib.Path(os.environ["RUNNER_TEMP"]); counts={"sf01-auth.log":9,"sf01-http.log":11}; [(None if re.search(r"test result: ok\. %d passed; 0 failed; 0 ignored;"%n,(root/f).read_text()) else (_ for _ in ()).throw(AssertionError(f))) for f,n in counts.items()]'
      - name: Check Host authority and HTTP entrypoint contracts on the target platform
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
      - name: Preserve Windows contract failure evidence
        if: failure()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: compiler-failures-windows-contracts
          path: build/compiler-failures/
          if-no-files-found: ignore

"""

MOVES = (
    (r"""      - name: Check SF06 signature diagnostic fake cases
        run: python -m unittest discover -s tests -p 'test_sf06_http_client_signature_diagnostic.py'
""", '      - uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # ci-pin:v4\n'),
    (r"""      - name: Check stalled send permit release and graceful join early on Windows
        if: runner.os == 'Windows'
        shell: bash
        run: |
          set -euo pipefail
          cargo test --locked -p nagi-runtime --lib stalled_response_send_ 2>&1 | tee "$RUNNER_TEMP/http-stalled-send.log"
          python - <<'PYCODE'
          import os, pathlib, re
          text=(pathlib.Path(os.environ["RUNNER_TEMP"])/"http-stalled-send.log").read_text(encoding="utf-8")
          required={
              "stalled_response_send_expires_and_releases_connection_permit",
              "graceful_shutdown_preserves_stalled_response_send_deadline",
          }
          summaries=re.findall(r"(?m)^test result: ok\. 2 passed; 0 failed; 0 ignored;.*$",text)
          if len(summaries)!=1:
              raise AssertionError("two actual stalled-send lifecycle cases required")
          for name in required:
              pattern=rf"(?m)^test http_server::tests::{re.escape(name)} \.\.\. ok$"
              if len(re.findall(pattern,text))!=1:
                  raise AssertionError("stalled-send case missing, duplicate or failed")
          PYCODE
""", '      - name: Check call_member_ recovery first with exact result on the target platform\n'),
    (r"""      - name: Check SF06 public HTTP client across package targets
        id: sf06_public_client
        env:
          CARGO_TARGET_DIR: ${{ github.workspace }}/native-target
          CARGO_PROFILE_DEV_DEBUG: '0'
          CARGO_PROFILE_TEST_DEBUG: '0'
          CARGO_INCREMENTAL: '0'
          NAGI_NATIVE_TARGET_DIR: ${{ github.workspace }}/native-target
          NAGI_TEST_ARTIFACT_DIR: ${{ runner.temp }}/sf06-http-client-native
        shell: bash
        run: |
          set -euo pipefail
          unset ICU4X_DATA_DIR
          cat > "$RUNNER_TEMP/sf06-http-client-guard.py" <<'PY'
          import json
          import os
          import pathlib
          import re
          import sys

          RUNTIME_CASES = {
              "http_client::ledger::operation::tests::operation_withholds_real_https_response_until_actual_driver_join",
              "http_client::ledger::tracked_dns::hickory::tests::owned_udp_two_families_actual_tracked_resolver_and_join",
              "http_client::ledger::admission::tests::drop_posts_stop_without_owner_or_waiter_lock",
              'http_client::address::socket_pin::tls_http1::tests::equal_numeric_service_identity_does_not_union_trust_stores',
              'http_client::address::socket_pin::tls_http1::tests::native_io_drops_stream_before_the_final_trust_claim',
              'http_client::address::socket_pin::tls_http1::tests::unpolled_driver_drop_and_abort_actual_join_release_native_trust',
              'http_client::ledger::operation::startup::retention::tests::all_three_exact_boundaries_aggregate_rollback_and_last_owner',
              'http_client::ledger::operation::startup::retention::tests::bearer_is_sensitive_budgeted_and_bound_to_catalog_service_and_operation',
              'http_client::ledger::operation::startup::retention::tests::explicit_positive_limits_and_checked_arithmetic_without_large_allocations',
              'http_client::ledger::operation::startup::retention::tests::native_header_clone_keeps_measured_capacity_and_claim_after_descriptor_drop',
              'http_client::ledger::operation::startup::retention::tests::normalized_der_keeps_order_and_some_empty_but_discards_hidden_capacity',
              'http_client::ledger::operation::startup::retention::tests::oversized_header_and_string_capacity_are_rejected_and_invalid_payloads_refund',
              'http_client::ledger::operation::startup::retention::tests::parent_no_runtime_refusal_and_cross_catalog_registration_refund_all_partial_config',
              'http_client::ledger::operation::startup::retention::tests::partial_allocation_failures_refund_after_accepted_partial_data_is_dropped',
              'http_client::ledger::operation::startup::retention::tests::root_byte_preflight_fails_before_der_copy_and_catalog_drop_closes_authority',
              'http_client::ledger::operation::startup::retention::tests::same_service_operations_share_one_trust_and_duplicate_registration_refunds',
              'http_client::ledger::operation::startup::retention::tests::unregistered_header_aggregate_uses_actual_capacity_and_refunds_rejected_constructor',
              'http_client::ledger::operation::startup::retention::tests::unregistered_services_count_individually_before_root_copy_and_refund_on_drop',
          }
          SUITES = {
              "runtime-http-client": (222, None, RUNTIME_CASES),
              "production-dns": (1, {
                  "production_public_handle_rejects_loopback_before_tcp_or_http",
              }, None),
              "registry": (3, {
                  "adopted_client_resource_inventory_has_owned_nonclone_and_distinct_operations",
                  "adopted_client_passing_and_borrow_inventory",
                  "adopted_client_failure_constants_keep_business_status_separate",
              }, None),
              "contracts": (11, {
                  "owned_response_body_views_and_transfer_check_in_three_forms",
                  "actual_grant_protected_operation_checks_in_three_forms",
                  "explicit_public_operation_checks_without_grant_in_three_forms",
                  "response_move_while_body_view_lives_rejects_at_checker_in_both_forms",
                  "owned_response_wrapper_copy_is_rejected_but_shared_handle_copy_remains_valid",
                  "protected_permission_mismatch_and_public_downgrade_are_checker_errors",
                  "protocol_view_and_non_nominal_permission_are_rejected_at_declaration",
                  "returned_header_views_do_not_borrow_lookup_name_in_three_forms",
                  "security_kind_cross_module_metadata_survives_saved_low_without_auth_alias",
                  "actual_future_grant_capture_cannot_be_delegated_to_spawn",
                  "only_context_payload_is_shared_and_permission_is_nominal",
              }, None),
              "native": (2, {
                  "public_signatures_and_failure_projections_build_and_run_in_three_forms",
                  "generated_operations_execute_real_tls_and_grant_in_three_test_network_paths",
              }, None),
          }
          ROW = re.compile(r"^test ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED|ignored)$", re.M)
          RUNNING = re.compile(r"^running ([0-9]+) tests?$", re.M)
          SUMMARY = re.compile(
              r"^test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; "
              r"0 measured; [0-9]+ filtered out; finished in [^\n]+$",
              re.M,
          )
          RESULT = re.compile(
              r"^test result: (ok|FAILED)\. ([0-9]+) passed; ([0-9]+) failed; "
              r"([0-9]+) ignored; [0-9]+ measured; [0-9]+ filtered out; finished in [^\n]+$",
              re.M,
          )
          NATIVE_FORM_CASE = re.compile(
              r"(?m)^test (generated_operation_oracle::real_tls_and_grant) \.\.\. (ok|FAILED|ignored)$"
          )

          DIAGNOSTIC_RAW_LIMIT = 256 * 1024
          DIAGNOSTIC_COUNT_LIMIT = 1023

          def bounded_diagnostic_text(path):
              try:
                  with path.open("rb") as stream:
                      raw = stream.read(DIAGNOSTIC_RAW_LIMIT + 1)
                  if len(raw) > DIAGNOSTIC_RAW_LIMIT:
                      return "", "oversize"
                  return raw.decode("utf-8"), "ok"
              except (OSError, UnicodeError):
                  return "", "unreadable"

          def marker_in_file(path, marker):
              text, _ = bounded_diagnostic_text(path)
              return marker in text

          def record_native_diagnostic(status, text):
              try:
                  root = pathlib.Path(os.environ["NAGI_TEST_ARTIFACT_DIR"])
                  root.mkdir(parents=True, exist_ok=True)
              except (KeyError, OSError, ValueError):
                  root = None
              running = RUNNING.findall(text)
              results = RESULT.findall(text)
              rows = ROW.findall(text)
              expected = SUITES["native"][1]
              case_status = {}
              for case in sorted(expected):
                  seen = [result for name, result in rows if name == case]
                  case_status[case] = seen[0] if len(seen) == 1 else ("duplicate" if seen else "not_observed")
              error_codes = sorted(set("E" + code for code in re.findall(r"\berror\[E([0-9]{4})\]", text)))
              forms = {}
              for form in ("high", "saved-low", "manual-low"):
                  stem = f"http-client-{form}"
                  files = ({
                      "generated_present": root / f"{stem}-generated.rs",
                      "wrapper_present": root / f"{stem}-native-wrapper.rs",
                      "build_stdout_present": root / f"{stem}-build.stdout",
                      "build_stderr_present": root / f"{stem}-build.stderr",
                      "native_stdout_present": root / f"{stem}-native.stdout",
                      "native_stderr_present": root / f"{stem}-native.stderr",
                  } if root else {})
                  present = {key: path.is_file() for key, path in files.items()} if root else {
                      "generated_present": False, "wrapper_present": False,
                      "build_stdout_present": False, "build_stderr_present": False,
                      "native_stdout_present": False, "native_stderr_present": False,
                  }
                  native_text, read_state = bounded_diagnostic_text(root / f"{stem}-native.stdout") if root else ("", "unreadable")
                  native_rows = NATIVE_FORM_CASE.findall(native_text)
                  form_status = (native_rows[0][1] if len(native_rows) == 1 else
                                 ("duplicate" if native_rows else "not_observed")) if read_state == "ok" else read_state
                  timeout_states = {}
                  for stage in ("build", "native"):
                      marker = f"{form}-{stage}-timeout"
                      observed = marker in text
                      if root:
                          observed = observed or any(
                              marker_in_file(root / f"{stem}-{stage}.{stream}", marker)
                              for stream in ("stdout", "stderr")
                          )
                      timeout_states[f"{stage}_timeout"] = "observed" if observed else "not_observed"
                  forms[form] = {**present, "native_test_status": form_status, **timeout_states}
              result = results[0] if len(results) == 1 else None
              record = {
                  "schema": "sf06-http-client-native-diagnostic-v1",
                  "cargo_exit": status if 0 <= status <= 255 else None,
                  "running_count": min(DIAGNOSTIC_COUNT_LIMIT, int(running[0])) if len(running) == 1 else None,
                  "result_kind": result[0] if result else ("multiple" if results else "missing"),
                  "passed_count": min(DIAGNOSTIC_COUNT_LIMIT, int(result[1])) if result else None,
                  "failed_count": min(DIAGNOSTIC_COUNT_LIMIT, int(result[2])) if result else None,
                  "ignored_count": min(DIAGNOSTIC_COUNT_LIMIT, int(result[3])) if result else None,
                  "rust_error_codes": error_codes[:16],
                  "rust_error_code_loss": len(error_codes) > 16,
                  "known_native_cases": case_status,
                  "unknown_test_row_count": min(DIAGNOSTIC_COUNT_LIMIT, sum(name not in expected for name, _ in rows)),
                  "forms": forms,
              }
              saved = False
              try:
                  if root:
                      temporary = root / "sf06-native-diagnostic.json.tmp"
                      temporary.write_text(json.dumps(record, sort_keys=True) + "\n", encoding="utf-8")
                      temporary.replace(root / "sf06-native-diagnostic.json")
                      saved = True
              except OSError:
                  pass
              forms_complete = sum(all(values[key] for key in (
                  "generated_present", "wrapper_present", "build_stdout_present",
                  "build_stderr_present", "native_stdout_present", "native_stderr_present",
              )) for values in forms.values())
              timeout_markers = [
                  f"{form}-{stage}-timeout"
                  for form, values in forms.items()
                  for stage in ("build", "native")
                  if values[f"{stage}_timeout"] == "observed"
              ]
              if status != 0 or timeout_markers or not (running == ["2"] and len(results) == 1 and
                                     result and result[0] == "ok" and result[2:] == ("0", "0")):
                  print(
                      f"sf06_http_client_native_diag=v1 cargo_exit={record['cargo_exit']} "
                      f"result={record['result_kind']} running={record['running_count']} "
                      f"passed={record['passed_count']} failed={record['failed_count']} "
                      f"ignored={record['ignored_count']} rust_error_codes={','.join(error_codes[:16]) or 'none'} "
                      f"timeout_markers={','.join(timeout_markers) or 'none'} "
                      f"forms_complete={forms_complete}/3 artifact_saved={'true' if saved else 'false'}"
                  )
                  for case, outcome in case_status.items():
                      if outcome != "ok":
                          print(f"sf06_http_client_native_case={case} status={outcome}")
              return record

          def fail(label):
              print(f"sf06_http_client_check={label} result=failed")
              raise SystemExit(1)

          def check_suite(label, status, log_path):
              suite = SUITES.get(label)
              if suite is None:
                  fail("unknown")
              count, exact_names, required_names = suite
              if status != 0 and label != "native":
                  fail(label)
              diagnostic_read_state = "ok"
              if label == "native":
                  text, diagnostic_read_state = bounded_diagnostic_text(pathlib.Path(log_path))
              else:
                  try:
                      text = pathlib.Path(log_path).read_text(encoding="utf-8")
                  except (OSError, UnicodeError):
                      fail(label)
              if label == "native":
                  record_native_diagnostic(status, text)
                  if diagnostic_read_state != "ok":
                      print(f"sf06_http_client_native_raw={diagnostic_read_state}")
                      fail(label)
              if status != 0:
                  fail(label)
              rows = ROW.findall(text)
              names = [name for name, result in rows]
              passed = {name for name, result in rows if result == "ok"}
              summaries = SUMMARY.findall(text)
              running = RUNNING.findall(text)
              if (running != [str(count)] or summaries != [str(count)] or
                      len(rows) != count or len(set(names)) != count or
                      len(passed) != count or
                      (exact_names is not None and passed != exact_names) or
                      (required_names is not None and
                       not required_names.issubset(passed))):
                  fail(label)
              print(f"sf06_http_client_check={label} tests={count} result=ok")

          def check_native_forms():
              root = pathlib.Path(os.environ["NAGI_TEST_ARTIFACT_DIR"])
              forms = ("high", "saved-low", "manual-low")
              for form in forms:
                  stem = f"http-client-{form}"
                  generated = root / f"{stem}-generated.rs"
                  wrapper = root / f"{stem}-native-wrapper.rs"
                  if (not generated.is_file() or not generated.stat().st_size or
                          not wrapper.is_file() or not wrapper.stat().st_size):
                      fail("native-forms")
                  for stage in ("build", "native"):
                      for stream in ("stdout", "stderr"):
                          if not (root / f"{stem}-{stage}.{stream}").is_file():
                              fail("native-forms")
                  try:
                      output = (root / f"{stem}-native.stdout").read_text(encoding="utf-8")
                  except (OSError, UnicodeError):
                      fail("native-forms")
                  rows = re.findall(
                      r"(?m)^test (generated_operation_oracle::real_tls_and_grant) \.\.\. (ok|FAILED)$",
                      output,
                  )
                  summaries = SUMMARY.findall(output)
                  if rows != [("generated_operation_oracle::real_tls_and_grant", "ok")] or summaries != ["1"]:
                      fail("native-forms")
              print("sf06_http_client_native_forms=3 result=ok")

          if len(sys.argv) != 4:
              fail("arguments")
          label, status_text, path = sys.argv[1:]
          if label == "native-forms":
              if status_text != "0":
                  fail("native-forms")
              check_native_forms()
          else:
              try:
                  status = int(status_text)
              except ValueError:
                  fail("arguments")
              check_suite(label, status, path)
          PY
          run_check() {
            local label="$1"
            shift
            local log="$RUNNER_TEMP/sf06-${label}.log"
            local status=0
            (unset GITHUB_OUTPUT; "$@") > "$log" 2>&1 || status=$?
            local guard_status=0
            (unset GITHUB_OUTPUT; python "$RUNNER_TEMP/sf06-http-client-guard.py" "$label" "$status" "$log") || guard_status=$?
            if [ "$guard_status" -ne 0 ]; then
              python -B scripts/ci/sf06_http_client_failure_diagnostic.py "$label" "$status" "$guard_status" "$log" "$RUNNER_TEMP" 2>/dev/null || printf '%s\n' 'sf06_http_client_failure_diag=v1 status=unavailable reason=invocation_error artifact_saved=false'
            fi
            return "$guard_status"
          }
          run_check runtime-http-client cargo test --locked -p nagi-runtime --lib http_client::
          run_check production-dns cargo test --locked -p nagi-runtime --test http_client_operation_public_api production_public_handle_rejects_loopback_before_tcp_or_http -- --exact
          run_check registry cargo test --locked -p nagic --test http_client_registry
          run_check contracts cargo test --locked -p nagic --test http_client_contracts
          run_check native cargo test --locked -p nagic --test http_client_native
          python "$RUNNER_TEMP/sf06-http-client-guard.py" native-forms 0 "$NAGI_TEST_ARTIFACT_DIR"
""", '      - name: Preserve closed SF06 native diagnostics\n'),
    (r"""      - name: Preserve closed SF06 native diagnostics
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: sf06-http-client-native-${{ matrix.platform }}
          path: |
            ${{ runner.temp }}/sf06-http-client-native/sf06-native-diagnostic.json
            ${{ steps.sf06_public_client.outputs.closed_failure_diagnostic }}
          if-no-files-found: ignore
""", '      - name: Classify SF06 public signature failure from captured output\n'),
    (r"""      - name: Classify SF06 public signature failure from captured output
        if: always()
        shell: bash
        run: |
          if ! python scripts/ci/sf06_http_client_signature_diagnostic.py \
            "$RUNNER_TEMP/sf06-native.log" \
            "$RUNNER_TEMP/sf06-http-client-native" 2>/dev/null; then
            printf '%s\n' 'sf06_http_client_signature_diag=v1 status=unavailable reason=invocation_error stage=unavailable form=unavailable rust_error_code=none code_status=not_applicable newline=unavailable artifact_saved=false'
          fi
""", '      - name: Preserve closed SF06 signature failure diagnostic\n'),
    (r"""      - name: Preserve closed SF06 signature failure diagnostic
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: sf06-http-client-signature-${{ matrix.platform }}
          path: ${{ runner.temp }}/sf06-http-client-native/sf06-signature-diagnostic.json
          if-no-files-found: ignore
""", '      - name: Diagnose G3 generation-token failure before crypto checks\n'),
    (r"""      - name: Check SF02 public Session runtime and one-use delivery before other checks
        shell: bash
        run: |
          set -euo pipefail
          if [ "${RUNNER_OS:-}" = "Windows" ]; then
            if [ -z "${RUNNER_TEMP:-}" ] || [ ! -d "${RUNNER_TEMP:-}" ]; then
              echo "RUNNER_TEMP must be a nonempty existing directory on Windows" >&2
              exit 2
            fi
            export TMP="$RUNNER_TEMP" TEMP="$RUNNER_TEMP"
          fi
          python -c 'import os; print("ICU4X_DATA_DIR present:", "ICU4X_DATA_DIR" in os.environ)'
          unset ICU4X_DATA_DIR
          cargo check --locked -p nagi-runtime --lib
          cargo test --locked -p nagi-runtime --lib auth::session::http_bridge::delivery_tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-delivery-wire.log"
          cargo test --locked -p nagi-runtime --lib auth::session::delivery::wire_tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-delivery-cell.log"
          cargo test --locked -p nagi-runtime --lib auth::session::delivery::tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-delivery-core.log"
          cargo test --locked -p nagi-runtime --lib auth::session::http_bridge::tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-http-bridge.log"
          cargo test --locked -p nagi-runtime --test session_public_api 2>&1 | tee "$RUNNER_TEMP/sf02-public-api.log"
          cargo test --locked -p nagi-runtime --lib http_server::tests::unsafe_session_tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-unsafe-session.log"
          cargo test --locked -p nagi-runtime --lib auth::session::tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-persistent-store.log"
          cargo test --locked -p nagi-runtime --lib auth::session::delivery::lease_tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-lease-owner.log"
          cargo test --locked -p nagi-runtime --lib auth::session::secrets::tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-secret-components.log"
          cargo test --locked -p nagi-runtime --lib auth::credential_metadata_tests:: 2>&1 | tee "$RUNNER_TEMP/sf02-credential-metadata.log"
          python - <<'PY'
          import os, pathlib, re
          root = pathlib.Path(os.environ["RUNNER_TEMP"])
          expected = {'sf02-delivery-wire.log': 11, 'sf02-delivery-cell.log': 2, 'sf02-delivery-core.log': 11, 'sf02-http-bridge.log': 7, 'sf02-public-api.log': 5, 'sf02-unsafe-session.log': 3, 'sf02-persistent-store.log': 41, 'sf02-lease-owner.log': 6, 'sf02-secret-components.log': 6, 'sf02-credential-metadata.log': 2}
          for name, count in expected.items():
              text = (root / name).read_text(encoding="utf-8")
              pattern = rf"test result: ok\. {count} passed; 0 failed; 0 ignored;"
              if not re.search(pattern, text):
                  raise AssertionError(f"{name}: expected {count} passed, 0 failed, 0 ignored")
          PY
""", '      - name: Capture bounded Session auth stages before broad checks\n'),
    (r"""      - name: Capture bounded Session auth stages before broad checks
        id: auth_stage_diag
        continue-on-error: true
        shell: bash
        run: |
          set -e
          if [ "${RUNNER_OS:-}" = "Windows" ]; then
            if [ -z "${RUNNER_TEMP:-}" ] || [ ! -d "${RUNNER_TEMP:-}" ]; then
              echo "RUNNER_TEMP must be a nonempty existing directory on Windows" >&2
              exit 2
            fi
            export TMP="$RUNNER_TEMP" TEMP="$RUNNER_TEMP"
          fi
          python -B scripts/tests/test_auth_stage_join.py
          python scripts/tests/test_auth_stage_diagnostic.py
          set +e
          join_failed=0
          RUSTFLAGS='--cfg nagi_auth_stage_diag' cargo test --locked -p nagi-runtime --lib auth::csrf::tests::same_queue_reopen_reuses_data_context_without_refreshing_token_expiry -- --exact --nocapture > "$RUNNER_TEMP/auth-stage-csrf.raw" 2> "$RUNNER_TEMP/auth-stage-csrf.stderr.raw"
          csrf_rc=$?
          python -B scripts/ci/auth_stage_join.py "$RUNNER_TEMP" csrf || join_failed=1
          RUSTFLAGS='--cfg nagi_auth_stage_diag' cargo test --locked -p nagi-runtime --lib auth::prelogin::tests::consumed_bearer_identity_can_issue_session_without_replacing_original_ceiling -- --exact --nocapture > "$RUNNER_TEMP/auth-stage-prelogin.raw" 2> "$RUNNER_TEMP/auth-stage-prelogin.stderr.raw"
          prelogin_rc=$?
          python -B scripts/ci/auth_stage_join.py "$RUNNER_TEMP" prelogin || join_failed=1
          RUSTFLAGS='--cfg nagi_auth_stage_diag --cfg nagi_browser_login_diag' cargo test --locked -p nagi-runtime --lib -- --nocapture > "$RUNNER_TEMP/auth-stage-broad.raw" 2> "$RUNNER_TEMP/auth-stage-broad.stderr.raw"
          broad_rc=$?
          python -B scripts/ci/auth_stage_join.py "$RUNNER_TEMP" broad || join_failed=1
          python -B scripts/tests/test_browser_login_stage_coverage.py
          stage_unit_rc=$?
          python -B scripts/ci/browser_login_stage_coverage.py "$RUNNER_TEMP/auth-stage-broad.raw" "$broad_rc" "$RUNNER_TEMP/auth-stage-browser-native.json"
          stage_rc=$?
          RUSTFLAGS='--cfg nagi_auth_stage_diag --cfg nagi_browser_login_diag' cargo test --locked -p nagi-runtime --lib -- --list > "$RUNNER_TEMP/auth-stage-cases.raw" 2> "$RUNNER_TEMP/auth-stage-cases.stderr.raw"
          cases_rc=$?
          python -B scripts/ci/auth_stage_join.py "$RUNNER_TEMP" cases || join_failed=1
          python scripts/ci/auth_stage_diagnostic.py "$RUNNER_TEMP" "$csrf_rc" "$prelogin_rc" "$broad_rc" "$cases_rc"
          sanitize_rc=$?
          if [ "$stage_unit_rc" -ne 0 ] || [ "$stage_rc" -ne 0 ] || [ "$join_failed" -ne 0 ]; then sanitize_rc=1; fi
          rm -f "$RUNNER_TEMP/auth-stage-csrf.raw" "$RUNNER_TEMP/auth-stage-prelogin.raw" "$RUNNER_TEMP/auth-stage-broad.raw" "$RUNNER_TEMP/auth-stage-cases.raw" "$RUNNER_TEMP/auth-stage-csrf.stderr.raw" "$RUNNER_TEMP/auth-stage-prelogin.stderr.raw" "$RUNNER_TEMP/auth-stage-broad.stderr.raw" "$RUNNER_TEMP/auth-stage-cases.stderr.raw"
          exit "$sanitize_rc"
""", '      - name: Preserve bounded Session auth-stage traces\n'),
    (r"""      - name: Preserve bounded Session auth-stage traces
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: auth-stage-${{ matrix.platform }}
          path: |
            ${{ runner.temp }}/auth-stage-csrf.trace
            ${{ runner.temp }}/auth-stage-prelogin.trace
            ${{ runner.temp }}/auth-stage-broad.trace
            ${{ runner.temp }}/auth-stage-browser-native.json
          if-no-files-found: ignore
""", '      - name: Capture bounded Session deadline public tests\n'),
    (r"""      - name: Capture bounded Session deadline public tests
        id: session_deadline_diag
        continue-on-error: true
        shell: bash
        run: |
          set -euo pipefail
          if [ "${RUNNER_OS:-}" = "Windows" ]; then
            if [ -z "${RUNNER_TEMP:-}" ] || [ ! -d "${RUNNER_TEMP:-}" ]; then
              echo "RUNNER_TEMP must be a nonempty existing directory on Windows" >&2
              exit 2
            fi
            export TMP="$RUNNER_TEMP" TEMP="$RUNNER_TEMP"
          fi
          python scripts/tests/test_session_deadline_diagnostic.py
          python -B scripts/tests/test_session_native_finish_diagnostic.py
          python -B scripts/tests/test_session_native_finish_summary.py
          set +e
          CARGO_TERM_COLOR=never RUSTFLAGS='--cfg nagi_auth_stage_diag' cargo test --locked -p nagi-runtime --test session_csrf_public_api 2>&1 | python -c 'import sys
          limit=256*1024+1
          remaining=limit
          while True:
           data=sys.stdin.buffer.read(8192)
           if not data:
            break
           if remaining:
            sys.stdout.buffer.write(data[:remaining])
            remaining-=min(remaining,len(data))
          sys.stdout.buffer.flush()' > "$RUNNER_TEMP/session-deadline-session.raw"
          pipeline_status=("${PIPESTATUS[@]}")
          session_rc=${pipeline_status[0]}
          if [ "${pipeline_status[1]}" -ne 0 ]; then rm -f "$RUNNER_TEMP/session-deadline-session.raw"; fi
          CARGO_TERM_COLOR=never RUSTFLAGS='--cfg nagi_auth_stage_diag --cfg nagi_browser_login_diag' cargo test --locked -p nagi-runtime --test browser_login_public_api 2>&1 | python -c 'import sys
          limit=256*1024+1
          remaining=limit
          while True:
           data=sys.stdin.buffer.read(8192)
           if not data:
            break
           if remaining:
            sys.stdout.buffer.write(data[:remaining])
            remaining-=min(remaining,len(data))
          sys.stdout.buffer.flush()' > "$RUNNER_TEMP/session-deadline-browser.raw"
          pipeline_status=("${PIPESTATUS[@]}")
          browser_rc=${pipeline_status[0]}
          if [ "${pipeline_status[1]}" -ne 0 ]; then rm -f "$RUNNER_TEMP/session-deadline-browser.raw"; fi
          python scripts/ci/session_deadline_diagnostic.py "$RUNNER_TEMP" "$session_rc" "$browser_rc" "$RUNNER_TEMP/session-deadline-diagnostic.json"
          diagnose_rc=$?
          rm -f "$RUNNER_TEMP/session-deadline-session.raw" "$RUNNER_TEMP/session-deadline-browser.raw"
          delivery_fail=0
          while read -r delivery_label delivery_name; do
            CARGO_TERM_COLOR=never RUSTFLAGS='--cfg nagi_auth_stage_diag' cargo test --locked -p nagi-runtime --lib "$delivery_name" -- --exact 2>&1 | python -c 'import sys
          limit=256*1024+1
          remaining=limit
          while True:
           data=sys.stdin.buffer.read(8192)
           if not data:
            break
           if remaining>0:
            sys.stdout.buffer.write(data[:remaining])
            remaining-=min(remaining,len(data))
          sys.stdout.buffer.flush()' > "$RUNNER_TEMP/session-deadline-delivery-$delivery_label.raw"
            delivery_status=("${PIPESTATUS[@]}")
            delivery_rc=${delivery_status[0]}
            if [ "${delivery_status[1]}" -ne 0 ]; then rm -f "$RUNNER_TEMP/session-deadline-delivery-$delivery_label.raw"; fi
            python -B scripts/ci/session_deadline_diagnostic.py --delivery "$delivery_label" "$RUNNER_TEMP/session-deadline-delivery-$delivery_label.raw" "$delivery_rc" "$RUNNER_TEMP/session-deadline-delivery-$delivery_label.json"
            if [ "$?" -ne 0 ]; then delivery_fail=1; fi
            rm -f "$RUNNER_TEMP/session-deadline-delivery-$delivery_label.raw"
          done <<'DELIVERY_CASES'
          applied auth::session::http_bridge::delivery_tests::applied_real_issue_commit_has_exactly_one_wire_cookie
          not_modified auth::session::http_bridge::delivery_tests::applied_304_and_private_cache_conflict_are_fixed_no_cookie_500
          head auth::session::http_bridge::delivery_tests::head_applied_response_transfers_cookie_once_without_body
          expiry auth::session::http_bridge::delivery_tests::expiry_after_real_commit_retires_applied_material_without_cookie
          DELIVERY_CASES
          if [ "$delivery_fail" -ne 0 ]; then diagnose_rc=1; fi
          CARGO_TERM_COLOR=never RUSTFLAGS='--cfg nagi_auth_stage_diag' cargo test --locked -p nagi-runtime --lib native_finish 2>&1 | python -B scripts/ci/session_native_finish_summary.py
          native_finish_status=("${PIPESTATUS[@]}")
          if [ "${native_finish_status[0]}" -ne 0 ] || [ "${native_finish_status[1]}" -ne 0 ]; then diagnose_rc=1; fi
          exit "$diagnose_rc"
""", '      - name: Preserve sanitized Session deadline diagnostics\n'),
    (r"""      - name: Preserve sanitized Session deadline diagnostics
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: session-deadline-${{ github.job }}-${{ runner.os }}-${{ runner.arch }}
          path: |
            ${{ runner.temp }}/session-deadline-diagnostic.json
            ${{ runner.temp }}/session-deadline-delivery-applied.json
            ${{ runner.temp }}/session-deadline-delivery-not_modified.json
            ${{ runner.temp }}/session-deadline-delivery-head.json
            ${{ runner.temp }}/session-deadline-delivery-expiry.json
          if-no-files-found: ignore
""", '      - name: Check owned BrowserLogin business stage and cancellation witnesses\n'),
    (r"""      - name: Check bounded SF01 diagnostic observer
        if: runner.os != 'Windows'
        env:
          RUSTFLAGS: --cfg nagi_sf01_diagnostic --cfg nagi_sf01_diag_oracle --check-cfg=cfg(nagi_sf01_diagnostic) --check-cfg=cfg(nagi_sf01_diag_oracle)
        run: cargo test --locked -p nagic --test security_sf01 sf01_diagnostic_observer_oracle -- --exact
""", '      - name: Check SF01 High, saved Low, handwritten Low and native policy contracts\n'),
    (r"""      - name: Check SF01 High, saved Low, handwritten Low and native policy contracts
        if: runner.os != 'Windows'
        env:
          RUSTFLAGS: --cfg nagi_sf01_diagnostic --check-cfg=cfg(nagi_sf01_diagnostic) --check-cfg=cfg(nagi_sf01_diag_oracle)
        run: cargo test --locked -p nagic --test security_sf01 --test security_sf01_native
""", '      - name: Check SF01 request lease and permit lifecycle through the public runtime\n'),
    (r"""      - name: Check SF01 request lease and permit lifecycle through the public runtime
        if: runner.os != 'Windows'
        shell: bash
        run: |
          set -o pipefail
          cargo test --locked -p nagi-runtime --test security_sf01
          cargo test --locked -p nagi-runtime --lib auth::tests:: 2>&1 | tee "$RUNNER_TEMP/sf01-auth.log"
          cargo test --locked -p nagi-runtime --lib http_server::tests::security_tests:: 2>&1 | tee "$RUNNER_TEMP/sf01-http.log"
          python -c 'import pathlib,re,os; root=pathlib.Path(os.environ["RUNNER_TEMP"]); counts={"sf01-auth.log":9,"sf01-http.log":11}; [(None if re.search(r"test result: ok\. %d passed; 0 failed; 0 ignored;"%n,(root/f).read_text()) else (_ for _ in ()).throw(AssertionError(f))) for f,n in counts.items()]'
""", '      - name: Check Host authority and HTTP entrypoint contracts on the target platform\n'),
    (r"""      - name: Check Host authority and HTTP entrypoint contracts on the target platform
        if: runner.os != 'Windows'
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
""", '      - name: Check SF05 Query migration and protected native SQL on the target platform\n'),
    (r"""      - name: Enforce Session auth-stage supplement after normal checks
        if: always() && steps.auth_stage_diag.outcome != 'skipped'
        shell: bash
        run: test '${{ steps.auth_stage_diag.outcome }}' = success
""", '      - name: Enforce Session deadline diagnostic after normal checks\n'),
    (r"""      - name: Enforce Session deadline diagnostic after normal checks
        if: always() && steps.session_deadline_diag.outcome != 'skipped'
        shell: bash
        run: test '${{ steps.session_deadline_diag.outcome }}' = success
""", '      - name: Measure bounded SF07 response and rate profile\n'),
)

OLD_CACHE = r"""      - name: Cache Windows contract builds
        uses: actions/cache/restore@0057852bfaa89a56745cba8c7296529d2fc39830 # ci-pin:v4
        id: nagi_rust_cache
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            native-target
          key: nagi-rust-v1-package-windows-x86_64-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**') }}
          restore-keys: |
            nagi-rust-v1-package-windows-x86_64-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-
            nagi-rust-v1-package-windows-x86_64-${{ steps.rust.outputs.cachekey }}-
"""

NEW_CACHE = r"""      - name: Restore isolated platform contract builds
        uses: actions/cache/restore@0057852bfaa89a56745cba8c7296529d2fc39830 # ci-pin:v4
        id: nagi_rust_cache
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            native-target
          key: nagi-platform-contracts-v1-${{ matrix.platform }}-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-config-default-auth-browser-sf01-oracle-sf06-nodebug-noincremental-v1-${{ hashFiles('Cargo.lock') }}-${{ hashFiles('.github/workflows/ci.yml', 'scripts/ci/**', 'scripts/tests/**', 'tests/test_sf06_http_client_signature_diagnostic.py') }}-${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**', 'runtime-core/**') }}
"""

CACHE_SAVE = r"""      - name: Save validated Rust cache (main only) # ci-cache-save
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

def job(source, name):
    found = list(re.finditer(r'(?ms)^  ' + re.escape(name) + r':\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', source))
    if len(found) != 1:
        raise ValueError('platform contract job missing or duplicated: ' + name)
    return found[0]


def _platform_job():
    block = OLD_WINDOWS.replace('  windows-contracts:\n', '  platform-contracts:\n', 1)
    block = block.replace('Compiler and runtime contracts (Windows)', 'Compiler and runtime contracts (${{ matrix.platform }})', 1)
    block = block.replace('    runs-on: windows-latest\n',
        '    strategy:\n      fail-fast: false\n      matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}\n    runs-on: ${{ matrix.os }}\n', 1)
    block = block.replace('RELEASE_RUST_HOST: x86_64-pc-windows-msvc', 'RELEASE_RUST_HOST: ${{ matrix.rust_host }}')
    block = block.replace(OLD_CACHE, NEW_CACHE, 1)
    extras = ''.join(body for body, _ in MOVES[:11])
    block = block.replace('      - name: Check bounded SF01 diagnostic observer\n', extras + '      - name: Check bounded SF01 diagnostic observer\n', 1)
    enforcers = ''.join(body for body, _ in MOVES[-2:])
    block = block.replace('      - name: Preserve Windows contract failure evidence\n', enforcers + '      - name: Preserve platform contract failure evidence\n', 1)
    block = block.replace('name: compiler-failures-windows-contracts', 'name: compiler-failures-platform-contracts-${{ matrix.platform }}', 1)
    return block.rstrip('\n') + '\n\n' + CACHE_SAVE


PLATFORM_JOB = _platform_job()


def apply_workflow(source):
    """Construct the one reviewed relocation from the frozen base layout."""
    old_job = job(source, 'windows-contracts')
    if old_job.group() != OLD_WINDOWS:
        raise ValueError('original Windows contract job changed')
    source = source[:old_job.start()] + PLATFORM_JOB + source[old_job.end():]
    package = job(source, 'nagi-package')
    block = package.group()
    for body, _ in MOVES:
        if block.count(body) != 1:
            raise ValueError('original platform contract step changed')
        block = block.replace(body, '', 1)
    source = source[:package.start()] + block + source[package.end():]
    return source.replace(', windows-contracts]', ', platform-contracts]').replace(', windows-contracts, ready]', ', platform-contracts, ready]')


def inverse_workflow(source):
    """Remove this layer only; missing, duplicated or drifted controls fail closed."""
    if '  platform-contracts:\n' not in source:
        # Historical callers may already have consumed this exact outer layer.
        if 'platform-contracts' in source:
            raise ValueError('partial platform contract wiring')
        return source
    found = job(source, 'platform-contracts')
    if found.group() != PLATFORM_JOB or '  windows-contracts:\n' in source:
        raise ValueError('platform contract commands, matrix, cache or evidence changed')
    source = source[:found.start()] + OLD_WINDOWS + source[found.end():]
    package = job(source, 'nagi-package')
    block = package.group()
    for body, anchor in reversed(MOVES):
        marker = body.splitlines()[0] + '\n'
        if marker in block or block.count(anchor) != 1:
            raise ValueError('platform contract step duplicated or original anchor changed')
        block = block.replace(anchor, body + anchor, 1)
    source = source[:package.start()] + block + source[package.end():]
    # Require both independent aggregate dependencies, with all other needs exact.
    for name in ('ready', 'publish-release'):
        found = job(source, name)
        block = found.group()
        if block.count('platform-contracts') != 1 or 'windows-contracts' in block:
            raise ValueError('platform contracts missing from aggregate dependencies')
        block = block.replace('platform-contracts', 'windows-contracts', 1)
        source = source[:found.start()] + block + source[found.end():]
    return source
