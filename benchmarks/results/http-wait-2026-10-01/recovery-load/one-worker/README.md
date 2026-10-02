# Recovery-load worker correction

October 2, 2026 (UTC). The original recovery harness set `TOKIO_WORKER_THREADS=1`. Nagi reads `NAGI_THREADS`, defaults to four, and sets Tokio's worker count explicitly. The inherited `NAGI_THREADS` was not recorded, so the original test's worker count cannot be recovered. The original harness and results remain unchanged.

These repeats explicitly set `NAGI_THREADS=1`, `NAGI_DB=:memory:`, and a ten-second request-wait deadline. Each environment records one Tokio worker thread, in addition to the main and SQLite threads. The same original modified 0.1.4 binary was reused; its SHA-256 is `624cfe348fd8dc91a665019d6fc6368b10e21b00555aefa00b97260b3e7eb0a9`. These are not measurements of current 0.1.5 main.

All runs use one server CPU, two other client CPUs, 350 silent connections, 350 partial-header connections, and 350 unused keep-alive connections. Vegeta requests 5,000 healthy responses/s for 25 seconds. The connection count is a client workload, not a server cap.

| Directory | Healthy requests / errors | Closure validation |
| --- | --- | --- |
| `initial/` | 125,000 / 0 | Failed; the harness stopped before saving connection samples. Response aggregate and environment are retained. |
| `eof/` | 125,000 / 0 | Failed; 25 silent clients had not observed closure at the final sample near 26.77s, although server FDs had returned to 10. |
| `tcp/` | 124,997 / 0 | All 1,050 clients observed closure by the sampled 12.37s. Server FDs returned to 10. |
| `peer/` | 125,000 / 0 | All 1,050 clients observed closure by the sampled 11.26s. Server FDs returned to 10. |

The initial harness checked `StreamReader.at_eof()` without draining possible terminal responses. Later harnesses read until EOF and record any terminal bytes or reset. All completed records observed zero terminal bytes; the data does not demonstrate that buffered responses caused the unsuccessful observations. Their cause remains unresolved, and a successful repeat does not erase them.

`tcp/` adds server TCP-state sampling and observes for 27 seconds. `peer/` adds both peers' TCP-state sampling and observes for 40 seconds. The latter run has no pending client closure observations after about 11.26s. Both successful runs still show 1,050 server-side `FIN_WAIT2` entries at the last sample: clients keep their writer side open until harness cleanup. FD recovery and complete removal of kernel TCP states are different observations.

Do not compare the p99 values as a before/after performance result: sampling overhead, observation periods, and worker verification differ. These brief loopback runs do not establish DDoS resistance or long-term stability.

Each directory includes the exact `harness.py` and `environment.json`. Records after the initial attempt use `results.json.gz`; compression retains every JSON field and sample. [manifest.json](manifest.json) lists the stored and uncompressed SHA-256 hashes and sizes.

```sh
gzip -dc peer/results.json.gz > /tmp/nagi-peer-results.json
python peer/harness.py --binary /path/to/recorded/nagi-crud \
  --vegeta /path/to/vegeta --out /path/to/new-results
```

The scripts use fixed loopback port 8080, verify it is free, start their own server, and stop it on exit. Existing output directories are refused. The reports are in [Japanese](../../../../../docs/http-capacity.md) and [English](../../../../../docs/en/http-capacity.md).
