"""Exact inverse of U1's narrow ownership changes, never a runtime proof."""
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


# Frozen from exact9b7 base bytes before U1, not adjusted to the candidate.
BASE_TOKENS = {'http': '8efe1130c731238099553c0e9fc58a19ff103fc57278c55c7ec8839de26d1f3e', 'browser': '2b61c339bfeecc0519ccf72517cc64346f40b6c51d2bab946004f2f6f2a71dab', 'stage': '8b5e8f0c3ab595d8fa6b93d2c4c400ef670f64933c576db994315c620bdcced4', 'csrf': '21ae359e321f254d831f0c29b43392f2b8490e8c14ccddaef733f58c8b7b95d3'}
HTTP_BEFORE = 'e6969a04326ed5fc4b3795f64a2909ffb5bbfae16f679589b93a819922084485'
HTTP_AFTER = '3fa9bf11de875398966537bb275057c6bc23ffffb888ed3bddbe0ea2ff8e25f6'
TESTS_BEFORE = '8752ae42a02af282b9a5d79f11e6e1bc1c95642fb1c1b4f4a23da34012130173'
TESTS_AFTER = '640f64c8c44fb085d04c445c14cbd7c4c46ce07ab7353351a195260a2740c64f'
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


OWNED_STATE_CI_ADDITIONS = ('          python -B scripts/tests/test_sf07_owned_state_summary.py\n', '          cargo test --locked -p nagi-runtime --lib security_sf07_private::owned_state_lock::tests:: 2>&1 | python -B scripts/ci/sf07_owned_state_summary.py lock\n', '          cargo test --locked -p nagi-runtime --lib security_sf07_private::owned_state_owner_tests:: 2>&1 | python -B scripts/ci/sf07_owned_state_summary.py owners\n', '          cargo test --locked -p nagi-runtime --lib http_server::response_send::owned_state_lock_tests:: 2>&1 | python -B scripts/ci/sf07_owned_state_summary.py send\n')


def inverse_workflow(source):
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


def inverse_collector(source):
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
        if hashlib.sha256(block.encode()).hexdigest() != '33a82908cee4b72230b2d5ac1c5b966ba010f673e33562c4092d2b49cc0d39a3':
            raise ValueError('U2 quota extension digest mismatch')
        source = source[:left] + source[right:]
    for before, after in QUOTA_CFG_ADDITIONS:
        if source.count(after) != 1:
            raise ValueError('U1 quota cfg inverse mismatch')
        source = source.replace(after, before)
    return source
