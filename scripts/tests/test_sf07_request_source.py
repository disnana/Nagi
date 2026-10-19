"""U1 exact inverses/registration controls, never native lifetime evidence."""
import ast
import hashlib
import importlib.util
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('request_projection', ROOT/'scripts/ci/sf07_request_projection.py')
projection = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(projection)


def read(path):
    raw = (ROOT/path).read_bytes().replace(b'\r\n', b'\n')
    if b'\r' in raw:
        raise ValueError('noncanonical source')
    return raw.decode('utf-8', errors='strict')


def function(source, name):
    node = next(n for n in ast.parse(source).body if isinstance(n, ast.FunctionDef) and n.name == name)
    return ast.dump(node, include_attributes=False)


class RequestSourceTests(unittest.TestCase):
    def test_every_native_send_fixture_binding_matches_actual_source(self):
        workflow = read('.github/workflows/ci.yml')
        paths = (
            'runtime/src/http_server.rs',
            'runtime/src/http_server/tests.rs',
            'runtime/src/http_server/send_deadline_tests.rs',
            'runtime/src/http_server/windows_native_fixture.rs',
            'runtime/src/http_server/transport_lifecycle_tests.rs',
        )
        for path in paths:
            digests = re.findall(re.escape("'" + path + "': '") + r'([0-9a-f]{64})' + "'", workflow)
            self.assertEqual(len(digests), 2, path)
            self.assertEqual(digests, [hashlib.sha256(read(path).encode()).hexdigest()] * 2, path)

    def test_actual_native_send_source_guards_keep_lf_crlf_and_reject_each_mutation(self):
        import contextlib
        import io
        import textwrap
        from unittest.mock import patch
        workflow = read('.github/workflows/ci.yml')
        fragments = workflow.split("python - <<'PYSOURCE'\n")[1:]
        guards = [textwrap.dedent(fragment.split('          PYSOURCE', 1)[0]) for fragment in fragments]
        self.assertEqual(len(guards), 2)
        self.assertEqual(guards[0], guards[1])
        paths = ('runtime/src/http_server.rs', 'runtime/src/http_server/tests.rs',
                 'runtime/src/http_server/send_deadline_tests.rs',
                 'runtime/src/http_server/windows_native_fixture.rs',
                 'runtime/src/http_server/transport_lifecycle_tests.rs')
        canonical = {path: read(path).encode() for path in paths}
        code = compile(guards[0], '<native-source-guard>', 'exec')
        def execute(files):
            def value(path):
                return files[path.as_posix()]
            with patch.object(Path, 'read_bytes', value), contextlib.redirect_stdout(io.StringIO()):
                exec(code, {})
        execute(canonical)
        execute({path: raw.replace(b'\n', b'\r\n') for path, raw in canonical.items()})
        for path in paths:
            for changed in (canonical[path] + b' ', canonical[path] + b'\r'):
                with self.assertRaises(AssertionError):
                    execute({**canonical, path: changed})

    def test_reviewed_public_production_profile_is_distinct_from_historical_u1(self):
        for name, path in (
            ('http', 'runtime/src/http_server.rs'),
            ('browser', 'runtime/src/http_server/browser_login.rs'),
            ('stage', 'runtime/src/http_server/browser_login_stage_tests.rs'),
            ('csrf', 'runtime/src/http_server/csrf_http.rs'),
        ):
            self.assertEqual(projection.digest(projection.approved_production(name, read(path))),
                             projection.PRODUCTION_SOURCES[name]['tokens'], name)

    def test_production_owner_extraction_keeps_quota_single_and_fixture_cfgs(self):
        lib = read('runtime/src/lib.rs')
        self.assertIn('mod security_sf07_private;', lib)
        self.assertNotIn('#[cfg(test)]\nmod security_sf07_private;', lib)
        modules = read('runtime/src/security_sf07_private/mod.rs')
        calibration = '#[cfg(test)]\npub(crate) fn calibrate_allocation_meter()'
        self.assertEqual(modules.count(calibration), 1)
        modules = modules.replace(calibration, '#[cfg(test)]\nfn calibrate_allocation_meter()', 1)
        for name in ('indexed', 'startup_rate'):
            declaration = 'pub(crate) mod ' + name + ';'
            self.assertEqual(modules.count(declaration), 1)
            self.assertNotIn('#[cfg(test)]\n' + declaration, modules)
            modules = modules.replace(declaration, '#[cfg(test)]\nmod ' + name + ';', 1)
        self.assertIn('#[cfg(test)]\nmod legacy_rate;', modules)
        self.assertEqual(hashlib.sha256(modules.encode()).hexdigest(),
                         '585dc4c6335c70bfae2293a28c682d89fc868a191bb7ae908118a8c4d801af6a')
        for name in ('allocations_in', 'calibrate_allocation_meter'):
            self.assertRegex(modules, r'#\[cfg\(test\)\]\s*fn ' + name)
        self.assertEqual(modules.count('pub(crate) mod quota;'), 1)
        quota = read('runtime/src/security_sf07_private/quota.rs')
        self.assertEqual(hashlib.sha256(projection.inverse_quota(quota).encode()).hexdigest(), 'a7031c5a4ad071d719f6efc41dd8889d9582169602197a337c94a54cbc4f6861')
        http = read('runtime/src/http_server.rs')
        for name in ('response_budget', 'response_send'):
            self.assertEqual(http.count('mod ' + name + ';'), 1)
            self.assertNotIn('#[cfg(test)]\nmod ' + name + ';', http)
        self.assertIn('mod request_headers;', http)
        self.assertIn('ResponseContext', http)
        public = read('runtime/src/http_server/response_public.rs')
        self.assertIn('pub fn response_context(request: &Request) -> &ResponseContext', public)
        self.assertNotIn('impl Default for ResponseContext', public)

    def test_same_map_and_prior_claim_drop_together_without_clone_or_copy_on_write(self):
        owner = read('runtime/src/http_server/request_headers.rs')
        fields = owner.split('struct ReceivedHeaders', 1)[1].split('impl ReceivedHeaders', 1)[0]
        self.assertLess(fields.index('map: HeaderMap'), fields.index('_prior: Option<Arc<Reservation>>'))
        self.assertIn('_extensions: extensions', owner)
        self.assertIn('_prior: prior', owner)
        self.assertIn('context: ContextOwner::Attached(context)', owner)
        self.assertLess(fields.index('_extensions: Extensions'), fields.index('_prior:'))
        self.assertLess(fields.index('context: ContextOwner'), fields.index('request_owner:'))
        self.assertIn('Arc::clone(&self.owner)', owner)
        self.assertIn('visibility: self.visibility', owner)
        self.assertNotIn('Arc::make_mut', owner)
        self.assertNotRegex(owner, r'(?:headers|map)\.clone\(')
        self.assertNotRegex(owner, r'impl Clone for (?:ReceivedHeaders|RequestHeaders)')
        send = read('runtime/src/http_server/response_send.rs')
        self.assertIn('pub(super) use super::request_headers::ReceivedHeaders;', send)
        self.assertIn('ReceivedHeaders::new_native(parts.headers, parts.extensions, prior)', send)
        self.assertNotIn('struct ReceivedHeaders', send)

    def test_production_native_owner_and_finite_admission_are_not_optional(self):
        http = read('runtime/src/http_server.rs')
        constructor = http.split('impl<B> IncomingRequest<B>', 1)[1]
        self.assertIn('ReceivedHeaders::configured_native(', constructor)
        self.assertIn('parts.extensions', constructor)
        self.assertIn('request.parts.headers.admit(permit)', http)
        self.assertLess(http.index('request.parts.headers.admit(permit)'), http.index('if let Err(reason)=rate.route()'))
        public = read('runtime/src/http_server/response_public.rs')
        self.assertIn('explicit response limits are required', public)
        for name, path in (('http', 'runtime/src/http_server.rs'), ('browser', 'runtime/src/http_server/browser_login.rs'), ('csrf', 'runtime/src/http_server/csrf_http.rs')):
            original = read(path)
            with self.assertRaises(ValueError):
                projection.approved_production(name, original + ' ')
            with self.assertRaises(ValueError):
                projection.approved_production(name, original.replace('Request', 'UncheckedRequest', 1))

    def test_visibility_is_per_request_and_all_public_getters_keep_validation(self):
        owner = read('runtime/src/http_server/request_headers.rs')
        for token in ('name.eq_ignore_ascii_case("cookie")', 'name.eq_ignore_ascii_case("x-csrf-token")',
                      'self.visibility.permits(name)', 'self.owner.map.get_all(name).iter().count()',
                      'Arc::get_mut(&mut self.owner)', 'owner._prior.is_none()'):
            self.assertIn(token, owner)
        http = read('runtime/src/http_server.rs')
        for name in ('headers', 'header'):
            start = http.index('pub fn ' + name + '<')
            self.assertIn('validate_lookup_name(name)?', http[start:http.index('\n}', start)])
        self.assertIn('.field("header_count", &self.headers.len())', http)
        self.assertIn('headers: self.headers.share()', http)
        self.assertIn('head.headers.sanitize()', http)
        self.assertNotIn('parts.headers.clone()', http)
        self.assertNotIn('self.headers.clone()', http)
        self.assertNotIn('parts.headers.clone()', read('runtime/src/http_server/csrf_http.rs'))

    def test_eight_new_request_oracles_preserve_current_wire_inventory(self):
        native = read('runtime/src/http_server/request_headers_tests.rs')
        names = re.findall(r'#\[(?:tokio::)?test\]\s*(?:async )?fn (\w+)\(', native)
        self.assertEqual(len(names), 8)
        summary_path = ROOT/'scripts/ci/sf07_checkpoint_summary.py'
        spec = importlib.util.spec_from_file_location('summary', summary_path)
        summary = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(summary)
        self.assertEqual(set(summary.EXPECTED['request']), {'http_server::request_headers::tests::' + n for n in names})
        self.assertEqual(tuple(len(summary.EXPECTED[n]) for n in ('wire', 'quota', 'transport')), (40, 9, 9))
        self.assertEqual(len(re.findall(r'#\[tokio::test\]\s*async fn', read('runtime/src/http_server/response_send_tests.rs'))), 9)
        import json
        inherited = {k: list(summary.EXPECTED[k]) for k in ('wire', 'quota', 'transport', 'request', 'rate-startup')}
        inherited['wire'] = inherited['wire'][:22]
        inherited['transport'] = inherited['transport'][:8]
        self.assertEqual(hashlib.sha256(json.dumps(inherited, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
                         '91fc28d8e6cff5efe7f56b95eae0b838766c1835eee0f5fbb0d34f3a65a71552')
        self.assertEqual(hashlib.sha256(read('runtime/src/http_server/response_send_tests.rs')[:19665].encode()).hexdigest(),
                         '0b35d16393447d1c77bf23399bed29a3e9bf0bdd40f8f1816be0bd392a807587')
        for token in ('same_owner', 'BodyOwner', 'header_text', 'header_only_last_shared_owner',
                      'catch_unwind', 'task.abort()', 'is_cancelled()', 'ExtensionProbe',
                      'dispatch(', 'Connection: close', 'mode == 1', 'mode == 2'):
            self.assertIn(token, native)
        self.assertNotIn('#[ignore', native)
        for token in ('runtime.security_snapshot()', 'drop(snapshot)', 'capacity(&ordinary)'):
            self.assertIn(token, read('runtime/src/http_server/response_send_tests.rs'))

    def test_required_registration_and_all_three_actual_http_digests(self):
        workflow = read('.github/workflows/ci.yml')
        self.assertEqual(workflow.count(projection.SOURCE_LINE), 2)
        self.assertEqual(workflow.count(projection.NATIVE_LINE), 2)
        self.assertEqual(hashlib.sha256(read('runtime/src/http_server.rs').encode()).hexdigest(), projection.HTTP_AFTER)
        self.assertEqual(workflow.count(projection.HTTP_AFTER), 2)
        self.assertEqual(read('scripts/ci/session_private_deadline_diagnostic.py').count(projection.HTTP_AFTER), 1)
        self.assertIn('projection.approved_production', read('scripts/tests/test_browser_login_stage_backport.py'))
        self.assertEqual(hashlib.sha256(projection.inverse_workflow(workflow).encode()).hexdigest(), BASE_WORKFLOW_SHA)
        self.assertEqual(hashlib.sha256(projection.inverse_collector(read('scripts/ci/session_private_deadline_diagnostic.py')).encode()).hexdigest(), BASE_PRIVATE_COLLECTOR_SHA)

    def test_semantic_and_diagnostic_mutations_cannot_hide_in_inverse(self):
        for name, path, inverse, before, after in (
            ('http', 'runtime/src/http_server.rs', projection.inverse_http, 'headers: self.headers.share()', 'headers: self.headers.clone()'),
            ('http', 'runtime/src/http_server.rs', projection.inverse_http, 'PreparationTimer', 'HandlerTimer'),
            ('browser', 'runtime/src/http_server/browser_login.rs', projection.inverse_browser, 'headers.sanitize()', 'let _ = &headers'),
            ('browser', 'runtime/src/http_server/browser_login.rs', projection.inverse_browser, 'Exit::PrepareTimer', 'Exit::PrepareLateReady'),
            ('csrf', 'runtime/src/http_server/csrf_http.rs', projection.inverse_csrf, 'headers.sanitize()', 'let _ = &headers'),
            ('csrf', 'runtime/src/http_server/csrf_http.rs', projection.inverse_csrf, 'store.checkpoint(', 'store.checkpoint_changed('),
        ):
            source = read(path)
            self.assertIn(before, source)
            changed = source.replace(before, after, 1)
            try:
                actual = projection.digest(projection.approved_production(name, changed))
            except ValueError:
                continue
            self.assertNotEqual(actual, projection.PRODUCTION_SOURCES[name]['tokens'])

    def test_u2_workflow_inverse_is_exact_and_keeps_unrelated_changes_visible(self):
        workflow = read('.github/workflows/ci.yml')
        self.assertEqual(hashlib.sha256(projection.inverse_workflow(workflow).encode()).hexdigest(), BASE_WORKFLOW_SHA)
        for line in projection.U2_NATIVE_LINES:
            for changed in (workflow.replace(line, '', 1), workflow.replace(line, line + line, 1)):
                with self.assertRaises(ValueError):
                    projection.inverse_workflow(changed)
        with self.assertRaises(ValueError):
            projection.inverse_workflow(workflow.replace('"sf03-b1-csrf-core.log":15', '"sf03-b1-csrf-core.log":16', 1))
        self.assertIn('set -euo pipefail', workflow)
        changed = workflow.replace('set -euo pipefail', 'set -eu pipefail', 1)
        self.assertNotEqual(hashlib.sha256(projection.inverse_workflow(changed).encode()).hexdigest(), BASE_WORKFLOW_SHA)

    def test_session_wait_inventory_is_exact_without_replacing_base_digest(self):
        workflow = read('.github/workflows/ci.yml')
        entry = "'sf02-http-bridge.log': 7"
        self.assertEqual(workflow.count(entry), 2)
        self.assertEqual(hashlib.sha256(projection.inverse_workflow(workflow).encode()).hexdigest(), BASE_WORKFLOW_SHA)
        for replacement in ('', entry + entry, "'sf02-http-bridge.log': 6", "'sf02-http-bridge.log': 8"):
            with self.subTest(replacement=replacement), self.assertRaises(ValueError):
                projection.inverse_workflow(workflow.replace(entry, replacement, 1))
        self.assertEqual(read('runtime/src/auth/session/http_bridge_tests.rs').count(
            'async fn session_deferred_touch_wait_spends_handler_budget_before_business_error()'), 1)

    def test_request_native_mode_fakes_are_exact_bounded_and_failure_closed(self):
        spec = importlib.util.spec_from_file_location('request_summary', ROOT/'scripts/ci/sf07_checkpoint_summary.py')
        summary = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(summary)
        names = summary.EXPECTED['request']
        raw = ('running 8 tests\n' + ''.join('test ' + n + ' ... ok\n' for n in names) + 'test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 400 filtered out; finished in 0.1s\n').encode()
        for value in (raw, raw.replace(b'\n', b'\r\n')):
            self.assertEqual(summary.classify(value, 'request'), 'passed')
        for value in (raw.replace(b'8 passed', b'0 passed'), raw.replace(b'0 ignored', b'1 ignored'),
                      raw.replace(names[0].encode(), b'unknown'), raw + b'test unknown ... ok\n',
                      raw + b'\r', raw + b'\xff', b'x' * 65537):
            self.assertEqual(summary.classify(value, 'request'), 'invalid')
        self.assertEqual(summary.classify(b'error[E0382]: private/path\n', 'request'), 'compile_failed')


BASE_WORKFLOW_SHA = '6113a9a6f22f9ffc2189316391cac6b35c024f52e854c65d387c925c835bca79'
BASE_PRIVATE_COLLECTOR_SHA = '1d0dd3c92e07c374de80d4f968c101141eb6290ba2b935bf9c06dcb85ac60913'

class ReleaseProfileWorkflowTests(unittest.TestCase):
    def test_release_profiles_are_exact_once_and_refuse_missing_duplicate_or_changed_steps(self):
        workflow = read('.github/workflows/ci.yml')
        self.assertEqual(len(projection.RELEASE_PROFILE_CI_STEPS), 2)
        for addition in projection.RELEASE_PROFILE_CI_STEPS:
            self.assertEqual(workflow.count(addition), 1)
            for mutated in (workflow.replace(addition, '', 1),
                            workflow.replace(addition, addition + addition, 1),
                            workflow.replace(addition, addition.replace('cargo test --locked', 'cargo test --offline', 1), 1)):
                with self.assertRaises(ValueError):
                    projection.inverse_workflow(mutated)

if __name__ == '__main__':
    unittest.main()
