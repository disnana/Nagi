# std.actor measurement data

These logs measure actual generated Nagi and equivalent plain Rust using the same supervised actor runtime. Both implementations are in one executable, with matching async call wrappers, manifest dependencies, lockfile, and release profile. The generated package is renamed once when building the combined probe; no dependency or profile changes are made.

| File | Contents |
| --- | --- |
| [summary.json](summary.json) | Median throughput and percentiles, with throughput ranges, from the ABBA comparison |
| [ready-admission-abba.jsonl](ready-admission-abba.jsonl) | Before→after→after→before timing runs for both mailbox modes; two batches of seven rounds per variant, 100,000 calls per round |
| [actor-probe-final-0.jsonl](actor-probe-final-0.jsonl) | After variant, immediate admission: seven timing rounds, separate allocation counts, five-second idle observation, and lifecycle probe |
| [actor-probe-final-5000.jsonl](actor-probe-final-5000.jsonl) | Same after binary with a 5000ms admission deadline |
| [before-ready-admission.patch](before-ready-admission.patch) | Applies to the after runtime to reconstruct the before ready/admission paths |

The host was a shared AMD EPYC 9V74 Linux environment with a four-core CPU quota. Tokio current_thread was pinned to CPU 0, which was not exclusive. Calls are sequential closed-loop. Each timing includes input creation, reply destruction, and per-call Instant observation, excluding startup, warmup, and sorting. Allocation counts use a separate untimed 10,000-call run including actor polling. The plain Rust baseline uses Supervisor too; it is not an unsupervised mpsc comparison.

The [probe source](../../actor_probe.rs) is appended to newly generated Nagi by [actor_probe.py](../../../scripts/actor_probe.py). Scalar and string handler futures have matching sizes of 24 and 40 bytes on the two sides. The child loop is erased once per incarnation and polled through a vtable; handlers do not add a task or BoxFuture for each message.

## Recorded identities

Environment records contain full SHA-256 values for binaries, sources, manifests, and lockfiles. The comparison used these common identities:

| Input | SHA-256 |
| --- | --- |
| Generated Rust | `dd781699a47921177bb03a1de8330402b0bc5a5e47c871b8bfaab8ab521b0c53` |
| Combined probe Rust | `540ae0008c7fe23f5b502f5d89e79718fb23654ce77a0dc6b375207e53955d6e` |
| Probe manifest | `0395756b78cb0f85912750af515d21038b8fd51a977c41edbf3571fc11a3d33f` |
| Probe lockfile | `8e4cf80b7221076aed0a0f4624118567a056f9249b892e93e6141f480e3f630f` |
| Before actor.rs | `bc9ac016793630ea13bb58ba08985c6d09eb2a98cbf339bc0bda53858b792570` |
| After actor.rs | `366e59d9e45704b2fb9a08c843116c50e5996484a534a26eec6e29e8346cd06c` |
| Before binary | `c930fe11f30e4543409e4c99ccb8ea1b6e31364c88445e7a4eeccec7978f9de2` |
| After binary | `8f0020c8397d0a18eb9369c6a2be08bf1959b6b277c786ea623ecd54dbeaca23` |

The final logs also record the Nagi input, benchmark input, and whole runtime-source hashes. Binary hashes identify the recorded artifacts; another toolchain or environment may produce different binaries.

## Reproduce

Build a release compiler from the commit containing this benchmark, then run from its repository root. The probe builds offline; populate Cargo's dependency cache first. Use a CPU allowed by your environment and separate output paths for every batch.

```sh
python scripts/actor_probe.py --nagic target/release/nagic --cpu 0 --iterations 100000 --rounds 7 --allocation-iterations 10000 --idle-ms 5000 --mailbox-ms 0 --output build/actor-final-0.jsonl
python scripts/actor_probe.py --nagic target/release/nagic --cpu 0 --iterations 100000 --rounds 7 --allocation-iterations 10000 --idle-ms 5000 --mailbox-ms 5000 --output build/actor-final-5000.jsonl
```

To reproduce the before variant, set `ACTOR_COMMIT` to that commit and create a clean isolated checkout. Do not apply the patch in your active working tree.

```sh
git worktree add --detach ../Nagi-actor-before "$ACTOR_COMMIT"
cd ../Nagi-actor-before
git apply benchmarks/results/actor-stdlib/before-ready-admission.patch
cargo build --release --locked -p nagic
python scripts/actor_probe.py --nagic target/release/nagic --cpu 0 --phase timing --iterations 100000 --rounds 7 --mailbox-ms 5000 --output build/actor-before-5000.jsonl
```

Use separate native target directories for the two checkouts. Run before, after, after, before for each of 0ms and 5000ms, retaining every output: seven rounds per batch gives 14 rounds per variant. Rebuild after switching runtime variants. `--skip-build` checks runtime and probe input signatures and refuses stale binaries; it also checks that the probe manifest and lock still match the generated project.

The idle RSS includes the process, warmup, timing sample storage, and allocator; it is not memory per actor. Lifecycle records cover a controlled failure with the default 10ms restart delay and fresh state, with no automatic replay. These results do not compare HTTP, DB, multicore scaling, Elixir, or unsupervised mpsc. See the concise [English guide](../../../docs/en/actor-performance.md), [Japanese guide](../../../docs/actor-performance.md), and [API limits](../../../docs/en/actor-reference.md).
