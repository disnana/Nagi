import unittest

from assert_call_member_test_summary import verify


def summary(status="ok", passed=5, failed=0, ignored=0):
    return (f"test result: {status}. {passed} passed; {failed} failed; {ignored} ignored; "
            "0 measured; 99 filtered out; finished in 0.01s\n")


class CallMemberSummaryTests(unittest.TestCase):
    def test_accepts_exact_nonzero_success(self):
        self.assertEqual(verify(summary()), 5)

    def test_rejects_zero_or_wrong_count(self):
        for passed in (0, 4, 6):
            with self.subTest(passed=passed), self.assertRaises(AssertionError):
                verify(summary(passed=passed))

    def test_rejects_failed_ignored_or_failed_status(self):
        for text in (summary(passed=4, failed=1), summary(passed=4, ignored=1),
                     summary(status="FAILED", passed=5)):
            with self.subTest(text=text), self.assertRaises(AssertionError):
                verify(text)

    def test_requires_one_summary(self):
        with self.assertRaises(AssertionError):
            verify("no summary\n")
        with self.assertRaises(AssertionError):
            verify(summary() + summary())


if __name__ == "__main__":
    unittest.main()
