# Actor measurements

This measures the unreleased `std.actor`: send one message to a supervised actor, wait for its reply, then repeat. Generated Nagi and equivalent Rust run in one executable with the same runtime, async call wrappers, dependencies, and release profile.

## Calls and replies

Tokio current_thread was pinned to CPU 0 on a shared AMD EPYC 9V74 host. The environment had a four-core CPU quota; the CPU was not exclusive. Runs followed before→after→after→before for the admission change, with 14 rounds of 100,000 calls per condition. Values below are medians.

These are the after results. M calls/s means million calls per second; p99 is the time within which 99% of calls finish.

| mailbox_ms | Input | Nagi M calls/s | Rust M calls/s | Nagi p99 µs | Rust p99 µs |
| --- | --- | ---: | ---: | ---: | ---: |
| 0 | i64 | 1.513 | 1.493 | 0.711 | 0.716 |
| 0 | str 64 bytes | 1.356 | 1.325 | 0.791 | 0.806 |
| 0 | str 4096 bytes | 1.260 | 1.244 | 0.842 | 0.862 |
| 5000 | i64 | 1.255 | 1.254 | 0.922 | 0.912 |
| 5000 | str 64 bytes | 1.129 | 1.086 | 1.027 | 1.021 |
| 5000 | str 4096 bytes | 1.017 | 1.033 | 1.122 | 1.096 |

Even with `mailbox_ms=5000`, free capacity permits immediate admission. Avoiding unnecessary wait registration raised Nagi's i64 result from 0.6954 to 1.2551 M calls/s and lowered p99 from 1.582 to 0.922 µs. The 0ms result changed from 1.447 to 1.513 M calls/s. See the [raw logs and before patch](../../benchmarks/results/actor-stdlib/README.md) for conditions and variation.

Timing includes input creation, reply destruction, and a clock observation for each call. It excludes startup, warmup, and sorting afterward. This is sequential closed-loop traffic; it does not measure overloaded admission latency or establish a fixed throughput limit.

## Allocation, idle time, and restart

Allocations were counted in a separate untimed run of 10,000 calls. Both Nagi and Rust requested one allocation and 104 bytes per i64 call. Strings needed two allocations, including input creation, totaling string capacity +104 bytes. These are cumulative allocation requests, not resident memory.

This path adds no BoxFuture or task per message. The child loop has one erased future per incarnation, whose polling uses a vtable.

Four ready actors idled for five seconds with no observed increase in process CPU ticks. Tick resolution was 0.01s, so observed CPU time was below that resolution. RSS stayed near 2.6 MiB. This is the whole process, including warmup, sample storage, and allocator effects; it is not memory per actor.

A controlled handler failure recovered to a generation 2 reply in about 11ms, including the default 10ms restart delay. A restart creates fresh state and does not automatically replay the failed message.

## Reproduce

Build a release compiler from the latest source, then run from the repository root. The probe builds offline, so fetch dependencies first. Choose an allowed CPU for `--cpu`.

```sh
python scripts/actor_probe.py --nagic target/release/nagic --cpu 0 --iterations 100000 --rounds 7 --allocation-iterations 10000 --idle-ms 5000 --mailbox-ms 0 --output build/actor-probe-0.jsonl
python scripts/actor_probe.py --nagic target/release/nagic --cpu 0 --iterations 100000 --rounds 7 --allocation-iterations 10000 --idle-ms 5000 --mailbox-ms 5000 --output build/actor-probe-5000.jsonl
```

`--skip-build` reuses a binary only when runtime and input signatures match. The [raw-data instructions](../../benchmarks/results/actor-stdlib/README.md) explain the before/after ABBA comparison.

HTTP, databases, multiple cores, Elixir, and unsupervised mpsc are not compared here. See the [API](actor-reference.md) and [Supervisors](supervisor.md) for capacity, cancellation, and retry rules.
