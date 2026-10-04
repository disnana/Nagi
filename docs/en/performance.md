# Reading the benchmarks

This page explains the [September 30, 2026 measurements](measurements.md). Raw logs are in `benchmarks/results/`. Separate standard-API tests cover [HTTP](http-stdlib-performance.md) and [actors](actor-performance.md). Numbers from different implementations and test conditions are not directly comparable.

## CPU work

Each measurement covers a function processing 100,000 elements. It excludes input creation.

| Unit | Meaning |
|---|---|
| ns/op | Time for one whole function call, in nanoseconds |
| ns/item | Time per element, calculated from the whole function's time |

These are not isolated measurements of an operation such as a single addition. Rust, Nagi, and CPython run separately on the same machine, with matching inputs and checksums.

Rust and Nagi use release builds with opt-level=3 and LTO disabled. Tests warm up before measuring seven repetitions and report the median.

## HTTP

Tests use wrk and pin each server to one logical CPU. Request bodies and responses match. Tests use 64 keep-alive connections, two load-generator threads, and three repetitions.

| Metric | Meaning |
|---|---|
| p50 / p95 / p99 | Response-time percentiles; 95% of responses finish within the p95 time |
| max | Longest observed response time |
| socket / status error | Connection failures / unexpected HTTP status codes |
| CPU / RSS | Server CPU usage / resident memory |
| context switch | Number of times the OS switches between executing tasks |

This test sends the next request after receiving a response (closed-loop). It does not measure overloaded latency when requests keep arriving independently of responses (open-loop).

The additional [HTTP load tests](http-capacity.md) increase the requested send rate and include sustained traffic. They separate requested and dispatched rates, recording latency, memory, connection counts, and recovery after traffic stops. Observed throughput also depends on the test environment and client; it is not a fixed limit of Nagi.

## Allocations and reports

The allocation counter records Rust GlobalAlloc requests on the calling thread. Reallocations also count as allocations, and requested bytes are cumulative. SQLite's internal C allocations and allocations on other threads are excluded.

Cumulative allocated bytes differ from resident memory at a given moment (RSS). Even when disabled, the counter checks thread-local state, so the measurement code can affect execution time.

Static cost reports identify code locations where costs such as allocations arise. They do not count executions. Dynamic counts, including loops, runtime internals, and optimizer removal, require runtime measurements.

Precise CPU cache-miss, branch-miss, instruction-count, and allocator-fragmentation measurements have not been performed.
