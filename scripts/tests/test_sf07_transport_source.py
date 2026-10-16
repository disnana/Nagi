"""Small private transport source/inventory guards; NOT native lifetime proof."""
import ast
import hashlib
import importlib.util
from pathlib import Path
import re
import unittest

_SPEC = importlib.util.spec_from_file_location("request_projection", Path(__file__).resolve().parents[1]/"ci/sf07_request_projection.py")
projection = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(projection)

ROOT = Path(__file__).resolve().parents[2]


def text(path):
    return (ROOT/path).read_text()


class TransportSourceTests(unittest.TestCase):
    def test_native_inventory_is_explicit_and_registered_in_both_required_steps(self):
        tests = text("runtime/src/http_server/response_send_tests.rs")
        names = re.findall(r"#\[tokio::test\]\s*async fn ([a-zA-Z0-9_]+)\(", tests)
        self.assertEqual(len(names), 9)
        self.assertEqual(len(set(names)), 9)
        # U2 appends one managed-response oracle; all eight original bodies
        # remain byte-identical after universal newline normalization.
        self.assertEqual(hashlib.sha256(tests[:19665].encode()).hexdigest(),
                         "0b35d16393447d1c77bf23399bed29a3e9bf0bdd40f8f1816be0bd392a807587")
        spec = importlib.util.spec_from_file_location("transport_summary", ROOT/"scripts/ci/sf07_checkpoint_summary.py")
        summary = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(summary)
        self.assertEqual(set(summary.EXPECTED["transport"]),
                         {"http_server::response_send::tests::" + name for name in names})
        workflow = text(".github/workflows/ci.yml")
        self.assertEqual(workflow.count("| python -B scripts/ci/sf07_checkpoint_summary.py transport"), 2)
        self.assertEqual(workflow.count("python -B scripts/tests/test_sf07_transport_source.py"), 2)

    def test_original_http_production_path_and_all_frozen_digest_copies(self):
        http = (ROOT/"runtime/src/http_server.rs").read_bytes().replace(b"\r\n", b"\n")
        self.assertNotIn(b"\r", http)
        self.assertEqual(http.count(b"#[cfg(test)]\nmod response_send;"), 1)
        # U1 reverses only explicit header ownership changes, preserving all
        # original order/diagnostic tokens against exact9b7 base32b3ac6.
        self.assertEqual(projection.digest(projection.inverse_http(http.decode())),
                         projection.BASE_TOKENS['http'])
        digest = hashlib.sha256(http).hexdigest()
        self.assertEqual(text(".github/workflows/ci.yml").count(digest), 2)
        self.assertEqual(text("scripts/ci/session_private_deadline_diagnostic.py").count(digest), 1)

    def test_response_claim_is_published_before_the_native_map_move(self):
        wire = text("runtime/src/http_server/response_wire.rs")
        transfer = wire.split("fn transfer_native(", 1)[1].split("fn suppress_body_for_head", 1)[0]
        self.assertLess(transfer.index("ledger.accepts(&self)"), transfer.index("ledger.publish("))
        self.assertLess(transfer.index("ledger.publish("), transfer.index("self.into_parts()"))
        self.assertIn("*response.headers_mut() = map", transfer)
        self.assertNotIn("extensions", transfer)
        self.assertNotRegex(transfer, r"\.map\.clone\(")

    def test_cached_map_and_prior_claim_share_one_header_owner_without_clone(self):
        send = text("runtime/src/http_server/response_send.rs")
        owner = text("runtime/src/http_server/request_headers.rs")
        headers = owner.split("struct ReceivedHeaders", 1)[1].split("impl ReceivedHeaders", 1)[0]
        self.assertLess(headers.index("map: HeaderMap"), headers.index("_prior: Option<Arc<Reservation>>"))
        self.assertIn("_prior: prior", owner)
        self.assertIn("ReceivedHeaders::new(parts.headers, prior)", send)
        self.assertIn("Arc::clone(&self.headers)", send)
        self.assertNotRegex(send, r"(?:headers|map)\.clone\(")
        state = send.split("struct SendState", 1)[1].split("struct SendLedger", 1)[0]
        self.assertNotIn("Vec", state)
        self.assertIn("cached: Option<Arc<Reservation>>", state)
        self.assertIn("if state.cached.is_some()", send)

    def test_native_destruction_precedes_cache_guard_and_original_permit(self):
        send = text("runtime/src/http_server/response_send.rs")
        driver = send.split("struct NativeDriver<F>", 1)[1].split("impl<F:", 1)[0]
        self.assertLess(driver.index("native: Option<F>"), driver.index("_after_native: AfterNative"))
        self.assertLess(driver.index("_after_native: AfterNative"), driver.index("_permit: OwnedSemaphorePermit"))
        self.assertIn("drop(this.native.take());", send)
        self.assertNotIn("impl<F> Drop for NativeDriver", send)
        self.assertNotIn("Box::", send)
        guard = send.split("impl Drop for AfterNative", 1)[1].split("struct NativeDriver", 1)[0]
        self.assertIn("state.cached.take()", guard)
        self.assertIn("state.closed = true", guard)

    def test_checked_peak_includes_concrete_native_owner_metadata(self):
        wire = text("runtime/src/http_server/response_wire.rs")
        for token in ("size_of::<super::super::response_send::SendLedger>()",
                      "size_of::<super::super::response_send::ReceivedHeaders>()",
                      "product(4, size_of::<usize>())?"):
            self.assertIn(token, wire)
        self.assertIn("peak_bytes: sum(", wire)

    def test_actual_native_oracles_cover_lifetimes_and_transport_close(self):
        tests = text("runtime/src/http_server/response_send_tests.rs")
        for token in ("TcpListener::bind", "head_and_empty", "past_native_shutdown",
                      "pipelined_requests", "fallback_is_independent", "foreign_wire",
                      "parse_failure", "abort_before_actual_join", "handler_panic",
                      "unpolled_and_ready", "task.abort()", "is_cancelled()",
                      "is_panic()", "fallback.allocation_attempts()", "eof(&mut client)"):
            self.assertIn(token, tests)
        self.assertNotIn("#[ignore", tests)
        self.assertNotIn("try_again", tests)


if __name__ == "__main__":
    unittest.main()
