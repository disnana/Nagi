# Reading the benchmarks

Results are in [Measurements](measurements.md) and `benchmarks/results/`. CPU results measure a whole kernel over 100,000 elements in ns/op, with ns/item also reported. They are not direct measurements of the isolated latency of one primitive operation.

Comparisons use the same machine, release opt-level=3, LTO disabled, warmup, and the median of seven repetitions. CPU kernel timing excludes input creation; inputs and checksums match. Rust, Nagi, and CPython run separately.

HTTP tests use wrk, pin every server to one logical CPU, and match request bodies and responses. They use 64 keep-alive connections, two load-generator threads, and three repetitions. Records include p50/p95/p99/max, socket/status errors, server CPU/RSS, and context switches. These closed-loop tests do not assess open-loop latency under overload.

The allocation counter records Rust GlobalAlloc requests on the calling thread. Reallocations also count as allocations; requested bytes are cumulative. SQLite's C allocator and allocations on other threads are excluded. RSS and allocated bytes are different metrics. Even when disabled, the counter has a TLS branch that can affect measurements.

Static cost reports identify locations where costs arise. They are not dynamic counts that account for loop iterations, runtime internals, or optimizer removal. Precise hardware cache-miss, branch-miss, instruction-count, and allocator-fragmentation measurements have not been performed.
