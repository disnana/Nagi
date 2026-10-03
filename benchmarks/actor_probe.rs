// Appended to freshly generated Nagi source by scripts/actor_probe.py. Both
// implementations use the same native actor runtime in this one executable.
use nagi_runtime::{self as rt, actor as native_actor};
use std::{
    future::Future,
    hint::black_box,
    sync::Arc,
    time::{Duration, Instant},
};

async fn rust_factory(_: Arc<()>) -> Result<i64, rt::Error> {
    Ok(0)
}
async fn rust_scalar(
    state: i64,
    message: i64,
) -> Result<native_actor::Turn<i64, i64, rt::Error>, rt::Error> {
    Ok(native_actor::turn(state + 1, Ok(message)))
}
async fn rust_string(
    state: i64,
    message: String,
) -> Result<native_actor::Turn<i64, String, rt::Error>, rt::Error> {
    Ok(native_actor::turn(state + 1, Ok(message)))
}
async fn rust_lifecycle(
    state: i64,
    message: i64,
) -> Result<native_actor::Turn<i64, i64, rt::Error>, rt::Error> {
    if message == 0 {
        rt::sleep(100).await;
    }
    if message < 0 {
        return Err(rt::Error::invalid("probe controlled crash"));
    }
    Ok(native_actor::turn(state + 1, Ok(state + 1)))
}

type Reply<T> = Result<Result<T, rt::Error>, native_actor::CallError>;
// Match the generated Nagi call wrappers, including the borrowed actor lifetime
// and owned argument captures. Both sides await the same native operation.
async fn rust_scalar_call<'a>(
    mut worker: &'a native_actor::Actor<i64, i64, rt::Error>,
    mut message: i64,
    mut mailbox_ms: i64,
) -> Reply<i64> {
    return native_actor::call(worker, message, mailbox_ms, 5000).await;
}
async fn rust_string_call<'a>(
    mut worker: &'a native_actor::Actor<String, String, rt::Error>,
    mut message: String,
    mut mailbox_ms: i64,
) -> Reply<String> {
    return native_actor::call(worker, message, mailbox_ms, 5000).await;
}

fn response<T>(reply: Reply<T>) -> T {
    reply.unwrap().unwrap()
}

async fn timed<M, R, F, Fut>(
    name: &str,
    bytes: usize,
    round: usize,
    iterations: usize,
    mut make: impl FnMut() -> M,
    mut operation: F,
) where
    F: FnMut(M) -> Fut,
    Fut: Future<Output = Reply<R>>,
{
    let mut samples = Vec::with_capacity(iterations);
    let total = Instant::now();
    for _ in 0..iterations {
        let started = Instant::now();
        black_box(response(operation(black_box(make())).await));
        samples.push(started.elapsed().as_nanos() as u64);
    }
    let elapsed = total.elapsed();
    samples.sort_unstable();
    let percentile =
        |percent: usize| samples[(iterations * percent).div_ceil(100).saturating_sub(1)];
    println!(
        "{}",
        rt::serde_json::json!({
            "phase":"timing", "implementation":name, "message_type":std::any::type_name::<M>(), "message_bytes":bytes, "round":round,
            "iterations":iterations, "elapsed_ns":elapsed.as_nanos(),
            "requests_per_second":iterations as f64 / elapsed.as_secs_f64(),
            "p50_ns":percentile(50), "p95_ns":percentile(95), "p99_ns":percentile(99),
            "scope":"sequential closed-loop; includes input creation, reply destruction, and per-call Instant observation; excludes startup/warmup/sorting"
        })
    );
}

async fn allocated<M, R, F, Fut>(iterations: usize, mut make: impl FnMut() -> M, mut operation: F)
where
    F: FnMut(M) -> Fut,
    Fut: Future<Output = Reply<R>>,
{
    for _ in 0..iterations {
        black_box(response(operation(black_box(make())).await));
    }
}

fn allocations<M, R, F, Fut>(
    runtime: &tokio::runtime::Runtime,
    name: &str,
    bytes: usize,
    iterations: usize,
    make: impl FnMut() -> M,
    operation: F,
) where
    F: FnMut(M) -> Fut,
    Fut: Future<Output = Reply<R>>,
{
    let (_, counts) =
        rt::metrics::measure(|| runtime.block_on(allocated(iterations, make, operation)));
    println!(
        "{}",
        rt::serde_json::json!({
            "phase":"allocation", "implementation":name, "message_type":std::any::type_name::<M>(), "message_bytes":bytes,
            "iterations":iterations, "allocation":counts,
            "allocations_per_request":counts.allocations as f64 / iterations as f64,
            "allocated_bytes_per_request":counts.allocated_bytes as f64 / iterations as f64,
            "scope":"separate untimed run; current_thread runtime including actor polling, input creation and reply destruction; excludes startup/warmup/reporting; bytes are cumulative allocation traffic, not retained RSS"
        })
    );
}

async fn lifecycle_probe(nagi: bool) {
    let name = if nagi { "nagi" } else { "plain_rust" };
    let group = native_actor::supervisor((), native_actor::default_options());
    let worker = if nagi {
        probe_lifecycle_actor(&group).unwrap()
    } else {
        native_actor::register(
            &group,
            "lifecycle",
            rust_factory,
            rust_lifecycle,
            native_actor::actor_options(
                1,
                1_048_576,
                1_048_576,
                5000,
                native_actor::RestartPolicy::TRANSIENT,
            )
            .unwrap(),
        )
        .unwrap()
    };
    let control = native_actor::control(&group);
    let running = tokio::spawn(native_actor::run(group));
    native_actor::ready(&worker, 5000).await.unwrap();
    // Both futures are polled on this task. Yielding the second until the first
    // has accepted its slow message makes mailbox refusal deterministic.
    let (accepted, refused) = tokio::join!(native_actor::call(&worker, 0, 5000, 5000), async {
        tokio::task::yield_now().await;
        native_actor::call(&worker, 1, 0, 5000).await
    });
    assert_eq!(response(accepted), 1);
    let overload = refused.unwrap_err().kind();
    assert_eq!(overload, native_actor::CallKind::MAILBOX_FULL);
    let began = Instant::now();
    let crash = native_actor::call(&worker, -1, 5000, 5000)
        .await
        .unwrap_err()
        .kind();
    assert_eq!(crash, native_actor::CallKind::REPLY_LOST);
    let generation = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let event = native_actor::next_event(&control).await.unwrap().unwrap();
            if event.kind() == native_actor::EventKind::STARTED && event.generation() > 1 {
                break event.generation();
            }
        }
    })
    .await
    .expect("restart observation deadline");
    let recovered = response(native_actor::call(&worker, 1, 5000, 5000).await);
    assert_eq!(
        recovered, 1,
        "a new factory state must replace failed state"
    );
    println!(
        "{}",
        rt::serde_json::json!({"phase":"lifecycle", "implementation":name,
        "mailbox_full":true,"reply_lost_after_crash":true,"recovered_state":recovered,
        "generation":generation,"restart_and_recovery_ns":began.elapsed().as_nanos()})
    );
    native_actor::shutdown(&control).await.unwrap();
    running.await.unwrap().unwrap();
}

fn process_ticks() -> Option<u64> {
    // Optional Linux evidence is captured by the child before it can exit.
    let status = std::fs::read_to_string("/proc/self/stat").ok()?;
    let fields = status
        .rsplit(')')
        .next()?
        .split_whitespace()
        .collect::<Vec<_>>();
    Some(fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?)
}

fn main() {
    let arguments = std::env::args().collect::<Vec<_>>();
    let get = |index: usize, default: &str| {
        arguments
            .get(index)
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let phase = get(1, "all");
    let iterations = get(2, "20000").parse::<usize>().unwrap().max(1);
    let rounds = get(3, "3").parse::<usize>().unwrap().max(1);
    let allocation_iterations = get(4, "1000").parse::<usize>().unwrap().max(1);
    let idle_ms = get(5, "1000").parse::<u64>().unwrap();
    let mailbox_ms = get(7, "5000").parse::<i64>().unwrap();
    assert!(matches!(mailbox_ms, 0 | 5000));
    let sizes = get(6, "64,4096")
        .split(',')
        .map(|value| value.parse::<usize>().unwrap())
        .collect::<Vec<_>>();
    assert!(sizes.iter().all(|bytes| *bytes <= 65_536));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    println!(
        "{}",
        rt::serde_json::json!({"phase":"configuration", "runtime":"tokio current_thread",
        "iterations":iterations,"rounds":rounds,"allocation_iterations":allocation_iterations,
        "message_sizes":sizes,"mailbox_capacity":64,"message_budget_bytes":1_048_576,
        "reply_budget_bytes":1_048_576,"admission_ms":mailbox_ms,"reply_ms":5000,
        "nagi_scalar_handler_future_bytes":std::mem::size_of_val(&probe_scalar_handler(0, 0)),
        "rust_scalar_handler_future_bytes":std::mem::size_of_val(&rust_scalar(0, 0)),
        "nagi_string_handler_future_bytes":std::mem::size_of_val(&probe_string_handler(0, String::new())),
        "rust_string_handler_future_bytes":std::mem::size_of_val(&rust_string(0, String::new())),
        "baseline":"plain Rust uses the same supervised runtime and same-signature async call wrappers; no unsupervised mpsc baseline"})
    );
    let group = native_actor::supervisor((), native_actor::default_options());
    let nagi_scalar = probe_scalar_actor(&group).unwrap();
    let nagi_string = probe_string_actor(&group).unwrap();
    let rust_options = native_actor::actor_options(
        64,
        1_048_576,
        1_048_576,
        5000,
        native_actor::RestartPolicy::TEMPORARY,
    )
    .unwrap();
    let rust_scalar_actor = native_actor::register(
        &group,
        "rust-scalar",
        rust_factory,
        rust_scalar,
        rust_options,
    )
    .unwrap();
    let rust_string_actor = native_actor::register(
        &group,
        "rust-string",
        rust_factory,
        rust_string,
        rust_options,
    )
    .unwrap();
    let control = native_actor::control(&group);
    let running = runtime.spawn(native_actor::run(group));
    runtime.block_on(async {
        native_actor::ready(&nagi_scalar, 5000).await.unwrap();
        native_actor::ready(&nagi_string, 5000).await.unwrap();
        native_actor::ready(&rust_scalar_actor, 5000).await.unwrap();
        native_actor::ready(&rust_string_actor, 5000).await.unwrap();
        for _ in 0..256 {
            assert_eq!(
                response(probe_scalar_call(&nagi_scalar, 7, mailbox_ms).await),
                7
            );
            assert_eq!(
                response(rust_scalar_call(&rust_scalar_actor, 7, mailbox_ms).await),
                7
            );
            for &bytes in &sizes {
                assert_eq!(
                    response(probe_string_call(&nagi_string, "x".repeat(bytes), mailbox_ms).await),
                    "x".repeat(bytes)
                );
                assert_eq!(
                    response(
                        rust_string_call(&rust_string_actor, "x".repeat(bytes), mailbox_ms).await
                    ),
                    "x".repeat(bytes)
                );
            }
        }
    });
    if phase == "all" || phase == "timing" {
        runtime.block_on(async {
            for round in 0..rounds {
                // Alternate pair order to avoid assigning warm/drift effects to
                // one implementation. Every message has its reply consumed.
                for nagi in if round % 2 == 0 {
                    [true, false]
                } else {
                    [false, true]
                } {
                    if nagi {
                        timed(
                            "nagi",
                            8,
                            round,
                            iterations,
                            || 7,
                            |value| probe_scalar_call(&nagi_scalar, value, mailbox_ms),
                        )
                        .await;
                    } else {
                        timed(
                            "plain_rust",
                            8,
                            round,
                            iterations,
                            || 7,
                            |value| rust_scalar_call(&rust_scalar_actor, value, mailbox_ms),
                        )
                        .await;
                    }
                    for &bytes in &sizes {
                        if nagi {
                            timed(
                                "nagi",
                                bytes,
                                round,
                                iterations,
                                || "x".repeat(bytes),
                                |value| probe_string_call(&nagi_string, value, mailbox_ms),
                            )
                            .await;
                        } else {
                            timed(
                                "plain_rust",
                                bytes,
                                round,
                                iterations,
                                || "x".repeat(bytes),
                                |value| rust_string_call(&rust_string_actor, value, mailbox_ms),
                            )
                            .await;
                        }
                    }
                }
            }
        });
    }
    if phase == "all" || phase == "allocation" {
        allocations(
            &runtime,
            "nagi",
            8,
            allocation_iterations,
            || 7,
            |value| probe_scalar_call(&nagi_scalar, value, mailbox_ms),
        );
        allocations(
            &runtime,
            "plain_rust",
            8,
            allocation_iterations,
            || 7,
            |value| rust_scalar_call(&rust_scalar_actor, value, mailbox_ms),
        );
        for &bytes in &sizes {
            allocations(
                &runtime,
                "nagi",
                bytes,
                allocation_iterations,
                || "x".repeat(bytes),
                |value| probe_string_call(&nagi_string, value, mailbox_ms),
            );
            allocations(
                &runtime,
                "plain_rust",
                bytes,
                allocation_iterations,
                || "x".repeat(bytes),
                |value| rust_string_call(&rust_string_actor, value, mailbox_ms),
            );
        }
    }
    if phase == "all" || phase == "idle" {
        let before = process_ticks();
        println!(
            "{}",
            rt::serde_json::json!({"phase":"idle_started", "idle_ms":idle_ms,"ready_actors":4,"cpu_ticks_before":before})
        );
        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(idle_ms)).await;
        });
        println!(
            "{}",
            rt::serde_json::json!({"phase":"idle_completed", "idle_ms":idle_ms,"ready_actors":4,"cpu_ticks_before":before,"cpu_ticks_after":process_ticks()})
        );
    }
    runtime.block_on(async {
        native_actor::shutdown(&control).await.unwrap();
        running.await.unwrap().unwrap();
    });
    if phase == "all" || phase == "lifecycle" {
        runtime.block_on(async {
            lifecycle_probe(true).await;
            lifecycle_probe(false).await;
        });
    }
}
