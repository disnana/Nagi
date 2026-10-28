"""B1/CORS5 source inverses and bounded native10 CI summary; no native proof."""
import hashlib
import inspect
from pathlib import Path
import re
import sys
import unittest
import importlib.util
_PROJECTION = Path(__file__).resolve().parents[1] / "ci/sf07_request_projection.py"
_SPEC = importlib.util.spec_from_file_location("request_projection", _PROJECTION)
projection = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(projection)
ROOT = Path(__file__).resolve().parents[2]

EXPECTED = ('http_server::browser_login::stage_tests::live_claimed_200_runs_verifier_and_handler_and_returns_200', 'http_server::browser_login::stage_tests::consumed_prepare_err_1000_returns_401_without_cookie', 'http_server::browser_login::stage_tests::consumed_prepare_panic_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::consumed_prepare_pending_200_returns_504_and_drops_once', 'http_server::browser_login::stage_tests::consumed_prepare_non_yielding_200_late_ready_returns_504', 'http_server::browser_login::stage_tests::committed_issue_live_200_delivers_auth_cookie_once', 'http_server::browser_login::stage_tests::committed_issue_handler_err_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::committed_issue_busy_mapper_returns_503_and_deletes_consumed_cookie', 'http_server::browser_login::stage_tests::committed_issue_handler_panic_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::committed_issue_handler_pending_200_returns_504_and_drops_once')


# Current U4 factory/config migration at the reviewed public RC; old CHILD_* is historical.
PRODUCTION_PUBLIC_INVERSE = 'cb8cf4965e60da22cf9577a96263e40353052f73686c82e879948470e0644f50'
PRODUCTION_WORKFLOW_CHILD = '05f5757739240a499be2b75afb58e2c80798f44a0324ac0eb3f87a1b18c5ba3b'

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
    source = norm(projection.inverse_browser(source))
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
    source = norm(projection.inverse_stage(source))
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


PUBLIC_STRICT_COUNTER_FRAGMENT = '''if good.status != 200 {
    let status = good.status;
    let body_bytes = good.body.len();
    let set_cookie_count = good.values("set-cookie").len();
    let verifier_calls = f.stats.verifier.load(Ordering::SeqCst);
    let handler_calls = f.stats.handler.load(Ordering::SeqCst);
    let mode6_cancel_seen = f.stats.cancelled.load(Ordering::SeqCst);
    eprintln!(
        "sf03_browser_failure_counters_v1 case=strict_json_valid phase=final_response status={} status_overflow={} body_bytes={} body_bytes_overflow={} set_cookie_count={} set_cookie_count_overflow={} auth_cookie={} deletion={} verifier_calls={} verifier_calls_overflow={} handler_calls={} handler_calls_overflow={} claim_committed={} issue_committed={} mode6_cancel_seen={} mode6_cancel_seen_overflow={}",
        status.min(999),
        status > 999,
        body_bytes.min(8193),
        body_bytes > 8193,
        set_cookie_count.min(1023),
        set_cookie_count > 1023,
        good.auth_cookie(),
        good.deletion(),
        verifier_calls.min(1023),
        verifier_calls > 1023,
        handler_calls.min(1023),
        handler_calls > 1023,
        f.stats.claim_committed.load(Ordering::SeqCst),
        f.stats.issue_committed.load(Ordering::SeqCst),
        mode6_cancel_seen.min(1023),
        mode6_cancel_seen > 1023,
    );
}'''
PUBLIC_WIRE_COUNTER_LOCALS_FRAGMENT = '''let status = r.status;
let body_bytes = r.body.len();
let expected_body_bytes = cap.len();
let set_cookie_count = r.values("set-cookie").len();
let verifier_calls = f.stats.verifier.load(Ordering::SeqCst);
let handler_calls = f.stats.handler.load(Ordering::SeqCst);
let mode6_cancel_seen = f.stats.cancelled.load(Ordering::SeqCst);'''
PUBLIC_WIRE_COUNTER_EPRINT_FRAGMENT = '''eprintln!(
    "sf03_browser_failure_counters_v1 case=wire_body_cap phase=exact_body_response iteration={} status={} status_overflow={} body_bytes={} body_bytes_overflow={} expected_body_bytes={} expected_body_bytes_overflow={} body_matches={} set_cookie_count={} set_cookie_count_overflow={} auth_cookie={} deletion={} verifier_calls={} verifier_calls_overflow={} handler_calls={} handler_calls_overflow={} claim_committed={} issue_committed={} mode6_cancel_seen={} mode6_cancel_seen_overflow={}",
    iteration,
    status.min(999),
    status > 999,
    body_bytes.min(8193),
    body_bytes > 8193,
    expected_body_bytes.min(8193),
    expected_body_bytes > 8193,
    r.body == cap,
    set_cookie_count.min(1023),
    set_cookie_count > 1023,
    r.auth_cookie(),
    r.deletion(),
    verifier_calls.min(1023),
    verifier_calls > 1023,
    handler_calls.min(1023),
    handler_calls > 1023,
    f.stats.claim_committed.load(Ordering::SeqCst),
    f.stats.issue_committed.load(Ordering::SeqCst),
    mode6_cancel_seen.min(1023),
    mode6_cancel_seen > 1023,
);'''


def inverse_public(source):
    """Remove only the two fixed counter branches, then undo two strengthening additions."""
    source = norm(source)
    source = rewrite(source, PUBLIC_STRICT_COUNTER_FRAGMENT, '')
    source = rewrite(source, PUBLIC_WIRE_COUNTER_LOCALS_FRAGMENT, '')
    source = rewrite(source, PUBLIC_WIRE_COUNTER_EPRINT_FRAGMENT, '')
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
B1_NATIVE_STEP = B1_STEP.replace(
    '          python -B scripts/tests/test_browser_login_stage_backport.py\n', '')

CHILD_TOKENS = {'dispatch': 'e15ba0fc1b4f45cabc893904d0028d51d2cda49edb8d83e236f3efc5d78e263f', 'stage': '61dbb99d727253ac4050226806d00e6e6807a2e1cfee93cdcd75c6a756c358cc', 'public': '95e8a9f83c93c7bb03ddff48d0d01ef104985b3e5cf5621c4d5369fbef5a2378'}
INVERSE_PUBLIC_SOURCE_SHA256 = 'cc30e6fe54b83806728c40215b091ab5495e0c6a828cdeee033b623f9a134dbf'
INVERSE_PUBLIC_HELPER_SHA256 = {
    'norm': '2d899b7072c49d72e673447a8fdce99f807934659cc05f379071ff484f96dca8',
    'rewrite': '8bd219920f1aadf84e1d3d75ffcc5d23fb1c1a21383d6ab977e9c54a0213374b',
}
INVERSE_PUBLIC_FRAGMENT_SHA256 = {
    'strict': 'e15dc95d6cdaa83638283ed191a09f2e917df9ccd44bd94af02203c777b03e9b',
    'wire_locals': 'fe27fc5dac6c5975072f7d6e5aa087f3baea4f8fc1c355a5d91b489d2b9d9266',
    'wire_eprintln': '2f88404d46f468c32dc7c4a321dd4cea7a6ac0324aca85aa939751f51599ab48',
}
CHILD_BYTES = {'.github/workflows/ci.yml': 'd6a2753c59ad16e9ab03af39abd09b7e4b30403f52429da4766a7d4c345ae202', 'runtime/src/http_server.rs': '3fa9bf11de875398966537bb275057c6bc23ffffb888ed3bddbe0ea2ff8e25f6', 'runtime/src/auth/prelogin/browser.rs': '2428b3e1cdc49dbd922c9b9ce026192972ded7bfa04ae6b82f704d0cc1301f74', 'runtime/src/auth/prelogin/http_bridge.rs': 'd64e052c191e0b8c3853feb07cf5b1e04f0c1d57da2c399099cd7dfc5a031d55', 'scripts/ci/session_private_deadline_diagnostic.py': '1d0dd3c92e07c374de80d4f968c101141eb6290ba2b935bf9c06dcb85ac60913', 'scripts/ci/browser_login_stage_coverage.py': '5ada31eed9ecdc841dd06bbcd8ce056a30c75916c8a7c596fab1c03428af5502'}

def native_summary(raw):
    if len(raw)>65536: return "invalid"
    raw=raw.replace(b"\r\n",b"\n")
    if b"\r" in raw: return "invalid"
    try: text=raw.decode("utf-8",errors="strict")
    except UnicodeError: return "invalid"
    if re.search(r"(?m)^error(?:\[E[0-9]+\]|:)",text): return "compile_failed"
    if text.splitlines().count("running 10 tests") != 1: return "unavailable"
    summaries=re.findall(r"(?m)^test result: ([^\n]+)$",text)
    if len(summaries)!=1: return "unavailable"
    if summaries[0].startswith("FAILED."): return "test_failed"
    if not re.fullmatch(r"ok\. 10 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in [0-9.]+s",summaries[0]): return "invalid"
    lines=[line for line in text.splitlines() if line.startswith("test ") and not line.startswith("test result:")]
    rows=re.findall(r"(?m)^test ([^ \n]+) \.\.\. (ok)$",text)
    return "passed" if len(lines)==len(rows)==10 and {name for name,_ in rows}==set(EXPECTED) else "invalid"


# These eight workflow-only RC deltas were reviewed independently: six exact
# Windows TMP/TEMP guards and two concrete SF03 deadline inventory counts.
_WINDOWS_TEMP_GUARD = (
    '          if [ "${RUNNER_OS:-}" = "Windows" ]; then\n'
    '            if [ -z "${RUNNER_TEMP:-}" ] || [ ! -d "${RUNNER_TEMP:-}" ]; then\n'
    '              echo "RUNNER_TEMP must be a nonempty existing directory on Windows" >&2\n'
    '              exit 2\n'
    '            fi\n'
    '            export TMP="$RUNNER_TEMP" TEMP="$RUNNER_TEMP"\n'
    '          fi\n'
)
_WINDOWS_TEMP_GUARD_SITES = (
    ('nagi-package', 'Diagnose G3 generation-token failure before crypto checks', 'set -euo pipefail',
     'cargo test --locked -p nagi-runtime --test session_csrf_public_api guarded_rotation_and_logout_invalidate_old_generation_tokens -- --exact 2>&1 | tee "$RUNNER_TEMP/g3-generation-target.log"'),
    ('nagi-package', 'Check SF02 public Session runtime and one-use delivery before other checks', 'set -euo pipefail',
     "python -c 'import os; print(\"ICU4X_DATA_DIR present:\", \"ICU4X_DATA_DIR\" in os.environ)'"),
    ('nagi-package', 'Diagnose private Session phase deadlines on Windows', 'set +e',
     'python -B scripts/tests/test_session_private_deadline_diagnostic.py'),
    ('nagi-package', 'Capture bounded Session auth stages before broad checks', 'set -e',
     'python scripts/tests/test_auth_stage_diagnostic.py'),
    ('nagi-package', 'Capture bounded Session deadline public tests', 'set -euo pipefail',
     'python scripts/tests/test_session_deadline_diagnostic.py'),
    ('nagi-package', 'Check B1 Session CSRF runtime and public socket contracts', 'set -euo pipefail',
     'python -B scripts/tests/test_csrf_deadline_mutations.py'),
)
_SF07_PROFILE_STEP_TITLE = 'Measure bounded SF07 response and rate profile'
_SF07_PROFILE_COLOR_ENV = '        env:\n          CARGO_TERM_COLOR: never\n'
_CSRF_DEADLINE_CURRENT = '"sf03-b1-csrf-deadline.log":14'
_CSRF_DEADLINE_HISTORICAL = '"sf03-b1-csrf-deadline.log":12'
_CSRF_DEADLINE_SITE = (
    '          cargo test --locked -p nagi-runtime --lib http_server::csrf_http::deadline_tests:: 2>&1 | tee "$RUNNER_TEMP/sf03-b1-csrf-deadline.log"\n'
    '          cargo test --locked -p nagi-runtime --test session_csrf_public_api 2>&1 | tee "$RUNNER_TEMP/sf03-b1-csrf-public.log"\n'
    "          python - <<'PYCODE'\n"
    '          import os, pathlib, re\n'
    '          root=pathlib.Path(os.environ["RUNNER_TEMP"])\n'
    '          for name, count in {"sf03-b1-csrf-core.log":14, "sf03-b1-csrf-deadline.log":14, "sf03-b1-csrf-public.log":10}.items():\n'
)


def _workflow_step_span(source, job, title):
    job_header = f'  {job}:\n'
    if source.count(job_header) != 1:
        raise ValueError('workflow job anchor mismatch')
    job_start = source.index(job_header)
    job_tail = job_start + len(job_header)
    next_job = re.search(r'(?m)^  [A-Za-z0-9_-]+:[ \t]*$', source[job_tail:])
    job_end = job_tail + next_job.start() if next_job else len(source)
    job_source = source[job_start:job_end]
    step_header = f'      - name: {title}\n'
    if job_source.count(step_header) != 1:
        raise ValueError('workflow step anchor mismatch')
    step_start = job_source.index(step_header)
    step_tail = step_start + len(step_header)
    next_step = re.search(r'(?m)^      - ', job_source[step_tail:])
    step_end = step_tail + next_step.start() if next_step else len(job_source)
    return job_start + step_start, job_start + step_end


def _replace_workflow_step(source, job, title, before, after):
    start, end = _workflow_step_span(source, job, title)
    step = source[start:end]
    if step.count(before) != 1:
        raise ValueError('workflow step residual anchor mismatch')
    return source[:start] + step.replace(before, after, 1) + source[end:]


def _inverse_reviewed_workflow_residuals(source):
    """Reverse only the six fixed Windows guards and two fixed SF03 counts."""
    source = projection.inverse_workflow(source)
    if source.count(_WINDOWS_TEMP_GUARD) != len(_WINDOWS_TEMP_GUARD_SITES):
        raise ValueError('Windows TMP/TEMP guard inventory mismatch')
    for job, title, set_line, first_command in _WINDOWS_TEMP_GUARD_SITES:
        run_open = f'        shell: bash\n        run: |\n          {set_line}\n'
        anchored = run_open + _WINDOWS_TEMP_GUARD + f'          {first_command}\n'
        restored = run_open + f'          {first_command}\n'
        source = _replace_workflow_step(source, job, title, anchored, restored)
    if source.count(_CSRF_DEADLINE_CURRENT) != 2 or source.count(_CSRF_DEADLINE_HISTORICAL) != 0:
        raise ValueError('SF03 deadline inventory mismatch')
    for job in ('linux', 'nagi-package'):
        source = _replace_workflow_step(
            source, job, 'Check B1 Session CSRF runtime and public socket contracts',
            _CSRF_DEADLINE_SITE,
            _CSRF_DEADLINE_SITE.replace(_CSRF_DEADLINE_CURRENT, _CSRF_DEADLINE_HISTORICAL, 1),
        )
    if _WINDOWS_TEMP_GUARD in source or _CSRF_DEADLINE_CURRENT in source:
        raise ValueError('reviewed workflow residual remains after inversion')
    return source


class BackportSourceTests(unittest.TestCase):
    def test_inverse_core_retains_child_decisions_and_every_diagnostic(self):
        source = read("runtime/src/http_server/browser_login.rs")
        self.assertEqual(digest(projection.approved_production('browser', source)), projection.PRODUCTION_SOURCES['browser']['tokens'])
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
        self.assertEqual(digest(projection.approved_production('stage', source)), projection.PRODUCTION_SOURCES['stage']['tokens'])
        cases = re.findall(r'#\[tokio::test\]\s*async fn (\w+)\(', source)
        self.assertEqual({'http_server::browser_login::stage_tests::' + n for n in cases}, set(EXPECTED))
        self.assertEqual(len(cases), 10)
        for token in ('drop(operation)', 'drop(resume)', 'f.stop().await', 'pending_progress',
                      'verifier_drops', 'handler_drops', 'stage_until < original_until',
                      'session::issue', 'session::apply', 'issue_ack', 'claim_stage_for_test',
                      'fault_entered', 'expected_fault', 'vary::finish(wire)'):
            self.assertIn(token, source)
        self.assertIn('Authorization, Content-Type, Cookie, Origin, X-CSRF-Token', source)
        self.assertNotIn('Cookie, Authorization, Origin', source)

    def test_public_inverse_keeps_all_child_assertions_and_observations(self):
        source = read("runtime/tests/browser_login_public_api.rs")
        self.assertEqual(digest(inverse_public(source)), PRODUCTION_PUBLIC_INVERSE)
        for token in ('assert_browser_deadline_state', 'assert_consumed_browser_replay',
                      'handler_calls <= verifier_calls', 'issue_committed',
                      '!retry.auth_cookie() && !retry.deletion()',
                      'Authorization, Content-Type, Cookie, Origin, X-CSRF-Token',
                      'assert!(response_ok)', 'assert!(retry_response_ok)', 'assert!(replay_proof_ok)'):
            self.assertIn(token, source)

    def test_public_inverse_source_hash_and_frozen_token_binding(self):
        self.assertEqual(CHILD_TOKENS['public'], '95e8a9f83c93c7bb03ddff48d0d01ef104985b3e5cf5621c4d5369fbef5a2378')
        self.assertEqual(
            hashlib.sha256(inspect.getsource(inverse_public).encode()).hexdigest(),
            INVERSE_PUBLIC_SOURCE_SHA256,
        )
        helpers = {'norm': norm, 'rewrite': rewrite}
        self.assertEqual(
            {name: hashlib.sha256(inspect.getsource(function).encode()).hexdigest() for name, function in helpers.items()},
            INVERSE_PUBLIC_HELPER_SHA256,
        )
        fragments = {
            'strict': PUBLIC_STRICT_COUNTER_FRAGMENT,
            'wire_locals': PUBLIC_WIRE_COUNTER_LOCALS_FRAGMENT,
            'wire_eprintln': PUBLIC_WIRE_COUNTER_EPRINT_FRAGMENT,
        }
        self.assertEqual(
            {name: hashlib.sha256(value.encode()).hexdigest() for name, value in fragments.items()},
            INVERSE_PUBLIC_FRAGMENT_SHA256,
        )

    def test_public_inverse_fails_closed_if_counter_fragments_are_deleted_or_changed(self):
        source = read("runtime/tests/browser_login_public_api.rs")
        mutants = (
            source.replace('sf03_browser_failure_counters_v1 case=strict_json_valid', '', 1),
            source.replace('sf03_browser_failure_counters_v1 case=wire_body_cap', '', 1),
            source.replace('expected_body_bytes_overflow={}', '', 1),
        )
        for mutant in mutants:
            with self.subTest(mutant_hash=hashlib.sha256(mutant.encode()).hexdigest()):
                with self.assertRaises(ValueError):
                    inverse_public(mutant)

    def test_workflow_is_only_two_required_native_steps_and_frozen_sources_remain(self):
        # Strip only reviewed CI-level, infrastructure and publication wiring;
        # unchanged native commands retain every legacy assertion and hash.
        workflow = projection._LEVELS.inverse_workflow(read('.github/workflows/ci.yml'))
        profile_step = projection.BROWSER_COUNTER_PROFILE_STEP
        self.assertEqual(workflow.count(projection.CI_SOURCE_GUARD_STEP), 1)
        self.assertEqual(workflow.count(B1_STEP), 0)
        self.assertEqual(workflow.count(B1_NATIVE_STEP), 2)
        for line in (*projection.CI_SOURCE_GUARD_LINES, projection.CI_BROWSER_SOURCE_LINE):
            self.assertEqual(workflow.count(line), 1)
        self.assertEqual(workflow.count('          python -B scripts/tests/test_sf07_owned_state_summary.py\n'), 2)
        self.assertEqual(workflow.count('          cargo test --locked -p nagi-runtime --lib http_server::browser_login::stage_tests:: 2>&1 | python -B scripts/tests/test_browser_login_stage_backport.py --native-summary\n'), 2)
        self.assertEqual(workflow.count(profile_step), 1)
        _, linux_tail = workflow.split('  linux:\n', 1)
        linux_job, following_jobs = linux_tail.split('\n  release-plan:', 1)
        self.assertEqual(linux_job.count(profile_step), 1)
        self.assertNotIn(profile_step, following_jobs)
        setup_node = ("      - uses: actions/setup-node@v4\n"
                      "        with:\n"
                      "          node-version: '22'\n")
        first_test = '      - name: Check call_member_ recovery first with exact result on the target platform\n'
        sf02_test = '      - name: Check SF02 public Session runtime and one-use delivery before other checks\n'
        self.assertEqual(linux_job.count(setup_node), 1)
        self.assertEqual(linux_job.count(first_test), 1)
        self.assertEqual(linux_job.count(sf02_test), 1)
        self.assertTrue(linux_job.split(setup_node, 1)[1].startswith(profile_step + first_test))
        self.assertLess(linux_job.index(profile_step), linux_job.index(first_test))
        self.assertLess(linux_job.index(profile_step), linux_job.index(sf02_test))
        broad_suite = '      - run: cargo test --locked\n'
        self.assertEqual(linux_job.count(broad_suite), 1)
        self.assertLess(linux_job.index(profile_step), linux_job.index(broad_suite))
        self.assertIn('input_and_strict_json_rejections_do_not_consume_or_call_verifier -- --exact', profile_step)
        self.assertIn('wire_body_cap_is_independent_of_db_quota_and_checked_before_claim -- --exact', profile_step)
        commands = (
            'if ! run_probe compile browser-counter-compile cargo test --locked -p nagi-runtime --test browser_login_public_api --no-run; then',
            'if ! run_probe list browser-counter-list cargo test --locked -p nagi-runtime --test browser_login_public_api -- --list; then',
            'if ! run_probe single-strict browser-counter-strict cargo test --locked -p nagi-runtime --test browser_login_public_api input_and_strict_json_rejections_do_not_consume_or_call_verifier -- --exact; then',
            'if ! run_probe single-wire browser-counter-wire cargo test --locked -p nagi-runtime --test browser_login_public_api wire_body_cap_is_independent_of_db_quota_and_checked_before_claim -- --exact; then',
            'if ! run_probe parallel-ten browser-counter-parallel cargo test --locked -p nagi-runtime --test browser_login_public_api; then',
        )
        for command in commands:
            self.assertEqual(profile_step.count(command), 1)
        cargo_command = 'cargo test --locked -p nagi-runtime --test browser_login_public_api'
        self.assertEqual(profile_step.count(cargo_command), 5)
        self.assertEqual(profile_step.count('CARGO_TERM_COLOR=never'), 1)
        self.assertIn('CARGO_TERM_COLOR=never "$@" 2>&1 | python -B scripts/ci/sf07_checkpoint_summary.py "$mode"', profile_step)
        self.assertNotIn('RUSTFLAGS', profile_step)
        self.assertNotIn('--test-threads', profile_step)
        restored = _inverse_reviewed_workflow_residuals(workflow)
        self.assertEqual(restored.count(B1_STEP), 2)
        self.assertEqual(hashlib.sha256(restored.replace(B1_STEP, '').encode()).hexdigest(), PRODUCTION_WORKFLOW_CHILD)
        mutants = (
            profile_step.replace('-- --list', '--', 1),
            profile_step.replace(' -- --exact', '', 1),
            profile_step.replace('| python -B scripts/ci/sf07_checkpoint_summary.py \"$mode\"', '| cat', 1),
            profile_step.replace('CARGO_TERM_COLOR=never "$@"', '"$@"', 1),
            profile_step.replace('exit "$profile_failed"', 'true', 1),
        )
        for mutant in mutants:
            self.assertNotEqual(mutant, profile_step)
            changed = workflow.replace(profile_step, mutant, 1)
            with self.assertRaises(ValueError):
                projection.inverse_workflow(changed)
        for path, expected in CHILD_BYTES.items():
            if path != '.github/workflows/ci.yml':
                value = read(path)
                if path == 'runtime/src/http_server.rs':
                    projection.approved_production('http', value)
                    continue
                if path == 'scripts/ci/session_private_deadline_diagnostic.py':
                    value = projection.inverse_collector(value)
                if path == 'scripts/ci/browser_login_stage_coverage.py':
                    value = projection.inverse_browser_handler_coverage(value)
                if path == 'runtime/src/auth/prelogin/http_bridge.rs':
                    value = projection.inverse_prelogin_wire(value)
                self.assertEqual(hashlib.sha256(value.encode()).hexdigest(), expected, path)
        http_sha = projection.HTTP_AFTER
        self.assertEqual(workflow.count("'runtime/src/http_server.rs': '" + http_sha + "'"), 2)
        self.assertEqual(read('scripts/ci/session_private_deadline_diagnostic.py').count('expected_digest = "' + http_sha + '"'), 1)


    def test_early_source_guard_is_exact_and_package_matrix_waits_for_it(self):
        # Strip only reviewed CI-level, infrastructure and publication wiring;
        # unchanged native commands retain every legacy assertion and hash.
        workflow = projection._LEVELS.inverse_workflow(read('.github/workflows/ci.yml'))
        source_step = projection.CI_SOURCE_GUARD_STEP
        package_gate = projection.CI_NAGI_PACKAGE_CURRENT_GATE
        self.assertEqual(workflow.count(source_step), 1)
        self.assertEqual(workflow.count(package_gate), 1)
        for line in (*projection.CI_SOURCE_GUARD_LINES, projection.CI_BROWSER_SOURCE_LINE):
            self.assertEqual(workflow.count(line), 1)
        identified_sf07 = projection.CI_SF07_STEP_ANCHOR.replace(
            '        shell: bash\n', projection.PRELOGIN_OWNER_PRIMARY_ID + '        shell: bash\n', 1)
        self.assertEqual(workflow.count(identified_sf07), 2)
        self.assertEqual(workflow.count(projection.CI_BROWSER_STEP_ANCHOR), 2)
        self.assertEqual(workflow.count('          python -B scripts/tests/test_sf07_owned_state_summary.py\n'), 2)
        self.assertEqual(
            workflow.count('          cargo test --locked -p nagi-runtime --lib http_server::browser_login::stage_tests:: 2>&1 | python -B scripts/tests/test_browser_login_stage_backport.py --native-summary\n'),
            2,
        )
        restored = _inverse_reviewed_workflow_residuals(workflow)
        self.assertEqual(restored.count(B1_STEP), 2)
        self.assertEqual(hashlib.sha256(restored.replace(B1_STEP, '').encode()).hexdigest(), PRODUCTION_WORKFLOW_CHILD)

        setup = projection.CI_CHANGES_SETUP
        moved = workflow.replace(source_step, '', 1)
        moved = moved.replace(setup, source_step + setup, 1)
        mutants = (
            ('missing source step', workflow.replace(source_step, '', 1)),
            ('duplicate source step', workflow.replace(source_step, source_step + source_step, 1)),
            ('edited source command', workflow.replace('test_sf07_wire_source.py', 'test_sf07_wire_src.py', 1)),
            ('moved source step before Python setup', moved),
            ('duplicated per-OS source command', workflow.replace(
                '          if [ "$RUNNER_OS" = "Windows" ]; then\n',
                projection.CI_SOURCE_GUARD_LINES[0] + '          if [ "$RUNNER_OS" = "Windows" ]; then\n', 1)),
            ('missing changes dependency', workflow.replace(
                package_gate, package_gate.replace('needs: [changes, release-plan]', 'needs: release-plan', 1), 1)),
            ('missing explicit changes success condition', workflow.replace(
                package_gate, package_gate.replace("needs.changes.result == 'success' && ", '', 1), 1)),
            ('changed package selector', workflow.replace(
                package_gate, package_gate.replace('outputs.package_nagi', 'outputs.package_vscode', 1), 1)),
        )
        for label, mutant in mutants:
            with self.subTest(label=label, mutant_sha=hashlib.sha256(mutant.encode()).hexdigest()):
                with self.assertRaises(ValueError):
                    projection.inverse_workflow(mutant)


    def test_canonical_workflow_rejects_unreviewed_action_and_tier_wiring(self):
        workflow = read('.github/workflows/ci.yml')
        reviewed_node = 'actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # ci-pin:v4'
        replacements = (
            (reviewed_node, 'actions/setup-node@' + '0' * 40 + ' # ci-pin:v4'),
            ('      matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}', '      matrix: {}'),
            ('    needs: [changes, release-plan, ci-plan]', '    needs: [release-plan, ci-plan]'),
            ('  critical:\n', '  critical-unreviewed:\n'),
        )
        for before, after in replacements:
            with self.subTest(anchor=before):
                self.assertIn(before, workflow)
                with self.assertRaises(ValueError):
                    projection._LEVELS.inverse_workflow(workflow.replace(before, after, 1))

    def test_sf07_profile_color_override_is_required_and_step_bound(self):
        workflow = read('.github/workflows/ci.yml')
        start, end = _workflow_step_span(workflow, 'nagi-package', _SF07_PROFILE_STEP_TITLE)
        step = workflow[start:end]
        self.assertEqual(workflow.count(_SF07_PROFILE_COLOR_ENV), 1)
        self.assertEqual(step.count(_SF07_PROFILE_COLOR_ENV), 1)
        commands = (
            'cargo test --locked -p nagi-runtime --test sf07_small_profile --no-run',
            'cargo test --locked -p nagi-runtime --test sf07_small_profile -- --list',
            'cargo test --locked -p nagi-runtime --test sf07_small_profile response_profile_reports_closed_measurements -- --exact --show-output',
        )
        for command in commands:
            self.assertEqual(step.count(command), 1)

        restored = _inverse_reviewed_workflow_residuals(workflow)
        self.assertEqual(hashlib.sha256(restored.replace(B1_STEP, '').encode()).hexdigest(), PRODUCTION_WORKFLOW_CHILD)

        missing = workflow.replace(_SF07_PROFILE_COLOR_ENV, '', 1)
        duplicate = workflow.replace(_SF07_PROFILE_COLOR_ENV, _SF07_PROFILE_COLOR_ENV + _SF07_PROFILE_COLOR_ENV, 1)
        edited = workflow.replace(_SF07_PROFILE_COLOR_ENV, _SF07_PROFILE_COLOR_ENV.replace('never', 'always'), 1)
        moved = workflow.replace(_SF07_PROFILE_COLOR_ENV, '', 1)
        wrong_start, wrong_end = _workflow_step_span(
            moved, 'nagi-package', 'Verify an extracted distribution outside the checkout'
        )
        wrong_step = moved[wrong_start:wrong_end]
        wrong_step = wrong_step.replace('        shell: bash\n', _SF07_PROFILE_COLOR_ENV + '        shell: bash\n', 1)
        moved = moved[:wrong_start] + wrong_step + moved[wrong_end:]
        for label, mutant in (
            ('missing', missing), ('duplicate', duplicate), ('edited', edited), ('moved', moved),
        ):
            with self.subTest(label=label, mutant_sha=hashlib.sha256(mutant.encode()).hexdigest()):
                with self.assertRaises(ValueError):
                    _inverse_reviewed_workflow_residuals(mutant)

        unrelated = _inverse_reviewed_workflow_residuals(workflow + '# unrelated workflow drift\n')
        self.assertNotEqual(
            hashlib.sha256(unrelated.replace(B1_STEP, '').encode()).hexdigest(),
            PRODUCTION_WORKFLOW_CHILD,
        )

    def test_reviewed_workflow_inverse_rejects_moved_or_changed_residuals(self):
        workflow = read('.github/workflows/ci.yml')
        first_site = _WINDOWS_TEMP_GUARD_SITES[0]
        _, first_title, first_set, first_command = first_site
        run_open = f'        shell: bash\n        run: |\n          {first_set}\n'
        first_anchor = run_open + _WINDOWS_TEMP_GUARD + f'          {first_command}\n'
        first_restored = run_open + f'          {first_command}\n'
        second_site = _WINDOWS_TEMP_GUARD_SITES[1]
        _, second_title, second_set, second_command = second_site
        second_command_anchor = (
            f'        shell: bash\n        run: |\n          {second_set}\n'
            f'          {second_command}\n'
        )
        moved_to_wrong_step = workflow.replace(first_anchor, first_restored, 1)
        moved_to_wrong_step = moved_to_wrong_step.replace(
            second_command_anchor,
            second_command_anchor.replace(f'          {second_command}\n',
                                          _WINDOWS_TEMP_GUARD + f'          {second_command}\n', 1),
            1,
        )
        missing = workflow.replace(_WINDOWS_TEMP_GUARD, '', 1)
        duplicated = workflow.replace(_WINDOWS_TEMP_GUARD,
                                     _WINDOWS_TEMP_GUARD + _WINDOWS_TEMP_GUARD, 1)
        edited = workflow.replace(
            'echo "RUNNER_TEMP must be a nonempty existing directory on Windows"',
            'echo "RUNNER_TEMP must be an existing directory on Windows"', 1)
        moved_within_step = workflow.replace(
            first_anchor, run_open + f'          {first_command}\n' + _WINDOWS_TEMP_GUARD, 1)
        # The raw workflow carries #183's fifteenth deadline case.  The shared
        # projection validates and restores that addition before this helper
        # handles its inherited fourteen-to-twelve residual.
        raw_deadline_count = '"sf03-b1-csrf-deadline.log":15'
        wrong_count_site = workflow.replace(raw_deadline_count,
                                             '"sf03-b1-csrf-deadline.log":14', 1)
        missing_count_site = workflow.replace(raw_deadline_count,
                                              _CSRF_DEADLINE_HISTORICAL, 1)
        duplicate_count_site = workflow.replace(
            '"sf04-html-core.log": 20,',
            '"sf04-html-core.log": 20, "sf03-b1-csrf-deadline.log":15', 1)
        for label, mutant in (
            ('missing guard', missing),
            ('duplicated guard', duplicated),
            ('edited guard', edited),
            ('guard moved to another step', moved_to_wrong_step),
            ('guard moved within step', moved_within_step),
            ('wrong count value', wrong_count_site),
            ('missing expected count site', missing_count_site),
            ('duplicate count at unrelated site', duplicate_count_site),
        ):
            with self.subTest(label=label), self.assertRaises(ValueError):
                _inverse_reviewed_workflow_residuals(mutant)

    def test_reviewed_workflow_inverse_leaves_unrelated_drift_for_frozen_hash(self):
        workflow = read('.github/workflows/ci.yml') + '# unrelated workflow drift\n'
        restored = _inverse_reviewed_workflow_residuals(workflow)
        self.assertNotEqual(
            hashlib.sha256(restored.replace(B1_STEP, '').encode()).hexdigest(),
            PRODUCTION_WORKFLOW_CHILD,
        )

    def test_fixed_owner_inverse_cannot_hide_changed_live_gate_or_cookie(self):
        path = 'runtime/src/auth/prelogin/http_bridge.rs'
        source = read(path)
        expected = CHILD_BYTES[path]
        self.assertEqual(hashlib.sha256(projection.inverse_prelogin_wire(source).encode()).hexdigest(), expected)
        for before, after in (('cookie.set_sensitive(true)', 'cookie.set_sensitive(false)'),
                              ('let clock = self.context.sample()', 'let clock = self.context.changed_sample()')):
            self.assertIn(before, source)
            changed = source.replace(before, after, 1)
            self.assertNotEqual(hashlib.sha256(projection.inverse_prelogin_wire(changed).encode()).hexdigest(), expected)
        with self.assertRaises(ValueError):
            projection.inverse_prelogin_wire(source.replace('FixedTokenBody::new', 'UncheckedBody::new', 1))

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
                ('assert!(good.status == 200)', 'assert!(good.status == 201)'),
                ('Duration::from_millis(400)', 'Duration::from_millis(401)'),
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
                        actual = digest(projection.approved_production('browser' if name == 'dispatch' else name, changed))
                    except ValueError:
                        continue
                    self.assertNotEqual(actual, projection.PRODUCTION_SOURCES['browser' if name == 'dispatch' else name]['tokens'])

    def test_native_cli_has_fixed_exit_and_never_echoes_unknown_material(self):
        import subprocess
        raw = ('running 10 tests\n' + ''.join('test ' + name + ' ... ok\n' for name in EXPECTED) + 'test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out; finished in 0.20s\n').encode()
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
                self.assertEqual(stdout, ('browser_login_stage native10=' + status + '\n').encode())
                self.assertEqual(result.stderr, b'')

    def test_lf_crlf_native10_and_closed_failures(self):
        raw = ('running 10 tests\n' + ''.join('test ' + name + ' ... ok\n' for name in EXPECTED) + 'test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out; finished in 0.20s\n').encode()
        for value in (raw, raw.replace(b'\n', b'\r\n')):
            self.assertEqual(native_summary(value), 'passed')
        for value in (raw.replace(b'10 passed', b'0 passed'), raw.replace(b'0 ignored', b'1 ignored'), raw.replace(EXPECTED[0].encode(), b'unknown'), raw + b'test unknown ... ok\n', raw + b'\r', raw + b'\xff', b'x' * 65537):
            self.assertEqual(native_summary(value), 'invalid')
        self.assertEqual(native_summary(b'error[E0382]: private\n'), 'compile_failed')
        self.assertEqual(native_summary(b''), 'unavailable')
        self.assertEqual(native_summary(b'running 10 tests\ntest result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s\n'), 'test_failed')

if __name__=="__main__":
    if sys.argv[1:]==["--native-summary"]:
        state=native_summary(sys.stdin.buffer.read(65537))
        print("browser_login_stage native10="+state)
        raise SystemExit(0 if state=="passed" else 1)
    unittest.main()
