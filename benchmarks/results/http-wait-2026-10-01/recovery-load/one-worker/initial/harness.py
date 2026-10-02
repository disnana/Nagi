"""Repeat the archived recovery load with Nagi's actual worker setting."""
import argparse
import asyncio
import hashlib
import json
import os
import platform
import resource
import signal
import socket
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--vegeta', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=False)
FULL = b'GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n'


def resources(pid):
    folder = Path(f'/proc/{pid}')
    status = dict(line.split(':', 1) for line in (folder / 'status').read_text().splitlines())
    return {'fds': len(list((folder / 'fd').iterdir())),
            'rss_kib': int(status['VmRSS'].split()[0])}


async def main():
    env = dict(os.environ, NAGI_THREADS='1', NAGI_DB=':memory:', NAGI_HTTP_REQUEST_WAIT_SECONDS='10')
    env.pop('TOKIO_WORKER_THREADS', None)
    cpus = sorted(os.sched_getaffinity(0))
    assert len(cpus) >= 3
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(('127.0.0.1', 8080))
    with (a.out / 'server.log').open('w') as log:
        server = subprocess.Popen(['taskset', '-c', str(cpus[0]), str(a.binary)],
                                  stdout=log, stderr=log, env=env)
        connections = []
        load = None
        loadfile = None
        try:
            for _ in range(200):
                if server.poll() is not None:
                    raise AssertionError('server exited: ' + (a.out / 'server.log').read_text())
                try:
                    r, w = await asyncio.open_connection('127.0.0.1', 8080)
                    w.write(FULL)
                    await w.drain()
                    await r.readuntil(b'\r\n\r\n')
                    assert await r.readexactly(2) == b'ok'
                    w.close()
                    await w.wait_closed()
                    break
                except (ConnectionRefusedError, OSError):
                    await asyncio.sleep(.025)
            else:
                raise AssertionError('server not ready')
            baseline = resources(server.pid)
            environment = {
                'measurement_date_utc': datetime.now(timezone.utc).isoformat(),
                'NAGI_THREADS': env['NAGI_THREADS'], 'NAGI_DB': env['NAGI_DB'],
                'NAGI_HTTP_REQUEST_WAIT_SECONDS': env['NAGI_HTTP_REQUEST_WAIT_SECONDS'],
                'server_cpus': str(cpus[0]), 'client_cpus': ','.join(map(str, cpus[1:3])),
                'server_threads_at_start': sorted(x.read_text().strip() for x in
                                                 Path(f'/proc/{server.pid}/task').glob('*/comm')),
                'cpu_quota': Path('/sys/fs/cgroup/cpu.max').read_text().strip(),
                'fd_limit': list(resource.getrlimit(resource.RLIMIT_NOFILE)),
                'platform': platform.platform(),
                'candidate_base_commit': '1c01386894f7c156909771f4e6554292dd664243',
                'candidate_note': 'Reused the original 0.1.4 candidate binary with the 10-second wait change; not current main.',
                'binary_sha256': hashlib.sha256(a.binary.read_bytes()).hexdigest(),
                'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                'vegeta_sha256': hashlib.sha256(a.vegeta.read_bytes()).hexdigest(),
            }
            (a.out / 'environment.json').write_text(json.dumps(environment, indent=2) + '\n')
            loadfile = (a.out / 'healthy.bin').open('wb')
            load = subprocess.Popen(['taskset', '-c', environment['client_cpus'], str(a.vegeta),
                                     'attack', '-rate=5000/s', '-duration=25s', '-timeout=5s',
                                     '-workers=32', '-max-workers=32'],
                                    stdin=subprocess.PIPE, stdout=loadfile, stderr=subprocess.PIPE)
            load.stdin.write(b'GET http://127.0.0.1:8080/health\n')
            load.stdin.close()
            started = time.monotonic()
            groups = {'silent': [], 'partial': [], 'idle': []}

            async def connect(kind):
                r, w = await asyncio.open_connection('127.0.0.1', 8080)
                if kind == 'partial':
                    w.write(b'GET /health HTTP/1.1\r\nHost:')
                    await w.drain()
                elif kind == 'idle':
                    w.write(FULL)
                    await w.drain()
                    await r.readuntil(b'\r\n\r\n')
                    assert await r.readexactly(2) == b'ok'
                groups[kind].append((r, w))
                connections.append((r, w))

            for kind in groups:
                await asyncio.gather(*(connect(kind) for _ in range(350)))
            samples = []
            while time.monotonic() - started < 27:
                samples.append({'elapsed_s': time.monotonic() - started, 'server': resources(server.pid),
                                'remaining': {kind: sum(not r.at_eof() for r, w in group)
                                              for kind, group in groups.items()}})
                await asyncio.sleep(.5)
            assert load.wait(timeout=5) == 0, load.stderr.read().decode()
            loadfile.close()
            result = json.loads(subprocess.check_output([str(a.vegeta), 'report', '-type=json',
                                                         str(a.out / 'healthy.bin')], text=True))
            assert result['status_codes'] == {'200': result['requests']} and result['requests'] >= 123750, result
            assert not result['errors'] and result['success'] == 1, result
            assert all(r.at_eof() for r, w in connections), 'connection did not expire'
            record = {'baseline': baseline, 'groups_per_kind': 350, 'healthy_rate': 5000,
                      'healthy_duration_s': 25, 'healthy_report': result, 'samples': samples,
                      'binary_sha256': environment['binary_sha256'],
                      'harness_sha256': environment['harness_sha256']}
            (a.out / 'results.json').write_text(json.dumps(record, indent=2) + '\n')
            print(json.dumps({'healthy_requests': result['requests'], 'errors': result['errors'],
                              'last': samples[-1]}), flush=True)
        finally:
            for r, w in connections:
                w.close()
            for r, w in connections:
                try:
                    await w.wait_closed()
                except OSError:
                    pass
            if load is not None and load.poll() is None:
                load.terminate()
                load.wait(timeout=5)
            if loadfile is not None:
                loadfile.close()
            if server.poll() is None:
                server.send_signal(signal.SIGINT)
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()


asyncio.run(main())
