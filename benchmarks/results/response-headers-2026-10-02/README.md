# JSON response headers, 2026-10-02

The change uses a static `application/json` header value in the JSON success and error response builders. `runtime-lib.patch` identifies the runtime change; `provenance.json` records source hashes. Neither version disables the ten-second HTTP header/idle deadline.

`allocations.json` compares the 0.1.5 builders and current builders in one process. Inputs are prepared before measuring. The example also checks that status, headers, and body bytes match. Each measured success/error response uses one fewer allocation and 16 fewer allocated bytes. This measures response construction, excluding network and body consumption.

The HTTP test alternates baseline/candidate order across five pairs per endpoint: health, small JSON, and a JSON echo with a 4,096-byte name. Each trial lasts ten seconds after one second of warm-up. The server uses CPU 0 and `NAGI_THREADS=1`; wrk uses CPUs 1,2, two threads, and 128 client connections. The recorded process thread count is three, including the DB worker. This is loopback HTTP/1.1 without TLS; the client connection count is a load condition.

Median changes were −1.2% for health, +1.2% for small JSON, and +0.5% for 4KiB JSON. All 30 trials had zero recorded errors. Individual paired results varied in both directions, including the unchanged health endpoint. These trials do not establish a throughput improvement; the allocation reduction is directly measured.

`summary.json` contains medians and every paired difference. `results.json.gz` contains all trial measurements and server process observations. `logs.tar.gz` contains raw wrk output and server logs. `environment.json` records binary hashes and CPU settings; `manifest.json` records the data and harness hashes.

Reproduce the allocation measurement:

```bash
cargo run --release --locked -p nagi-runtime --example response_allocations
```

Build a baseline CRUD executable from the baseline commit and a candidate from this change. Preserve each executable before the next build overwrites it. Then run:

```bash
python3 benchmarks/results/response-headers-2026-10-02/paired_load.py \
  --repository "$PWD" --baseline /path/to/baseline/nagi-crud \
  --candidate /path/to/candidate/nagi-crud --wrk /path/to/wrk \
  --out /path/to/new-results
```
