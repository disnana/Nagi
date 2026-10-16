"""B1/CORS5 source inverses and bounded native9 CI summary; no native proof."""
import hashlib
from pathlib import Path
import re
import sys
import unittest
ROOT = Path(__file__).resolve().parents[2]

EXPECTED = ('http_server::browser_login::stage_tests::live_claimed_200_runs_verifier_and_handler_and_returns_200', 'http_server::browser_login::stage_tests::consumed_prepare_err_1000_returns_401_without_cookie', 'http_server::browser_login::stage_tests::consumed_prepare_panic_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::consumed_prepare_pending_200_returns_504_and_drops_once', 'http_server::browser_login::stage_tests::consumed_prepare_non_yielding_200_late_ready_returns_504', 'http_server::browser_login::stage_tests::committed_issue_live_200_delivers_auth_cookie_once', 'http_server::browser_login::stage_tests::committed_issue_handler_err_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::committed_issue_handler_panic_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::committed_issue_handler_pending_200_returns_504_and_drops_once')


def norm(s):
    # Compare all string literals and ordered control tokens; ignore formatting,
    # comments and trailing commas only, as in the parent projection.
    tokens = re.findall(r'"(?:\\.|[^"\\])*"|//[^\n]*|/\*.*?\*/|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]', s, re.S)
    return ' '.join(t for t in tokens if not t.startswith('//') and not t.startswith('/*') and t != ',')


def read(path):
    raw = (ROOT / path).read_bytes().replace(b"\r\n", b"\n")
    if b"\r" in raw:
        raise ValueError("noncanonical source")
    return raw.decode("utf-8", errors="strict")


def digest(source):
    return hashlib.sha256(norm(source).encode()).hexdigest()


def rewrite(source, before, after):
    before, after = norm(before), norm(after)
    if source.count(before) != 1:
        raise ValueError("inverse anchor mismatch")
    return source.replace(before, after, 1)


def inverse_dispatch(source):
    """Undo only the parent's ownership repair; retain EVERY child diag token."""
    source = norm(source)
    source = rewrite(source, '''#[cfg_attr(test, allow(clippy::large_enum_variant,
        reason = "Test-only continuation must not allocate the stock borrowed wire input."))]''', '')
    source = rewrite(source, """enum Input<'a, B> { Wire {
        body: &'a mut Option<B>, parts: &'a mut axum::http::request::Parts,
    }""", """enum Input<B> { Wire { parts: axum::http::request::Parts, body: B, }""")
    a = source.index(norm('pub(super) async fn dispatch_browser'))
    b = source.index(norm('async fn dispatch_owned'), a)
    wrapper = source[a:b]
    wrapper = rewrite(wrapper, 'let (mut parts, body) = request.into_parts();', 'let (parts, body) = request.into_parts();')
    wrapper = rewrite(wrapper, 'let mut body = Some(body);', '')
    wrapper = rewrite(wrapper, 'Input::Wire { parts: &mut parts, body: &mut body }', 'Input::Wire { parts, body }')
    wrapper = rewrite(wrapper, 'options, &owner, admitted', 'options, owner, admitted')
    core = source[b:]
    core = rewrite(core, "input: Input<'_, B>", 'input: Input<B>')
    core = rewrite(core, 'owner: &HttpOwner', 'owner: HttpOwner')
    core = rewrite(core, 'Input::Wire { parts, body } =>', 'Input::Wire { mut parts, body } =>')
    for field in ('method', 'uri', 'headers'):
        core = rewrite(core, 'std::mem::take(&mut parts.' + field + ')', 'parts.' + field)
    core = rewrite(core, 'body.take().expect("owned wire body present")', 'body')
    core = core.replace(norm('pool, owner'), norm('pool, &owner'))
    core = rewrite(core, 'pool.claim_http(owner, input)', 'pool.claim_http(&owner, input)')
    return source[:a] + wrapper + core


def inverse_stage(source):
    """Undo the new fault witness and borrowed-owner test adapter only."""
    source = norm(source)
    source = rewrite(source, 'fault_entered: AtomicUsize,', '')
    for mode, value, operation in (
        ('PrepareErr', 1, 'return Err(auth::invalid_credential());'),
        ('PreparePanic', 2, 'panic!("owned stage verifier panic");'),
        ('HandlerErr', 3, 'Err(Error::internal("owned stage handler Err"))'),
        ('HandlerPanic', 4, 'panic!("owned stage handler panic");'),
    ):
        old = 'Mode::' + mode + ' => { state.stats.fault_entered.store(' + str(value) + ', Ordering::SeqCst); ' + operation + ' }'
        original = 'Mode::' + mode + ' => ' + operation.rstrip(';')
        # norm omits the comma following the original return arm.
        if mode == 'PrepareErr':
            original += ','
        source = rewrite(source, old, original)
    source = rewrite(source, "input: Input<'static, hyper::body::Incoming>", 'input: Input<hyper::body::Incoming>')
    source = rewrite(source, '''AssertUnwindSafe(async {
        let owner = witness.owner;
        dispatch_owned(&pool, &prepare, Arc::clone(&state), witness.input,
            &options, &owner, admitted).await
    })''', '''AssertUnwindSafe(dispatch_owned(&pool, &prepare, Arc::clone(&state),
        witness.input, &options, witness.owner, admitted))''')
    source = rewrite(source, '''let fault_entered = state.stats.fault_entered.load(Ordering::SeqCst);
    let expected_fault = match mode {
        Mode::PrepareErr => 1, Mode::PreparePanic => 2,
        Mode::HandlerErr => 3, Mode::HandlerPanic => 4, _ => 0,
    };''', '')
    source = rewrite(source, '''assert_eq!(fault_entered, expected_fault,
        "browser_stage_oracle phase=business_fault_entered");''', '')
    return source


def inverse_public(source):
    """Remove exactly two strengthening additions, without projecting out diag."""
    source = norm(source)
    # This exact zero-row assertion is new only in the committed retry504 arm.
    source = rewrite(source, '''assert!(f.stats.handler.load(Ordering::SeqCst) == 0);
        assert!(number(&f.auth, "SELECT count(*) AS n FROM __nagi_session_rows").await == 0);
        s.stop().await;''', '''assert!(f.stats.handler.load(Ordering::SeqCst) == 0); s.stop().await;''')
    source = rewrite(source, '''let h = login_headers(&c, &t);
        let r = request(s.address, "POST", "/login", &h, b"{}").await;
        if r.status == 504 {
            let row_absent = assert_browser_deadline_state(&f, &r).await;
            if row_absent { assert_consumed_browser_replay(&f, &s, &h).await; }
            s.stop().await;''', '''let r = request(s.address, "POST", "/login", &login_headers(&c, &t), b"{}").await;
        if r.status == 504 { assert_browser_deadline_state(&f, &r).await; s.stop().await;''')
    return source


B1_STEP = """      - name: Check owned BrowserLogin business stage and cancellation witnesses
        shell: bash
        run: |
          set -euo pipefail
          python -B scripts/tests/test_browser_login_stage_backport.py
          cargo test --locked -p nagi-runtime --lib http_server::browser_login::stage_tests:: 2>&1 | python -B scripts/tests/test_browser_login_stage_backport.py --native-summary
"""

CHILD_TOKENS = {'dispatch': 'e15ba0fc1b4f45cabc893904d0028d51d2cda49edb8d83e236f3efc5d78e263f', 'stage': '61dbb99d727253ac4050226806d00e6e6807a2e1cfee93cdcd75c6a756c358cc', 'public': '95e8a9f83c93c7bb03ddff48d0d01ef104985b3e5cf5621c4d5369fbef5a2378'}
CHILD_BYTES = {'.github/workflows/ci.yml': 'd6a2753c59ad16e9ab03af39abd09b7e4b30403f52429da4766a7d4c345ae202', 'runtime/src/http_server.rs': 'e6969a04326ed5fc4b3795f64a2909ffb5bbfae16f679589b93a819922084485', 'runtime/src/auth/prelogin/browser.rs': '2428b3e1cdc49dbd922c9b9ce026192972ded7bfa04ae6b82f704d0cc1301f74', 'runtime/src/auth/prelogin/http_bridge.rs': 'd64e052c191e0b8c3853feb07cf5b1e04f0c1d57da2c399099cd7dfc5a031d55', 'scripts/ci/session_private_deadline_diagnostic.py': '1d0dd3c92e07c374de80d4f968c101141eb6290ba2b935bf9c06dcb85ac60913', 'scripts/ci/browser_login_stage_coverage.py': '19ef03623734ac9bbb505b02cab555fa8fd96a09fbc23dcda6f06f691b6ac33a'}

def native_summary(raw):
    if len(raw)>65536: return "invalid"
    raw=raw.replace(b"\r\n",b"\n")
    if b"\r" in raw: return "invalid"
    try: text=raw.decode("utf-8",errors="strict")
    except UnicodeError: return "invalid"
    if re.search(r"(?m)^error(?:\[E[0-9]+\]|:)",text): return "compile_failed"
    if text.splitlines().count("running 9 tests") != 1: return "unavailable"
    summaries=re.findall(r"(?m)^test result: ([^\n]+)$",text)
    if len(summaries)!=1: return "unavailable"
    if summaries[0].startswith("FAILED."): return "test_failed"
    if not re.fullmatch(r"ok\. 9 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s",summaries[0]): return "invalid"
    lines=[line for line in text.splitlines() if line.startswith("test ") and not line.startswith("test result:")]
    rows=re.findall(r"(?m)^test ([^ \n]+) \.\.\. (ok)$",text)
    return "passed" if len(lines)==len(rows)==9 and {name for name,_ in rows}==set(EXPECTED) else "invalid"

class BackportSourceTests(unittest.TestCase):
    def test_inverse_core_retains_child_decisions_and_every_diagnostic(self):
        source = read("runtime/src/http_server/browser_login.rs")
        self.assertEqual(digest(inverse_dispatch(source)), CHILD_TOKENS['dispatch'])
        self.assertEqual(source.count('async fn dispatch_owned'), 1)
        for phase in ('Body', 'Prepare', 'Handler'):
            self.assertIn('observed!(', source)
            for terminal in ('Timer', 'LateReady'):
                self.assertIn('Exit::' + phase + terminal, source)
        self.assertIn('observer.scope(dispatch).await', source)
        self.assertIn('diag::emit(wire.status().as_u16(), owner.was_consumed())', source)

    def test_borrowed_wire_retains_native_remainder_and_actual_owner(self):
        source = read("runtime/src/http_server/browser_login.rs")
        wrapper = source.split('pub(super) async fn dispatch_browser', 1)[1].split('async fn dispatch_owned', 1)[0]
        self.assertLess(wrapper.index('request.into_parts()'), wrapper.index('let mut body = Some(body)'))
        self.assertLess(wrapper.index('let mut body = Some(body)'), wrapper.index('HttpOwner::new(lease)'))
        self.assertIn('parts: &mut parts', wrapper)
        self.assertIn('body: &mut body', wrapper)
        self.assertIn('&owner', wrapper)
        self.assertIn('owner: &HttpOwner', source)
        self.assertEqual(source.count('body.take()'), 1)
        self.assertNotIn('Box::', wrapper)

    def test_stage_inverse_preserves_old_native_oracles_and_cors5(self):
        source = read("runtime/src/http_server/browser_login_stage_tests.rs")
        self.assertEqual(digest(inverse_stage(source)), CHILD_TOKENS['stage'])
        cases = re.findall(r'#\[tokio::test\]\s*async fn (\w+)\(', source)
        self.assertEqual({'http_server::browser_login::stage_tests::' + n for n in cases}, set(EXPECTED))
        self.assertEqual(len(cases), 9)
        for token in ('drop(operation)', 'drop(resume)', 'f.stop().await', 'pending_progress',
                      'verifier_drops', 'handler_drops', 'stage_until < original_until',
                      'session::issue', 'session::apply', 'issue_ack', 'claim_stage_for_test',
                      'fault_entered', 'expected_fault', 'vary::finish(wire)'):
            self.assertIn(token, source)
        self.assertIn('Authorization, Content-Type, Cookie, Origin, X-CSRF-Token', source)
        self.assertNotIn('Cookie, Authorization, Origin', source)

    def test_public_inverse_keeps_all_child_assertions_and_observations(self):
        source = read("runtime/tests/browser_login_public_api.rs")
        self.assertEqual(digest(inverse_public(source)), CHILD_TOKENS['public'])
        for token in ('assert_browser_deadline_state', 'assert_consumed_browser_replay',
                      'handler_calls <= verifier_calls', 'issue_committed',
                      '!retry.auth_cookie() && !retry.deletion()',
                      'Authorization, Content-Type, Cookie, Origin, X-CSRF-Token',
                      'assert!(response_ok)', 'assert!(retry_response_ok)', 'assert!(replay_proof_ok)'):
            self.assertIn(token, source)

    def test_workflow_is_only_two_required_native_steps_and_frozen_sources_remain(self):
        workflow = read('.github/workflows/ci.yml')
        self.assertEqual(workflow.count(B1_STEP), 2)
        self.assertEqual(hashlib.sha256(workflow.replace(B1_STEP, '').encode()).hexdigest(), CHILD_BYTES['.github/workflows/ci.yml'])
        for path, expected in CHILD_BYTES.items():
            if path != '.github/workflows/ci.yml':
                self.assertEqual(hashlib.sha256(read(path).encode()).hexdigest(), expected, path)
        http_sha = CHILD_BYTES['runtime/src/http_server.rs']
        self.assertEqual(workflow.count("'runtime/src/http_server.rs': '" + http_sha + "'"), 2)
        self.assertEqual(read('scripts/ci/session_private_deadline_diagnostic.py').count('expected_digest = "' + http_sha + '"'), 1)

    def test_decision_observer_fault_and_cors_mutations_cannot_hide_in_inverse(self):
        paths = {
            'dispatch': ('runtime/src/http_server/browser_login.rs', inverse_dispatch, (
                ('Status::BAD_REQUEST', 'Status::INTERNAL_SERVER_ERROR'),
                ('Exit::PrepareTimer', 'Exit::PrepareLateReady'),
                ('observer.scope(dispatch).await', 'dispatch.await'),
                ('body.take()', 'body.clone()'),
            )),
            'stage': ('runtime/src/http_server/browser_login_stage_tests.rs', inverse_stage, (
                ('fault_entered.store(3', 'fault_entered.store(0'),
                ('vary::finish(wire)', 'wire'),
                ('drop(operation)', 'drop(resume)'),
            )),
            'public': ('runtime/tests/browser_login_public_api.rs', inverse_public, (
                ('assert!(response_ok)', 'assert!(true)'),
                ('X-CSRF-Token', 'X-CSRF-TokeX'),
            )),
        }
        for name, (path, inverse, variants) in paths.items():
            source = read(path)
            for before, after in variants:
                with self.subTest(name=name, before=before):
                    self.assertIn(before, source)
                    changed = source.replace(before, after, 1)
                    try:
                        actual = digest(inverse(changed))
                    except ValueError:
                        continue
                    self.assertNotEqual(actual, CHILD_TOKENS[name])

    def test_native_cli_has_fixed_exit_and_never_echoes_unknown_material(self):
        import subprocess
        raw = ('running 9 tests\n' + ''.join('test ' + name + ' ... ok\n' for name in EXPECTED) + 'test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out; finished in 0.20s\n').encode()
        cases = (
            (raw, 'passed', 0),
            (raw.replace(b'\n', b'\r\n'), 'passed', 0),
            (b'Cookie=sentinel SQL /private/path https://secret.invalid', 'unavailable', 1),
            (b'error[E0382]: sentinel /private/path\n', 'compile_failed', 1),
            (raw + b'test secret::name ... ok\n', 'invalid', 1),
        )
        path = str(Path(__file__).resolve())
        force_crlf = "import runpy,sys; path=sys.argv[1]; sys.stdout.reconfigure(newline='\\r\\n'); sys.argv=[path,'--native-summary']; runpy.run_path(path,run_name='__main__')"
        commands = (
            [sys.executable, '-B', path, '--native-summary'],
            [sys.executable, '-B', '-c', force_crlf, path],
        )
        for command in commands:
            for value, status, code in cases:
                result = subprocess.run(command, input=value, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
                self.assertEqual(result.returncode, code)
                stdout = result.stdout.replace(b'\r\n', b'\n')
                self.assertNotIn(b'\r', stdout)
                self.assertEqual(stdout, ('browser_login_stage native9=' + status + '\n').encode())
                self.assertEqual(result.stderr, b'')

    def test_lf_crlf_native9_and_closed_failures(self):
        raw = ('running 9 tests\n' + ''.join('test ' + name + ' ... ok\n' for name in EXPECTED) + 'test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out; finished in 0.20s\n').encode()
        for value in (raw, raw.replace(b'\n', b'\r\n')):
            self.assertEqual(native_summary(value), 'passed')
        for value in (raw.replace(b'9 passed', b'0 passed'), raw.replace(b'0 ignored', b'1 ignored'), raw.replace(EXPECTED[0].encode(), b'unknown'), raw + b'test unknown ... ok\n', raw + b'\r', raw + b'\xff', b'x' * 65537):
            self.assertEqual(native_summary(value), 'invalid')
        self.assertEqual(native_summary(b'error[E0382]: private\n'), 'compile_failed')
        self.assertEqual(native_summary(b''), 'unavailable')
        self.assertEqual(native_summary(b'running 9 tests\ntest result: FAILED. 8 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s\n'), 'test_failed')

if __name__=="__main__":
    if sys.argv[1:]==["--native-summary"]:
        state=native_summary(sys.stdin.buffer.read(65537))
        print("browser_login_stage native9="+state)
        raise SystemExit(0 if state=="passed" else 1)
    unittest.main()
