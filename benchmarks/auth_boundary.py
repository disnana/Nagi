"""Compare handwritten Rust and generated Nagi load/policy with identical HTTP boundaries."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import socket
import subprocess
import time


def proc_metrics(pid):
    fields = Path(f"/proc/{pid}/stat").read_text().split()
    return {"cpu_ticks": int(fields[13]) + int(fields[14]), "rss_bytes": int(fields[23]) * os.sysconf("SC_PAGE_SIZE")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--client", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=int, default=3)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--concurrency", type=int, default=8)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    for key in ("NAGI_AUTH_MICRO", "NAGI_AUTH_PROBES"):
        env.pop(key, None)
    rows = []
    for run in range(args.runs):
        # Alternate order to limit fixed warmup/host-load bias.
        for mode in (("nagi", "rust") if run % 2 == 0 else ("rust", "nagi")):
            with socket.socket() as available:
                available.bind(("127.0.0.1",0))
                port = available.getsockname()[1]
            with (args.output / f"{run}-{mode}.stdout").open("w") as stdout, (args.output / f"{run}-{mode}.stderr").open("w") as stderr:
                process = subprocess.Popen([str(args.executable.resolve())], env=dict(env,NAGI_SAMPLE_PORT=str(port),NAGI_AUTH_POLICY_MODE=mode), stdout=stdout,stderr=stderr)
                try:
                    deadline = time.monotonic() + 10
                    while True:
                        assert process.poll() is None, "server exited"
                        try:
                            with socket.create_connection(("127.0.0.1",port),timeout=0.2):
                                break
                        except OSError:
                            assert time.monotonic() < deadline, "server startup timeout"
                            time.sleep(0.02)
                    subprocess.run([str(args.client.resolve()),str(port),str(args.concurrency),"1"],check=True,capture_output=True)
                    before = proc_metrics(process.pid)
                    completed = subprocess.run([str(args.client.resolve()),str(port),str(args.concurrency),str(args.seconds)],check=True,capture_output=True,text=True,timeout=args.seconds+10)
                    after = proc_metrics(process.pid)
                    row = json.loads(completed.stdout)
                    row.update(mode=mode,run=run,server_cpu_seconds=(after["cpu_ticks"]-before["cpu_ticks"])/os.sysconf("SC_CLK_TCK"),rss_before=before["rss_bytes"],rss_after=after["rss_bytes"])
                    rows.append(row)
                    assert sum(row[key] for key in ("transport_errors","status_errors","body_errors")) == 0, row
                    print(json.dumps(row),flush=True)
                finally:
                    if process.poll() is None:
                        process.send_signal(signal.SIGINT)
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            process.kill(); process.wait()
                            raise AssertionError("shutdown timeout") from None
                    assert process.returncode == 0, "server shutdown failed"
    micro = subprocess.run([str(args.executable.resolve())],env=dict(env,NAGI_AUTH_MICRO="1",NAGI_BENCH_LOOPS="100000"),capture_output=True,text=True,check=True,timeout=30)
    (args.output / "micro.jsonl").write_text(micro.stdout)
    report = {"platform":platform.platform(),"binary_bytes":args.executable.stat().st_size,"binary_sha256":hashlib.sha256(args.executable.read_bytes()).hexdigest(),"same_executable":str(args.executable.resolve()),"client_sha256":hashlib.sha256(args.client.read_bytes()).hexdigest(),"nagi_runtime_threads":env.get("NAGI_THREADS","default4"),"host_cpus":os.cpu_count(),"closed_loop":True,"workers":args.concurrency,"seconds_per_run":args.seconds,"runs":rows,"comparison":"handwritten Rust load/policy versus generated Nagi load/policy; shared Rust/Axum verifier/DB/DTO/limits; one shared host, not language-wide performance, production SLO, or HTTP backend replacement evidence"}
    (args.output / "results.json").write_text(json.dumps(report,indent=2)+"\n")


if __name__ == "__main__":
    main()
