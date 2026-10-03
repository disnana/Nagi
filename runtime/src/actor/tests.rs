use super::*;
use futures_util::FutureExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Notify;

struct Counter(u64);

async fn counter(_: Arc<()>) -> Result<Counter, Error> {
    Ok(Counter(10))
}
async fn update(mut state: Counter, change: i64) -> Result<Turn<Counter, u64, String>, Error> {
    if change == -2 {
        return Err(Error::internal("restart this incarnation"));
    }
    if change < 0 {
        return Ok(turn(state, Err("negative change".into())));
    }
    state.0 += change as u64;
    let value = state.0;
    Ok(turn(state, Ok(value)))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn business_error_preserves_owned_state_and_restart_is_one_for_one() {
    let group = supervisor((), default_options());
    let actor = register(&group, "counter", counter, update, default_actor_options()).unwrap();
    let sibling = register(&group, "sibling", counter, update, default_actor_options()).unwrap();
    let stop = control(&group);
    assert_eq!(
        ready(&actor, 0).await.unwrap_err().kind(),
        CallKind::NOT_READY
    );
    let monitor = tokio::spawn(run(group));
    ready(&actor, 1000).await.unwrap();
    ready(&sibling, 1000).await.unwrap();
    assert_eq!(call(&actor, 5, 0, 1000).await.unwrap(), Ok(15));
    assert_eq!(
        call(&actor, -1, 0, 1000).await.unwrap(),
        Err("negative change".into())
    );
    assert_eq!(call(&actor, 0, 0, 1000).await.unwrap(), Ok(15));
    assert_eq!(call(&sibling, 3, 0, 1000).await.unwrap(), Ok(13));
    assert_eq!(
        call(&actor, -2, 0, 1000).await.unwrap_err().kind(),
        CallKind::REPLY_LOST
    );
    ready(&actor, 1000).await.unwrap();
    assert_eq!(call(&actor, 0, 0, 1000).await.unwrap(), Ok(10));
    assert_eq!(call(&sibling, 0, 0, 1000).await.unwrap(), Ok(13));
    shutdown(&stop).await.unwrap();
    monitor.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn global_restart_intensity_counts_attempts_and_stops_a_pending_sibling() {
    let count = Arc::new(AtomicUsize::new(0));
    let opts = restart_delay(options(2, 16, 2, 10_000, 1000).unwrap(), 0).unwrap();
    let group = supervisor(Arc::clone(&count), opts);
    task(
        &group,
        "permanent",
        |context| async move {
            context.fetch_add(1, Ordering::AcqRel);
            Ok(())
        },
        RestartPolicy::PERMANENT,
    )
    .unwrap();
    task(
        &group,
        "sibling",
        |_| async {
            std::future::pending::<()>().await;
            Ok(())
        },
        RestartPolicy::TEMPORARY,
    )
    .unwrap();
    let stop = control(&group);
    assert!(timeout(Duration::from_secs(1), run(group))
        .await
        .unwrap()
        .is_err());
    assert_eq!(
        count.load(Ordering::Acquire),
        3,
        "initial start plus two scheduled restarts"
    );
    assert_eq!(stop.snapshot().phase, Phase::Complete);
    assert_eq!(stop.snapshot().active_children, 0);
    assert!(stop.snapshot().context_destroyed);
    assert!(shutdown(&stop).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_business_reply_keeps_next_state_and_retained_input_capacity_is_checked() {
    let opts = actor_options(2, 512, 128, 1000, RestartPolicy::TRANSIENT).unwrap();
    let group = supervisor((), default_options());
    let actor = register(
        &group,
        "capacity",
        |_| async { Ok(Counter(0)) },
        |mut state: Counter, message: String| async move {
            state.0 += 1;
            let value = if message == "large reply" {
                let mut reply = String::with_capacity(4096);
                reply.push_str("tiny");
                reply
            } else {
                state.0.to_string()
            };
            Ok(turn(state, Ok::<_, ()>(value)))
        },
        opts,
    )
    .unwrap();
    let stop = control(&group);
    let monitor = tokio::spawn(run(group));
    ready(&actor, 1000).await.unwrap();
    let mut oversized = String::with_capacity(4096);
    oversized.push_str("tiny");
    assert_eq!(
        call(&actor, oversized, 0, 1000).await.unwrap_err().kind(),
        CallKind::MESSAGE_TOO_LARGE
    );
    assert_eq!(
        call(&actor, "large reply".into(), 0, 1000)
            .await
            .unwrap_err()
            .kind(),
        CallKind::REPLY_TOO_LARGE
    );
    assert_eq!(
        call(&actor, "next".into(), 0, 1000).await.unwrap(),
        Ok("2".into())
    );
    shutdown(&stop).await.unwrap();
    monitor.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn caller_timeout_keeps_accepted_work_and_all_reservations_until_handler_finishes() {
    let gate = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let group = supervisor((), default_options());
    let actor = register(
        &group,
        "writer",
        |_| async { Ok(Counter(0)) },
        {
            let gate = Arc::clone(&gate);
            let entered = Arc::clone(&entered);
            move |mut state: Counter, wait: bool| {
                let gate = Arc::clone(&gate);
                let entered = Arc::clone(&entered);
                async move {
                    if wait {
                        entered.notify_one();
                        gate.notified().await;
                    }
                    state.0 += 1;
                    let value = state.0;
                    Ok(turn(state, Ok::<_, ()>(value)))
                }
            }
        },
        actor_options(1, 512, 512, 1000, RestartPolicy::TRANSIENT).unwrap(),
    )
    .unwrap();
    let stop = control(&group);
    let monitor = tokio::spawn(run(group));
    ready(&actor, 1000).await.unwrap();
    let first_actor = clone_actor(&actor);
    let first = tokio::spawn(async move { call(&first_actor, true, 0, 20).await });
    timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    assert_eq!(
        first.await.unwrap().unwrap_err().kind(),
        CallKind::REPLY_TIMEOUT
    );
    assert_eq!(actor.hub.count.available_permits(), 0);
    assert!(actor.hub.bytes.available_permits() < 512);
    // This refusal does not enqueue/spawn/box or allocate diagnostic text.
    // Polling once measures only the calling thread, with the handler gated.
    let (refusal, allocations) =
        crate::metrics::measure(|| call(&actor, false, 0, 1000).now_or_never());
    assert_eq!(
        refusal
            .expect("nonwaiting full mailbox must be immediately ready")
            .unwrap_err()
            .kind(),
        CallKind::MAILBOX_FULL
    );
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.allocated_bytes, 0);
    gate.notify_one();
    assert_eq!(call(&actor, false, 1000, 1000).await.unwrap(), Ok(2));
    shutdown(&stop).await.unwrap();
    monitor.await.unwrap().unwrap();
    assert_eq!(actor.hub.count.available_permits(), 1);
    assert_eq!(actor.hub.bytes.available_permits(), 512);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_smaller_immediate_call_cannot_bypass_a_waiting_byte_reservation() {
    let retained = |capacity, text: &str| {
        let mut message = String::with_capacity(capacity);
        message.push_str(text);
        message
    };
    let first_message = retained(256, "hold");
    let waiting_message = retained(128, "large");
    let small_message = retained(8, "small");
    let metadata = size_of::<Envelope<String, u64, ()>>() - size_of::<String>();
    let charge = |message: &String| charged_bytes(message, metadata, 1_048_576, None).unwrap();
    let first_bytes = charge(&first_message);
    let waiting_bytes = charge(&waiting_message);
    let budget = first_bytes + waiting_bytes - 1;
    assert!(charge(&small_message) < waiting_bytes - 1);

    let gate = Arc::new(Notify::new());
    let entered = Arc::new(Notify::new());
    let group = supervisor((), default_options());
    let actor = register(
        &group,
        "fairness",
        |_| async { Ok(Counter(0)) },
        {
            let gate = Arc::clone(&gate);
            let entered = Arc::clone(&entered);
            move |mut state: Counter, message: String| {
                let gate = Arc::clone(&gate);
                let entered = Arc::clone(&entered);
                async move {
                    if message == "hold" {
                        entered.notify_one();
                        gate.notified().await;
                    }
                    state.0 += 1;
                    let value = state.0;
                    Ok(turn(state, Ok::<_, ()>(value)))
                }
            }
        },
        actor_options(3, budget as i64, 512, 1000, RestartPolicy::TRANSIENT).unwrap(),
    )
    .unwrap();
    let stop = control(&group);
    let monitor = tokio::spawn(run(group));
    ready(&actor, 1000).await.unwrap();
    let first_actor = clone_actor(&actor);
    let first = tokio::spawn(async move { call(&first_actor, first_message, 0, 5000).await });
    timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    assert_eq!(actor.hub.bytes.available_permits(), budget - first_bytes);

    // Poll the larger call until it queues for bytes while the first handler
    // is held. It keeps its place and count reservation during this wait.
    let waiting = call(&actor, waiting_message, 5000, 5000);
    tokio::pin!(waiting);
    assert!(matches!(
        futures_util::poll!(waiting.as_mut()),
        std::task::Poll::Pending
    ));
    assert_eq!(
        call(&actor, small_message, 0, 1000)
            .await
            .unwrap_err()
            .kind(),
        CallKind::MAILBOX_FULL
    );
    gate.notify_one();
    assert_eq!(first.await.unwrap().unwrap(), Ok(1));
    assert_eq!(waiting.await.unwrap(), Ok(2));
    assert_eq!(
        call(&actor, "after".to_owned(), 1000, 1000).await.unwrap(),
        Ok(3)
    );
    shutdown(&stop).await.unwrap();
    monitor.await.unwrap().unwrap();
    assert_eq!(actor.hub.count.available_permits(), 3);
    assert_eq!(actor.hub.bytes.available_permits(), budget);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_expired_admission_deadline_rejects_even_immediately_ready_operations() {
    let group = supervisor((), default_options());
    let actor = register(&group, "deadline", counter, update, default_actor_options()).unwrap();
    let stop = control(&group);
    let monitor = tokio::spawn(run(group));
    ready(&actor, 1000).await.unwrap();
    let sender = actor.sender(None).await.unwrap();
    let expired = Instant::now() - Duration::from_millis(1);
    assert_eq!(
        actor
            .admission_wait(expired, std::future::ready(()), Some(&sender))
            .await
            .unwrap_err()
            .kind(),
        CallKind::MAILBOX_TIMEOUT
    );
    assert_eq!(
        actor.sender(Some(expired)).await.unwrap_err().kind(),
        CallKind::MAILBOX_TIMEOUT
    );
    shutdown(&stop).await.unwrap();
    monitor.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_factory_construction_is_included_in_the_startup_deadline() {
    let group = supervisor((), options(1, 16, 0, 10_000, 1000).unwrap());
    let actor = register(
        &group,
        "constructor",
        |_| {
            std::thread::sleep(Duration::from_millis(15));
            std::future::ready(Ok(Counter(0)))
        },
        update,
        actor_options(1, 512, 512, 1, RestartPolicy::TRANSIENT).unwrap(),
    )
    .unwrap();
    let stop = control(&group);
    let monitor = tokio::spawn(run(group));
    assert!(ready(&actor, 1000).await.is_err());
    assert!(timeout(Duration::from_secs(1), monitor)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(stop.snapshot().context_destroyed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_deadline_and_zero_restart_budget_end_the_group_without_a_ready_actor() {
    let group = supervisor((), options(1, 16, 0, 10_000, 1000).unwrap());
    let actor = register(
        &group,
        "startup",
        |_| async {
            std::future::pending::<()>().await;
            Ok(Counter(0))
        },
        update,
        actor_options(1, 512, 512, 5, RestartPolicy::TRANSIENT).unwrap(),
    )
    .unwrap();
    let stop = control(&group);
    let monitor = tokio::spawn(run(group));
    assert!(ready(&actor, 1000).await.is_err());
    assert!(timeout(Duration::from_secs(1), monitor)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert_eq!(stop.snapshot().phase, Phase::Complete);
    assert!(stop.snapshot().context_destroyed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lifecycle_events_are_ordered_bounded_and_keep_utf8_truncation_metadata() {
    let group = supervisor((), options(1, 16, 0, 10_000, 1000).unwrap());
    let first = control(&group);
    let second = clone_control(&first);
    task(
        &group,
        "failure",
        |_| async { Err(Error::invalid("é".repeat(2000))) },
        RestartPolicy::TEMPORARY,
    )
    .unwrap();
    assert!(run(group).await.is_err());
    assert_eq!(
        next_event(&first).await.unwrap().unwrap().kind(),
        EventKind::STARTING
    );
    assert_eq!(
        next_event(&first).await.unwrap().unwrap().kind(),
        EventKind::STARTED
    );
    let failure = next_event(&first).await.unwrap().unwrap();
    assert_eq!(failure.kind(), EventKind::FAILED);
    assert!(failure.truncated());
    assert_eq!(failure.message().len(), 1024);
    assert_eq!(
        next_event(&second).await.unwrap().unwrap().kind(),
        EventKind::STARTING
    );
    while next_event(&first).await.unwrap().is_some() {}
    while next_event(&second).await.unwrap().is_some() {}
}
