import unittest

import assert_go_backend_summary as summary


def log(running=14, status="ok", passed=14, failed=0, ignored=0, measured=0, filtered=0):
    return (
        f"running {running} tests\n"
        f"test result: {status}. {passed} passed; {failed} failed; {ignored} ignored; "
        f"{measured} measured; {filtered} filtered out; finished in 1.00s\n"
    )


class GoBackendSummaryTests(unittest.TestCase):
    def test_accepts_one_exact_successful_execution(self):
        summary.validate(log(), 14)
        summary.validate(log(running=13, passed=13), 13)

    def test_rejects_zero_missing_duplicate_partial_or_failed_execution(self):
        bad = (
            "",
            log(running=0, passed=0),
            log() + log(),
            log(passed=13, filtered=1),
            log(status="FAILED", passed=13, failed=1),
            log(ignored=1, passed=13),
        )
        for text in bad:
            with self.subTest(text=text), self.assertRaises(ValueError):
                summary.validate(text, 14)

    def test_rejects_unrelated_or_truncated_success_text(self):
        for text in ("14 tests passed\n", "running 14 tests\n", "test result: ok. 14 passed\n"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                summary.validate(text, 14)


if __name__ == "__main__":
    unittest.main()
