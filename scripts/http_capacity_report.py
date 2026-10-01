"""Validate saved HTTP capacity results and render publication figures.

The measurement scripts do not require matplotlib. Only this report step does.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import statistics
from datetime import datetime
from pathlib import Path


def read(path: Path):
    return json.loads(path.read_text())


def metrics(value: dict) -> dict:
    requests = value["requests"]
    assert requests > 0, "Empty load result"
    assert sum(value["status_codes"].values()) == requests, "Status counts do not match requests"
    assert sum(value["buckets"].values()) == requests, "Histogram does not match requests"
    successes = value["status_codes"].get("200", 0)
    return {"requests": requests, "non_200_or_transport_errors": requests - successes,
            "dispatched_rps": value["rate"], "successful_rps": value["throughput"],
            "p50_ms": value["latencies"]["50th"] / 1e6,
            "p95_ms": value["latencies"]["95th"] / 1e6,
            "p99_ms": value["latencies"]["99th"] / 1e6,
            "max_ms": value["latencies"]["max"] / 1e6}


def windows(reports: list[dict]) -> list[dict]:
    """Difference cumulative reports; latency estimates are histogram bounds."""
    result = []
    previous_counts = {}
    previous_requests = previous_successes = 0
    start = previous_end = datetime.fromisoformat(reports[0]["earliest"])
    for report in reports:
        metrics(report)
        end = datetime.fromisoformat(report["end"])
        count = report["requests"] - previous_requests
        successes = report["status_codes"].get("200", 0) - previous_successes
        histogram = {int(key): value - previous_counts.get(key, 0)
                     for key, value in report["buckets"].items()}
        assert count >= 0 and all(value >= 0 for value in histogram.values()), "Non-cumulative report"
        assert sum(histogram.values()) == count and 0 <= successes <= count
        elapsed = (end - previous_end).total_seconds()
        if count:
            assert elapsed > 0
            edges = sorted(histogram)
            total = 0
            bound = None
            for index, lower in enumerate(edges):
                total += histogram[lower]
                if total >= math.ceil(count * .99):
                    bound = edges[index + 1] / 1e6 if index + 1 < len(edges) else None
                    break
            result.append({"end_s": (end - start).total_seconds(), "seconds": elapsed,
                           "requests": count, "non_200_or_transport_errors": count - successes,
                           "successful_rps": successes / elapsed, "p99_bucket_upper_ms": bound})
        previous_end, previous_requests = end, report["requests"]
        previous_successes, previous_counts = report["status_codes"].get("200", 0), report["buckets"]
    return result


def summarize(data: Path) -> dict:
    trials = read(data / "limits/results.json")
    assert len(trials) == 54, "This report expects the complete 54-trial rate sweep"
    limits = []
    for case in dict.fromkeys(row["case"] for row in trials):
        for rate in sorted({row["target_rps"] for row in trials if row["case"] == case}):
            group = [row for row in trials if row["case"] == case and row["target_rps"] == rate]
            assert len(group) == 3 and {row["repeat"] for row in group} == {0, 1, 2}
            assert all(row["aborted"] is None for row in group), "Aborted limit trial"
            values = [metrics(row["metrics"]) for row in group]
            limits.append({"case": case, "target_rps": rate, "trials": len(group),
                           **{key + "_median": statistics.median(row[key] for row in values)
                              for key in ("dispatched_rps", "successful_rps", "p50_ms", "p95_ms", "p99_ms")},
                           "successful_rps_min": min(row["successful_rps"] for row in values),
                           "successful_rps_max": max(row["successful_rps"] for row in values),
                           "requests_total": sum(row["requests"] for row in values),
                           "errors_total": sum(row["non_200_or_transport_errors"] for row in values),
                           "server_cpu_percent_median": statistics.median(row["server_cpu_percent_of_one_core"] for row in group),
                           "client_cores_median": statistics.median(row["client_cpu_seconds_total"] / row["wall_seconds"] for row in group),
                           "peak_rss_mib": max(row["server_peak_rss_kib"] for row in group) / 1024,
                           "peak_fds": max(row["server_peak_fds"] for row in group),
                           "recovery_fds": [row["recovery"][-1]["server"]["fds"] for row in group]})
    soaks = {}
    for name in ("soak-30m", "soak-fixed-128"):
        row = read(data / name / "result.json")
        assert row["aborted"] is None, "Aborted endurance trial"
        reports = [json.loads(line) for line in (data / name / "reports.jsonl").read_text().splitlines()]
        assert reports[-1] == row["metrics"], "Final report mismatch"
        windows_data = windows(reports)
        assert sum(window["requests"] for window in windows_data) == row["metrics"]["requests"]
        soaks[name] = {**metrics(row["metrics"]), "duration_s": row["duration_s"],
                       "target_rps": row["target_rps"],
                       "peak_rss_mib": row["server_peak_rss_kib"] / 1024,
                       "peak_fds": row["server_peak_fds"], "fd_baseline": row["server_before"]["fds"],
                       "recovery_last": row["recovery"][-1],
                       "recovery_min_fds": min(probe["server"]["fds"] for probe in row["recovery"]),
                       "recovery_max_fds": max(probe["server"]["fds"] for probe in row["recovery"]),
                       "windows": windows_data}
    connections = read(data / "tcp/results.json")
    saturation = []
    raw_saturation = read(data / "saturation/results.json")
    assert len(raw_saturation) == 18
    for case in ("health", "json_4k"):
        for count in (128, 512, 2048):
            group = [row for row in raw_saturation if row["case"] == case and row["connections"] == count]
            assert len(group) == 3
            saturation.append({"case": case, "connections": count,
                               "rps_median": statistics.median(row["metrics"]["requests_s"] for row in group),
                               "rps_min": min(row["metrics"]["requests_s"] for row in group),
                               "rps_max": max(row["metrics"]["requests_s"] for row in group),
                               "p99_ms_median": statistics.median(row["metrics"]["p99_us"] for row in group) / 1000,
                               "socket_status_timeout_errors_total": sum(sum(row["metrics"][key] for key in
                                   ("connect_errors", "read_errors", "write_errors", "timeouts", "status_errors")) for row in group),
                               "server_cpu_percent_median": statistics.median(row["server_cpu_percent_of_one_core"] for row in group)})
    return {"source_commit": read(data / "soak-30m/environment.json")["source_commit"],
            "nagi_version": read(data / "soak-30m/environment.json")["nagi_version"],
            "limits": limits, "soaks": soaks, "saturation": saturation,
            "recovery_diagnostic": metrics(read(data / "recovery-high/result.json")["metrics"]),
            "connections": [{"requested": row["requested"], "tcp_opened": row["tcp_opened"],
                             "initial_http_ok": row["initial_http_ok"],
                             "verified_held_http_ok": row["verified_held_http_ok"],
                             "peak_rss_mib": row["server_held"]["rss_kib"] / 1024,
                             "fds_held": row["server_held"]["fds"],
                             "fds_after": row["server_after"]["fds"]} for row in connections]}


def figures(data: Path, output: Path, summary: dict) -> None:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    plt.rcParams.update({"font.size": 10, "svg.hashsalt": "nagi-http-capacity-2026-10-01",
                         "axes.spines.top": False, "axes.spines.right": False})
    output.mkdir(parents=True, exist_ok=True)

    def save(fig, name):
        fig.tight_layout()
        svg = output / (name + ".svg")
        fig.savefig(svg, metadata={"Date": None})
        svg.write_text("\n".join(line.rstrip() for line in svg.read_text().splitlines()) + "\n")
        fig.savefig(output / (name + ".png"), dpi=170, metadata={"Software": "Nagi benchmark report"})
        plt.close(fig)

    titles = {"health": "GET /health", "json_4k": "POST /echo (4 KiB string)",
              "db_read": "SQLite SELECT (in memory)", "db_write": "SQLite INSERT (in memory)"}
    fig, axes = plt.subplots(2, 2, figsize=(11, 7))
    for ax, (case, title) in zip(axes.flat, titles.items()):
        rows = [row for row in summary["limits"] if row["case"] == case]
        x = [row["target_rps"] / 1000 for row in rows]
        ax.plot(x, x, color="#999999", linestyle=":", label="Requested")
        ax.plot(x, [row["successful_rps_median"] / 1000 for row in rows], marker="o", color="#246e70", label="Successful (median)")
        ax.fill_between(x, [row["successful_rps_min"] / 1000 for row in rows],
                        [row["successful_rps_max"] / 1000 for row in rows], color="#246e70", alpha=.15)
        ax.set(title=title, xlabel="Requested rate (thousand/s)", ylabel="Successful rate (thousand/s)")
        ax.grid(alpha=.2)
        right = ax.twinx()
        right.plot(x, [row["p99_ms_median"] for row in rows], "s--", color="#a95b23", label="p99 (median of trials)")
        right.set_ylabel("p99 latency (ms)", color="#a95b23")
        right.set_ylim(bottom=0)
        handles, labels = ax.get_legend_handles_labels()
        h2, l2 = right.get_legend_handles_labels()
        ax.legend(handles + h2, labels + l2, fontsize=8, loc="upper left")
    save(fig, "http-rates")

    fig, axes = plt.subplots(3, 1, figsize=(10, 9), sharex=True)
    for name, label, color in [("soak-30m", "30 min / unrestricted client connections", "#246e70"),
                                ("soak-fixed-128", "10 min / client capped at 128", "#a95b23")]:
        row = read(data / name / "result.json")
        window = summary["soaks"][name]["windows"]
        axes[0].plot([r["end_s"] / 60 for r in window], [r["successful_rps"] / 1000 for r in window], color=color, label=label)
        resources = [json.loads(line) for line in (data / name / "resources.jsonl").read_text().splitlines()]
        samples = [(r["elapsed_s"], r["server"]) for r in resources if "rss_kib" in r["server"]]
        samples += [(row["wall_seconds"] + r["elapsed_s"], r["server"]) for r in row["recovery"]]
        axes[1].plot([s / 60 for s, _ in samples], [r["rss_kib"] / 1024 for _, r in samples], color=color, label=label)
        axes[2].plot([s / 60 for s, _ in samples], [r["fds"] for _, r in samples], color=color, label=label)
        for ax in axes:
            ax.axvline(row["duration_s"] / 60, color=color, linestyle=":", alpha=.6)
    axes[0].axhline(15, color="#777777", linestyle=":", label="Requested: 15,000/s")
    axes[0].set_ylabel("Successful rate (thousand/s)")
    axes[0].set_ylim(0, 16.5)
    axes[0].legend(fontsize=9)
    axes[1].set_ylabel("Server RSS (MiB)")
    axes[2].set(ylabel="Server file descriptors", xlabel="Time after load started (minutes)")
    for ax in axes:
        ax.grid(alpha=.2)
        ax.set_ylim(bottom=0)
    save(fig, "http-endurance")

    rows = summary["connections"]
    x = [row["requested"] / 1000 for row in rows]
    fig, axes = plt.subplots(2, 1, figsize=(9, 7), sharex=True)
    axes[0].plot(x, x, ":", color="#999999", label="Requested")
    axes[0].plot(x, [row["verified_held_http_ok"] / 1000 for row in rows], "o-", color="#246e70", label="Held and HTTP-verified")
    axes[0].set_ylabel("Connections (thousand)")
    axes[0].legend()
    axes[1].plot(x, [row["peak_rss_mib"] for row in rows], "o-", color="#246e70")
    axes[1].set(ylabel="Server RSS while held (MiB)", xlabel="Requested connections (thousand)")
    for ax in axes:
        ax.axvline(16.384, linestyle=":", color="#a95b23", label="OS FD limit: 16,384")
        ax.grid(alpha=.2)
        ax.set_ylim(bottom=0)
    save(fig, "http-connections")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data", type=Path, required=True)
    parser.add_argument("--assets", type=Path, required=True)
    args = parser.parse_args()
    result = summarize(args.data)
    (args.data / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    figures(args.data, args.assets, result)
    files = sorted(path for path in args.data.rglob("*") if path.is_file() and path.name != "SHA256SUMS")
    (args.data / "SHA256SUMS").write_text("".join(
        hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.relative_to(args.data).as_posix() + "\n"
        for path in files))
    print(f"Validated {len(result['limits'])} rate points, two endurance runs, and {len(result['connections'])} connection trials")


if __name__ == "__main__":
    main()
