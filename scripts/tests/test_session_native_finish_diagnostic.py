"""Source/fake grammar and collector cases; no Cargo/native execution."""
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts/ci"))
import session_native_finish_diagnostic as native
import session_deadline_diagnostic as public
import session_private_deadline_diagnostic as private
import test_session_deadline_diagnostic as old_public
import test_session_private_deadline_diagnostic as old_private


def facts(**changes):
    values = {"status": 504, "origin": 1, "terminal": 5, "diagnostic_valid": 1,
              "native_state": 0, "commit_result": 0, "cleanup_result": 0, "cleanup_reason": 0,
              **{name: native.UNSEEN_US for name in native.TIMES},
              "begins": 1, "commits": 0, "selected_deadline_passed": 1}
    values.update(changes)
    return values


def line(**changes):
    values = facts(**changes)
    return native.PREFIX + " " + " ".join(f"{name}={values[name]}" for name in native.FIELDS)


TOKEN = ("csrf_token_deadline_diag status=504 phase=2 event=1 db=10 exit=1 "
         "polls=3 begins=1 commands=14 commits=0 budget_ms=1000 entry_remaining_ms=999 "
         "phase_remaining_ms=870 exit_remaining_ms=0 terminal=5")
CASE = "guarded_partial_body_preparation_uses_original_security_deadline"


class NativeGrammarTests(unittest.TestCase):
    def test_all_closed_stages_and_error_cleanup_are_observable(self):
        controls = [facts(), facts(native_state=1, commit_enter_us=0),
                    facts(native_state=2, commit_enter_us=1, commit_return_us=2, commit_result=1),
                    facts(native_state=3, commit_enter_us=1, commit_return_us=2, commit_result=2)]
        for state, result, returned in [(4, 1, native.UNSEEN_US), (5, 2, 4), (6, 3, 4)]:
            controls.append(facts(native_state=state, commit_enter_us=1, commit_return_us=2,
                                  commit_result=2, cleanup_reason=2, cleanup_result=result,
                                  cleanup_enter_us=3, cleanup_return_us=returned))
        controls += [facts(native_state=7, settled_us=0),
                     facts(native_state=7, commit_enter_us=1, commit_return_us=2, commit_result=2,
                           cleanup_reason=2, cleanup_result=3, cleanup_enter_us=3, cleanup_return_us=4, settled_us=5)]
        for values in controls:
            with self.subTest(state=values["native_state"]):
                self.assertTrue(native.valid(values))
                self.assertEqual(native.parse(native.PREFIX + " " + " ".join(f"{k}={values[k]}" for k in native.FIELDS)), values)

    def test_zero_is_observed_and_sentinel_is_never_zero_duration(self):
        self.assertIsNotNone(native.parse(line(native_state=2, commit_result=1, commit_enter_us=0, commit_return_us=0)))
        self.assertIsNone(native.parse(line(native_state=2, commit_result=1, commit_enter_us=0)))
        self.assertIsNotNone(native.parse(line(native_state=2, commit_result=1, commit_enter_us=native.LIMIT_US, commit_return_us=native.LIMIT_US)))

    def test_state_result_reason_timestamp_and_overlap_inconsistency_fail_closed(self):
        variants = [dict(diagnostic_valid=0), dict(native_state=8), dict(origin=4), dict(commits=2), dict(begins=1024),
                    dict(commit_enter_us=native.UNSEEN_US + 1), dict(commit_return_us=0), dict(settled_us=0),
                    dict(native_state=1), dict(native_state=3, commit_result=1, commit_enter_us=1, commit_return_us=2),
                    dict(native_state=5, cleanup_result=2, cleanup_enter_us=3, cleanup_return_us=2, cleanup_reason=1),
                    dict(native_state=4, cleanup_result=1, cleanup_enter_us=3, cleanup_return_us=4, cleanup_reason=1),
                    dict(native_state=5, cleanup_result=2, cleanup_enter_us=3, cleanup_return_us=4, cleanup_reason=2),
                    dict(native_state=7, cleanup_result=1, cleanup_enter_us=3, cleanup_reason=1, settled_us=5),
                    dict(native_state=7, commit_result=1, commit_enter_us=1, commit_return_us=2, settled_us=1)]
        for changes in variants:
            with self.subTest(changes=changes):
                self.assertIsNone(native.parse(line(**changes)))

    def test_exact_whitelist_no_secret_tail_raw_instant_or_boolean_coercion(self):
        secret = "PRIVATE_cookie_SQL_error_/owned/path"
        for bad in [line()+" secret="+secret, secret+line(), line().replace("begins=1", "begins=01"),
                    line()+"\x00", line().replace("native_state=0", "native_state=-1")]:
            self.assertIsNone(native.parse(bad))
        values = facts(); values["diagnostic_valid"] = True
        self.assertFalse(native.valid(values))
        values = facts(); values["payload"] = secret
        self.assertFalse(native.valid(values))

    def test_pair_must_match_original_terminal_and_transaction_counts(self):
        legacy = public.parse_token_marker(TOKEN)
        self.assertTrue(native.paired(facts(), legacy, 1))
        for changes in [dict(origin=0, terminal=0), dict(terminal=4), dict(begins=2), dict(commits=1)]:
            self.assertFalse(native.paired(facts(**changes), legacy, 1))
        self.assertFalse(native.paired(facts(), None, 1))
        self.assertTrue(native.paired(facts(selected_deadline_passed=0), legacy, 1))


class ExistingCollectorIntegrationTests(unittest.TestCase):
    def public_record(self, markers):
        raw = old_public.cargo_log(public.SESSION_CASES, failed={CASE}, markers={CASE: markers})
        return public.diagnose(raw, "valid", 101, old_public.cargo_log(public.BROWSER_CASES), "valid", 0)

    def test_public_failed_case_retains_new_closed_snapshot_with_legacy(self):
        record, status = self.public_record([TOKEN, line(native_state=1, commit_enter_us=130000)])
        self.assertEqual((status, record["status"]), (1, "tests_failed"))
        native_facts = record["suites"]["session"]["native_finish"]
        self.assertEqual(native_facts[0]["test"], CASE)
        self.assertEqual(native_facts[0]["commit_return_us"], native.UNSEEN_US)
        self.assertNotEqual(public.closed_verdict_line(record), "session_deadline_verdict unavailable")
        native_facts[0]["native_state"] = 99
        self.assertEqual(public.closed_verdict_line(record), "session_deadline_verdict unavailable")

    def test_orphan_duplicate_mixed_origin_and_late_unattributed_lines_are_invalid(self):
        variants = [[line()], [TOKEN, line(), line()], [TOKEN, line(origin=3, terminal=3)],
                    [TOKEN, "unrelated callback", line()], [TOKEN, line()+" snapshot=1"],
                    [TOKEN, line(begins=2)], [TOKEN, line(diagnostic_valid=0)]]
        for markers in variants:
            with self.subTest(markers=markers):
                record, _ = self.public_record(markers)
                self.assertEqual(record["status"], "diagnostic_invalid")

    def test_private_calibration_and_functional_case_share_strict_native_pair(self):
        legacy = old_private.witness()
        native_line = line(origin=3, terminal=1, begins=0)
        result = private.collect("prep_timer", old_private.raw(markers=[legacy, native_line]), 0)
        self.assertEqual(result["state"], "passed")
        self.assertEqual(result["native_finish"][0]["native_state"], 0)
        observed_early = private.collect("prep_timer", old_private.raw(markers=[legacy, native_line.replace("selected_deadline_passed=1", "selected_deadline_passed=0")]), 0)
        self.assertEqual(observed_early["state"], "passed")
        self.assertEqual(observed_early["native_finish"][0]["selected_deadline_passed"], 0)
        for lines in [[native_line], [legacy, native_line, native_line], [legacy, native_line.replace("begins=0", "begins=1")],
                      [legacy, native_line + " extra=1"]]:
            self.assertEqual(private.collect("prep_timer", old_private.raw(markers=lines), 0)["state"], "invalid")

    def test_delivery_private_timeout_is_captured_without_new_public_getter(self):
        case = public.DELIVERY_CASES["applied"]
        legacy = old_private.witness(3, 1)
        native_line = line(origin=3, terminal=3, begins=0)
        raw = old_public.cargo_log({case}, failed={case}, markers={case: [legacy, native_line]})
        record, status = public.diagnose_delivery(raw, "valid", 101, "applied")
        self.assertEqual((status, record["status"]), (1, "tests_failed"))
        self.assertEqual(record["suites"]["delivery"]["native_finish"][0]["terminal"], 3)

    def test_bad_input_does_not_render_payload_or_reclassify_failure(self):
        secret = "PRIVATE_cookie_SQL_error_/owned/path"
        record, _ = self.public_record([TOKEN, line()+" error="+secret])
        self.assertEqual(record["status"], "diagnostic_invalid")
        self.assertNotIn(secret, json.dumps(record))
        # Legacy archived observations remain readable; no new fact is invented.
        record, status = self.public_record([TOKEN])
        self.assertEqual((status, record["status"]), (1, "tests_failed"))
        self.assertEqual(record["suites"]["session"]["native_finish"], [])


if __name__ == "__main__":
    unittest.main()
