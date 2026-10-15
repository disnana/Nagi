import unittest

from assert_sqlite_test_summary import expected_count, verify


def summary(status="ok", passed=80, failed=0, ignored=0):
    return (f"test result: {status}. {passed} passed; {failed} failed; {ignored} ignored; "
            "0 measured; 339 filtered out; finished in 1.23s\n")


class SqliteTestSummaryTests(unittest.TestCase):
    def test_platform_counts_are_exact_and_nonzero(self):
        self.assertEqual(expected_count("win32"), 80)
        self.assertEqual(expected_count("linux"), 80)
        self.assertEqual(expected_count("darwin"), 80)
        self.assertEqual(verify(summary(), "win32"), 80)
        self.assertEqual(verify(summary(), "linux"), 80)
        self.assertEqual(verify(summary(), "darwin"), 80)

    def test_rejects_wrong_or_zero_count(self):
        for platform, passed in (("win32", 81), ("linux", 81), ("linux", 0)):
            with self.subTest(platform=platform, passed=passed), self.assertRaises(AssertionError):
                verify(summary(passed=passed), platform)

    def test_rejects_failures_ignored_and_failed_status(self):
        for text in (summary(failed=1, passed=79), summary(ignored=1, passed=79),
                     summary(status="FAILED", failed=1, passed=80)):
            with self.subTest(text=text), self.assertRaises(AssertionError):
                verify(text, "linux")

    def test_requires_one_summary_and_supported_platform(self):
        with self.assertRaises(AssertionError):
            verify("no test summary\n", "linux")
        with self.assertRaises(AssertionError):
            verify(summary() + summary(), "linux")
        with self.assertRaises(ValueError):
            expected_count("unknown")


if __name__ == "__main__":
    unittest.main()
