#!/usr/bin/env python3
"""Bounded one-shot protocol benchmark, with independent candidate/position assertions."""
import argparse
import json
from pathlib import Path
import resource
import statistics
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument('binary')
parser.add_argument('output')
parser.add_argument('--profile', default='release')
parser.add_argument('--runs', type=int, default=7)
parser.add_argument('--persistent', action='store_true')
args = parser.parse_args()
assert 1 <= args.runs <= 20
binary = str(Path(args.binary).resolve())
output = Path(args.output)
report = {'profile': args.profile, 'binary': binary, 'measurement': 'persistent wall; no IDE cache' if args.persistent else 'one-shot wall and child CPU; no IDE cache', 'cases': []}
with tempfile.TemporaryDirectory(prefix='nagi-assist-perf-', dir=output.parent) as temp:
    path = Path(temp) / 'main.nagi'
    path.write_text('def main():\n    saved = True\n')
    peer = subprocess.Popen([binary, 'assist', str(path), '--no-project', '--editor-input', '--serve'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE) if args.persistent else None
    for size in (10, 100, 500):
        text = 'def main():\n' + ''.join(f'    value{n} = {n}\n' for n in range(size)) + '    pr\n'
        request = json.dumps({'files': [{'file': str(path), 'text': text}], 'query': {'file': str(path), 'line': size + 2, 'column': 7}}).encode()
        times, cpus = [], []
        for _ in range(args.runs):
            before = resource.getrusage(resource.RUSAGE_CHILDREN)
            start = time.perf_counter()
            if peer is not None:
                peer.stdin.write(request + b'\n')
                peer.stdin.flush()
                import select
                assert select.select([peer.stdout], [], [], 30)[0], 'response timeout'
                payload = peer.stdout.readline()
                assert payload, 'persistent compiler exited before its response'
            else:
                run = subprocess.run([binary, 'assist', str(path), '--no-project', '--editor-input'], input=request, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
                assert run.returncode == 0, run.stderr
                payload = run.stdout
            times.append((time.perf_counter() - start) * 1000)
            after = resource.getrusage(resource.RUSAGE_CHILDREN)
            cpus.append((after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime) * 1000)
            value = json.loads(payload)
            assert value['format'] == 'nagi-assist-v1'
            assert value['semantic_status'] == 'editor-partial'
            assert value['full_compile_checked'] is False
            assert value['frontend_accepted'] is False
            assert len(value['diagnostics']) == 1
            items = {item['name']: item for item in value['completion']['items']}
            assert set(items) == {'main'} | {f'value{n}' for n in range(size)}
            for n in range(size):
                item = items[f'value{n}']
                assert item['type'] == 'i64'
                assert item['target'] == {'file': str(path), 'line': n + 2, 'column': 5, 'length': len(f'value{n}')}
        report['cases'].append({'assignments': size, 'source_bytes': len(text.encode()), 'candidates': size + 1, 'elapsed_ms': times, 'p50_ms': statistics.median(times), 'max_ms': max(times), **({} if peer is not None else {'cpu_ms': cpus, 'cpu_p50_ms': statistics.median(cpus)})})
    if peer is not None:
        peer.stdin.close()
        assert peer.wait(timeout=5) == 0, peer.stderr.read()
output.write_text(json.dumps(report, indent=2))
for case in report['cases']:
    print(f"{case['assignments']} vars: p50 {case['p50_ms']:.1f} ms, max {case['max_ms']:.1f} ms")
