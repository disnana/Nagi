# Public Nagi actor / Supervisor probe

This is a finite **Nagi application**, compiled by the normal CLI against the
production Rust runtime. `main.nagi` constructs `std.actor.Supervisor`, registers
Nagi factories/handlers, calls actors, observes the public event stream, checks
restart generation/state, and awaits public shutdown. `observe.rs` only records
clocks, completed replies and Tokio's public task metric; it never spawns a task,
implements an actor, supervises, restarts, or substitutes a mailbox.

The contract regression additionally runs actual High, independently checked
saved Low (after deleting High), and handwritten Low through the existing
`native_triple` harness. Its Rust adapter only injects finite failures and
observes guard/state/context destruction. It uses neither private runtime
snapshots nor an alternative Supervisor.

## Reproduce on Linux 64-bit

Use an installed compiler whose runtime source matches the checkout. Provide a
shared Cargo target, one build job, and check **at least 8 GiB free before and
during compilation**. The runner handles the corresponding checks during each
native observation. Example (adjust the explicit paths):

```sh
export CARGO_TARGET_DIR=/absolute/shared-target CARGO_BUILD_JOBS=1
nagic build benchmarks/public-supervisor/main.nagi --no-project --profile release \
  --rust benchmarks/public-supervisor/observe.rs --rust-dep tokio=1.48 \
  --out /absolute/owned-build-directory
python scripts/public_supervisor_probe.py \
  --binary /absolute/native-path-printed-by-build \
  --output /absolute/new-measurement-directory
```

The output directory must not exist. The binary's generated Cargo.lock records
the resolved Tokio version; `1.48` is a compatible version requirement, not a
claim that Cargo always selects exactly 1.48. No production options are changed.

Defaults are fixed before measurement: actor counts **1, 32, 128, 512**; **three
rounds** for each of **one and two CPU affinities**; **20,000 sequential calls per
client**, two concurrent clients, three phases (healthy, one worker failure,
recovered). The one-actor baseline shares that actor between both clients and
has no failure phase; larger stages use two distinct healthy actors and fault a
third. Remaining actors are ready and idle. CPU affinity limits host logical
CPUs, not reserved vCPUs; cgroup CPU quota and affinity are recorded. `NAGI_THREADS`
matches the selected affinity count. The load generators and observations share
the process and measured CPU. These are closed-loop roundtrip observations with
two outstanding calls, **not maximum mailbox saturation or HTTP capacity**.

No request-rate limiter is present. Mailbox 64 / message-and-reply 4 KiB bounds
are ample for the 8-byte messages in this workload. Successful completed nested
`Ok(Ok(reply))` alone counts as work. Refusal, timeout, business Err and admission
do not. Each measured healthy phase requires 40,000 successful replies and zero
failures; the deliberately failed call is checked separately and excluded.

Each child has wall 30 s, aggregate process CPU 15 s (hard limit 16 s), sampled
RSS 512 MiB, and an 8 GiB free-disk guard. The runner stops on the first failed
case, terminates only its owned process group, and never retries to success.
Each stage is far below a memory exhaustion test. Units are binary: **1 GiB =
1,073,741,824 bytes**. RSS is the target process, not the container or host.

Raw JSONL gzip files preserve every reply timing and fault interval. CPU time
covers the whole phase, including clients and observation overhead; zero clock
observations are unavailable, not free work. p99 is nearest rank over successful
complete roundtrips (including mailbox and scheduling), not handler-only time.
Fairness is a Jain completion index inside both clients' common active interval;
equal finite totals alone are not a fairness guarantee. The fault interval runs
from the deliberate call through public generation-2 event observation and
state-reset reply, and includes the default 10 ms restart delay. The healthy
completion rate over that interval includes any idle tail after clients finish.

Checkpoints record before registration, all actors ready, public shutdown while
handles are held, and after the Nagi scope releases handles plus one fixed 50 ms
settle observation. Final FD and Tokio task counts must match baseline. OS
threads are distinct from Tokio tasks. Retained RSS is reported: allocator pages,
event storage and timing vectors need not return to the OS immediately. RSS
alone cannot prove an owned object leak; exact guard/Drop assertions belong to
the contract test. A short passing run establishes neither a maximum actor
count nor long-term stability.

Results and source identities: [2026-10-27 observations](../results/public-supervisor-2026-10-27/README.md).
