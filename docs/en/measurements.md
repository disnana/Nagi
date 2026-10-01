# Nagi 0.1 performance and verification report

This version is a prototype language with a two-layer backend, a Rust compiler, and a working runtime. High generates editable Low text, which is parsed and checked again before compilation to native code through Rust. Working paths include HTTP body → typed class → SQLite → class → JSON CRUD, ordinary Low calls and replacements, async/scopes, actor communication, and worker restarts.

A general-purpose language, a production custom runtime, and BEAM-level fault isolation remain incomplete. Actors, supervisors, and queues are standard test functions calling the real runtime; general High declarations are not implemented. The numbers below come from logs in the stated environment. Public example names were anonymized while preserving their original ASCII byte lengths. Measurement values were not changed.

This is the English version of the September 30, 2026 report. Its test counts and implementation status describe that measurement snapshot. The [Japanese source](https://github.com/disnana/Nagi/blob/main/PERFORMANCE.md) is available on GitHub.

## Environment and reproduction conditions

| Item | Conditions |
|---|---|
| OS | Linux-6.18.44-x86_64-with-glibc2.39 |
| CPU | INTEL(R) XEON(R) PLATINUM 8573C |
| CPU allocation | Affinity 0–8; cgroup quota 8 CPUs. Shared host activity and frequency were not controlled |
| Memory limit | 8 GiB cgroup |
| Rust / Cargo | 1.98.1 (48a229cea 2026-09-01) / 1.98.1 |
| Build | Release, opt-level=3, LTO=false, codegen-units=1, panic=unwind; no target-cpu=native |
| Main dependencies | Tokio 1.53.1, Axum 0.8.9, serde 1.0.229, serde_json 1.0.151, rusqlite 0.40.2 with bundled SQLite; Cargo.lock included |
| CPython / Node | 3.12.14 / v24.19.0 |
| HTTP Python | aiohttp 3.13.5 + slots dataclass |
| Load generator | Native wrk 4.2.0, commit a211dd5a7050b1f9e8a9870b95513060e72ac4a0 |
| Date | September 30, 2026 UTC; details in environment.json |

CPU/microbenchmarks ran individually on CPU 0, without overlapping server load or builds. Rust used five warmups, a pilot to set loop counts, and the median of seven repetitions. Python used five warmups and 7×25 runs; Node used 50 JIT warmups and 7×100 runs. All used the same prepared 100,000 elements, excluding array creation from timing. Integer input was `(i*17+13)%997-498`; float input divided that value by seven. Node Number represents these inputs/results exactly but does not share the full i64 range semantics.

Frequency, shared-host contention, and cache state were not fully controlled. These are exploratory measurements in one environment. A few percent difference between runs does not establish language superiority. Raw timings are retained rather than selecting only favorable results.

Allocation counts cover the calling thread's Rust GlobalAlloc requests, excluding SQLite's C allocator and jobs/tasks allocated on other threads. Reallocations also count as allocations. Requested bytes are cumulative, not live/peak memory. Returned values are dropped after counts are read, so their deallocations are absent. The counter is disabled during timing, although its TLS branch remains. Static cost reports identify locations rather than dynamic allocation counts.

## CPU: matching inputs and results

Whole-kernel times are µs/op. ns/item divides by 100,000 elements and is an amortized value, not the isolated latency of one addition.

| kernel | Nagi µs/op | Rust µs/op | Python µs/op | Node µs/op | Nagi ns/item | Python/Nagi |
| --- | --- | --- | --- | --- | --- | --- |
| integer_sum | 14.667 | 13.895 | 2568.563 | 95.678 | 0.147 | 175.1× |
| map_reduce | 33.973 | 35.908 | 5622.608 | 136.259 | 0.340 | 165.5× |
| filter_reduce | 65.030 | 101.582 | 2317.947 | 139.674 | 0.650 | 35.6× |
| branch | 69.915 | 74.350 | 4514.531 | 271.324 | 0.699 | 64.6× |
| float_sum | 75.123 | 80.055 | 1637.475 | 79.255 | 0.751 | 21.8× |
| loop | 118.099 | 134.439 | 5173.244 | 193.316 | 1.181 | 43.8× |

Checksums matched across all four implementations, with float tolerance 1e-7: `{"integer_sum": -3184, "map_reduce": 90448, "filter_reduce": 12461527, "branch": 2106, "float_sum": -454.85714285709753, "loop": 299995}`. Nagi/Rust kernels all had zero Rust allocations. Python/Node allocations were not measured and cannot be compared with that zero.

CPython's built-in sum was a separate reference at 686.172 µs/op. The table uses explicit Python loops, not native NumPy kernels. Repeated input warms caches, so the results also do not measure DRAM bandwidth or huge datasets.

Nagi uses native integers and contiguous arrays without dynamic numeric boxing or type checks inside loops. Generated and handwritten Rust integer-sum disassemblies both contain four packed i64 `paddq` additions (integer-sum.asm/rust-integer-sum.asm). LLVM vectorization and unrolling reduce ns/item. Differences from handwritten Rust include generated code, layout, and run variance; they do not show an independent Nagi backend outperforming Rust.

## JSON, views, and strings

The JSON tests use the same small input: id=42, name=alice, age=18. ns/op covers the whole parsing/encoding operation; allocations are measured dynamically for one operation.

| Operation | ns/op | Allocations | Reallocations | Cumulative requested bytes |
| --- | --- | --- | --- | --- |
| json_parse_tree | 281.35 | 5 | 0 | 646 |
| json_tree_to_class | 325.72 | 5 | 0 | 646 |
| json_direct_class | 142.31 | 1 | 0 | 5 |
| json_borrowed_class | 178.68 | 0 | 0 | 0 |
| json_class_encode | 113.07 | 1 | 0 | 128 |
| json_encode_reused_buffer | 67.14 | 0 | 0 | 0 |
| json_parse_modify_encode | 289.75 | 2 | 0 | 133 |
| string_parse_i64_1000 | 10486.23 | 0 | 0 | 0 |
| view_1mib | 7.74 | 0 | 0 | 0 |
| copy_1mib | 57511.23 | 1 | 0 | 1048575 |

Direct decoding into User was 2.29× faster than going through an intermediate Value tree in this run. Allocations fell from 5 to 1 and requested bytes from 646 to 5. Deserialization goes straight to a Rust struct without a dictionary/map/general JSON tree. The remaining allocation is the owned String field.

Borrowed classes are handwritten Rust experiments using &str into the input. Despite zero allocations, they were not faster than owned classes in this run. Escaped JSON strings or retention beyond the input lifetime need owned data. High borrowed class fields are not implemented.

Encoding creates one Vec with capacity 128 bytes. Buffer reuse measured zero allocations but is not generally applied to HTTP. Responses follow native class → Vec<u8> → Body, without fully streaming serialization to sockets. Parse-modify-encode allocates a String and an output Vec.

The view test only creates a range reference into a 1 MiB buffer. It confirms pointer equality with input+1 and zero allocations. Copy actually duplicates 1,048,575 bytes. These do different work, so their time ratio is not a speedup for an equivalent operation. Borrowing HTTP bodies avoids an additional copy; it does not make the entire network/kernel/input-buffer path zero-copy.

## SQLite to classes: conversion and ownership

An in-memory database is preloaded with 10,000 rows. Cached SELECT retrieves 1/100/1,000/10,000 rows with id i64, name TEXT "alice", and age i32. Timing includes SQL execution, stepping, class construction, and Vec storage, excluding connection creation and seed inserts.

| Rows | Names per row µs | Indices µs | Indices + reserve µs | ID-only scan µs | Index allocations | Index cumulative bytes |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 0.790 | 0.767 | 0.853 | 0.598 | 3 | 229 |
| 100 | 30.197 | 23.004 | 22.772 | 10.039 | 107 | 10644 |
| 1000 | 327.006 | 184.723 | 209.405 | 94.325 | 1010 | 86824 |
| 10000 | 3398.379 | 2197.449 | 2384.866 | 980.799 | 10014 | 1360624 |

Resolving column names once at query start was 1.55× faster at 10,000 rows than resolving them per row. FromRow reads native fields directly without tuple/dictionary/ORM intermediates. TEXT outlives SQLite stepping and therefore needs an owned String per row; whole-row conversion is not allocation-free.

The difference from id-only scanning combines TEXT reads, String allocation, three-field conversion, and Vec storage. It does not isolate pure conversion time. Reserve reduces requested bytes and reallocations, but increased time in this 10,000-row run. No extra COUNT or guessed reserve was added to every query.

Inserting 1,000 rows in a transaction and rolling back took 488.667 µs/op; one insert/rollback took 3.463 µs/op. Zero Rust allocations do not imply zero SQLite C allocation, page processing, or disk I/O. This used memory storage and did not measure persistence/fsync.

HTTP database jobs go through a dedicated thread, bounded channel 64, and oneshot replies. Their roundtrip, job boxing, and scheduling are excluded from the synchronous microbenchmark but included in HTTP db_single_row below. Typed column checks happen at runtime; compile-time schema/SQL validation is absent.

## Changes after measurement

Column indices previously used a Vec for every query. Up to 16 columns now fit inline, with overflow using a Vec. Existing reordered/missing-column tests passed before rerunning the same benchmark.

| Operation | Before ns/op | After ns/op | Before alloc | After alloc | Before bytes | After bytes |
| --- | --- | --- | --- | --- | --- | --- |
| db_indexed_1 | 819.70 | 767.44 | 4 | 3 | 261 | 229 |
| db_indexed_1000 | 213607.44 | 184723.43 | 1011 | 1010 | 86856 | 86824 |
| db_indexed_10000 | 2219904.20 | 2197449.04 | 10015 | 10014 | 1360656 | 1360624 |

The established effect is one fewer allocation and 32 fewer bytes per query. The 10,000-row timing difference is small and is not treated as statistically established acceleration. Initial logs are micro-before.jsonl; later logs are micro-after.jsonl.

Actors gained pipeline sending with a maximum of 32 messages. Individual messages still pass through the same mailbox, and all oneshot replies are received to verify 20,000. This does not batch everything into one +20,000 operation. JSON buffer reuse and DB Vec reserve remain experiments, outside the standard HTTP/DB paths.

## HTTP: matching successful payloads

All servers use CPU 0 and one executor/event-loop worker. Wrk uses CPUs 7/8, two threads, and 64 keep-alive connections. Each case has a one-second warmup followed by three seconds of measurement, with three trials per framework/case. Framework order is reshuffled per trial with a fixed seed. Nagi/Axum share timeout/body-limit middleware. Only Nagi has an additional persistent SQLite thread. The middleware was not fully ported to Node/aiohttp, so the comparison does not guarantee identical internal work across frameworks.

| Case | Workload |
|---|---|
| plaintext | GET /health → ok |
| small_json | GET /small → a newly generated User JSON response |
| post_small | POST /echo, five-character name + age, type/unknown-field checks, then matching JSON |
| post_medium | Same POST with a 4,096-character name |
| query_parameter | Nagi only: typed Query limit/name → User JSON |
| db_single_row | Nagi only: typed Path id → DB worker → cached SELECT → User → JSON |

Nagi/Axum use the same native struct. Node uses JSON.parse → field checks → JSON.stringify; aiohttp uses json.loads → slots dataclass → type checks → json.dumps. Successful statuses and bodies match. Nagi does not skip schema checks. small_json does not resend fixed JSON bytes.

req/s and p50/p95/p99 are medians of the three individual trials; max is the largest maximum. Percentiles do not come from merged histograms. Latencies are µs; errors total socket/status/timeout failures.

| framework | case | req/s | p50 | p95 | p99 | max | errors |
| --- | --- | --- | --- | --- | --- | --- | --- |
| nagi | plaintext | 82,701 | 662 | 2318 | 4344 | 14501 | 0 |
| nagi | small_json | 74,344 | 754 | 2355 | 4321 | 109324 | 0 |
| nagi | post_small | 39,860 | 1154 | 9128 | 34048 | 121675 | 0 |
| nagi | post_medium | 36,641 | 1527 | 4072 | 6883 | 89114 | 0 |
| nagi | query_parameter | 68,076 | 814 | 2680 | 4930 | 25215 | 0 |
| nagi | db_single_row | 42,544 | 1410 | 3487 | 6039 | 181247 | 0 |
| axum | plaintext | 87,040 | 632 | 2557 | 6019 | 125302 | 0 |
| axum | small_json | 77,406 | 709 | 2336 | 4224 | 124887 | 0 |
| axum | post_small | 60,842 | 917 | 2568 | 5211 | 16715 | 0 |
| axum | post_medium | 36,366 | 1535 | 3979 | 6783 | 85825 | 0 |
| node | plaintext | 28,606 | 1725 | 6893 | 17529 | 110900 | 0 |
| node | small_json | 35,402 | 1618 | 3741 | 6269 | 25919 | 0 |
| node | post_small | 25,713 | 2203 | 5335 | 10869 | 79994 | 0 |
| node | post_medium | 17,537 | 3275 | 6992 | 15715 | 139972 | 0 |
| aiohttp | plaintext | 17,076 | 3442 | 7607 | 13375 | 90396 | 0 |
| aiohttp | small_json | 14,098 | 3999 | 8526 | 13581 | 47104 | 0 |
| aiohttp | post_small | 11,137 | 5154 | 10084 | 14567 | 27593 | 0 |
| aiohttp | post_medium | 6,518 | 8163 | 21121 | 40894 | 108190 | 0 |

These are closed-loop wrk results, not fixed-arrival-rate open-loop or overload tail-latency tests. Handwritten Axum using the same runtime is the main native reference. Generated wrappers, handlers, bodies, allocations, and SQLite workers should be assessed separately from CPU kernels.

For small POSTs, Nagi achieved about 39.9k req/s versus Axum's 60.8k, lower in this run. Large POSTs were both around 36k. No profile isolates a particular function behind the small-POST gap. Further measurements must separate wrapper and router/state costs. Native compilation alone does not optimize every HTTP path.

The following table shows server CPU usage during measurement, maximum final RSS across three trials, context switches/s summed over server threads, and maximum thread/FD counts. One logical CPU is 100%; CPU usage divides /proc CPU tick differences by wall time. Load-generator CPU/RSS are excluded.

| Framework | Case | CPU % | Final RSS MiB | Switches/s | Threads | FDs |
| --- | --- | --- | --- | --- | --- | --- |
| nagi | plaintext | 97.8 | 6.35 | 95 | 3 | 10 |
| nagi | small_json | 99.0 | 6.45 | 83 | 3 | 10 |
| nagi | post_small | 98.0 | 6.49 | 66 | 3 | 10 |
| nagi | post_medium | 98.0 | 6.52 | 37 | 3 | 36 |
| nagi | query_parameter | 97.9 | 6.52 | 73 | 3 | 50 |
| nagi | db_single_row | 98.6 | 6.67 | 65,710 | 3 | 10 |
| axum | plaintext | 98.2 | 4.28 | 100 | 2 | 10 |
| axum | small_json | 97.3 | 4.35 | 87 | 2 | 31 |
| axum | post_small | 98.4 | 4.36 | 60 | 2 | 13 |
| axum | post_medium | 99.1 | 4.38 | 32 | 2 | 10 |
| node | plaintext | 99.9 | 51.29 | 856 | 7 | 82 |
| node | small_json | 99.4 | 55.79 | 817 | 7 | 65 |
| node | post_small | 99.6 | 58.84 | 1,072 | 7 | 85 |
| node | post_medium | 99.5 | 68.02 | 1,659 | 7 | 86 |
| aiohttp | plaintext | 99.9 | 28.58 | 14 | 1 | 71 |
| aiohttp | small_json | 99.8 | 28.61 | 13 | 1 | 71 |
| aiohttp | post_small | 99.9 | 28.91 | 13 | 1 | 71 |
| aiohttp | post_medium | 99.0 | 29.83 | 11 | 1 | 71 |

All 54 trials recorded zero socket/status/timeout errors. These successful-response load tests do not assess production SLOs with authentication, TLS, HTTP/2, slow clients/DoS, or large streams.

## Continuous load for 120 seconds

Nagi /echo ran for 120 seconds with a 4,096-character name, 128 connections, two wrk threads, and one server worker. It achieved 35,238 req/s, p50 3222 µs, p95 7209 µs, p99 16531 µs, max 176174 µs, and zero socket/status/timeout errors.

One-second RSS samples ranged from 6.05–8.06 MiB, ending at 8.06 MiB under load and 7.16–7.16 MiB in five samples after stopping. FD counts were 10–138 under load and 10–10 afterward.

This checks for observed abnormalities and unreleased FDs within a finite period. It does not prove absence of memory leaks, multi-day stability, allocator fragmentation, or GC pauses. Retained high-water RSS needs further observation separating drops, allocator retention, and fragmentation. Whole-process allocation/free tracking and heap profiling were not performed.

## Concurrent tasks and TCP connections

The task test uses four Tokio workers. It creates 1,000/10,000/50,000/100,000 tasks and confirms each reaches its first poll using an atomic counter. No task can complete until the semaphore gate opens. RSS is measured with all tasks waiting, followed by release and full joining. Times are medians of three trials; RSS values are maxima. The harness includes gate/Arc/counter/spawn/join costs, not isolated application-task cost.

| Waiting tasks | Creation → all joined ms | All-waiting RSS MiB | Final RSS MiB | Unfinished |
| --- | --- | --- | --- | --- |
| 1,000 | 4.812 | 2.34 | 2.34 | 0 |
| 10,000 | 18.389 | 5.62 | 5.62 | 0 |
| 50,000 | 93.424 | 20.28 | 20.28 | 0 |
| 100,000 | 162.528 | 38.64 | 38.64 | 0 |

The older concurrency-before.jsonl counts total spawned tasks without establishing simultaneous waiting or peak usage. It is not directly comparable to the gated test. Final RSS includes allocator retention, not unfinished task counts. CPU work uses a separate spawn_blocking path with semaphore limit four; forcibly stopping started work is not implemented.

The TCP test holds 1,000/10,000 real simultaneous connections and receives a health response from each. Opening uses concurrency 128; the server has one worker. 50,000/100,000 TCP connections were not tested with the FD limit 16,384 and IPv4 ephemeral ports 32768–60999. A result for 100,000 tasks is not a result for 100,000 TCP connections.

| Requested TCP | Held TCP | Open + response seconds | Held RSS MiB | After-disconnect RSS MiB | Initial FDs | Held FDs | Final FDs | Errors |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1000 | 1000 | 0.387 | 28.35 | 28.35 | 10 | 1010 | 10 | 0 |
| 10000 | 10000 | 4.484 | 249.58 | 249.59 | 10 | 10010 | 10 | 0 |

FDs returned to baseline in both trials. RSS remained at its high-water mark immediately after disconnect. Around 250 MiB held 10,000 connections; the approximate 25 KiB/connection difference covers the whole process, including HTTP buffers, runtime, and allocator. Kernel socket memory is outside RSS.

## Actors, supervisors, and queues

Five trials use the same CounterActor, mailbox capacity 64, four Tokio workers, and 20,000 messages. Serial RPC waits for each reply before sending another; the pipeline allows up to 32 in flight. µs/message is amortized elapsed/messages, not individual p50/p99 latency.

| Path | Messages/s | Amortized µs/message | Mailbox | Maximum send batch |
| --- | --- | --- | --- | --- |
| actor_rpc | 22,204 | 45.038 | 64 | 1 |
| actor_pipelined | 554,022 | 1.805 | 64 | 32 |

Pipeline throughput was 24.95× the serial version in this test. It overlaps waits and scheduler roundtrips; individual RPC latency did not necessarily fall by that factor. Mailbox/reply allocations and ownership conversions remain. High shared uses explicit Arc, while internal task/channel/DB-state Arcs also remain.

The supervisor detected three worker panics and restarted three times. Restart latency was 1.951–2.137 ms from detection to first poll, including 1 ms backoff. An independent worker continued 12 times. Crash-loop testing stopped restarts after reaching a limit of two in a one-second window. SIGSEGV/abort/process kill isolation, request replay/loss, and arbitrary supervisor trees were outside the test.

The queue test used 1,000 jobs, eight workers, channel capacity 64, at most three attempts, and 1/2 ms retry backoff. It recorded 909 completions, 91 dead letters, 312 retries, and peak in-flight eight. Failures were injected from IDs. This counts dead letters without persistent DLQ, replay after process restart, or an exactly-once API.

After creating/stopping 100 database workers, live workers stayed 0→0. Unit tests confirm scope error/panic cancels and joins siblings and releases guards. External parent-future drops and body panics only request abort through JoinSet Drop; they cannot await asynchronous cleanup at that point.

## Safety and correctness checks

| Check | Result and scope |
|---|---|
| Rust unit tests | 23 runtime + 39 compiler = 62 passed; types/ranges/moves/view escape, Low roundtrip, replacement signatures, real SQLite, scopes, actors, supervisors, queues |
| Sample builds | 11 High + one standalone Low = 12 passed; Low calls/replacements returned 42; handwritten Low bytes survived regeneration |
| Real HTTP | 29 checks passed: CRUD, types/unknown fields/malformed JSON/UTF-8/ranges, body limits, bound SQL parameters, keep-alive, streams, WS text/binary, survival after timeout |
| Seeded mutation | 10,000 parser/check mutations + 10,000 malformed JSON inputs, seed 305419896, zero panics; not coverage-guided fuzzing |
| Code checks | fmt, clippy all-targets -D warnings, locked builds/tests passed; final-checks.txt |
| Distribution | ZIP extracted elsewhere and rebuilt with empty compiler/native targets; High values 10/16, standalone Low 4; relative runtime paths checked; distribution-checks.txt |
| Negative ownership | Owned/local view escape, use-after-move, movement/mutation during borrowing, task view escape rejected; partial moves/complex flow depend on final Rust checks |
| Pointers/zero-copy | Slice offsets and zero allocations checked; safe slices and backend borrow checks |
| Resources/faults | All 100k waiting tasks joined; all 10k TCP responses and FD return; worker restarts/intensity; DB worker termination |
| Not performed | ASan/TSan/Miri/Valgrind, coverage-guided fuzzing, hardware counters, flamegraphs, cache/branch misses, full allocator fragmentation, multi-day soak |

A successful High check alone does not guarantee soundness. Safe Rust is generated and must pass Rust borrow/Send checks before becoming a native binary. Runtime unsafe is limited to observation implementations such as System GlobalAlloc forwarding; arbitrary Low unsafe/FFI is not available. Generated code and runtime dependencies have not had an independent security audit.

## Implementation status and assessment

| Area | Working in this snapshot | Still needed |
|---|---|---|
| Two-layer compiler | Indentation High, text Low, parsers/checkers, native calls, signature-preserving replacements, Rust native builds | Crate separation, modules/imports, source spans, general generics/traits/function types, self-hosting |
| Native values | Primitives, Copy classes, contiguous Vecs, nullable, Result, UUID/timestamp | Complete Map/owned APIs, methods/interfaces, consistent overflow rules |
| Memory | Moves, conservative lexical borrows, views, explicit copy/shared, Rust drop | High request arenas, borrowed classes, precise escape/partial-move analysis |
| Async/concurrency | Tokio tasks, scopes, bounded-channel actors, restart/intensity, retry/DLQ experiments | General actor/queue/supervisor syntax, custom scheduler, durable delivery, cancellable CPU jobs |
| HTTP/JSON | HTTP/1, typed routes/query/body/responses, keep-alive, limits, timeouts, WS/stream examples | TLS/auth, HTTP/2, general routing, streaming JSON, buffer pools |
| Database | SQLite CRUD, dedicated worker, prepared cache, indexed FromRow, typed native fields | General parameters/pools/transactions, compile-time SQL/schema checks, PostgreSQL binary protocol |
| Low native control | Shared value types/functions/views for handwritten/generated code | Pointers/layout/alignment/alloc/free/unsafe/C ABI/SIMD |

Effective directions included native primitives with contiguous arrays, direct typed JSON, once-per-query column resolution, removal of small unnecessary Vecs, and actor pipelines. Borrowed JSON and reserve results show why allocation reductions must be assessed separately from speed. HTTP includes handlers/runtime/serialization/scheduling and cannot be predicted from CPU-kernel speed ratios.

Next priorities are arithmetic/borrow rules, modules/generics, arbitrary-state actor lowering, general typed database parameters, and measured request arenas/buffer reuse. Prototype development speed does not establish the effort needed to finish the language. Low self-hosting comes after String/Map/module/allocator APIs and bootstrap-equivalence tests.

## Rerunning and raw logs

Use the repository README for builds and functional tests. During performance measurements, avoid concurrent builds or other CPU benchmarks and adjust CPU affinity to your environment.

```bash
taskset -c 0 ./native-target/release/nagi-cpu > benchmarks/results/cpu-nagi.jsonl
taskset -c 0 ./target/release/examples/microbench > benchmarks/results/micro-after.jsonl
taskset -c 0 python3 benchmarks/python_cpu.py > benchmarks/results/cpu-python.jsonl
taskset -c 0 node benchmarks/node_cpu.js > benchmarks/results/cpu-node.jsonl
./target/release/examples/concurrency_bench > benchmarks/results/concurrency-after.jsonl 2> benchmarks/results/fault-after.log
python3 tests/connections.py > benchmarks/results/connections.json
python3 scripts/http_bench.py --wrk /absolute/path/to/wrk --soak 120
python3 scripts/summarize_results.py
```

results/ contains CPU/JSON/SQLite/allocation/concurrency JSONL, individual wrk text logs, aggregated HTTP JSON, one-second resource samples, fault stderr, environment data, and checksum verification. The full wrk source is excluded; its commit and Lua scripts are recorded. Linux-specific taskset/proc measurements need replacements on other OSes. CI definitions were included but had not been run remotely when this report was written.

The original snapshot refers to 22 design/specification pages in docs/. Reproduction sources are in compiler/runtime/examples/tests/benchmarks/scripts. Later Docs and compiler additions are described in the current guides.
