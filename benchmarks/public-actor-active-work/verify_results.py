#!/usr/bin/env python3
"""Recompute each saved phase and aggregate, without executing the benchmark."""
import argparse
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('probe', ROOT / 'scripts/public_supervisor_probe.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    directory = args.directory
    summary = json.loads((directory / 'summary.json').read_text())
    assert summary['status'] == 'passed' and len(summary['cases']) == 18
    total = 0
    for case in summary['cases']:
        assert case['status'] == 'passed' and case['exit_code'] == 0
        assert case['released_fd_delta'] == case['released_task_delta'] == 0
        cpus = len(case['cpu_affinity'])
        label = f'cpus-{cpus}-actors-{case["actors"]}-round-{case["round"]}'
        with gzip.open(directory / f'{label}.jsonl.gz', 'rt') as stream:
            records = [json.loads(line) for line in stream]
        assert [r['checkpoint'] for r in records if 'checkpoint' in r] == [0, 1, 2, 3]
        phases = [r for r in records if 'phase' in r]
        assert len(phases) == 1 and phases[0]['recovery'] == [1, 1, 2, 0]
        phase = probe.summarize_phase(phases[0], case['iterations_per_client'], True,
                                      case['active_clients'])
        phase.update(public_failure_count=1, public_restart_count=1,
                     recovered_generation=2, recovered_state=0)
        assert phase == case['phases'][0]
        assert phase['failures'] == 0
        assert phase['successes'] == case['active_clients'] * case['iterations_per_client']
        assert phase['cpu_clock_positive'] and phase['healthy_completions_during_fault'] > 0
        assert case['observed_peak_rss_bytes'] < summary['limits']['sampled_rss_bytes']
        total += phase['successes']
    assert total == 2_520_000
    aggregate = []
    fields = ['completed_successes_per_second', 'process_cpu_ns_per_success', 'elapsed_ns',
              'latency_p50_ns', 'latency_p95_ns', 'latency_p99_ns',
              'healthy_completions_during_fault', 'fault_duration_ns',
              'healthy_latency_p99_ns_during_fault', 'common_interval_jain_index']
    for cpus in (1, 2):
        for clients in (4, 16, 64):
            cases = [c for c in summary['cases']
                     if len(c['cpu_affinity']) == cpus and c['active_clients'] == clients]
            assert len(cases) == 3
            row = {'cpu_affinity_count': cpus, 'active_clients': clients, 'total_actors': clients + 1}
            for field in fields:
                values = [c['phases'][0][field] for c in cases]
                row[field] = {'median': statistics.median(values), 'min': min(values), 'max': max(values)}
            for field in ['observed_peak_rss_bytes', 'retained_rss_delta_bytes_after_release']:
                values = [c[field] for c in cases]
                row[field] = {'median': statistics.median(values), 'min': min(values), 'max': max(values)}
            row['rss_at_ready_median'] = statistics.median(c['checkpoints'][1]['rss_bytes'] for c in cases)
            row['rss_after_release_median'] = statistics.median(c['checkpoints'][3]['rss_bytes'] for c in cases)
            aggregate.append(row)
    (directory / 'aggregate.json').write_text(json.dumps(aggregate, indent=2) + '\n')
    verification = {'cases_recomputed': 18, 'successful_replies': total, 'normal_failures': 0,
                    'injected_worker_failures': 18, 'public_restarts': 18,
                    'released_fd_task_deltas_zero': True,
                    'raw_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                                   for p in sorted(directory.glob('*.jsonl.gz'))}}
    (directory / 'verification.json').write_text(json.dumps(verification, indent=2) + '\n')
    print(json.dumps({k: v for k, v in verification.items() if k != 'raw_sha256'}))


if __name__ == '__main__':
    main()
