"""Small source-only checkpoint guards, never native ownership evidence."""
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import unittest

_SPEC = importlib.util.spec_from_file_location("request_projection", Path(__file__).resolve().parents[1]/"ci/sf07_request_projection.py")
projection = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(projection)

ROOT = Path(os.environ.get("SF07_SOURCE_ROOT", Path(__file__).resolve().parents[2]))


def canonical_bytes(data):
    data = data.replace(b"\r\n", b"\n")
    if b"\r" in data:
        raise ValueError("lone carriage return")
    return data


def original_bytes(path, data):
    data = canonical_bytes(data)
    return projection.inverse_quota(data.decode()).encode() if path == "runtime/src/security_sf07_private/quota.rs" else data


def source(path):
    target = ROOT / path
    return canonical_bytes(target.read_bytes()).decode("utf-8") if target.exists() else ""


class BridgeBoundaryTests(unittest.TestCase):
    def test_checked_native_profile_includes_peak_before_quota_publication(self):
        text = source("runtime/src/http_server/response_budget.rs")
        self.assertTrue("fn checked_wire(" in text, "native planned constructor missing")
        self.assertLess(text.index("WireLayout::checked"), text.index("Quota::checked_catalog"))
        self.assertIn("sum(charge, wire.map_or(0, |layout| layout.peak_bytes))?", text)

    def test_native_headers_and_frame_use_original_claim_and_retained_wrapper(self):
        text = source("runtime/src/http_server/response_wire.rs")
        self.assertTrue("pub(in crate::http_server) struct ChargedNativeHeaders" in text,
                        "charged native wrapper missing")
        self.assertIn("Arc::clone(&produced.headers.charge)", text)
        self.assertIn("Bytes::from_owner(produced.body)", text)
        self.assertLess(text.index("for header in &produced.headers.entries"), text.index("drop(produced.headers)"))
        headers = text.split("struct ChargedNativeHeaders", 1)[1].split("impl ChargedNativeHeaders", 1)[0]
        self.assertLess(headers.index("map: HeaderMap"), headers.index("charge: Arc<Reservation>"))

    def test_no_raw_native_map_or_clone_escape_and_head_keeps_wrapper(self):
        text = source("runtime/src/http_server/response_wire.rs")
        self.assertTrue("fn suppress_body_for_head" in text, "HEAD owner boundary missing")
        self.assertIn("-> (ChargedNativeHeaders, Bytes)", text)
        borrowed_map = "pub(in crate::http_server) fn map(&self) -> &HeaderMap {\n        &self.map\n    }"
        self.assertEqual(text.count(borrowed_map), 1)
        self.assertNotRegex(text.replace(borrowed_map, '', 1), r"pub[^\n]*fn[^\n]*->[^\n]*(?:&|\b)HeaderMap")
        self.assertNotIn('-> &mut HeaderMap', text)
        self.assertNotRegex(text, r"impl Clone for (?:ChargedWire|ChargedNativeHeaders)")
        self.assertNotIn("self.headers.map.clone()", text)

    def test_fallback_has_independent_checked_ledger_and_single_final_close(self):
        text = source("runtime/src/http_server/response_wire.rs")
        self.assertTrue("impl FallbackLedger" in text, "independent fallback missing")
        constructor = text.split("impl FallbackLedger",1)[1]
        self.assertIn("Producer::checked_with(", constructor)
        self.assertIn("limits.responses < accepted_connections", constructor)
        self.assertIn("product(producer.limits.charge, accepted_connections)?", constructor)
        self.assertIn("Err(_) => FallbackOutcome::close()", constructor)
        self.assertNotIn("self.produce(", constructor)
        self.assertNotIn("serde_json", constructor)
        self.assertNotIn("self.producer.text(", constructor)
        self.assertNotIn("self.producer.bytes(", constructor)

    def test_existing_quota_and_ten_producer_oracles_are_unchanged(self):
        expected = {
            "runtime/src/security_sf07_private/quota.rs":"a7031c5a4ad071d719f6efc41dd8889d9582169602197a337c94a54cbc4f6861",
            "runtime/src/http_server/response_budget_tests.rs":"c9337486bac6d16c405c577447de1e36160cb898573afba271c5b38d7cf77bad",
        }
        for path, digest in expected.items():
            self.assertEqual(hashlib.sha256(original_bytes(path, (ROOT/path).read_bytes())).hexdigest(), digest)
        producer = source("runtime/src/http_server/response_budget.rs")
        self.assertIn("use crate::security_sf07_private::quota;", producer)
        self.assertNotIn('#[path = "../security_sf07_private/quota.rs"]', producer)
        self.assertEqual(source("runtime/src/security_sf07_private/mod.rs").count("pub(crate) mod quota;"), 1)
        self.assertFalse((ROOT/"runtime/src/http_server/response_budget_quota.rs").exists())

    def test_source_hash_preserves_lf_crlf_identity_and_rejects_mutation_and_lone_cr(self):
        expected = {
            "runtime/src/security_sf07_private/quota.rs":"a7031c5a4ad071d719f6efc41dd8889d9582169602197a337c94a54cbc4f6861",
            "runtime/src/http_server/response_budget_tests.rs":"c9337486bac6d16c405c577447de1e36160cb898573afba271c5b38d7cf77bad",
        }
        for path, digest in expected.items():
            lf = canonical_bytes((ROOT/path).read_bytes())
            for data in (lf, lf.replace(b"\n", b"\r\n")):
                self.assertEqual(hashlib.sha256(original_bytes(path, data)).hexdigest(), digest)
                self.assertNotEqual(hashlib.sha256(original_bytes(path, data + b" ")).hexdigest(), digest)
            with self.assertRaises(ValueError):
                canonical_bytes(lf + b"\r")

    def test_catalog_extension_inverse_requires_the_exact_audited_block(self):
        text = source("runtime/src/security_sf07_private/quota.rs")
        start = "// SF07_U2_RETAINED_CATALOG_BEGIN"
        end = "// SF07_U2_RETAINED_CATALOG_END\n\n"
        self.assertEqual(text.count(start), 1)
        self.assertEqual(text.count(end), 1)
        block = text[text.index(start):text.index(end) + len(end)]
        expected = "a7031c5a4ad071d719f6efc41dd8889d9582169602197a337c94a54cbc4f6861"
        self.assertEqual(hashlib.sha256(projection.inverse_quota(text).encode()).hexdigest(), expected)
        # Restoring U1's test-only retained catalogue inside the U4 production
        # profile must fail, even though that exact block is historical evidence.
        historical_block = block.replace("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub(crate) struct CatalogRetention", "#[cfg(test)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub(crate) struct CatalogRetention", 1)
        historical_block = historical_block.replace("impl CatalogRetention {", "#[cfg(test)]\nimpl CatalogRetention {", 1)
        historical_block = historical_block.replace("impl Quota {", "#[cfg(test)]\nimpl Quota {", 1)
        self.assertEqual(hashlib.sha256(historical_block.encode()).hexdigest(),
                         "33a82908cee4b72230b2d5ac1c5b966ba010f673e33562c4092d2b49cc0d39a3")
        with self.assertRaises(ValueError):
            projection.inverse_quota(text.replace(block, historical_block, 1))
        without = text.replace(block, "", 1)
        self.assertEqual(hashlib.sha256(projection.inverse_quota(without).encode()).hexdigest(), expected)
        for invalid in (text.replace(start, start + " altered", 1),
                        text.replace(end, end.rstrip(), 1),
                        text.replace(block, block + block, 1),
                        text.replace(block, block.replace("retained_catalog_minimum", "changed", 1), 1)):
            with self.assertRaises(ValueError):
                projection.inverse_quota(invalid)

    def test_all_thirteen_native_oracles_registered_with_existing_nineteen(self):
        tests = source("runtime/src/http_server/response_wire_tests.rs")
        self.assertEqual(tests.count("#[test]"), 13)
        for token in ("header_only", "frame_clone", "head_empty", "fallback_uses_independent",
                      "without_another_allocation", "foreign_owner", "send_sync",
                      "closed_transport_and_internal_bodies_keep_independent_finite_fallback_credit"):
            self.assertTrue(token in tests, "native boundary oracle missing")
        self.assertEqual(source("runtime/src/http_server.rs").count("mod response_budget;"), 1)
        self.assertNotIn("#[cfg(test)]\nmod response_budget;", source("runtime/src/http_server.rs"))
        self.assertIn('#[path = "response_wire_tests.rs"]', source("runtime/src/http_server/response_wire.rs"))


if __name__ == "__main__":
    unittest.main()
