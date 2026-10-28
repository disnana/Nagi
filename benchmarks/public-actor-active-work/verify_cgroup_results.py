#!/usr/bin/env python3
"""Recompute saved hard-cgroup Actor results without executing Docker."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("probe", ROOT / "scripts/public_supervisor_probe.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


def distribution(values: list[int | float | None]) -> dict[str, int | float | None]:
    defined = [value for value in values if value is not None]
    return {
        "median": statistics.median(defined) if defined else None,
        "min": min(defined) if defined else None,
        "max": max(defined) if defined else None,
        "defined": len(defined),
        "missing": len(values) - len(defined),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    directory = args.directory
    summary = json.loads((directory / "summary.json").read_text())
    assert summary["status"] == "passed" and len(summary["cases"]) == 24
    assert summary["limits"] == {
        "wall_seconds": 30,
        "memory_bytes": 1024**3,
        "memory_swap_bytes": 1024**3,
        "pids": 512,
        "minimum_free_bytes": 8 * 1024**3,
        "hard_cgroup_enforced": True,
    }
    raw_hashes = {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(directory.glob("*.jsonl.gz"))
    }
    assert raw_hashes == summary["output_sha256"]
    total = 0
    for case in summary["cases"]:
        assert case["status"] == "passed"
        assert case["cleanup"] == {"rm_returncode": 0, "residual_container_ids": []}
        clients = case["active_clients"]
        assert clients in (4, 16, 64, 128)
        assert case["actors"] == clients + 1 and case["iterations_per_client"] == 5000
        cpus = case["cpu_quota_count"]
        assert cpus in (1, 2) and len(case["cpuset_cpus"]) == cpus
        config = case["host_config"]
        assert config == {
            "Memory": 1024**3,
            "MemorySwap": 1024**3,
            "NanoCpus": cpus * 1_000_000_000,
            "CpusetCpus": ",".join(str(value) for value in case["cpuset_cpus"]),
            "PidsLimit": 512,
            "ReadonlyRootfs": True,
            "NetworkMode": "none",
            "CapDrop": ["ALL"],
            "SecurityOpt": ["no-new-privileges"],
        }
        checkpoints = case["checkpoints"]
        assert [value["checkpoint"] for value in checkpoints] == [0, 1, 2, 3]
        assert case["released_task_delta"] == 0
        assert all(value["memory_current_bytes"] <= 1024**3 for value in checkpoints)
        assert all(value["memory_peak_bytes"] <= 1024**3 for value in checkpoints)
        assert all(value["memory_events"]["oom"] == 0 and
                   value["memory_events"]["oom_kill"] == 0
                   for value in checkpoints)
        for value in checkpoints:
            assert value["memory_max"] == str(1024**3)
            assert value["memory_swap_max"] == "0"
            quota, period = (int(item) for item in value["cpu_max"].split())
            assert quota == cpus * period and period > 0
            assert value["pids_max"] == "512"
        label = f"cpus-{cpus}-actors-{clients + 1}-round-{case['round']}"
        with gzip.open(directory / f"{label}.jsonl.gz", "rt") as stream:
            records = [json.loads(line) for line in stream]
        assert [value["checkpoint"] for value in records if "checkpoint" in value] == [0, 1, 2, 3]
        phases = [value for value in records if "phase" in value]
        assert len(phases) == 1 and phases[0]["recovery"] == [1, 1, 2, 0]
        phase = probe.summarize_phase(phases[0], 5000, True, clients)
        phase.update(public_failure_count=1, public_restart_count=1,
                     recovered_generation=2, recovered_state=0)
        assert phase == case["phases"][0]
        assert phase["failures"] == 0 and phase["successes"] == clients * 5000
        assert phase["healthy_completions_during_fault"] > 0
        total += phase["successes"]
    assert total == 6_360_000

    fields = [
        "completed_successes_per_second", "process_cpu_ns_per_success", "elapsed_ns",
        "latency_p50_ns", "latency_p95_ns", "latency_p99_ns",
        "healthy_completions_during_fault", "fault_duration_ns",
        "healthy_latency_p99_ns_during_fault", "common_interval_jain_index",
    ]
    aggregate = []
    for cpus in (1, 2):
        for clients in (4, 16, 64, 128):
            cases = [case for case in summary["cases"]
                     if case["cpu_quota_count"] == cpus and case["active_clients"] == clients]
            assert len(cases) == 3 and {case["round"] for case in cases} == {1, 2, 3}
            row = {"cpu_quota_count": cpus, "active_clients": clients,
                   "total_actors": clients + 1}
            for field in fields:
                values = [case["phases"][0][field] for case in cases]
                row[field] = distribution(values)
            peaks = [max(point["memory_peak_bytes"] for point in case["checkpoints"])
                     for case in cases]
            finals = [case["checkpoints"][3]["memory_current_bytes"] for case in cases]
            row["cgroup_memory_peak_bytes"] = {
                "median": statistics.median(peaks), "min": min(peaks), "max": max(peaks)}
            row["cgroup_final_memory_current_bytes"] = {
                "median": statistics.median(finals), "min": min(finals), "max": max(finals)}
            aggregate.append(row)
    (directory / "aggregate.json").write_text(json.dumps(aggregate, indent=2) + "\n")
    verification = {
        "cases_recomputed": 24,
        "successful_replies": total,
        "normal_failures": 0,
        "injected_worker_failures": 24,
        "public_restarts": 24,
        "released_task_deltas_zero": True,
        "oom_events_zero": True,
        "hard_resource_config_matched": True,
        "raw_sha256": raw_hashes,
    }
    (directory / "verification.json").write_text(json.dumps(verification, indent=2) + "\n")
    print(json.dumps({key: value for key, value in verification.items() if key != "raw_sha256"}))


if __name__ == "__main__":
    main()
