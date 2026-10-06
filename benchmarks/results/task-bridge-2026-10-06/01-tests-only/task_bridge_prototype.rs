//! ADR012のprivate bridge先行試験。公開Task/checker/旧Scopeの実装ではない。

#[cfg(test)]
mod tests {
    use super::{Exit, FaultKind, Scope, Task};
    use crate::Error;
    use futures_util::FutureExt;
    use std::{
        future::{pending, Future},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Condvar, Mutex,
        },
        task::Poll,
        time::Duration,
    };
    use tokio::sync::{oneshot, Notify};

    #[derive(Default)]
    struct Signal {
        flag: AtomicBool,
        changed: Notify,
    }
    impl Signal {
        fn set(&self) {
            self.flag.store(true, Ordering::SeqCst);
            self.changed.notify_waiters();
        }
        async fn wait(&self) {
            loop {
                let changed = self.changed.notified();
                if self.flag.load(Ordering::SeqCst) {
                    return;
                }
                changed.await;
            }
        }
    }
    // child Futureの同期Dropを止める。parentは別runtime threadでgateを解放する。
    #[derive(Default)]
    struct DropGate {
        entered: Signal,
        released: Mutex<bool>,
        changed: Condvar,
        completed: Signal,
    }
    impl DropGate {
        fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.changed.notify_all();
        }
        fn block(&self) {
            self.entered.set();
            let released = self.released.lock().unwrap();
            let (released, timeout) = self
                .changed
                .wait_timeout_while(released, Duration::from_secs(10), |ready| !*ready)
                .unwrap();
            assert!(
                *released && !timeout.timed_out(),
                "Drop gate watchdog expired"
            );
            self.completed.set();
        }
    }
    struct Cleanup(Arc<DropGate>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.0.block();
        }
    }
    struct Release(Arc<DropGate>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.release();
        }
    }
    async fn watch<F: Future>(future: F) -> Result<F::Output, &'static str> {
        match tokio::time::timeout(
            Duration::from_secs(10),
            std::panic::AssertUnwindSafe(future).catch_unwind(),
        )
        .await
        {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => Err("bridge observation panicked"),
            Err(_) => Err("bridge watchdog expired"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn heterogeneous_results_and_business_error_keep_sibling_healthy() {
        let mut scope = Scope::new();
        let started = Arc::new(Signal::default());
        let (release, receiver) = oneshot::channel();
        let child_started = Arc::clone(&started);
        let sibling = scope.spawn(async move {
            child_started.set();
            receiver.await.unwrap();
            88_u64
        });
        let business = scope.spawn(async { Err::<String, u8>(9) });
        let unit = scope.spawn(async {});
        let owned = scope.spawn(async { vec![String::from("owned")] });
        let ready = watch(started.wait()).await;
        let rejection = watch(scope.receive(business)).await;
        let observed_unit = watch(scope.receive(unit)).await;
        let observed_owned = watch(scope.receive(owned)).await;
        let before_release = scope.stats();
        let released = release.send(());
        let observed_sibling = watch(scope.receive(sibling)).await;
        let joined = watch(scope.join()).await;
        assert!(ready.is_ok() && released.is_ok());
        assert!(matches!(rejection, Ok(Ok(Err(9)))));
        assert!(matches!(observed_unit, Ok(Ok(()))));
        assert_eq!(observed_owned.unwrap().unwrap(), vec!["owned"]);
        assert!(!before_release.failed);
        assert_eq!(before_release.unjoined, 1);
        assert_eq!(observed_sibling.unwrap().unwrap(), 88);
        assert!(matches!(joined, Ok(Ok(()))));
        assert_eq!(scope.stats().joined, 4);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn receiver_notification_cannot_replace_actual_join() {
        let mut scope = Scope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let task = scope.spawn_with_cleanup(async { 21_u64 }, Cleanup(Arc::clone(&gate)));
        let entered = watch(gate.entered.wait()).await;
        let output_ready = task.output_ready();
        let mut receiving = Box::pin(scope.receive(task));
        let observed = futures_util::poll!(&mut receiving);
        let stayed_pending = observed.is_pending();
        drop(receiving);
        gate.release();
        let completed = watch(gate.completed.wait()).await;
        let joined = watch(scope.join()).await;
        assert!(entered.is_ok() && output_ready);
        assert!(completed.is_ok() && matches!(joined, Ok(Ok(()))));
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
        assert!(
            stayed_pending,
            "typed notification arrived before child Future Drop/join"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn received_fault_remains_sticky_at_scope_exit() {
        let mut scope = Scope::new();
        let task: Task<()> = scope.spawn(async { panic!("controlled child fault") });
        let received = watch(scope.receive(task)).await;
        let exit = watch(scope.join()).await;
        let repeated = watch(scope.join()).await;
        let received = received.unwrap().unwrap_err();
        assert_eq!(received.kind, FaultKind::Panicked);
        assert_eq!(exit.unwrap().unwrap_err(), received);
        assert_eq!(repeated.unwrap().unwrap_err(), received);
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fault_return_waits_for_requested_sibling_cleanup_and_actual_join() {
        let mut scope = Scope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let started = Arc::new(Signal::default());
        let child_started = Arc::clone(&started);
        let sibling = scope.spawn_with_cleanup(
            async move {
                child_started.set();
                pending::<()>().await;
            },
            Cleanup(Arc::clone(&gate)),
        );
        let ready = watch(started.wait()).await;
        let failed: Task<()> = scope.spawn(async { panic!("primary") });
        let mut receiving = Box::pin(scope.receive(failed));
        let observed = tokio::select! {
            entered = watch(gate.entered.wait()) => entered.is_ok(),
            _ = &mut receiving => false,
        };
        drop(receiving); // 生存Scopeのfault/drain記録は失われてはならない。
        let interrupted = scope.stats();
        scope.discard(sibling).unwrap();
        gate.release();
        let resumed = watch(scope.join()).await;
        assert!(ready.is_ok() && observed);
        assert!(interrupted.failed && interrupted.unjoined != 0);
        assert_eq!(resumed.unwrap().unwrap_err().kind, FaultKind::Panicked);
        assert_eq!(scope.stats().joined, 2);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unpolled_and_pending_receive_drop_keep_scope_join_responsibility() {
        let mut scope = Scope::new();
        let unpolled = scope.spawn(pending::<u64>());
        drop(scope.receive(unpolled));
        let target = scope.spawn(pending::<u64>());
        let other = scope.spawn(async { 7_u64 });
        // targetは完了不能なので、受取がPendingになった後もScopeへ責任が残る。
        let mut receiving = Box::pin(scope.receive(target));
        let stayed_pending = futures_util::poll!(&mut receiving).is_pending();
        drop(receiving);
        let other_value = watch(scope.receive(other)).await;
        let cancelled = watch(scope.cancel()).await;
        assert!(stayed_pending);
        assert_eq!(other_value.unwrap().unwrap(), 7);
        assert!(matches!(cancelled, Ok(Ok(()))));
        assert_eq!(scope.stats().joined, 3);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_drain_can_resume_after_child_drop_gate() {
        let mut scope = Scope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let started = Arc::new(Signal::default());
        let child_started = Arc::clone(&started);
        let task = scope.spawn_with_cleanup(
            async move {
                child_started.set();
                pending::<()>().await;
            },
            Cleanup(Arc::clone(&gate)),
        );
        let ready = watch(started.wait()).await;
        scope.discard(task).unwrap();
        let mut cancelling = Box::pin(scope.cancel());
        let observed = tokio::select! {
            entered = watch(gate.entered.wait()) => entered.is_ok(),
            _ = &mut cancelling => false,
        };
        drop(cancelling);
        let interrupted = scope.stats();
        gate.release();
        let resumed = watch(scope.cancel()).await;
        let exit = watch(scope.join()).await;
        assert!(ready.is_ok() && observed);
        assert_eq!(interrupted.joined, 0);
        assert_eq!(interrupted.unjoined, 1);
        assert!(matches!(resumed, Ok(Ok(()))) && matches!(exit, Ok(Ok(()))));
        assert_eq!(scope.stats().joined, 1);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test]
    async fn discard_keeps_fault_and_legacy_error_observation() {
        let mut scope = Scope::new();
        let fault: Task<()> = scope.spawn(async { panic!("discarded child still faults") });
        scope.discard(fault).unwrap();
        let failed = watch(scope.join()).await;
        assert_eq!(failed.unwrap().unwrap_err().kind, FaultKind::Panicked);
        assert_eq!(scope.stats().entries, 0);
        let mut legacy = Scope::new();
        legacy.spawn_legacy(async { Err(Error::invalid("legacy terminal")) });
        let failed = watch(legacy.join()).await;
        assert_eq!(failed.unwrap().unwrap_err().kind, FaultKind::LegacyError);
        assert!(legacy.stats().failed);
        assert_eq!(legacy.stats().joined, 1);
    }

    #[tokio::test]
    async fn fake_native_id_reuse_does_not_mix_unreceived_tickets_and_retire_records() {
        let mut scope = Scope::new();
        let first = scope.spawn_fake_id(async { String::from("first") }, 55);
        let joined_first = watch(scope.observe_one()).await;
        let retained = scope.stats();
        let second = scope.spawn_fake_id(async { 42_u64 }, 55);
        let second_value = watch(scope.receive(second)).await;
        let first_value = watch(scope.receive(first)).await;
        let exit = watch(scope.join()).await;
        assert!(joined_first.unwrap());
        assert_eq!(retained.joined, 1);
        assert_eq!(retained.entries, 1);
        assert_eq!(retained.unjoined, 0);
        assert_eq!(second_value.unwrap().unwrap(), 42);
        assert_eq!(first_value.unwrap().unwrap(), "first");
        assert!(matches!(exit, Ok(Ok(()))));
        assert_eq!(scope.stats().entries, 0);
        assert_eq!(scope.stats().joined, 2);
        // 完了履歴を保持しない。業務Err/値の受取を反復してもentryは退役する。
        for value in 0..32_u64 {
            let task = scope.spawn(async move { value });
            assert_eq!(watch(scope.receive(task)).await.unwrap().unwrap(), value);
            assert_eq!(scope.stats().entries, 0);
        }
        assert_eq!(scope.stats().joined, 34);
    }

    #[tokio::test]
    async fn target_other_than_first_join_and_body_error_primary_are_preserved() {
        let mut scope = Scope::new();
        let first = scope.spawn(async { 11_u64 });
        let observed = watch(scope.observe_one()).await;
        let second = scope.spawn(async { 12_u64 });
        let second_value = watch(scope.receive(second)).await;
        let first_value = watch(scope.receive(first)).await;
        assert!(observed.unwrap());
        assert_eq!(second_value.unwrap().unwrap(), 12);
        assert_eq!(first_value.unwrap().unwrap(), 11);
        scope.spawn_legacy(async { Err(Error::invalid("secondary legacy fault")) });
        let finished = watch(scope.finish(Err::<(), _>(String::from("original body error")))).await;
        assert!(
            matches!(finished, Ok(Err(Exit::Body(message))) if message == "original body error")
        );
        assert_eq!(scope.stats().joined, 3);
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn parent_drop_requests_abort_but_does_not_report_child_destruction_complete() {
        let mut scope = Scope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let started = Arc::new(Signal::default());
        let child_started = Arc::clone(&started);
        let task = scope.spawn_with_cleanup(
            async move {
                child_started.set();
                pending::<()>().await;
            },
            Cleanup(Arc::clone(&gate)),
        );
        let ready = watch(started.wait()).await;
        drop(scope);
        let entered = watch(gate.entered.wait()).await;
        let was_complete = gate.completed.flag.load(Ordering::SeqCst);
        drop(task);
        gate.release();
        let completed = watch(gate.completed.wait()).await;
        assert!(ready.is_ok() && entered.is_ok() && completed.is_ok());
        assert!(
            !was_complete,
            "Scope Drop pretended to join a blocked child"
        );
    }

    #[derive(Debug)]
    struct Payload {
        events: Arc<Mutex<Vec<(&'static str, std::thread::ThreadId)>>>,
        label: &'static str,
    }
    impl Clone for Payload {
        fn clone(&self) -> Self {
            panic!("bridge must not clone payload")
        }
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.events
                .lock()
                .unwrap()
                .push((self.label, std::thread::current().id()));
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn payload_drop_locations_distinguish_failed_send_buffer_and_received_value() {
        let mut scope = Scope::new();
        let events = Arc::new(Mutex::new(vec![]));
        let parent = std::thread::current().id();
        let (release, receiver) = oneshot::channel();
        let child_events = Arc::clone(&events);
        let failed_send = scope.spawn(async move {
            receiver.await.unwrap();
            Payload {
                events: child_events,
                label: "child failed send",
            }
        });
        scope.discard(failed_send).unwrap();
        let released = release.send(());
        let joined = watch(scope.join()).await;
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        let child_events = Arc::clone(&events);
        let buffered = scope.spawn_with_cleanup(
            async move {
                Payload {
                    events: child_events,
                    label: "parent buffer",
                }
            },
            Cleanup(Arc::clone(&gate)),
        );
        let entered = watch(gate.entered.wait()).await;
        let ready = buffered.output_ready();
        scope.discard(buffered).unwrap();
        gate.release();
        let buffer_joined = watch(scope.join()).await;
        let child_events = Arc::clone(&events);
        let received = scope.spawn(async move {
            Payload {
                events: child_events,
                label: "parent received",
            }
        });
        let value = watch(scope.receive(received)).await;
        if let Ok(Ok(value)) = value {
            drop(value);
        } else {
            panic!("payload receive failed")
        }
        let exit = watch(scope.join()).await;
        assert!(released.is_ok() && entered.is_ok() && ready);
        assert!(
            matches!(joined, Ok(Ok(())))
                && matches!(buffer_joined, Ok(Ok(())))
                && matches!(exit, Ok(Ok(())))
        );
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].0, "child failed send");
        assert_ne!(events[0].1, parent);
        assert_eq!(events[1], ("parent buffer", parent));
        assert_eq!(events[2], ("parent received", parent));
        assert_eq!(scope.stats().entries, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn requested_cancel_does_not_hide_already_returned_legacy_error() {
        let mut scope = Scope::new();
        let gate = Arc::new(DropGate::default());
        let _release = Release(Arc::clone(&gate));
        scope.spawn_legacy_with_cleanup(
            async { Err(Error::invalid("already failed")) },
            Cleanup(Arc::clone(&gate)),
        );
        let entered = watch(gate.entered.wait()).await;
        let mut cancelling = Box::pin(scope.cancel());
        let stayed_pending = futures_util::poll!(&mut cancelling).is_pending();
        drop(cancelling);
        gate.release();
        let resumed = watch(scope.cancel()).await;
        assert!(entered.is_ok() && stayed_pending);
        assert_eq!(resumed.unwrap().unwrap_err().kind, FaultKind::LegacyError);
        assert_eq!(scope.stats().joined, 1);
    }

    #[tokio::test]
    async fn wrong_owner_and_missing_output_are_protocol_errors_without_stolen_join() {
        let mut original = Scope::new();
        let mut wrong = Scope::new();
        let task = original.spawn(async { 1_u64 });
        let rejected = watch(wrong.receive(task)).await;
        let original_joined = watch(original.join()).await;
        assert_eq!(rejected.unwrap().unwrap_err().kind, FaultKind::Internal);
        assert!(matches!(original_joined, Ok(Ok(()))));
        assert_eq!(original.stats().joined, 1);
        assert_eq!(wrong.stats().joined, 0);
        let mut missing = Scope::new();
        let task = missing.spawn_missing_output::<u64>();
        let rejected = watch(missing.receive(task)).await;
        let repeated = watch(missing.join()).await;
        assert_eq!(rejected.unwrap().unwrap_err().kind, FaultKind::Internal);
        assert_eq!(repeated.unwrap().unwrap_err().kind, FaultKind::Internal);
        assert_eq!(missing.stats().entries, 0);
    }
}
