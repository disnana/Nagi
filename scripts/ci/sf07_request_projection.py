"""Historical U1 inverses and reviewed U4 production profiles; no native proof."""
import hashlib
import re
import importlib.util
from pathlib import Path

_LEVELS_SPEC = importlib.util.spec_from_file_location(
    "levels_projection", Path(__file__).with_name("levels_projection.py"))
_LEVELS = importlib.util.module_from_spec(_LEVELS_SPEC)
_LEVELS_SPEC.loader.exec_module(_LEVELS)

_PARALLEL_SPEC = importlib.util.spec_from_file_location(
    "parallel_regression_projection", Path(__file__).with_name("parallel_regression_projection.py"))
_PARALLEL = importlib.util.module_from_spec(_PARALLEL_SPEC)
_PARALLEL_SPEC.loader.exec_module(_PARALLEL)

_B1_SPEC = importlib.util.spec_from_file_location(
    "b1_bootstrap_observation", Path(__file__).with_name("b1_bootstrap_observation.py"))
_B1 = importlib.util.module_from_spec(_B1_SPEC)
_B1_SPEC.loader.exec_module(_B1)

_STREAM_SPEC = importlib.util.spec_from_file_location(
    "auth_stage_stream_projection", Path(__file__).with_name("auth_stage_stream_projection.py"))
_STREAM = importlib.util.module_from_spec(_STREAM_SPEC)
_STREAM_SPEC.loader.exec_module(_STREAM)


def norm(source):
    tokens = re.findall(r'"(?:\\.|[^"\\])*"|//[^\n]*|/\*.*?\*/|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]', source, re.S)
    return ' '.join(t for t in tokens if not t.startswith('//') and not t.startswith('/*') and t != ',')


def rewrite(source, before, after, count=1):
    before, after = norm(before), norm(after)
    if source.count(before) != count:
        raise ValueError('U1 inverse anchor mismatch')
    return source.replace(before, after)


def inverse_http(source):
    source = norm(source)
    source = rewrite(source, 'mod request_headers; use request_headers::RequestHeaders;', '')
    source = rewrite(source, 'pub struct Request { pub method: Method, uri: Uri, headers: RequestHeaders, body: Bytes, }',
                     'pub struct Request { pub method: Method, uri: Uri, headers: HeaderMap, body: Bytes, }')
    source = rewrite(source, 'store.lookup_http_identity(head.headers.authentication_headers())', 'store.lookup_http_identity(&head.headers)', 2)
    source = rewrite(source, 'head.headers.sanitize();', 'csrf_http::strip_credentials(&mut head.headers);')
    source = rewrite(source, 'headers: self.headers.share()', 'headers: self.headers.clone()')
    source = rewrite(source, 'let (mut parts, body) = request.into_parts();', 'let (parts, body) = request.into_parts();')
    source = rewrite(source, 'let headers = RequestHeaders::from(std::mem::take(&mut parts.headers));', '')
    source = rewrite(source, 'headers: headers.share()', 'headers: parts.headers.clone()')
    source = rewrite(source, 'let request = Request { method: parts.method, uri: parts.uri, headers, body, };',
                     'let request = Request { method: parts.method, uri: parts.uri, headers: parts.headers, body, };')
    return source


def inverse_browser(source):
    source = norm(source)
    source = rewrite(source, 'let mut headers = RequestHeaders::from(std::mem::take(&mut parts.headers)); headers.sanitize();',
                     'parts.headers.remove(names::COOKIE); parts.headers.remove("x-csrf-token");')
    source = rewrite(source, 'let mut request = Request { method: std::mem::take(&mut parts.method), uri: std::mem::take(&mut parts.uri), headers, body: Bytes::new(), };',
                     'let mut request = Request { method: std::mem::take(&mut parts.method), uri: std::mem::take(&mut parts.uri), headers: std::mem::take(&mut parts.headers), body: Bytes::new(), };')
    return source


def inverse_stage(source):
    return rewrite(norm(source), 'headers: headers.into(), body: Bytes::from_static(b"{}")',
                   'headers, body: Bytes::from_static(b"{}")')


def inverse_csrf(source):
    source = norm(source)
    marker = norm('pub(super) fn register<S, E>')
    if source.count(marker) != 1:
        raise ValueError('U1 csrf inverse marker')
    source = source.replace(marker, norm('pub(super) fn strip_credentials(headers: &mut HeaderMap) { headers.remove(names::COOKIE); headers.remove("x-csrf-token"); }') + ' ' + marker)
    source = rewrite(source, 'state: Arc<S>, mut parts: axum::http::request::Parts', 'state: Arc<S>, parts: axum::http::request::Parts')
    source = rewrite(source, 'let mut headers = RequestHeaders::from(std::mem::take(&mut parts.headers));', '')
    source = rewrite(source, 'headers: headers.share()', 'headers: parts.headers.clone()')
    source = rewrite(source, 'headers.sanitize();', 'let mut headers = parts.headers; strip_credentials(&mut headers);')
    return source


# U4/U5 made the producer, native driver and attached Request Context production.
# Historical U1 inverses above intentionally remain frozen; current source is
# instead bound to the independently reviewed public RC commit/tree below.
PRODUCTION_COMMIT = '1004a908c4e9090b1ed764d2a51900a5bdb2d5bf'
PRODUCTION_TREE = '9bbfe19664f715f2bead02e50e5e79f703afba07'
PRODUCTION_SOURCES = {'http': {'path': 'runtime/src/http_server.rs',
          'blob': 'dd56e351196ded77e8e92ac8cede54236af29fe6',
          'sha256': '397652b0f412e012c207c39288e2bda8771c486ea24f6d1f3e76da69e9204d54',
          'tokens': '9c4f072d0674f252a364010d0299dc16c246e296560a16ed65c494c67921c10c'},
 'browser': {'path': 'runtime/src/http_server/browser_login.rs',
             'blob': '7d9f62bcd723d985a227e13b8ce4ec15a8bd3a3a',
             'sha256': '363cf9c8e60369ad0e9996b6323c7c4f9e2a47eddc7bb160e514c1522b58b3a4',
             'tokens': 'f215ac43d66d09aed10f8af5d52c0cedc07ce94b64a8c22432fdc7b584bce4b6'},
 'stage': {'path': 'runtime/src/http_server/browser_login_stage_tests.rs',
           'blob': 'c06bc058e5b95851e079d4b24e31b1b9cae558d4',
           'sha256': '6e31ac420dbcd06f63651b93c8a12f2acffff319f4c7c4c55f50181a5723d110',
           'tokens': 'a0dc3d69b0b877aadbaf91a6d1d72389b886871362b450822352fdf6c58d7f2e'},
 'csrf': {'path': 'runtime/src/http_server/csrf_http.rs',
          'blob': '8b0728b8eb3aab578f8b27a7836cecc4c846ad1f',
          'sha256': '5f597750de0a5b07a29a1d05649e272bc11103b6e093711e0c5d00118d442943',
          'tokens': '3ff87ae24cdc8244a912d6105d11c04ea29ff0e4af98e3543256f57f4f248a1f'},
 'public': {'path': 'runtime/tests/browser_login_public_api.rs',
            'blob': 'aeafab34a1842038538b71bf15b1989eaac268bd',
            'sha256': '4999e44ad29bf7777a9f92d349eee025c9d3c1112162b7cc7d0524c74e745050',
            'tokens': 'fe23e1f3098b190113722979aed4fbaffeb10153f0869ba083dabf3674ac8156'}}


def approved_production(name, source):
    expected = PRODUCTION_SOURCES.get(name)
    if expected is None or hashlib.sha256(source.encode()).hexdigest() != expected['sha256']:
        raise ValueError('approved U4 production source identity mismatch')
    if digest(source) != expected['tokens']:
        raise ValueError('approved U4 production token mismatch')
    return norm(source)


# Frozen from exact9b7 base bytes before U1, not adjusted to the candidate.
BASE_TOKENS = {'http': '8efe1130c731238099553c0e9fc58a19ff103fc57278c55c7ec8839de26d1f3e', 'browser': '2b61c339bfeecc0519ccf72517cc64346f40b6c51d2bab946004f2f6f2a71dab', 'stage': '8b5e8f0c3ab595d8fa6b93d2c4c400ef670f64933c576db994315c620bdcced4', 'csrf': '21ae359e321f254d831f0c29b43392f2b8490e8c14ccddaef733f58c8b7b95d3'}
HTTP_BEFORE = 'e6969a04326ed5fc4b3795f64a2909ffb5bbfae16f679589b93a819922084485'
HISTORICAL_HTTP_AFTER = '3fa9bf11de875398966537bb275057c6bc23ffffb888ed3bddbe0ea2ff8e25f6'
HTTP_AFTER = '397652b0f412e012c207c39288e2bda8771c486ea24f6d1f3e76da69e9204d54'
TESTS_BEFORE = '8752ae42a02af282b9a5d79f11e6e1bc1c95642fb1c1b4f4a23da34012130173'
HISTORICAL_TESTS_AFTER = '640f64c8c44fb085d04c445c14cbd7c4c46ce07ab7353351a195260a2740c64f'
TESTS_AFTER = '25ebad5039d20ec7109e40b44ac87a85a72bc4bd8c9c85f78fd97164c5b29de8'
SOURCE_LINE = '          python -B scripts/tests/test_sf07_request_source.py\n'
NATIVE_LINE = '          cargo test --locked -p nagi-runtime --lib http_server::request_headers::tests:: 2>&1 | python -B scripts/ci/sf07_checkpoint_summary.py request\n'
RATE_SOURCE_LINE = '          python -B scripts/tests/test_sf07_rate_source.py\n'
RATE_NATIVE_LINE = '          cargo test --locked -p nagi-runtime --lib security_sf07_private::startup_rate::tests:: 2>&1 | python -B scripts/ci/sf07_checkpoint_summary.py rate-startup\n'
U2_NATIVE_LINES = (
    '          cargo test --locked -p nagi-runtime --lib security_sf07_private::quota::retained_catalog_tests:: 2>&1 | python -B scripts/ci/sf07_checkpoint_summary.py catalog-retained\n',
    '          cargo test --locked -p nagi-runtime --lib auth::csrf::fixed_body::tests:: 2>&1 | python -B scripts/ci/sf07_checkpoint_summary.py fixed-owner\n',
    '          cargo test --locked -p nagi-runtime --lib auth::csrf::tests::fixed_wire_owner_tests:: 2>&1 | python -B scripts/ci/sf07_checkpoint_summary.py csrf-issuer\n',
    '          cargo test --locked -p nagi-runtime --lib auth::prelogin::tests::fixed_wire_owner_tests:: 2>&1 | python -B scripts/ci/sf07_checkpoint_summary.py prelogin-issuer\n',
)


SESSION_NATIVE_FINISH_ADDITIONS = ('          python -B scripts/tests/test_session_native_finish_diagnostic.py\n', '          python -B scripts/tests/test_session_native_finish_summary.py\n', '          CARGO_TERM_COLOR=never RUSTFLAGS=\'--cfg nagi_auth_stage_diag\' cargo test --locked -p nagi-runtime --lib native_finish 2>&1 | python -B scripts/ci/session_native_finish_summary.py\n          native_finish_status=("${PIPESTATUS[@]}")\n          if [ "${native_finish_status[0]}" -ne 0 ] || [ "${native_finish_status[1]}" -ne 0 ]; then diagnose_rc=1; fi\n')


OWNED_STATE_CI_ADDITIONS = (
    '          if [ "$RUNNER_OS" = "Windows" ]; then\n'
    '            NAGI_TEST_BASH="$(cygpath -w "$BASH")"\n'
    '          else\n'
    '            NAGI_TEST_BASH="$BASH"\n'
    '          fi\n'
    '          export NAGI_TEST_BASH\n'
    '          python -B scripts/tests/test_sf07_owned_state_summary.py\n',
    '          CARGO_TERM_COLOR=never cargo test --locked -p nagi-runtime --lib security_sf07_private::owned_state_lock::tests:: 2>&1 | python -B scripts/ci/sf07_owned_state_summary.py lock\n',
    '          CARGO_TERM_COLOR=never cargo test --locked -p nagi-runtime --lib security_sf07_private::owned_state_owner_tests:: 2>&1 | python -B scripts/ci/sf07_owned_state_summary.py owners\n',
    '          CARGO_TERM_COLOR=never cargo test --locked -p nagi-runtime --lib http_server::response_send::owned_state_lock_tests:: 2>&1 | python -B scripts/ci/sf07_owned_state_summary.py send\n')

BROWSER_COUNTER_PROFILE_STEP = r"""      - name: Compare BrowserLogin failure cases alone and with the parallel public suite
        shell: bash
        run: |
          set +e
          run_probe() {
            local profile="$1"
            local mode="$2"
            shift 2
            local -a pipeline_status
            CARGO_TERM_COLOR=never "$@" 2>&1 | python -B scripts/ci/sf07_checkpoint_summary.py "$mode"
            pipeline_status=("${PIPESTATUS[@]}")
            local cargo_exit="${pipeline_status[0]:-1}"
            local inventory_exit="${pipeline_status[1]:-1}"
            printf "browser_counter_probe profile=%s cargo_exit=%s inventory_exit=%s\n" "$profile" "$cargo_exit" "$inventory_exit"
            if [ "$cargo_exit" -ne 0 ] || [ "$inventory_exit" -ne 0 ]; then
              return 1
            fi
            return 0
          }
          if ! run_probe compile browser-counter-compile cargo test --locked -p nagi-runtime --test browser_login_public_api --no-run; then
            echo "browser_counter_probe stop=compile"
            exit 1
          fi
          if ! run_probe list browser-counter-list cargo test --locked -p nagi-runtime --test browser_login_public_api -- --list; then
            echo "browser_counter_probe stop=list"
            exit 1
          fi
          profile_failed=0
          if ! run_probe single-strict browser-counter-strict cargo test --locked -p nagi-runtime --test browser_login_public_api input_and_strict_json_rejections_do_not_consume_or_call_verifier -- --exact; then
            profile_failed=1
          fi
          if ! run_probe single-wire browser-counter-wire cargo test --locked -p nagi-runtime --test browser_login_public_api wire_body_cap_is_independent_of_db_quota_and_checked_before_claim -- --exact; then
            profile_failed=1
          fi
          if ! run_probe parallel-ten browser-counter-parallel cargo test --locked -p nagi-runtime --test browser_login_public_api; then
            profile_failed=1
          fi
          exit "$profile_failed"
"""

PRIVATE_CORE_COLOR_COMMAND = '          CARGO_TERM_COLOR=never cargo test --locked -p nagi-runtime --lib security_sf07_private:: 2>&1 | tee "$RUNNER_TEMP/sf07-private-core.log"\n'
PRIVATE_CORE_COLOR_BASELINE = '          cargo test --locked -p nagi-runtime --lib security_sf07_private:: 2>&1 | tee "$RUNNER_TEMP/sf07-private-core.log"\n'


RELEASE_PROFILE_CI_STEPS = ('      - name: Measure bounded SF07 response and rate profile\n        env:\n          CARGO_TERM_COLOR: never\n        shell: bash\n        run: |\n          set -euo pipefail\n          python -B scripts/tests/test_sf07_profile_summary.py\n          cargo test --locked -p nagi-runtime --test sf07_small_profile --no-run\n          cargo test --locked -p nagi-runtime --test sf07_small_profile -- --list > "$RUNNER_TEMP/sf07-profile-list.log"\n          python - <<\'PYSF07LIST\'\n          import os,pathlib,re\n          text=(pathlib.Path(os.environ["RUNNER_TEMP"])/"sf07-profile-list.log").read_text()\n          if re.findall(r"(?m)^([a-z0-9_]+): test$",text) != ["response_profile_reports_closed_measurements"]:\n              raise SystemExit("sf07_profile_inventory=invalid")\n          PYSF07LIST\n          set +e\n          cargo test --locked -p nagi-runtime --test sf07_small_profile response_profile_reports_closed_measurements -- --exact --show-output > "$RUNNER_TEMP/sf07-profile-native.log" 2>&1\n          profile_rc=$?\n          set -e\n          python -B scripts/ci/sf07_profile_summary.py --cargo-exit-code "$profile_rc" < "$RUNNER_TEMP/sf07-profile-native.log"\n', '      - name: Validate production SF06 with isolated owned DNS and TLS\n        shell: bash\n        run: |\n          set -euo pipefail\n          python -B scripts/tests/test_sf06_owned_network.py\n          NAGI_SF06_NAMESPACE_PROBE=1 python -B scripts/tests/test_sf06_owned_network.py HostedLifecycleProbe.test_namespace_eof_reaps_setsid_descendant\n          set +e\n          cargo test --locked -p nagi-runtime --test http_client_production_profile --no-run --message-format=json > "$RUNNER_TEMP/sf06-profile-build.jsonl"\n          compile_rc=$?\n          set -e\n          binary=$(SF06_COMPILE_RC="$compile_rc" python - <<\'PYSF06BUILD\'\n          import json,os,pathlib,sys\n          rows=[json.loads(line) for line in (pathlib.Path(os.environ["RUNNER_TEMP"])/"sf06-profile-build.jsonl").read_text().splitlines() if line.strip()]\n          if os.environ["SF06_COMPILE_RC"] != "0":\n              for row in rows:\n                  message=row.get("message",{})\n                  if row.get("reason")=="compiler-message" and message.get("level")=="error":\n                      print("sf06_profile_compile_error="+json.dumps({"code":message.get("code"),"message":message.get("message","")[:400]}),file=sys.stderr)\n              raise SystemExit(1)\n          paths=[row["executable"] for row in rows if row.get("reason")=="compiler-artifact" and row.get("target",{}).get("name")=="http_client_production_profile" and row.get("executable")]\n          if len(paths)!=1 or "\\n" in paths[0] or "\\r" in paths[0]:\n              raise SystemExit("sf06_profile_binary_inventory=invalid")\n          print(paths[0])\n          PYSF06BUILD\n          )\n          python -B scripts/ci/sf06_owned_network.py --binary "$binary" --source-sha "$GITHUB_SHA"\n')


SF08_BROWSER_JOB = """  sf08-browser:
    needs: [changes, release-plan]
    if: >-
      always() && needs.changes.result == 'success' && needs.release-plan.result == 'success' &&
      (needs.changes.outputs.full_checks == 'true' || needs.release-plan.outputs.package_nagi == 'true')
    uses: ./.github/workflows/sf08-browser.yml

"""
READY_NEEDS_WITH_SF08 = (
    "    needs: [changes, linux, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package]\n"
)
READY_NEEDS_WITHOUT_SF08 = (
    "    needs: [changes, linux, rust-1-88, jetbrains, release-plan, vscode-package, nagi-package]\n"
)
PUBLISH_NEEDS_WITH_SF08 = (
    "    needs: [changes, linux, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package]\n"
)
PUBLISH_NEEDS_WITHOUT_SF08 = (
    "    needs: [linux, rust-1-88, release-plan, jetbrains, vscode-package, nagi-package]\n"
)
PUBLISH_IF_WITH_SF08 = """    if: >-
      always() && github.event_name == 'push' && github.ref == 'refs/heads/main' &&
      (
        (needs.linux.result == 'success' && needs.rust-1-88.result == 'success' &&
         needs.sf08-browser.result == 'success') ||
        (needs.release-plan.outputs.release_jetbrains == 'true' &&
         needs.release-plan.outputs.release_nagi != 'true' &&
         needs.release-plan.outputs.release_vscode != 'true' &&
         needs.linux.result == 'skipped' &&
         needs.rust-1-88.result == 'skipped' &&
         needs.sf08-browser.result == 'skipped')
      ) && needs.release-plan.result == 'success' &&
      needs.changes.result == 'success' &&
      (needs.jetbrains.result == 'success' || needs.jetbrains.result == 'skipped') &&
      (needs.vscode-package.result == 'success' || needs.vscode-package.result == 'skipped') &&
      (needs.nagi-package.result == 'success' || needs.nagi-package.result == 'skipped') &&
      (needs.release-plan.outputs.release_jetbrains != 'true' || needs.jetbrains.result == 'success') &&
      (needs.release-plan.outputs.release_vscode != 'true' || needs.vscode-package.result == 'success') &&
      (needs.release-plan.outputs.release_nagi != 'true' || needs.nagi-package.result == 'success') &&
      (needs.release-plan.outputs.release_vscode == 'true' || needs.release-plan.outputs.release_nagi == 'true' || needs.release-plan.outputs.release_jetbrains == 'true')
"""
PUBLISH_IF_WITHOUT_SF08 = """    if: >-
      always() && github.event_name == 'push' && github.ref == 'refs/heads/main' &&
      (
        (needs.linux.result == 'success' && needs.rust-1-88.result == 'success') ||
        (needs.release-plan.outputs.release_jetbrains == 'true' &&
         needs.release-plan.outputs.release_nagi != 'true' &&
         needs.release-plan.outputs.release_vscode != 'true' &&
         needs.linux.result == 'skipped' &&
         needs.rust-1-88.result == 'skipped')
      ) && needs.release-plan.result == 'success' &&
      (needs.jetbrains.result == 'success' || needs.jetbrains.result == 'skipped') &&
      (needs.vscode-package.result == 'success' || needs.vscode-package.result == 'skipped') &&
      (needs.nagi-package.result == 'success' || needs.nagi-package.result == 'skipped') &&
      (needs.release-plan.outputs.release_jetbrains != 'true' || needs.jetbrains.result == 'success') &&
      (needs.release-plan.outputs.release_vscode != 'true' || needs.vscode-package.result == 'success') &&
      (needs.release-plan.outputs.release_nagi != 'true' || needs.nagi-package.result == 'success') &&
      (needs.release-plan.outputs.release_vscode == 'true' || needs.release-plan.outputs.release_nagi == 'true' || needs.release-plan.outputs.release_jetbrains == 'true')
"""


CI_SOURCE_GUARD_STEP = """      - name: Check OS-independent SF07 and BrowserLogin source guards
        shell: bash
        run: |
          set -euo pipefail
          python -B scripts/tests/test_sf07_wire_source.py
          python -B scripts/tests/test_sf07_transport_source.py
          python -B scripts/tests/test_sf07_request_source.py
          python -B scripts/tests/test_sf07_rate_source.py
          python -B scripts/tests/test_sf07_checkpoint_summary.py
          python -B scripts/tests/test_browser_login_stage_backport.py
"""
CI_SOURCE_GUARD_LINES = (
    '          python -B scripts/tests/test_sf07_wire_source.py\n',
    '          python -B scripts/tests/test_sf07_transport_source.py\n',
    '          python -B scripts/tests/test_sf07_request_source.py\n',
    '          python -B scripts/tests/test_sf07_rate_source.py\n',
    '          python -B scripts/tests/test_sf07_checkpoint_summary.py\n',
)
CI_BROWSER_SOURCE_LINE = '          python -B scripts/tests/test_browser_login_stage_backport.py\n'
CI_CHANGES_SETUP = """      - uses: actions/setup-python@v5
        with:
          python-version: '3.12'
"""
CI_NAGI_PACKAGE_CURRENT_GATE = """  nagi-package:
    needs: [changes, release-plan]
    if: >-
      always() && needs.changes.result == 'success' && needs.release-plan.result == 'success' &&
      needs.release-plan.outputs.package_nagi == 'true'
"""
CI_NAGI_PACKAGE_BASE_GATE = """  nagi-package:
    needs: release-plan
    if: needs.release-plan.outputs.package_nagi == 'true'
"""
CI_SF07_STEP_ANCHOR = """      - name: Check SF07 charged native wire checkpoint
        shell: bash
        run: |
          set -euo pipefail
          if [ \"$RUNNER_OS\" = \"Windows\" ]; then
"""
CI_SF07_STEP_RESTORED = """      - name: Check SF07 charged native wire checkpoint
        shell: bash
        run: |
          set -euo pipefail
          python -B scripts/tests/test_sf07_wire_source.py
          python -B scripts/tests/test_sf07_transport_source.py
          python -B scripts/tests/test_sf07_request_source.py
          python -B scripts/tests/test_sf07_rate_source.py
          python -B scripts/tests/test_sf07_checkpoint_summary.py
          if [ \"$RUNNER_OS\" = \"Windows\" ]; then
"""
CI_BROWSER_STEP_ANCHOR = """      - name: Check owned BrowserLogin business stage and cancellation witnesses
        shell: bash
        run: |
          set -euo pipefail
          cargo test --locked -p nagi-runtime --lib http_server::browser_login::stage_tests:: 2>&1 | python -B scripts/tests/test_browser_login_stage_backport.py --native-summary
"""
CI_BROWSER_STEP_RESTORED = """      - name: Check owned BrowserLogin business stage and cancellation witnesses
        shell: bash
        run: |
          set -euo pipefail
          python -B scripts/tests/test_browser_login_stage_backport.py
          cargo test --locked -p nagi-runtime --lib http_server::browser_login::stage_tests:: 2>&1 | python -B scripts/tests/test_browser_login_stage_backport.py --native-summary
"""


def inverse_source_guard_dedup(source):
    source = _STREAM.inverse(source)
    return _inverse_source_guard_dedup_base(inverse_prelogin_owner_diagnostic(source))


def _inverse_source_guard_dedup_base(source):
    # Direct callers use this intermediate inverse as well as inverse_workflow.
    # Validate and remove the reviewed tier/infrastructure layer before checking
    # the historical source-guard placement; no native command is normalized.
    if "\n  ci-plan:\n" in source:
        source = _LEVELS.inverse_workflow(source)
    # Restore the reviewed per-OS source calls and package dependency before
    # applying older inverses. The new early step is exact, unique, and located
    # immediately after Python setup in the existing changes gate. Keeping this
    # normalization separate lets source guards inspect their historical
    # two-platform registration contract while still rejecting any drift in
    # the deduplicated workflow itself.
    if source.count(CI_SOURCE_GUARD_STEP) != 1:
        raise ValueError('early CI source guard registration mismatch')
    for line in (*CI_SOURCE_GUARD_LINES, CI_BROWSER_SOURCE_LINE):
        if source.count(line) != 1:
            raise ValueError('early CI source guard command inventory mismatch')
    changes_header = '  changes:\n'
    if source.count(changes_header) != 1:
        raise ValueError('changes job anchor mismatch')
    changes_start = source.index(changes_header)
    next_job = re.search(r'(?m)^  [A-Za-z0-9_-]+:\n', source[changes_start + len(changes_header):])
    if next_job is None:
        raise ValueError('changes job boundary mismatch')
    changes_end = changes_start + len(changes_header) + next_job.start()
    changes_block = source[changes_start:changes_end]
    if changes_block.count(CI_CHANGES_SETUP + CI_SOURCE_GUARD_STEP) != 1:
        raise ValueError('early CI source guard placement mismatch')
    source = source.replace(CI_SOURCE_GUARD_STEP, '', 1)

    if source.count(CI_NAGI_PACKAGE_CURRENT_GATE) != 1:
        raise ValueError('nagi-package changes gate mismatch')
    source = source.replace(CI_NAGI_PACKAGE_CURRENT_GATE, CI_NAGI_PACKAGE_BASE_GATE, 1)

    if source.count(CI_SF07_STEP_ANCHOR) != 2:
        raise ValueError('per-OS SF07 native step anchor mismatch')
    source = source.replace(CI_SF07_STEP_ANCHOR, CI_SF07_STEP_RESTORED, 2)
    if source.count(CI_BROWSER_STEP_ANCHOR) != 2:
        raise ValueError('per-OS BrowserLogin native step anchor mismatch')
    source = source.replace(CI_BROWSER_STEP_ANCHOR, CI_BROWSER_STEP_RESTORED, 2)
    return source


# This failure-only supplement is removed exactly before historical projection.
# The primary SF07 commands and frozen workflow contract remain unchanged.
PRELOGIN_OWNER_DIAGNOSTIC_STEP = """      - name: Capture bounded SF07 prelogin owner stages after checkpoint failure
        id: sf07_prelogin_owner_diag
        if: failure() && steps.sf07_checkpoint.outcome == 'failure' && !cancelled()
        shell: bash
        run: |
          python -B scripts/ci/sf07_prelogin_owner_diagnostic.py "$RUNNER_TEMP/sf07-prelogin-owner"
      - name: Preserve bounded SF07 prelogin owner stages
        if: always() && steps.sf07_checkpoint.outcome == 'failure' && steps.sf07_prelogin_owner_diag.outputs.sanitized == 'true'
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: sf07-prelogin-owner-${{ github.job }}-${{ runner.os }}-${{ runner.arch }}
          path: ${{ runner.temp }}/sf07-prelogin-owner/auth-stage-prelogin.trace
          if-no-files-found: ignore
"""
PRELOGIN_OWNER_PRIMARY_ID = "        id: sf07_checkpoint\n"
PRELOGIN_OWNER_UNIT_LINE = "          python -B scripts/tests/test_sf07_prelogin_owner_diagnostic.py\n"
PRELOGIN_OWNER_INHERITED_STEP = PRELOGIN_OWNER_DIAGNOSTIC_STEP.replace(
    "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4",
    "actions/upload-artifact@v4",
).replace("          retention-days: 7 # ci-retention\n", "")


def inverse_prelogin_owner_diagnostic(source):
    # Existing callers also supply the exact historical infrastructure profile
    # after levels.inverse_workflow. Never accept that profile in raw CI input.
    step = (PRELOGIN_OWNER_DIAGNOSTIC_STEP if "\n  ci-plan:\n" in source
            else PRELOGIN_OWNER_INHERITED_STEP)
    primary = "      - name: Check SF07 charged native wire checkpoint\n"
    preceding = ("          CARGO_TERM_COLOR=never cargo test --locked -p nagi-runtime --lib "
                 "http_server::response_send::owned_state_lock_tests:: 2>&1 | python -B "
                 "scripts/ci/sf07_owned_state_summary.py send\n")
    following = "      - name: Check SF02 public Session runtime and one-use delivery before other checks\n"
    unit = "          python -B scripts/tests/test_sf07_owned_state_summary.py\n"
    if (source.count(primary + PRELOGIN_OWNER_PRIMARY_ID) != 2
            or source.count(preceding + step + following) != 2
            or source.count(unit + PRELOGIN_OWNER_UNIT_LINE) != 2):
        raise ValueError("prelogin owner diagnostic placement mismatch")
    for added in (step, PRELOGIN_OWNER_PRIMARY_ID,
                  PRELOGIN_OWNER_UNIT_LINE):
        if source.count(added) != 2:
            raise ValueError("prelogin owner diagnostic registration mismatch")
        source = source.replace(added, "")
    return source


def inverse_workflow(source):
    source = _STREAM.inverse(source)
    # Validate the new pinned artifact step before the infrastructure inverse
    # restores historical action spellings and retention fields.
    source = inverse_prelogin_owner_diagnostic(source)
    if "\n  ci-plan:\n" in source:
        source = _LEVELS.inverse_workflow(source)
    source = _B1.inverse_failure_diagnostic(source)
    # The guarded Busy regression is the fifteenth case in the exact SF03
    # deadline inventory.  Restore the inherited fourteen-case workflow only
    # after validating both original acceptance sites; missing, duplicated, or
    # independently edited counts remain visible to the frozen history proof.
    deadline_current = '"sf03-b1-csrf-deadline.log":15'
    deadline_inherited = '"sf03-b1-csrf-deadline.log":14'
    if source.count(deadline_current) != 2 or source.count(deadline_inherited) != 0:
        raise ValueError('guarded Busy deadline inventory mismatch')
    source = source.replace(deadline_current, deadline_inherited)
    source = _PARALLEL.inverse_workflow(source)
    source = _inverse_source_guard_dedup_base(source)
    for added, inherited, label in (
        (SF08_BROWSER_JOB, '', 'SF08 browser reusable job'),
        (READY_NEEDS_WITH_SF08, READY_NEEDS_WITHOUT_SF08, 'ready dependencies'),
        (PUBLISH_NEEDS_WITH_SF08, PUBLISH_NEEDS_WITHOUT_SF08, 'publish dependencies'),
        (PUBLISH_IF_WITH_SF08, PUBLISH_IF_WITHOUT_SF08, 'publish condition'),
    ):
        if source.count(added) != 1:
            raise ValueError(f'{label} inverse mismatch')
        source = source.replace(added, inherited, 1)
    # Add the real deferred-touch deadline regression without replacing any
    # historical source digest or accepting a missing/duplicated registration.
    current_session_inventory = "'sf02-http-bridge.log': 7"
    if source.count(current_session_inventory) != 2:
        raise ValueError("Session HTTP bridge inventory mismatch")
    source = source.replace(current_session_inventory, "'sf02-http-bridge.log': 6")
    for addition in RELEASE_PROFILE_CI_STEPS:
        if source.count(addition) != 1:
            raise ValueError("release profile workflow registration mismatch")
        source = source.replace(addition, "")
    if source.count(BROWSER_COUNTER_PROFILE_STEP) != 1:
        raise ValueError('BrowserLogin counter profile step mismatch')
    source = source.replace(BROWSER_COUNTER_PROFILE_STEP, '')
    if source.count(PRIVATE_CORE_COLOR_COMMAND) != 2:
        raise ValueError("private-core color command mismatch")
    source = source.replace(PRIVATE_CORE_COLOR_COMMAND, PRIVATE_CORE_COLOR_BASELINE)
    for addition in OWNED_STATE_CI_ADDITIONS:
        if source.count(addition) != 2:
            raise ValueError('owned-state workflow registration mismatch')
        source = source.replace(addition, '')
    for addition in SESSION_NATIVE_FINISH_ADDITIONS:
        if source.count(addition) != 2:
            raise ValueError('Session native finish workflow registration mismatch')
        source = source.replace(addition, '')
    for line in U2_NATIVE_LINES:
        if source.count(line) != 2:
            raise ValueError('U2 workflow registration mismatch')
        source = source.replace(line, '')
    if source.count('"sf03-b1-csrf-core.log":15') != 2:
        raise ValueError('U2 concrete issuer inventory mismatch')
    source = source.replace('"sf03-b1-csrf-core.log":15', '"sf03-b1-csrf-core.log":14')
    if source.count(SOURCE_LINE) != 2 or source.count(NATIVE_LINE) != 2:
        raise ValueError('U1 workflow registration mismatch')
    if source.count(RATE_SOURCE_LINE) != 2 or source.count(RATE_NATIVE_LINE) != 2:
        raise ValueError('U3 workflow registration mismatch')
    if source.count(TESTS_AFTER) != 2:
        raise ValueError('U1 native fixture binding mismatch')
    return source.replace(SOURCE_LINE, '').replace(NATIVE_LINE, '').replace(RATE_SOURCE_LINE, '').replace(RATE_NATIVE_LINE, '').replace(HTTP_AFTER, HTTP_BEFORE).replace(TESTS_AFTER, TESTS_BEFORE)


def inverse_prelogin_wire(source):
    """Undo only U2's fixed original body owner for inherited source guards."""
    new = '''        let body = super::super::csrf::fixed_body::FixedTokenBody::new(&pair.token)
            .ok_or_else(Failure::wire_internal)?;
        // Finite cookie/body construction is not forcibly preemptible. Recheck
'''
    old = '''        #[derive(serde::Serialize)]
        struct Json<'a> {
            csrf_token: &'a str,
        }
        let body = serde_json::to_vec(&Json {
            csrf_token: &pair.token,
        })
        .map_err(|_| Failure::wire_internal())?;
        if body.len() != 60 {
            return Err(Failure::wire_internal());
        }
        // Finite library serialization is not forcibly preemptible. Recheck
'''
    converted = '            body: body.into_bytes(),\n'
    prior = '            body: hyper::body::Bytes::from(body),\n'
    if source.count(new) != 1 or source.count(converted) != 1:
        raise ValueError('U2 prelogin original-owner inverse mismatch')
    return source.replace(new, old).replace(converted, prior)


SESSION_NATIVE_COLLECTOR_INVERSE = (('\nfrom collections import Counter\nimport hashlib\nimport importlib.util\nimport json\nimport os\nfrom pathlib import Path\n', '\nfrom collections import Counter\nimport hashlib\nimport json\nimport os\nfrom pathlib import Path\n'), ('import subprocess\nimport sys\nimport threading\n\n_NATIVE_SPEC = importlib.util.spec_from_file_location(\n    "session_native_finish_diagnostic", Path(__file__).with_name("session_native_finish_diagnostic.py"))\nnative_finish = importlib.util.module_from_spec(_NATIVE_SPEC)\n_NATIVE_SPEC.loader.exec_module(native_finish)\n\nPREFIX = "session_private_deadline_diag"\nRAW_LIMIT = 256 * 1024\n', 'import subprocess\nimport sys\nimport threading\n\nPREFIX = "session_private_deadline_diag"\nRAW_LIMIT = 256 * 1024\n'), ('def collect(case: str, raw: bytes, returncode: int, *, oversized: bool = False) -> dict:\n    # Unknown identity is never echoed. Callers cannot enlarge the native suite.\n    if case not in CASES:\n        return {"case": "unknown", "state": "invalid", "reason": 1, "facts": [], "native_finish": []}\n    result = {"case": case, "state": "invalid", "reason": 2, "facts": [], "native_finish": []}\n    if oversized or len(raw) > RAW_LIMIT:\n        return result\n    try:\n', 'def collect(case: str, raw: bytes, returncode: int, *, oversized: bool = False) -> dict:\n    # Unknown identity is never echoed. Callers cannot enlarge the native suite.\n    if case not in CASES:\n        return {"case": "unknown", "state": "invalid", "reason": 1, "facts": []}\n    result = {"case": case, "state": "invalid", "reason": 2, "facts": []}\n    if oversized or len(raw) > RAW_LIMIT:\n        return result\n    try:\n'), ('        result["reason"] = 3\n        return result\n    facts = []\n    native_facts = []\n    seen = set()\n    headings = [index for index, line in enumerate(lines)\n                if line.startswith("---- ") and line.endswith(" stdout ----")]\n', '        result["reason"] = 3\n        return result\n    facts = []\n    seen = set()\n    headings = [index for index, line in enumerate(lines)\n                if line.startswith("---- ") and line.endswith(" stdout ----")]\n'), ('    for index, line in enumerate(lines):\n        # Prefix embedded in an arbitrary tail, alternate shape, or truncation\n        # is invalid rather than silently ignored or rendered.\n        if native_finish.PREFIX in line:\n            value = native_finish.parse(line)\n            legacy = marker(lines[index - 1]) if index > 0 else None\n            if (not block_start < index < block_end or value is None\n                    or not native_finish.paired(value, legacy, 3)\n                    or line in seen or len(native_facts) >= MARKER_LIMIT):\n                result["reason"] = 4\n                return result\n            native_facts.append(value)\n            seen.add(line)\n            continue\n        if PREFIX not in line:\n            continue\n        if not block_start < index < block_end:\n', '    for index, line in enumerate(lines):\n        # Prefix embedded in an arbitrary tail, alternate shape, or truncation\n        # is invalid rather than silently ignored or rendered.\n        if PREFIX not in line:\n            continue\n        if not block_start < index < block_end:\n'), ('                item[key] != 0 for key in ("begins", "commands", "commits")) for item in facts):\n            result["reason"] = 6\n            return result\n    result.update(state="passed" if good else "tests_failed", reason=0, facts=facts, native_finish=native_facts)\n    if len(json.dumps(result, separators=(",", ":")).encode()) > JSON_LIMIT:\n        return {"case": case, "state": "invalid", "reason": 2, "facts": [], "native_finish": []}\n    return result\n\n\n', '                item[key] != 0 for key in ("begins", "commands", "commits")) for item in facts):\n            result["reason"] = 6\n            return result\n    result.update(state="passed" if good else "tests_failed", reason=0, facts=facts)\n    if len(json.dumps(result, separators=(",", ":")).encode()) > JSON_LIMIT:\n        return {"case": case, "state": "invalid", "reason": 2, "facts": []}\n    return result\n\n\n'), ('            raw, code, oversized = capture(native_command(name), env)\n            result = collect(case, raw, code, oversized=oversized)\n        except (OSError, ValueError, subprocess.SubprocessError):\n            result = {"case": case, "state": "unavailable", "reason":8, "facts": [], "native_finish": []}\n        failed |= result["state"] != "passed"\n        if name.startswith(CALIBRATION):\n            calibration_passed &= result["state"] == "passed"\n', '            raw, code, oversized = capture(native_command(name), env)\n            result = collect(case, raw, code, oversized=oversized)\n        except (OSError, ValueError, subprocess.SubprocessError):\n            result = {"case": case, "state": "unavailable", "reason":8, "facts": []}\n        failed |= result["state"] != "passed"\n        if name.startswith(CALIBRATION):\n            calibration_passed &= result["state"] == "passed"\n'))


PRIVATE_PAIR_HELPER_START = "\ndef _prep_timer_pair_control_passes("
PRIVATE_PAIR_HELPER_END = "\ndef wiring_controls("
PRIVATE_PAIR_HELPER_SHA256 = "e2e5e6c31dbed416b76f201baa6cb5da453459ed781598851f64875c17728057"
PRIVATE_PAIR_CONTROL_AFTER = (
    '            else:\n'
    '                valid_red = _prep_timer_pair_control_passes(raw, code, oversized=oversized)\n'
)
PRIVATE_PAIR_CONTROL_BEFORE = (
    '            else:\n'
    '                # Repair only the deliberately wrong terminal in captured\n'
    '                # evidence; every other grammar/summary/bound must then pass.\n'
    '                wrong = b" terminal=3"\n'
    '                valid_red = (code == 0 and raw.count(wrong) == 1 and result["reason"] == 4\n'
    '                             and collect("prep_timer", raw.replace(wrong, b" terminal=1"), code,\n'
    '                                         oversized=oversized)["state"] == "passed")\n'
)


def inverse_private_pair_control(source):
    if source.count(PRIVATE_PAIR_HELPER_START) != 1 or source.count(PRIVATE_PAIR_HELPER_END) != 1:
        raise ValueError("private paired-control helper registration mismatch")
    start = source.index(PRIVATE_PAIR_HELPER_START)
    end = source.index(PRIVATE_PAIR_HELPER_END, start)
    helper = source[start:end]
    if hashlib.sha256(helper.encode()).hexdigest() != PRIVATE_PAIR_HELPER_SHA256:
        raise ValueError("private paired-control helper digest mismatch")
    source = source[:start] + source[end:]
    if source.count(PRIVATE_PAIR_CONTROL_AFTER) != 1:
        raise ValueError("private paired-control branch mismatch")
    return source.replace(PRIVATE_PAIR_CONTROL_AFTER, PRIVATE_PAIR_CONTROL_BEFORE, 1)


# Diagnostic-only assertion/selector delta: remove exactly the reviewed bytes,
# then retain all historical paired-control, native-finish and U1 inverses.
PRIVATE_ASSERTION_HELPER_SHA256 = 'a5be529565662e407ecba943696268adcc0b4b6f935f7c7f04132515f47b4be5'
PRIVATE_ASSERTION_INVERSE = (('    if bad and case == "head_finalizer" and block_start >= 0:\n        result["assertions"] = _head_finalizer_assertion(lines, block_start, block_end)\n    for index, line in enumerate(lines):\n        # Prefix embedded', '    for index, line in enumerate(lines):\n        # Prefix embedded'), ('        calibration = (root / "runtime/src/http_server/private_deadline_calibration.rs").read_text()\n        if hashlib.sha256(private.encode()).hexdigest() != (\n                "46c1ec095b7cf7e3879ce5a0cde4511192d7550ac0d31e5ec3747b63f3012cd1"):\n            return False\n', '        calibration = (root / "runtime/src/http_server/private_deadline_calibration.rs").read_text()\n'), ('    focused = sys.argv[1:] == ["--run-case", "head_finalizer"]\n    if not focused and sys.argv[1:] != ["--run"]:\n', '    if sys.argv[1:] != ["--run"]:\n'), ('    selected = {"head_finalizer": CASES["head_finalizer"]} if focused else CASES\n    for case, (name, _) in selected.items():\n', '    for case, (name, _) in CASES.items():\n'), ('    if focused:\n        return int(failed)\n    if calibration_passed:\n        try:\n', '    if calibration_passed:\n        try:\n'))


def inverse_private_assertion(source):
    start_marker = "\ndef _head_finalizer_assertion("
    end_marker = "\ndef collect("
    if source.count(start_marker) != 1 or source.count(end_marker) != 1:
        raise ValueError("private assertion helper registration mismatch")
    start, end = source.index(start_marker), source.index(end_marker)
    if hashlib.sha256(source[start:end].encode()).hexdigest() != PRIVATE_ASSERTION_HELPER_SHA256:
        raise ValueError("private assertion helper digest mismatch")
    source = source[:start] + source[end:]
    for added, inherited in PRIVATE_ASSERTION_INVERSE:
        if source.count(added) != 1:
            raise ValueError("private assertion inverse mismatch")
        source = source.replace(added, inherited, 1)
    return source


def inverse_collector(source):
    # The reviewed Session fixture separates deferred SQL wait from mapper
    # finalization. Rebind only its exact source and relocated HEAD assertion;
    # keep the historical helper digest and all other controls unchanged.
    current_session = '8c7bdabff12f8b9e08c2ea9eae0ac3e05bf3c5f0afaf0e31e875d6a1488d2821'
    current_assertions = 'assertions = {282: 1, 283: 2, 284: 3, 285: 4, 495: 5}'
    if source.count(current_session) != 1 or source.count(current_assertions) != 1:
        raise ValueError('Session fixture collector binding mismatch')
    source = source.replace(current_session, '46c1ec095b7cf7e3879ce5a0cde4511192d7550ac0d31e5ec3747b63f3012cd1')
    source = source.replace(current_assertions, 'assertions = {282: 1, 283: 2, 284: 3, 285: 4, 424: 5}')
    source = inverse_private_assertion(source)
    source = inverse_private_pair_control(source)
    for added, inherited in SESSION_NATIVE_COLLECTOR_INVERSE:
        if source.count(added) != 1:
            raise ValueError('Session native finish collector inverse mismatch')
        source = source.replace(added, inherited, 1)
    if not HTTP_AFTER or source.count(HTTP_AFTER) != 1:
        raise ValueError('U1 collector binding mismatch')
    return source.replace(HTTP_AFTER, HTTP_BEFORE)


def digest(source):
    return hashlib.sha256(norm(source).encode()).hexdigest()


# Only still-unattached constructors/reserve and their catalog-only fields are
# test-only during U1. Quota/Reservation/Drop remain production. U4 must restore
# the full quota core and recheck the actual production layout before admission.
QUOTA_CFG_ADDITIONS = (
    ('use std::mem::size_of;', '#[cfg(test)]\nuse std::mem::size_of;'),
    ('const MAX_CATALOG_ITEMS:', '#[cfg(test)]\nconst MAX_CATALOG_ITEMS:'),
    ('pub(crate) struct ResourceLimit', '#[cfg(test)]\npub(crate) struct ResourceLimit'),
    ('impl ResourceLimit {', '#[cfg(test)]\nimpl ResourceLimit {'),
    ('impl Claim {', '#[cfg(test)]\nimpl Claim {'),
    ('pub(crate) enum StartupError', '#[cfg(test)]\npub(crate) enum StartupError'),
    ('pub(crate) enum QuotaError', '#[cfg(test)]\npub(crate) enum QuotaError'),
    ('struct ResourceState {\n    capacity:', 'struct ResourceState {\n    #[cfg(test)]\n    capacity:'),
    ('impl Quota {', '#[cfg(test)]\nimpl Quota {'),
    ('fn checked_catalog_bytes(', '#[cfg(test)]\nfn checked_catalog_bytes('),
    ('pub(crate) fn try_reserve(', '#[cfg(test)]\npub(crate) fn try_reserve('),
)


def inverse_quota_owned_state_lock(source):
    alias = 'use super::OwnedStateLock as Mutex;\n'
    current_arc = 'use std::sync::Arc;\n'
    previous = 'use std::sync::{Arc, Mutex};\n'
    if source.count(alias) != 1 or source.count(current_arc) != 1 or previous in source:
        raise ValueError('owned-state quota import inverse mismatch')
    return source.replace(alias, '', 1).replace(current_arc, previous, 1)


def inverse_quota(source):
    source = inverse_quota_owned_state_lock(source)
    production_doc = ('//! The production HTTP producer uses the private retained catalogue and atomic\n'
                      '//! reservation path. No public Nagi quota API is exposed.')
    # U2 extends the existing owner with a retained-catalog receipt. Remove
    # only that independently reviewed exact block, never arbitrary new code.
    start = '// SF07_U2_RETAINED_CATALOG_BEGIN'
    end = '// SF07_U2_RETAINED_CATALOG_END\n\n'
    if start in source or '// SF07_U2_RETAINED_CATALOG_END' in source:
        if source.count(start) != 1 or source.count(end) != 1:
            raise ValueError('U2 quota extension marker mismatch')
        left = source.index(start)
        right = source.index(end) + len(end)
        block = source[left:right]
        expected_block = ('577790bd4be8d2cfdfdc90526c834e7a3006538d8687eb9c5fa63f216ba62828'
                          if production_doc in source else
                          '33a82908cee4b72230b2d5ac1c5b966ba010f673e33562c4092d2b49cc0d39a3')
        if hashlib.sha256(block.encode()).hexdigest() != expected_block:
            raise ValueError('U2 quota extension digest mismatch')
        source = source[:left] + source[right:]
    if production_doc in source:
        # Full U4 restoration is one exact profile, not a mixed set of cfgs.
        for before, after in QUOTA_CFG_ADDITIONS:
            if after in source or source.count(before) != 1:
                raise ValueError('U4 quota production restoration mismatch')
        source = source.replace(production_doc,
            '//! This file is deliberately unregistered and exposes no public Nagi API.', 1)
    else:
        for before, after in QUOTA_CFG_ADDITIONS:
            if source.count(after) != 1:
                raise ValueError('U1 quota cfg inverse mismatch')
            source = source.replace(after, before)
    return source
