"""Small source/fake checks and bounded closed native9 CI summary; never native proof."""
import hashlib
from pathlib import Path
import re
import sys
import unittest
ROOT = Path(__file__).resolve().parents[2]
GOLDEN = {'security': '9214761e80b9f8af0c11a046b4639a2e3c28c59a34f78c36a35d63331abccc9f', 'wire': '708c0851563ce63cea3d437eae7f6da4415e6a0a872242e10b4ede91b4b6bf99', 'prepare': '343eaa3bf8a03d0d8afb19ffd9a197a92e21dbe9c7c28b5fc8722235c04bb65b', 'handler': '4e0dcc018ba7862b98be44f856584aeb1bd0f8c8bbd9e4d74c1e8484d0ef5371', 'helpers': '22e672fb6f79e51de71f08161a1dfb3757a352dd6d7c053e43e54c4df6df7242'}
EXPECTED = ('http_server::browser_login::stage_tests::live_claimed_200_runs_verifier_and_handler_and_returns_200', 'http_server::browser_login::stage_tests::consumed_prepare_err_1000_returns_401_without_cookie', 'http_server::browser_login::stage_tests::consumed_prepare_panic_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::consumed_prepare_pending_200_returns_504_and_drops_once', 'http_server::browser_login::stage_tests::consumed_prepare_non_yielding_200_late_ready_returns_504', 'http_server::browser_login::stage_tests::committed_issue_live_200_delivers_auth_cookie_once', 'http_server::browser_login::stage_tests::committed_issue_handler_err_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::committed_issue_handler_panic_1000_returns_500_without_cookie', 'http_server::browser_login::stage_tests::committed_issue_handler_pending_200_returns_504_and_drops_once')

def norm(s):
 s=re.sub(r'=>\s*\{\s*return ([^{}]*?)\s*;?\s*\}',r'=> return \1',s)
 tokens=re.findall(r'"(?:\\.|[^"\\])*"|//[^\n]*|/\*.*?\*/|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]',s,re.S)
 tokens=[t for t in tokens if not t.startswith('//') and not t.startswith('/*') and t not in (',',)]
 return ' '.join(tokens).replace(' ; }',' }')
def pieces(s, candidate):
 if not candidate:
  a=s.index('    let Some(security_until)');b=s.index('    let input = match pool.login_input',a);c=s.index('    // Claim tightened',b);d=s.index('    let Some(handler_until)',c)
  return {'security':s[a:b],'wire':s[b:c],'prepare':s[c:d],'handler':s[d:s.rfind('\n}')],'helpers':s[:s.index('pub(super) async fn dispatch_browser')].replace(', ClaimContext','')}
 a=s.index('    let Some(security_until)');b=s.index('    let stage = match input',a)
 start=s.index('            let input = match pool.login_input',b);end=s.index('            Stage::Claimed { request, context }',start)
 wire=s[start:end]
 for field in ('method','uri','headers'):wire=wire.replace('std::mem::take(&mut parts.'+field+')','parts.'+field)
 wire=re.sub(r'body.take\(\).expect\("owned wire body present"\)', 'body',wire)
 start=s.index('                // Claim tightened') if '                // Claim tightened' in s else s.index('// Claim tightened')
 # Include the original comment and prepare through its real timeout match.
 end=s.index('            HandlerStep::Prepared { request, prepared }',start)
 prepare=s[start:end]
 start=s.index('    let Some(handler_until)',end);end=s.index('\n}\n\n#[cfg(test)]',start);handler=s[start:end]
 f=handler.index('    let future = AssertUnwindSafe(async {');g=handler.index('    match tokio::time::timeout_at',f)
 handler=handler[:f]+'    let future = AssertUnwindSafe(async { prepared(request, state).await }).catch_unwind();\n'+handler[g:]
 result={'security':s[a:b],'wire':wire,'prepare':prepare,'handler':handler,'helpers':s[:s.index('// The stock wrapper')].replace(', ClaimContext','')}
 for name in ('security','wire','prepare','handler'):result[name]=result[name].replace('pool, owner','pool, &owner').replace('pool.claim_http(owner, input)','pool.claim_http(&owner, input)')
 return result

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
    def test_shared_production_control_tokens_match_original_order(self):
        source=(ROOT/"runtime/src/http_server/browser_login.rs").read_text()
        actual=pieces(source,True)
        self.assertEqual({k:hashlib.sha256(norm(v).encode()).hexdigest() for k,v in actual.items()},GOLDEN)
        self.assertEqual(source.count("async fn dispatch_owned"),1)
        self.assertIn("body: &mut body",source)
        self.assertIn("options,",source)
        self.assertNotIn("nagi_browser_login_diag",source)
        self.assertNotIn("observed!",source)

    def test_stock_owner_and_native_remainder_stay_in_wrapper(self):
        source=(ROOT/"runtime/src/http_server/browser_login.rs").read_text()
        wrapper=source.split("pub(super) async fn dispatch_browser",1)[1].split("async fn dispatch_owned",1)[0]
        self.assertLess(wrapper.index("request.into_parts()"),wrapper.index("let mut body = Some(body)"))
        self.assertLess(wrapper.index("let mut body = Some(body)"),wrapper.index("HttpOwner::new(lease)"))
        self.assertIn("parts: &mut parts",wrapper)
        self.assertIn("&owner",wrapper)
        self.assertIn("owner: &HttpOwner",source)

    def test_all_nine_native_stage_tests_are_required_in_both_ci_branches(self):
        stage=(ROOT/"runtime/src/http_server/browser_login_stage_tests.rs").read_text()
        cases=re.findall(r"#\[tokio::test\]\s*async fn (\w+)\(",stage)
        self.assertEqual({"http_server::browser_login::stage_tests::"+n for n in cases},set(EXPECTED))
        for token in ("drop(operation)","drop(resume)","f.stop().await", "pending_progress", "verifier_drops", "handler_drops", "stage_until < original_until", "session::issue", "session::apply", "issue_ack", "claim_stage_for_test", "fault_entered", "expected_fault"):
            self.assertIn(token,stage)
        self.assertNotIn("vary::finish",stage)
        self.assertNotIn("Authorization, Content-Type",stage)
        workflow=(ROOT/".github/workflows/ci.yml").read_text()
        self.assertEqual(workflow.count("| python -B scripts/tests/test_browser_login_stage_backport.py --native-summary"),2)

    def test_three_name_vary_preserved_without_public_diagnostics(self):
        for path in ("runtime/tests/browser_login_public_api.rs","runtime/src/http_server/browser_login_stage_tests.rs"):
            source=(ROOT/path).read_text()
            self.assertIn("Cookie, Authorization, Origin",source)
            self.assertNotIn("Authorization, Content-Type, Cookie, Origin, X-CSRF-Token",source)
            self.assertNotIn("nagi_browser_login_diag",source)

    def test_original_public_fault_assertions_remain_and_504_has_state_proofs(self):
        source=(ROOT/"runtime/tests/browser_login_public_api.rs").read_text()
        self.assertIn('assert_eq!(r.status, status, "committed_claim_status mode={mode}")',source)
        self.assertIn('assert_eq!(r.status, status, "issue_commit_status mode={mode}")',source)
        for token in ("assert_browser_deadline_state", "assert_consumed_browser_replay", "session_rows == 1", "handler_calls <= verifier_calls", "issue_committed", "prelogin_rows == 0", "!retry.auth_cookie() && !retry.deletion()"):
            self.assertIn(token,source)

    def test_lf_crlf_native9_and_closed_failures(self):
        raw=("running 9 tests\n"+"".join("test "+name+" ... ok\n" for name in EXPECTED)+"test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 200 filtered out; finished in 0.20s\n").encode()
        for value in (raw,raw.replace(b"\n",b"\r\n")):
            self.assertEqual(native_summary(value),"passed")
        for value in (raw.replace(b"9 passed",b"0 passed"),raw.replace(b"0 ignored",b"1 ignored"),raw.replace(EXPECTED[0].encode(),b"unknown"),raw+b"test unknown ... ok\n",raw+b"\r",raw+b"\xff",b"x"*65537):
            self.assertEqual(native_summary(value),"invalid")
        self.assertEqual(native_summary(b"error[E0382]: private\n"),"compile_failed")
        self.assertEqual(native_summary(b""),"unavailable")
        self.assertEqual(native_summary(b"running 9 tests\ntest result: FAILED. 8 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s\n"),"test_failed")

if __name__=="__main__":
    if sys.argv[1:]==["--native-summary"]:
        state=native_summary(sys.stdin.buffer.read(65537))
        print("browser_login_stage native9="+state)
        raise SystemExit(0 if state=="passed" else 1)
    unittest.main()
