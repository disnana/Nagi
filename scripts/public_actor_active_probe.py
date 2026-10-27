#!/usr/bin/env python3
"""Finite actual public Nagi Actor active-work measurement, Linux64 only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sys

import public_supervisor_probe as probe


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--clients', type=int, nargs='+', default=[4, 16, 64])
    parser.add_argument('--cpus', type=int, nargs='+', default=[1, 2])
    parser.add_argument('--iterations', type=int, default=5000)
    parser.add_argument('--rounds', type=int, default=3)
    args = parser.parse_args()
    if platform.system() != 'Linux' or platform.architecture()[0] != '64bit':
        parser.error('Linux64 only')
    available = sorted(os.sched_getaffinity(0))
    if (not args.clients or len(args.clients) > 3 or len(set(args.clients)) != len(args.clients)
            or any(not 1 <= n <= 64 for n in args.clients)
            or not 1 <= args.iterations <= 5000 or not 1 <= args.rounds <= 3):
        parser.error('clients1..64 (max3 stages), iterations1..5000, rounds1..3')
    if (not args.cpus or len(set(args.cpus)) != len(args.cpus)
            or any(n not in (1, 2) or n > len(available) for n in args.cpus)):
        parser.error('available CPU affinity counts1 and/or2')
    binary = args.binary.resolve(strict=True)
    probe.capacity(args.output.parent)
    args.output.mkdir()
    result = {
        'schema': 1, 'status': 'failed', 'binary': str(binary),
        'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'platform': platform.platform(), 'host_cpu_affinity': available,
        'cpu_quota': Path('/sys/fs/cgroup/cpu.max').read_text().strip(),
        'limits': {'wall_seconds': probe.WALL_SECONDS, 'cpu_seconds': probe.CPU_SECONDS,
                   'sampled_rss_bytes': probe.MAX_RSS, 'minimum_free_bytes': probe.MIN_FREE,
                   'target_budget_bytes': 1024**3, 'hard_cgroup_enforced': False},
        'disk_free_before': probe.capacity(args.output), 'cases': [],
    }
    try:
        for cpus in args.cpus:
            for clients in args.clients:
                for repeat in range(1, args.rounds + 1):
                    case = probe.run_case(binary, args.output, clients + 1, args.iterations,
                                          repeat, available[:cpus], active_clients=clients)
                    result['cases'].append(case)
                    print(f'cpus={cpus} clients={clients} round={repeat}: {case["status"]}', flush=True)
                    if case['status'] != 'passed':
                        raise RuntimeError(case['error'])
                    if case['phases'][0]['successes'] != clients * args.iterations:
                        raise RuntimeError('completed useful-work count mismatch')
        result['status'] = 'passed'
    except Exception as error:
        result['error'] = f'{type(error).__name__}: {error}'
    result['disk_free_after'] = shutil.disk_usage(args.output).free
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
