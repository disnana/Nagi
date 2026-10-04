# Standard HTTP performance and memory

This test compares `std.http.server` in Nagi with an equivalent server written directly in Rust. Measurements were taken on October 3, 2026.

The Rust baseline also uses `nagi-runtime::http_server`. This compares generated code and handwritten handlers; it is not a matched comparison between Axum Router and Hyper. The legacy Axum server included in the measurement script uses different admission and deadline policies. A comparison with matched API, limits, and fault behavior to evaluate adopting Axum/Tower behind the standard API has not been performed.

## Response speed

Each endpoint ran for five seconds with 64 connections, repeated three times. The table reports the median of those three runs.

| Endpoint | Nagi requests/s | Rust requests/s | Nagi p99 | Rust p99 |
| --- | ---: | ---: | ---: | ---: |
| GET, short text | 131,630 | 111,923 | 2.054 ms | 2.214 ms |
| GET, small JSON | 122,464 | 110,257 | 1.001 ms | 1.066 ms |
| POST, decode and encode JSON with a 4KB name | 60,444 | 59,250 | 2.805 ms | 1.907 ms |

p99 is the time within which 99% of responses finished. POST throughput was close, but Nagi's slower responses took longer. Tail latency remains an improvement target.

These short runs on a shared host cannot establish a language speed ranking or production capacity. The initial POST measurement put Nagi about 10.8% behind. That gap did not recur after aligning dependency and build conditions. Its cause has not been isolated; the [initial records](../../benchmarks/results/http-stdlib/) remain available.

## Two minutes of continuous load

The same Nagi POST endpoint ran for 120 seconds.

| Metric | Result |
| --- | ---: |
| Requests completed | 7,653,211 |
| Connection, read, write, timeout, or status errors | 0 |
| p95 / p99 | 1.375 / 1.891 ms |
| Process RSS | 3,244 KiB at the start; 3,428 KiB maximum and final |
| Open file descriptors | 74 maximum; 10 after the load ended |

The server stayed running for a further five seconds. Its open file descriptors returned to the original 10. RSS includes memory retained by the process and allocator. Stability over two minutes does not prove long-term reliability or absence of leaks.

## Avoiding extra work

`Status` is a two-byte numeric type; status checks do not create strings. `Method` reuses the HTTP parser's native type. Neither `request.is_get` nor comparison with `Method.GET` constructs an extra string.

A single-header lookup borrows the value. Tests confirm no heap allocation for standard, custom, or mixed-case names. A body received in one chunk reuses that buffer. Joining several chunks and decoding or encoding JSON still require allocations.

Typed handlers compile to concrete Rust types. The route table stores different handlers, so it places one future on the heap per request. The measured POST allocated a 528-byte future in both Nagi and Rust. The whole request path is not allocation-free.

## Conditions and reproduction

The test used a shared Linux x86_64 host with an AMD EPYC 9V74. The server used one pinned CPU and one worker; wrk used two other pinned CPUs and two threads. The container had a four-core CPU quota. Traffic was local HTTP/1.1 without a database. Nagi and Rust used the same runtime, dependency lock, release configuration, and limits.

The client maintains 64 connections and sends another request after a response. This differs from a fixed arrival-rate queueing test, an external network test, or DDoS protection validation.

```sh
nagic build benchmarks/http_stdlib.nagi
cargo build --release -p nagi-runtime --example axum_baseline
python scripts/http_stdlib_bench.py --wrk /path/to/wrk --build-matching-rust
```

`nagic build` generates a release application. Set `CARGO_TARGET_DIR` and `NAGI_NATIVE_TARGET_DIR` if using custom target locations. The [matched raw results](../../benchmarks/results/http-stdlib-matched/) retain each run's latency, CPU, RSS, and binary SHA-256.

The [HTTP load tests](http-capacity.md) record saturation, slow clients, and connection recovery for the legacy API. Its limits and implementation differ, so those results do not establish guarantees for standard HTTP. See the [HTTP guide](http.md) for current limits and configuration.
