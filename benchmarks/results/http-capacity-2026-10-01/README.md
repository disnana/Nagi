# HTTP capacity measurements — October 1, 2026

Measured implementation: Nagi 0.1.2 at `c602abb349641cf1d5a274e2221e33d29a0acea4`.
The application binary hash is the same in every phase. Test drivers were developed
during the experiment; their exact measured snapshots and hashes are retained.

The readable report is [Japanese](../../../docs/http-capacity.md) or
[English](../../../docs/en/http-capacity.md).

| Directory | Contents |
| --- | --- |
| `limits/` | Paced rate sweep: 18 rate points, three 8-second trials each; separate loopback source address per trial |
| `client-port-exhaustion/` | Initial sweep retained for transparency; 12,177 client-side bind failures due to source-port exhaustion |
| `soak-30m/` | 30 minutes at a requested 15,000/s; unrestricted client connection count |
| `soak-fixed-128/` | 10-minute control at 15,000/s with the client capped at 128 connections |
| `saturation/` | Fixed-concurrency wrk runs: two workloads, 128/512/2,048 connections, three trials each |
| `tcp/` | Six connection-count trials; initial HTTP and held-connection verification are separate from TCP handshakes |
| `lifecycle/` | Silent, incomplete-header, incomplete-body, idle keep-alive, and abrupt disconnect observations |
| `recovery-high/` | 2-minute trial requesting 30,000/s, followed by 120 seconds of recovery with FD-type and TCP-state observations |

`results.json` and `result.json` retain native client metrics and recovery samples.
`resources.jsonl` contains individual process/cgroup observations. Limits resource
files combine the trials with a `trial` field; the original measurement values are
unchanged. Vegeta's `reports.jsonl` records cumulative reports; the summarizer
differences the histogram counts to produce interval bounds. It does not subtract
or average cumulative percentiles.

`*.py.txt` files are archival snapshots, not runnable entry points at this location.
Run the drivers under `scripts/` in a source checkout. Snapshots whose helpers are
separate have the matching `http_helpers.py.txt` alongside them. Absolute paths in
captured commands describe the original workspace and are not installation paths.

`build-provenance.json`, tool provenance, and each phase's `environment.json`
record the build, tool identities, and execution conditions. No runtime or OS
limits were changed. Traffic stayed on loopback. SQLite was in memory. The
Vegeta client fully reads responses; `-max-body=0` avoids storing their bodies in
the result stream, so `bytes_in` is not a measurement of wire bandwidth.

The 30-minute run returned HTTP 200 for all requests but left 1,049 FDs after
30 seconds of recovery. Their types were not captured in that run. This dataset
does not establish leak-free operation or DDoS resistance. The 10-minute control
does not establish what the same control would do over 30 minutes.

Regenerate figures and aggregate data from the retained measurements:

```sh
python -m unittest discover -s scripts -p 'test_http_capacity_report.py'
python scripts/http_capacity_report.py \
  --data benchmarks/results/http-capacity-2026-10-01 \
  --assets website/assets/http-capacity
cd benchmarks/results/http-capacity-2026-10-01
sha256sum -c SHA256SUMS
```

Plotting requires matplotlib; the measurement drivers use the Python standard
library and their specified native clients. `SHA256SUMS` covers all retained
measurement files and the generated summary. The chart files are separately
tracked under `website/assets/http-capacity/`.
