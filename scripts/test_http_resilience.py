import argparse
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from http_resilience import (
    fixed_request_budget,
    percentile,
    validate_exact_pacing_rate,
    validate_overload_metrics,
    validate_request_ceiling,
)


class HttpResilienceTests(unittest.TestCase):
    def test_fixed_request_budget_includes_load_and_server_preflight(self):
        args = argparse.Namespace(
            normal_rate=100,
            normal_seconds=3,
            overload_rate=10_000,
            overload_seconds=3,
            recovery_seconds=5,
            probe_interval=.1,
        )

        self.assertEqual(fixed_request_budget(args), 30_405)

    def test_rate_must_divide_nanosecond_pacing_base(self):
        validate_exact_pacing_rate(10_000)
        with self.assertRaisesRegex(ValueError, "exact divisor"):
            validate_exact_pacing_rate(15_000)

    def test_actual_request_count_cannot_exceed_conservative_ceiling(self):
        validate_request_ceiling({"requests": 301}, 100, 3)
        with self.assertRaisesRegex(RuntimeError, "request ceiling"):
            validate_request_ceiling({"requests": 302}, 100, 3)

    def test_percentile_uses_an_observed_value(self):
        self.assertEqual(percentile([5.0, 1.0, 3.0, 2.0, 4.0], .95), 4.0)

    def test_percentile_rejects_empty_input(self):
        with self.assertRaises(ValueError):
            percentile([], .99)

    def test_overload_accepts_200_429_and_bounded_capacity_503(self):
        validate_overload_metrics({
            "requests": 4,
            "status_codes": {"200": 1, "429": 2, "503": 1},
            "errors": ["429 Too Many Requests", "503 Service Unavailable"],
        })

    def test_overload_rejects_hidden_server_error(self):
        with self.assertRaisesRegex(RuntimeError, "unexpected statuses"):
            validate_overload_metrics({
                "requests": 3,
                "status_codes": {"200": 1, "429": 1, "500": 1},
                "errors": ["429 Too Many Requests", "500 Internal Server Error"],
            })


if __name__ == "__main__":
    unittest.main()
