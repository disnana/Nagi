"""Historical U1 inverses and reviewed U4 production profiles; no native proof."""
import hashlib
import re


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
PRODUCTION_COMMIT = '9dd70dc73b2722fb537edc34452916030670d027'
PRODUCTION_TREE = '726d6c476d7e4a69c2a4ba8ab84dd922eac1d2dc'
PRODUCTION_SOURCES = {'http': {'path': 'runtime/src/http_server.rs',
          'blob': '581551f3f2544b4d59ed806af106996d5795ed54',
          'sha256': 'e4af47720616a8f6818a47d2cb35408004cf35f3be1f4e6f71567e6318efca31',
          'tokens': '4dea895e5e943d98cecd93c2f1e28b0c348c0b6581364ff6fa8c885cc5b7a12e'},
 'browser': {'path': 'runtime/src/http_server/browser_login.rs',
             'blob': '41878db9c1e5cf78d4a8c682f7f7d53f8ddfe557',
             'sha256': '17252effa839788999212f90b1418bdba27173ef3c230ee4223311ebd4ce64b6',
             'tokens': '071429be0bc17b652cbff8589312ea8104f367e14ba390c3de0eec94f9a2bc01'},
 'stage': {'path': 'runtime/src/http_server/browser_login_stage_tests.rs',
           'blob': '6d57cdbf3cff4051b061c3f71bddb6827ad4e3a5',
           'sha256': '220975a888800934f209be4ec4a96449e32f3cedef90119e517d6c01b2dcb3a3',
           'tokens': 'a13eac9ab415613067261d9927594ce023d421bcd4518d6feb76f0819f6d8164'},
 'csrf': {'path': 'runtime/src/http_server/csrf_http.rs',
          'blob': '93095cab1709b4753237c0e8a8705f6bfa1a64a8',
          'sha256': '4e97c5c0268ad6caa49df5a4d5e2cb6e9b4e7cdc5c74e7d900b0439b1ff0a6e1',
          'tokens': 'aa7607f96728acb1a74c964a441c97799d68409027e999c09c596838ce45d533'},
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
HTTP_AFTER = 'e4af47720616a8f6818a47d2cb35408004cf35f3be1f4e6f71567e6318efca31'
TESTS_BEFORE = '8752ae42a02af282b9a5d79f11e6e1bc1c95642fb1c1b4f4a23da34012130173'
HISTORICAL_TESTS_AFTER = '640f64c8c44fb085d04c445c14cbd7c4c46ce07ab7353351a195260a2740c64f'
TESTS_AFTER = '4c744e65f03518ab7f2a871c0b001d6a4556f6d1efefe7633c287cd5d620b3e8'
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


RELEASE_PROFILE_CI_STEPS = ('      - name: Measure bounded SF07 response and rate profile\n        shell: bash\n        run: |\n          set -euo pipefail\n          python -B scripts/tests/test_sf07_profile_summary.py\n          cargo test --locked -p nagi-runtime --test sf07_small_profile --no-run\n          cargo test --locked -p nagi-runtime --test sf07_small_profile -- --list > "$RUNNER_TEMP/sf07-profile-list.log"\n          python - <<\'PYSF07LIST\'\n          import os,pathlib,re\n          text=(pathlib.Path(os.environ["RUNNER_TEMP"])/"sf07-profile-list.log").read_text()\n          if re.findall(r"(?m)^([a-z0-9_]+): test$",text) != ["response_profile_reports_closed_measurements"]:\n              raise SystemExit("sf07_profile_inventory=invalid")\n          PYSF07LIST\n          set +e\n          cargo test --locked -p nagi-runtime --test sf07_small_profile response_profile_reports_closed_measurements -- --exact --show-output > "$RUNNER_TEMP/sf07-profile-native.log" 2>&1\n          profile_rc=$?\n          set -e\n          python -B scripts/ci/sf07_profile_summary.py --cargo-exit-code "$profile_rc" < "$RUNNER_TEMP/sf07-profile-native.log"\n', '      - name: Validate production SF06 with isolated owned DNS and TLS\n        shell: bash\n        run: |\n          set -euo pipefail\n          python -B scripts/tests/test_sf06_owned_network.py\n          NAGI_SF06_NAMESPACE_PROBE=1 python -B scripts/tests/test_sf06_owned_network.py HostedLifecycleProbe.test_namespace_eof_reaps_setsid_descendant\n          set +e\n          cargo test --locked -p nagi-runtime --test http_client_production_profile --no-run --message-format=json > "$RUNNER_TEMP/sf06-profile-build.jsonl"\n          compile_rc=$?\n          set -e\n          binary=$(SF06_COMPILE_RC="$compile_rc" python - <<\'PYSF06BUILD\'\n          import json,os,pathlib,sys\n          rows=[json.loads(line) for line in (pathlib.Path(os.environ["RUNNER_TEMP"])/"sf06-profile-build.jsonl").read_text().splitlines() if line.strip()]\n          if os.environ["SF06_COMPILE_RC"] != "0":\n              for row in rows:\n                  message=row.get("message",{})\n                  if row.get("reason")=="compiler-message" and message.get("level")=="error":\n                      print("sf06_profile_compile_error="+json.dumps({"code":message.get("code"),"message":message.get("message","")[:400]}),file=sys.stderr)\n              raise SystemExit(1)\n          paths=[row["executable"] for row in rows if row.get("reason")=="compiler-artifact" and row.get("target",{}).get("name")=="http_client_production_profile" and row.get("executable")]\n          if len(paths)!=1 or "\\n" in paths[0] or "\\r" in paths[0]:\n              raise SystemExit("sf06_profile_binary_inventory=invalid")\n          print(paths[0])\n          PYSF06BUILD\n          )\n          python -B scripts/ci/sf06_owned_network.py --binary "$binary" --source-sha "$GITHUB_SHA"\n')


def inverse_workflow(source):
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
PRIVATE_PAIR_HELPER_SHA256 = "ffc45286d83222cd1f349b47ab337987e6a9871acd0bdb23594b14c0084c14a2"
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
