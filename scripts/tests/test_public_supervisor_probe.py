"""Measurement accounting must never turn rejected work into throughput."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('public_supervisor_probe', Path(__file__).parents[1] / 'public_supervisor_probe.py')
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


class MeasurementAccounting(unittest.TestCase):
    def phase(self):
        return {'phase': 1, 'elapsed_ns': 1000, 'cpu_ns': 500,
                'clients': [[[10, 20, True], [30, 80, False]], [[10, 40, True], [50, 90, True]]],
                'fault_interval_ns': [15, 45]}

    def test_failed_reply_is_excluded_from_rate_cpu_denominator_and_latency(self):
        result = probe.summarize_phase(self.phase(), 2, True)
        self.assertEqual(result['successes'], 3)
        self.assertEqual(result['failures'], 1)
        self.assertEqual(result['completed_successes_per_second'], 3_000_000)
        self.assertEqual(result['process_cpu_ns_per_success'], 500/3)
        self.assertEqual(result['latency_p99_ns'], 40)
        self.assertEqual(result['healthy_completions_during_fault'], 2)
        self.assertEqual(result['healthy_latency_p99_ns_during_fault'], 30)

    def test_zero_cpu_observation_is_unavailable_not_free_work(self):
        value = self.phase()
        value['cpu_ns'] = 0
        result = probe.summarize_phase(value, 2, True)
        self.assertFalse(result['cpu_clock_positive'])
        self.assertIsNone(result['process_cpu_ns_per_success'])

    def test_incomplete_samples_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'missing completed'):
            probe.summarize_phase(self.phase(), 3, True)

    def test_missing_recovery_interval_is_rejected(self):
        value = self.phase()
        value['fault_interval_ns'][1] = None
        with self.assertRaisesRegex(ValueError, 'fault interval'):
            probe.summarize_phase(value, 2, True)

    def test_fault_cannot_be_reported_in_unfaulted_phase(self):
        with self.assertRaisesRegex(ValueError, 'unexpected fault'):
            probe.summarize_phase(self.phase(), 2, False)

    def test_negative_latency_is_rejected(self):
        value = self.phase()
        value['clients'][0][0][1] = 0
        with self.assertRaisesRegex(ValueError, 'invalid call'):
            probe.summarize_phase(value, 2, True)

    def test_no_success_has_no_latency_or_cpu_per_success(self):
        value = self.phase()
        value['clients'] = [[[10, 20, False]], [[10, 20, False]]]
        result = probe.summarize_phase(value, 1, True)
        self.assertIsNone(result['process_cpu_ns_per_success'])
        self.assertIsNone(result['latency_p99_ns'])
        self.assertIsNone(result['common_interval_jain_index'])

    def test_capacity_guard_is_enforced(self):
        with patch.object(probe.shutil, 'disk_usage', return_value=type('Disk', (), {'free': probe.MIN_FREE-1})()):
            with self.assertRaisesRegex(RuntimeError, 'disk guard'):
                probe.capacity(Path('.'))

    def test_exact_capacity_boundary_is_permitted(self):
        with patch.object(probe.shutil, 'disk_usage', return_value=type('Disk', (), {'free': probe.MIN_FREE})()):
            self.assertEqual(probe.capacity(Path('.')), probe.MIN_FREE)

    def test_nearest_rank_quantile_is_not_interpolated(self):
        self.assertEqual(probe.percentile(list(range(1, 101)), .99), 99)
        self.assertIsNone(probe.percentile([], .99))


if __name__ == '__main__':
    unittest.main()
