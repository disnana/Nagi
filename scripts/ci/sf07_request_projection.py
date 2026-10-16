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


def inverse_workflow(source):
    if source.count(SOURCE_LINE) != 2 or source.count(NATIVE_LINE) != 2:
        raise ValueError('U1 workflow registration mismatch')
    if source.count(RATE_SOURCE_LINE) != 2 or source.count(RATE_NATIVE_LINE) != 2:
        raise ValueError('U3 workflow registration mismatch')
    if source.count(TESTS_AFTER) != 2:
        raise ValueError('U1 native fixture binding mismatch')
    return source.replace(SOURCE_LINE, '').replace(NATIVE_LINE, '').replace(RATE_SOURCE_LINE, '').replace(RATE_NATIVE_LINE, '').replace(HTTP_AFTER, HTTP_BEFORE).replace(TESTS_AFTER, TESTS_BEFORE)


def inverse_collector(source):
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


def inverse_quota(source):
    for before, after in QUOTA_CFG_ADDITIONS:
        if source.count(after) != 1:
            raise ValueError('U1 quota cfg inverse mismatch')
        source = source.replace(after, before)
    return source
