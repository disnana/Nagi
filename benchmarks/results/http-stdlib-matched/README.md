# Dependency-matched standard HTTP measurements

Collected on 2026-10-03 using `scripts/http_stdlib_bench.py --build-matching-rust`, based on main `d958dba` with the standard HTTP change.

The Nagi application is `benchmarks/http_stdlib.nagi`; the direct Rust equivalent is `runtime/examples/http_stdlib_baseline.rs`. The script copies the generated Nagi Cargo.toml and Cargo.lock, changes only the executable package name, and builds the Rust equivalent with `--release --locked`. `environment.json` verifies lock equality after that package rename and records binary hashes. Both servers use identical standard HTTP runtime defaults.

There are 27 measurements: three endpoints, three implementations, three repeats. Each measured run lasts five seconds after a one-second warmup. Server order is shuffled with a recorded fixed seed sequence (714 plus repeat index). The server uses CPU 0 and one worker; wrk uses CPUs 3 and 4, two threads, and 64 connections. The host is shared; its container has a four-core CPU quota. This is local closed-loop saturation traffic, not a fixed arrival-rate or external-network test.

`summary.json` takes the median of each metric from three runs; its percentiles are not pooled request percentiles. `runs.json` and `wrk-*.txt` retain all individual runs and errors. All 27 runs reported zero transport/status errors. Nagi/Rust median POST throughput was 60,444/59,250 requests/s; median p99 was 2.805/1.907 ms. The short run does not establish a language performance ranking.

The [initial records](../http-stdlib/) contain a differently locked Rust baseline and a 120-second Nagi soak using the same Nagi executable. The initial 10.8% POST deficit was not reproduced here. Its cause was not isolated.

`json-probe.jsonl` isolates decode → JSON response → drop inside one executable with the actual generated types and equivalent ordinary Rust types. Seven alternating rounds use 10,000 iterations per input/type; allocation counting is separate from timing. Both concrete handler futures are 256 bytes. Small JSON median operation times were 208/206 ns, with five allocations and 813 requested bytes each. The 4KB-name input measured 2,894/2,874 ns, with the same seven allocation/reallocation operations and 17,219 requested bytes each. These include buffer growth, not peak live memory. The probe excludes request construction, route erasure, network, and scheduling; it is not an HTTP capacity measurement. Reproduce it with `scripts/http_json_probe.py` after generating `build/http_stdlib`.
