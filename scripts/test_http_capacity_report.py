"""Check that cumulative load reports cannot conceal a bad later interval."""
import copy
import unittest

from http_capacity_report import metrics, windows


def report(requests, buckets, end, statuses=None):
    return {"requests": requests, "buckets": buckets, "status_codes": statuses or {"200": requests},
            "rate": 100, "throughput": 100, "latencies": {"50th": 100, "95th": 200, "99th": 300, "max": 500},
            "earliest": "2026-10-01T00:00:00Z", "end": end}


class ReportTests(unittest.TestCase):
    def test_slow_later_interval_is_not_hidden_by_cumulative_counts(self):
        early = report(10000, {"0": 10000, "1000000": 0, "32000000": 0, "64000000": 0}, "2026-10-01T00:01:00Z")
        late = report(10100, {"0": 10000, "1000000": 0, "32000000": 100, "64000000": 0}, "2026-10-01T00:02:00Z")
        rows = windows([early, late])
        self.assertEqual(rows[1]["requests"], 100)
        self.assertEqual(rows[1]["p99_bucket_upper_ms"], 64)

    def test_transport_failures_are_counted(self):
        value = report(10, {"0": 10, "1000000": 0}, "2026-10-01T00:00:01Z", {"200": 7, "0": 3})
        self.assertEqual(metrics(value)["non_200_or_transport_errors"], 3)

    def test_inconsistent_counts_are_rejected(self):
        value = report(10, {"0": 9, "1000000": 0}, "2026-10-01T00:00:01Z")
        with self.assertRaises(AssertionError):
            metrics(value)

    def test_histogram_regression_is_rejected_even_if_total_grows(self):
        early = report(10, {"0": 10, "1000000": 0, "2000000": 0}, "2026-10-01T00:00:01Z")
        late = copy.deepcopy(early)
        late.update(requests=11, status_codes={"200": 11}, buckets={"0": 9, "1000000": 2, "2000000": 0}, end="2026-10-01T00:00:02Z")
        with self.assertRaises(AssertionError):
            windows([early, late])

    def test_overflow_is_reported_as_unknown_bound(self):
        value = report(10, {"0": 0, "1000000": 10}, "2026-10-01T00:00:01Z")
        self.assertIsNone(windows([value])[0]["p99_bucket_upper_ms"])

    def test_current_vegeta_json_without_buckets_keeps_counts_and_labels_cumulative_p99(self):
        value = report(10, {}, "2026-10-01T00:00:01Z")
        del value["buckets"]

        self.assertEqual(metrics(value)["requests"], 10)
        row = windows([value])[0]
        self.assertIsNone(row["p99_bucket_upper_ms"])
        self.assertEqual(row["p99_cumulative_ms"], 0.0003)

    def test_mixed_histogram_formats_are_rejected(self):
        old = report(10, {"0": 10}, "2026-10-01T00:00:01Z")
        current = report(20, {}, "2026-10-01T00:00:02Z")
        del current["buckets"]

        with self.assertRaisesRegex(AssertionError, "Mixed histogram"):
            windows([old, current])


if __name__ == "__main__":
    unittest.main()
