// Q004の既存oracleを公開adapter上で再実行する。native coreは本体と同じ。
use super::adapter::{Adapter, AdapterSeams, AdmissionGate, Observer};
use super::{Config, Failure, Finish, Gate, Kind, Outcome, Tx};
use crate::FromRow;
use futures_util::FutureExt;
use rusqlite::{types::Value, Row};
use std::{sync::Arc, time::Duration};

// 多接続fixtureは同じfilesystem DBを使う。exclusive作成に成功したdirectoryだけ所有。
pub(super) struct MultiFile {
    directory: std::path::PathBuf,
    pub(super) path: std::path::PathBuf,
    observer: Option<Observer>,
}
impl MultiFile {
    pub(super) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let directory = loop {
            let ordinal = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let candidate = std::env::temp_dir().join(format!(
                "nagi-sqlite-multi-{}-{ordinal}",
                std::process::id()
            ));
            match std::fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("exclusive sqlite fixture: {error}"),
            }
        };
        let file = Self {
            path: directory.join("shared.sqlite"),
            directory,
            observer: None,
        };
        let connection = rusqlite::Connection::open(&file.path).unwrap();
        connection.execute_batch("CREATE TABLE items(n)").unwrap();
        connection.close().unwrap();
        file
    }
    fn config(&self) -> Config {
        Config {
            path: Some(self.path.clone()),
            ..Config::default()
        }
    }
    pub(super) fn observe(&mut self, adapter: &Adapter) {
        self.observer = Some(adapter.observer());
    }
}
impl Drop for MultiFile {
    fn drop(&mut self) {
        if let Some(observer) = &self.observer {
            let stats = observer.snapshot();
            if stats.pending_workers != 0 || stats.starting != 0 {
                eprintln!(
                    "unjoined sqlite fixture retained at {}: pending={}, starting={}",
                    self.directory.display(),
                    stats.pending_workers,
                    stats.starting
                );
                return;
            }
        }
        if let Err(error) = std::fs::remove_dir_all(&self.directory) {
            if std::thread::panicking() {
                eprintln!("sqlite fixture cleanup during unwind: {error}");
            } else {
                panic!("joined sqlite fixture cleanup: {error}");
            }
        }
    }
}

// watchdog/panicは結果として保持し、Gate解放・active Tx終端・closeの後に失敗assertする。
async fn multi_watch<F: std::future::Future>(future: F) -> Result<F::Output, &'static str> {
    match tokio::time::timeout(
        Duration::from_secs(10),
        std::panic::AssertUnwindSafe(future).catch_unwind(),
    )
    .await
    {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err("multi observation panicked"),
        Err(_) => Err("multi watchdog expired"),
    }
}

// setupも失敗assertより先にbarrier解放・既存Tx終端・closeを試す。
async fn multi_setup<T>(
    adapter: &Adapter,
    result: Result<Result<T, Failure>, &'static str>,
    active: &mut Option<Tx>,
    gates: &[&Gate],
) -> T {
    let error = match result {
        Ok(Ok(value)) => return value,
        Ok(Err(error)) => format!("{error:?}"),
        Err(error) => error.to_owned(),
    };
    let mut release_failed = false;
    for gate in gates {
        release_failed |=
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gate.release())).is_err();
    }
    let finished = match active.take() {
        Some(tx) => Some(multi_watch(tx.finish(Finish::Rollback)).await),
        None => None,
    };
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let stats = adapter.observer().snapshot();
    panic!(
        "multi setup failed: {error}; gate_release_failed={release_failed}; rollback={finished:?}; close={closed:?}; pending={}, starting={}",
        stats.pending_workers, stats.starting
    );
}

const DEADLINE: Duration = Duration::from_secs(2);
#[derive(Debug, PartialEq)]
struct Number(i64);
impl FromRow for Number {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(row: &Row<'_>, indices: &[usize]) -> rusqlite::Result<Self> {
        assert!(!super::management_active());
        Ok(Self(row.get(indices[0])?))
    }
}

#[tokio::test]
async fn tokio_adapter_adapter_reuses_same_native_worker_and_sql_session_core() {
    let adapter = Adapter::new(
        Config {
            seed: "CREATE TABLE items(n)",
            ..Config::default()
        },
        AdapterSeams::default(),
    );
    let first = adapter.begin().await.unwrap();
    assert!(first.exec("COMMIT", vec![]).await.is_err());
    assert_eq!(
        first
            .exec("INSERT INTO items VALUES (?)", vec![Value::Integer(7)])
            .await
            .unwrap(),
        1
    );
    first.finish(Finish::Commit).await.unwrap();
    let second = adapter.begin().await.unwrap();
    assert_eq!(
        second
            .query::<Number>("SELECT n FROM items", vec![])
            .await
            .unwrap(),
        Some(Number(7))
    );
    second.finish(Finish::Rollback).await.unwrap();
    assert_eq!(adapter.observer().snapshot().created, 1);
    adapter.close(DEADLINE).await.unwrap();
    let stats = adapter.observer().snapshot();
    assert_eq!(stats.native_closed, 1);
    assert_eq!(stats.joined, 1);
}

#[tokio::test]
async fn close_retires_idle_worker_while_semaphore_permit_is_reserved_for_unpolled_waiter() {
    let adapter = Adapter::new(Config::default(), AdapterSeams::default());
    let observer = adapter.observer();
    let held = multi_setup(
        &adapter,
        multi_watch(adapter.checkout_without_begin()).await,
        &mut None,
        &[],
    )
    .await;
    let mut waiting = Box::pin(adapter.begin());
    let first_poll = futures_util::poll!(&mut waiting);
    let waiting_pending = first_poll.is_pending();
    let early_cleanup = if let std::task::Poll::Ready(Ok(tx)) = first_poll {
        Some(multi_watch(tx.finish(Finish::Rollback)).await)
    } else {
        None
    };
    // Objectはidle queueへ戻る。公平semaphoreはpermitを既にPendingのwaiterへ
    // 割り当てるが、再pollしないのでObjectはqueueから取り出されていない。
    // permit予約済みでもidle handleはqueueに残るraceを固定する。
    drop(held);
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let after_close = observer.snapshot();
    // 異常なReadyを観測したfixtureでも完了Futureを再pollしない。
    let waited = if waiting_pending {
        Some(multi_watch(&mut waiting).await)
    } else {
        None
    };
    let (waiting_error, late_cleanup) = match waited {
        Some(Ok(Err(error))) => (Some(error), None),
        Some(Ok(Ok(tx))) => (None, Some(multi_watch(tx.finish(Finish::Rollback)).await)),
        Some(Err(_)) | None => (None, None),
    };
    // 元実装のCloseTimeoutやwatchdogでも、残るfutureとPoolをDropしてidle
    // handleの所有者を解放し、actual joinを観測してから失敗assertする。
    drop(waiting);
    drop(adapter);
    let joined = multi_watch(observer.wait_joined(1)).await;
    let done = observer.snapshot();
    assert!(
        joined.is_ok(),
        "native cleanup did not join after Pool Drop"
    );
    assert!(waiting_pending && early_cleanup.is_none());
    assert!(late_cleanup.is_none());
    assert_eq!(
        waiting_error
            .expect("closed waiter, not watchdog/success")
            .kind,
        Kind::Closed
    );
    assert!(
        matches!(&closed, Ok(Ok(()))),
        "close must retire the idle worker despite a reserved permit: {closed:?}"
    );
    assert_eq!(after_close.native_closed, 1);
    assert_eq!(after_close.joined, 1);
    assert_eq!(after_close.pending_workers, 0);
    assert_eq!(done.created, 1);
    assert_eq!(done.native_started, 1);
    assert_eq!(done.native_closed, 1);
    assert_eq!(done.joined, 1);
    assert_eq!(done.start_failed, 0);
    assert_eq!(done.starting, 0);
    assert_eq!(done.pending_workers, 0);
}

#[tokio::test]
async fn cancelled_tx_keeps_tokio_adapter_checkout_until_native_cleanup_completes() {
    let gate = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config {
            seed: "CREATE TABLE items(n)",
            cleanup_gate: Some(Arc::clone(&gate)),
            ..Config::default()
        },
        AdapterSeams::default(),
    );
    let tx = adapter.begin().await.unwrap();
    tx.exec("INSERT INTO items VALUES (1)", vec![])
        .await
        .unwrap();
    drop(tx.finish(Finish::Commit));
    gate.wait().await;
    assert_eq!(adapter.available(), 0);
    assert_eq!(adapter.observer().snapshot().returned, 0);
    let mut next = Box::pin(adapter.begin());
    assert!(futures_util::poll!(&mut next).is_pending());
    assert_eq!(adapter.observer().snapshot().admitted, 1);
    gate.release();
    let tx = next.await.unwrap();
    assert_eq!(
        tx.query::<Number>("SELECT count(*) AS n FROM items", vec![])
            .await
            .unwrap(),
        Some(Number(0))
    );
    tx.finish(Finish::Rollback).await.unwrap();
    adapter.close(DEADLINE).await.unwrap();
}

#[tokio::test]
async fn cancelled_get_during_create_preserves_startup_close_and_join_owner() {
    let gate = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            startup: Some(Arc::clone(&gate)),
            ..AdapterSeams::default()
        },
    );
    let observer = adapter.observer();
    let mut get = Box::pin(adapter.begin());
    tokio::select! { _ = &mut get => panic!("get passed startup barrier"), _ = gate.wait() => {} }
    assert_eq!(observer.snapshot().starting, 1);
    drop(get);
    assert_eq!(observer.snapshot().starting, 0);
    assert_eq!(
        adapter.close(Duration::ZERO).await.unwrap_err().kind,
        Kind::CloseTimeout
    );
    assert_eq!(observer.snapshot().joined, 0);
    gate.release();
    adapter.close(DEADLINE).await.unwrap();
    assert_eq!(observer.snapshot().created, 1);
    assert_eq!(observer.snapshot().native_closed, 1);
    assert_eq!(observer.snapshot().joined, 1);
    assert_eq!(observer.snapshot().admitted, 0);
}

#[tokio::test]
async fn close_waits_for_registered_create_and_late_create_cannot_begin() {
    let gate = Arc::new(AdmissionGate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            created: Some(Arc::clone(&gate)),
            ..AdapterSeams::default()
        },
    );
    let observer = adapter.observer();
    let mut get = Box::pin(adapter.begin());
    tokio::select! { _ = &mut get => panic!("get passed create barrier"), _ = gate.wait() => {} }
    assert_eq!(observer.snapshot().starting, 1);
    assert_eq!(
        adapter.close(Duration::ZERO).await.unwrap_err().kind,
        Kind::CloseTimeout
    );
    gate.release();
    assert_eq!(get.await.unwrap_err().kind, Kind::Closed);
    adapter.close(DEADLINE).await.unwrap();
    assert_eq!(observer.snapshot().admitted, 0);
    assert_eq!(observer.snapshot().joined, 1);
    assert_eq!(observer.snapshot().created, 1);
}

#[tokio::test]
async fn acquired_tokio_adapter_object_after_close_is_rejected_before_user_begin() {
    let gate = Arc::new(AdmissionGate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            checkout: Some(Arc::clone(&gate)),
            ..AdapterSeams::default()
        },
    );
    let mut get = Box::pin(adapter.begin());
    tokio::select! { _ = &mut get => panic!("get passed checkout barrier"), _ = gate.wait() => {} }
    assert_eq!(
        adapter.close(Duration::ZERO).await.unwrap_err().kind,
        Kind::CloseTimeout
    );
    gate.release();
    assert_eq!(get.await.unwrap_err().kind, Kind::Closed);
    adapter.close(DEADLINE).await.unwrap();
    assert_eq!(adapter.observer().snapshot().admitted, 0);
}

#[tokio::test]
async fn close_clone_and_cancel_keep_closing_while_active_checkout_can_finish() {
    let gate = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config {
            exit_gate: Some(Arc::clone(&gate)),
            ..Config::default()
        },
        AdapterSeams::default(),
    );
    let clone = adapter.clone_handle();
    let observer = adapter.observer();
    let tx = adapter.begin().await.unwrap();
    let mut close = Box::pin(adapter.close(DEADLINE));
    assert!(futures_util::poll!(&mut close).is_pending());
    drop(close);
    assert!(observer.snapshot().closing);
    assert_eq!(clone.begin().await.unwrap_err().kind, Kind::Closed);
    tx.finish(Finish::Rollback).await.unwrap();
    gate.wait().await;
    assert_eq!(observer.snapshot().native_closed, 1);
    assert_eq!(observer.snapshot().joined, 0);
    assert_eq!(
        clone.close(Duration::ZERO).await.unwrap_err().kind,
        Kind::CloseTimeout
    );
    gate.release();
    clone.close(DEADLINE).await.unwrap();
    adapter.close(Duration::ZERO).await.unwrap();
}

#[tokio::test]
async fn native_cleanup_failure_closes_tokio_adapter_and_never_creates_replacement() {
    let adapter = Adapter::new(
        Config {
            deny_rollback: true,
            ..Config::default()
        },
        AdapterSeams::default(),
    );
    let tx = adapter.begin().await.unwrap();
    let mut waiting = Box::pin(adapter.begin());
    assert!(futures_util::poll!(&mut waiting).is_pending());
    let error = tx.finish(Finish::Rollback).await.unwrap_err();
    assert!(error.retired && error.cleanup.is_some());
    assert_eq!(error.outcome, Outcome::Unknown);
    assert_eq!(waiting.await.unwrap_err().kind, Kind::Worker);
    assert_eq!(adapter.begin().await.unwrap_err().kind, Kind::Worker);
    assert!(adapter.close(DEADLINE).await.is_err());
    assert_eq!(adapter.observer().snapshot().created, 1);
    assert_eq!(adapter.observer().snapshot().joined, 1);
}

#[tokio::test]
async fn tokio_adapter_recycle_error_stops_create_instead_of_default_replacement() {
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            fail_recycle: true,
            ..AdapterSeams::default()
        },
    );
    let tx = adapter.begin().await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    assert_eq!(adapter.begin().await.unwrap_err().kind, Kind::Worker);
    assert!(adapter.close(DEADLINE).await.is_err());
    assert_eq!(adapter.observer().snapshot().created, 1);
}

#[tokio::test]
async fn resize_zero_uses_worker_drop_even_when_manager_detach_is_bypassed() {
    let adapter = Adapter::new(Config::default(), AdapterSeams::default());
    let tx = adapter.begin().await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    let observer = adapter.observer();
    observer.wait_returned(1).await;
    adapter.resize_zero();
    observer.wait_joined(1).await;
    assert_eq!(observer.snapshot().detached, 0);
    assert_eq!(observer.snapshot().handle_drops, 1);
    assert_eq!(observer.snapshot().native_closed, 1);
    adapter.close(DEADLINE).await.unwrap();
}

#[tokio::test]
async fn object_take_and_last_pool_drop_both_preserve_native_close_join_observation() {
    for take in [false, true] {
        let adapter = Adapter::new(Config::default(), AdapterSeams::default());
        let observer = adapter.observer();
        let checkout = adapter.checkout_without_begin().await.unwrap();
        if take {
            checkout.take_and_drop();
        } else {
            drop(adapter);
            drop(checkout);
        }
        observer.wait_joined(1).await;
        assert_eq!(observer.snapshot().native_closed, 1);
        assert_eq!(observer.snapshot().handle_drops, 1);
        assert_eq!(observer.snapshot().detached, usize::from(take));
    }
}

#[tokio::test]
async fn last_adapter_drop_closes_idle_worker_without_observer_owning_pool_or_tx_sender() {
    let adapter = Adapter::new(Config::default(), AdapterSeams::default());
    let observer = adapter.observer();
    let tx = adapter.begin().await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    observer.wait_returned(1).await;
    drop(adapter);
    observer.wait_joined(1).await;
    assert_eq!(observer.snapshot().native_closed, 1);
    assert!(observer.snapshot().closing);
}

#[tokio::test]
async fn native_close_failure_is_kept_after_tokio_adapter_objects_and_workers_are_gone() {
    let adapter = Adapter::new(
        Config {
            leak_statement_at_close: true,
            ..Config::default()
        },
        AdapterSeams::default(),
    );
    let tx = adapter.begin().await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    let error = adapter.close(DEADLINE).await.unwrap_err();
    assert!(error.cleanup.is_some());
    assert_eq!(adapter.observer().snapshot().joined, 1);
    assert_eq!(adapter.observer().snapshot().native_closed, 0);
}

#[tokio::test]
async fn last_adapter_drop_preserves_active_tx_until_its_explicit_finish() {
    let adapter = Adapter::new(Config::default(), AdapterSeams::default());
    let observer = adapter.observer();
    let tx = adapter.begin().await.unwrap();
    drop(adapter);
    assert!(observer.snapshot().closing);
    assert_eq!(observer.snapshot().native_closed, 0);
    assert_eq!(observer.snapshot().joined, 0);
    assert_eq!(
        tx.query::<Number>("SELECT 5 AS n", vec![]).await.unwrap(),
        Some(Number(5))
    );
    tx.finish(Finish::Commit).await.unwrap();
    observer.wait_joined(1).await;
    assert_eq!(observer.snapshot().native_closed, 1);
}

#[tokio::test]
async fn cancelled_begin_reply_keeps_checkout_until_eof_rollback_is_observed() {
    let begin_gate = Arc::new(Gate::new());
    let cleanup_gate = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config {
            begin_gate: Some(Arc::clone(&begin_gate)),
            cleanup_gate: Some(Arc::clone(&cleanup_gate)),
            ..Config::default()
        },
        AdapterSeams::default(),
    );
    let mut begin = Box::pin(adapter.begin());
    tokio::select! { _ = &mut begin => panic!("begin passed reply barrier"), _ = begin_gate.wait() => {} }
    drop(begin);
    begin_gate.release();
    cleanup_gate.wait().await;
    assert_eq!(adapter.available(), 0);
    assert_eq!(adapter.observer().snapshot().returned, 0);
    let mut next = Box::pin(adapter.begin());
    assert!(futures_util::poll!(&mut next).is_pending());
    cleanup_gate.release();
    let tx = next.await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    adapter.close(DEADLINE).await.unwrap();
    assert_eq!(adapter.observer().snapshot().created, 1);
}

#[tokio::test]
async fn callback_panic_retires_pool_and_join_does_not_hide_native_close_failure() {
    struct PanicRow;
    impl FromRow for PanicRow {
        fn columns() -> &'static [&'static str] {
            &["n"]
        }
        fn read(_: &Row<'_>, _: &[usize]) -> rusqlite::Result<Self> {
            assert!(!super::management_active());
            panic!("adapter callback panic seam")
        }
    }
    for fail_close in [false, true] {
        let adapter = Adapter::new(
            Config {
                leak_statement_at_close: fail_close,
                ..Config::default()
            },
            AdapterSeams::default(),
        );
        let tx = adapter.begin().await.unwrap();
        let mut waiting = Box::pin(adapter.begin());
        assert!(futures_util::poll!(&mut waiting).is_pending());
        assert!(tx.query::<PanicRow>("SELECT 1 AS n", vec![]).await.is_err());
        drop(tx);
        assert_eq!(waiting.await.unwrap_err().kind, Kind::Worker);
        assert_eq!(adapter.begin().await.unwrap_err().kind, Kind::Worker);
        let error = adapter.close(DEADLINE).await.unwrap_err();
        assert!(error.primary.is_some());
        if fail_close {
            assert!(error.cleanup.is_some());
        }
        let stats = adapter.observer().snapshot();
        assert_eq!(stats.created, 1);
        assert_eq!(stats.joined, 1);
        assert_eq!(stats.native_closed, usize::from(!fail_close));
    }
}

#[tokio::test]
async fn joined_is_published_only_after_terminal_native_failure_is_in_ledger() {
    let gate = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config {
            leak_statement_at_close: true,
            ..Config::default()
        },
        AdapterSeams {
            publication: Some(Arc::clone(&gate)),
            ..AdapterSeams::default()
        },
    );
    let tx = adapter.begin().await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    let mut closing = Box::pin(adapter.close(DEADLINE));
    tokio::select! { _ = &mut closing => panic!("close passed publication barrier"), _ = gate.wait() => {} }
    let observed = std::panic::AssertUnwindSafe(adapter.close(Duration::ZERO))
        .catch_unwind()
        .await;
    let prematurely_joined = adapter.observer().snapshot().joined;
    gate.release();
    drop(closing);
    assert!(
        observed.is_ok(),
        "close panicked before native failure publication"
    );
    assert_eq!(observed.unwrap().unwrap_err().kind, Kind::CloseTimeout);
    assert_eq!(prematurely_joined, 0);
    let error = adapter.close(DEADLINE).await.unwrap_err();
    assert!(error.cleanup.is_some());
    assert_eq!(adapter.observer().snapshot().joined, 1);
}

#[test]
fn tokio_runtime_drop_cannot_return_native_owned_checkout_before_cleanup() {
    let gate = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config {
            cleanup_gate: Some(Arc::clone(&gate)),
            ..Config::default()
        },
        AdapterSeams::default(),
    );
    let observer = adapter.observer();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let tx = runtime.block_on(adapter.begin()).unwrap();
    drop(tx.finish(Finish::Commit));
    runtime.block_on(gate.wait());
    assert_eq!(observer.snapshot().returned, 0);
    drop(runtime);
    assert_eq!(observer.snapshot().returned, 0);
    assert_eq!(observer.snapshot().joined, 0);
    gate.release();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(adapter.close(DEADLINE)).unwrap();
    assert_eq!(observer.snapshot().returned, 1);
    assert_eq!(observer.snapshot().native_closed, 1);
    assert_eq!(observer.snapshot().joined, 1);
}

#[tokio::test]
async fn cancelled_create_cannot_start_replacement_before_old_native_worker_joins() {
    let startup = Arc::new(Gate::new());
    let created = Arc::new(AdmissionGate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            startup: Some(Arc::clone(&startup)),
            created: Some(Arc::clone(&created)),
            ..AdapterSeams::default()
        },
    );
    let observer = adapter.observer();
    let mut first = Box::pin(adapter.begin());
    tokio::select! { _ = &mut first => panic!("first get passed startup barrier"), _ = startup.wait() => {} }
    drop(first);
    let mut second = Box::pin(adapter.begin());
    assert!(futures_util::poll!(&mut second).is_pending());
    let before_old_join = observer.snapshot();
    startup.release();
    created.release();
    let tx = tokio::time::timeout(Duration::from_secs(10), second)
        .await
        .expect("replacement get watchdog expired")
        .unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    adapter.close(DEADLINE).await.unwrap();
    assert_eq!(
        before_old_join.created, 1,
        "max1 started another native worker while cancelled startup still owned one"
    );
    assert_eq!(before_old_join.admitted, 0);
    assert_eq!(before_old_join.joined, 0);
    assert_eq!(before_old_join.pending_workers, 1);
    assert_eq!(observer.snapshot().created, 2);
    assert_eq!(observer.snapshot().joined, 2);
    assert_eq!(observer.snapshot().native_closed, 2);
    assert_eq!(observer.snapshot().pending_workers, 0);
}

#[tokio::test]
async fn taken_object_cannot_start_replacement_before_old_native_worker_joins() {
    let exit = Arc::new(Gate::new());
    let created = Arc::new(AdmissionGate::new());
    created.release();
    let adapter = Adapter::new(
        Config {
            exit_gate: Some(Arc::clone(&exit)),
            ..Config::default()
        },
        AdapterSeams {
            created: Some(Arc::clone(&created)),
            ..AdapterSeams::default()
        },
    );
    let observer = adapter.observer();
    adapter
        .checkout_without_begin()
        .await
        .unwrap()
        .take_and_drop();
    exit.wait().await;
    let mut second = Box::pin(adapter.begin());
    let early = futures_util::poll!(&mut second);
    let before_old_join = observer.snapshot();
    exit.release();
    let tx = match early {
        std::task::Poll::Ready(result) => result.unwrap(),
        std::task::Poll::Pending => tokio::time::timeout(Duration::from_secs(10), second)
            .await
            .expect("replacement get watchdog expired")
            .unwrap(),
    };
    tx.finish(Finish::Rollback).await.unwrap();
    adapter.close(DEADLINE).await.unwrap();
    assert_eq!(
        before_old_join.created, 1,
        "Object::take returned a logical slot before old native join completed"
    );
    assert_eq!(before_old_join.joined, 0);
    assert_eq!(before_old_join.native_closed, 1);
    assert_eq!(before_old_join.pending_workers, 1);
    assert_eq!(observer.snapshot().created, 2);
    assert_eq!(observer.snapshot().joined, 2);
    assert_eq!(observer.snapshot().native_closed, 2);
    assert_eq!(observer.snapshot().pending_workers, 0);
}

#[tokio::test]
async fn checkout_detach_permit_before_handle_drop_cannot_start_another_native_worker() {
    let detaching = Arc::new(Gate::new());
    let adapter = Adapter::new(
        Config::default(),
        AdapterSeams {
            detaching: Some(Arc::clone(&detaching)),
            ..AdapterSeams::default()
        },
    );
    let observer = adapter.observer();
    let object = adapter.checkout_without_begin().await.unwrap();
    let taking = std::thread::spawn(move || object.take_and_drop());
    detaching.wait().await; // Tokio semaphore permitは返却済み、WorkerHandleは生存。
    let mut next = Box::pin(adapter.begin());
    let early = futures_util::poll!(&mut next);
    let before_handle_drop = observer.snapshot();
    detaching.release();
    taking.join().unwrap();
    let tx = match early {
        std::task::Poll::Ready(result) => result.unwrap(),
        std::task::Poll::Pending => tokio::time::timeout(Duration::from_secs(10), next)
            .await
            .expect("detached replacement watchdog expired")
            .unwrap(),
    };
    tx.finish(Finish::Rollback).await.unwrap();
    adapter.close(DEADLINE).await.unwrap();
    assert_eq!(before_handle_drop.handle_drops, 0);
    assert_eq!(before_handle_drop.joined, 0);
    assert_eq!(before_handle_drop.pending_workers, 1);
    assert_eq!(
        before_handle_drop.created, 1,
        "semaphore permit preceded handle retirement"
    );
    assert_eq!(observer.snapshot().created, 2);
    assert_eq!(observer.snapshot().native_closed, 2);
    assert_eq!(observer.snapshot().joined, 2);
    assert_eq!(observer.snapshot().pending_workers, 0);
}

#[tokio::test]
async fn multi_two_filesystem_connections_begin_while_first_tx_stays_active() {
    let mut file = MultiFile::new();
    let adapter = Adapter::with_capacity(file.config(), AdapterSeams::default(), 2);
    file.observe(&adapter);
    let observer = adapter.observer();
    let a = multi_setup(&adapter, multi_watch(adapter.begin()).await, &mut None, &[]).await;
    let b = multi_watch(adapter.begin()).await;
    let b_work = match b {
        Ok(Ok(b)) => {
            let inserted = multi_watch(b.exec("INSERT INTO items VALUES (7)", vec![])).await;
            let operation = if matches!(inserted, Ok(Ok(1))) {
                Finish::Commit
            } else {
                Finish::Rollback
            };
            let finished = multi_watch(b.finish(operation)).await;
            matches!(inserted, Ok(Ok(1))) && matches!(finished, Ok(Ok(())))
        }
        _ => false,
    };
    let both_live = observer.snapshot();
    let shared_row = multi_watch(a.query::<Number>("SELECT n FROM items", vec![])).await;
    let a_finished = multi_watch(a.finish(Finish::Rollback)).await;
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let done = observer.snapshot();
    assert!(
        b_work,
        "second connection was blocked or did not share filesystem DB"
    );
    assert!(matches!(shared_row, Ok(Ok(Some(Number(7))))));
    assert!(matches!(a_finished, Ok(Ok(()))));
    assert!(matches!(closed, Ok(Ok(()))));
    assert_eq!(both_live.created, 2);
    assert_eq!(both_live.native_started, 2);
    assert_eq!(both_live.pending_workers, 2);
    assert_eq!(both_live.joined, 0);
    assert_eq!(done.native_closed, 2);
    assert_eq!(done.joined, 2);
    assert_eq!(done.start_failed, 0);
    assert_eq!(done.pending_workers, 0);
}

#[tokio::test]
async fn multi_cancelled_b_joins_and_c_begins_before_healthy_a_finishes() {
    let mut file = MultiFile::new();
    let startup = Arc::new(Gate::new());
    let publication = Arc::new(Gate::new());
    let adapter = Adapter::with_capacity(
        file.config(),
        AdapterSeams {
            target_worker: Some(2),
            startup: Some(Arc::clone(&startup)),
            publication: Some(Arc::clone(&publication)),
            ..AdapterSeams::default()
        },
        2,
    );
    file.observe(&adapter);
    let observer = adapter.observer();
    let a = multi_setup(
        &adapter,
        multi_watch(adapter.begin()).await,
        &mut None,
        &[&startup, &publication],
    )
    .await;
    let mut b = Box::pin(adapter.begin());
    let started = multi_watch(async {
        tokio::select! { _ = &mut b => panic!("B passed startup barrier"), _ = startup.wait() => {} }
    })
    .await;
    drop(b);
    let mut c = Box::pin(adapter.begin());
    let c_before_join = futures_util::poll!(&mut c);
    let c_waited = c_before_join.is_pending();
    let before_b_join = observer.snapshot();
    startup.release();
    let independently_joined = if started.is_ok() {
        multi_watch(publication.wait()).await
    } else {
        Err("B startup failed")
    };
    let c_at_publication = match c_before_join {
        std::task::Poll::Ready(result) => std::task::Poll::Ready(result),
        std::task::Poll::Pending if independently_joined.is_ok() => futures_util::poll!(&mut c),
        std::task::Poll::Pending => std::task::Poll::Pending,
    };
    let c_kept_pending = c_at_publication.is_pending();
    let at_b_join = observer.snapshot();
    publication.release();
    let c_result = match c_at_publication {
        std::task::Poll::Ready(result) => Ok(result),
        std::task::Poll::Pending if independently_joined.is_ok() => multi_watch(c).await,
        std::task::Poll::Pending => {
            drop(c);
            Err("B startup failed")
        }
    };
    let c_finished = match c_result {
        Ok(Ok(c)) => matches!(multi_watch(c.finish(Finish::Rollback)).await, Ok(Ok(()))),
        _ => false,
    };
    let a_healthy = multi_watch(a.query::<Number>("SELECT 1 AS n", vec![])).await;
    let a_finished = multi_watch(a.finish(Finish::Rollback)).await;
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let done = observer.snapshot();
    assert!(started.is_ok());
    assert!(
        independently_joined.is_ok(),
        "B join was queued behind active A"
    );
    assert!(c_waited && c_kept_pending && c_finished);
    assert_eq!(before_b_join.created, 2);
    assert_eq!(before_b_join.pending_workers, 2);
    assert_eq!(at_b_join.created, 2);
    assert_eq!(at_b_join.pending_workers, 2);
    assert_eq!(at_b_join.native_closed, 1);
    assert_eq!(
        at_b_join.joined, 0,
        "publication barrier precedes completion ledger"
    );
    assert!(matches!(a_healthy, Ok(Ok(Some(Number(1))))));
    assert!(matches!(a_finished, Ok(Ok(()))));
    assert!(matches!(closed, Ok(Ok(()))));
    assert_eq!(done.created, 3);
    assert_eq!(done.native_started, 3);
    assert_eq!(done.joined, 3);
    assert_eq!(done.native_closed, 3);
    assert_eq!(done.start_failed, 0);
    assert_eq!(done.pending_workers, 0);
}

#[tokio::test]
async fn multi_checkout_detach_permit_gap_respects_native_cap_with_a_active() {
    let mut file = MultiFile::new();
    let detaching = Arc::new(Gate::new());
    let publication = Arc::new(Gate::new());
    let adapter = Adapter::with_capacity(
        file.config(),
        AdapterSeams {
            target_worker: Some(2),
            detaching: Some(Arc::clone(&detaching)),
            publication: Some(Arc::clone(&publication)),
            ..AdapterSeams::default()
        },
        2,
    );
    file.observe(&adapter);
    let observer = adapter.observer();
    let a = multi_setup(
        &adapter,
        multi_watch(adapter.begin()).await,
        &mut None,
        &[&detaching, &publication],
    )
    .await;
    let mut active = Some(a);
    let b = multi_setup(
        &adapter,
        multi_watch(adapter.checkout_without_begin()).await,
        &mut active,
        &[&detaching, &publication],
    )
    .await;
    let a = active.take().unwrap();
    let taking = std::thread::spawn(move || b.take_and_drop());
    let detached = multi_watch(detaching.wait()).await;
    let mut c = Box::pin(adapter.begin());
    let c_before_join = futures_util::poll!(&mut c);
    let c_waited = c_before_join.is_pending();
    let permit_gap = observer.snapshot();
    detaching.release();
    let take_finished = taking.join();
    let b_joined = multi_watch(publication.wait()).await;
    let c_at_publication = match c_before_join {
        std::task::Poll::Ready(result) => std::task::Poll::Ready(result),
        std::task::Poll::Pending if b_joined.is_ok() => futures_util::poll!(&mut c),
        std::task::Poll::Pending => std::task::Poll::Pending,
    };
    let c_kept_pending = c_at_publication.is_pending();
    let before_publication = observer.snapshot();
    publication.release();
    let c_result = match c_at_publication {
        std::task::Poll::Ready(result) => Ok(result),
        std::task::Poll::Pending if b_joined.is_ok() => multi_watch(c).await,
        std::task::Poll::Pending => {
            drop(c);
            Err("B join failed")
        }
    };
    let c_finished = match c_result {
        Ok(Ok(c)) => matches!(multi_watch(c.finish(Finish::Rollback)).await, Ok(Ok(()))),
        _ => false,
    };
    let a_healthy = multi_watch(a.query::<Number>("SELECT 1 AS n", vec![])).await;
    let a_finished = multi_watch(a.finish(Finish::Rollback)).await;
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let done = observer.snapshot();
    assert!(detached.is_ok() && take_finished.is_ok());
    assert!(b_joined.is_ok(), "detached B join waited for healthy A");
    assert!(c_waited && c_kept_pending && c_finished);
    assert_eq!(permit_gap.handle_drops, 0);
    assert_eq!(permit_gap.native_started, 2);
    assert_eq!(permit_gap.created, 2);
    assert_eq!(permit_gap.pending_workers, 2);
    assert_eq!(before_publication.created, 2);
    assert_eq!(before_publication.pending_workers, 2);
    assert_eq!(before_publication.joined, 0);
    assert_eq!(before_publication.native_closed, 1);
    assert!(matches!(a_healthy, Ok(Ok(Some(Number(1))))));
    assert!(matches!(a_finished, Ok(Ok(()))));
    assert!(matches!(closed, Ok(Ok(()))));
    assert_eq!(done.created, 3);
    assert_eq!(done.joined, 3);
    assert_eq!(done.native_closed, 3);
    assert_eq!(done.start_failed, 0);
    assert_eq!(done.pending_workers, 0);
}

#[tokio::test]
async fn multi_b_terminal_failure_is_published_without_waiting_for_a() {
    let mut file = MultiFile::new();
    let publication = Arc::new(Gate::new());
    let adapter = Adapter::with_capacity(
        file.config(),
        AdapterSeams {
            target_worker: Some(2),
            // 実SQLite closeは完了させ、observerがterminal causeだけ注入する。
            // filesystemへ永久Statement leakを残す実close Err試験ではない。
            fail_terminal_result: true,
            publication: Some(Arc::clone(&publication)),
            ..AdapterSeams::default()
        },
        2,
    );
    file.observe(&adapter);
    let observer = adapter.observer();
    let a = multi_setup(
        &adapter,
        multi_watch(adapter.begin()).await,
        &mut None,
        &[&publication],
    )
    .await;
    let mut active = Some(a);
    let b = multi_setup(
        &adapter,
        multi_watch(adapter.checkout_without_begin()).await,
        &mut active,
        &[&publication],
    )
    .await;
    let a = active.take().unwrap();
    b.take_and_drop();
    let independent = multi_watch(publication.wait()).await;
    let mut waiting = Box::pin(adapter.begin());
    let waiting_before_failure = futures_util::poll!(&mut waiting).is_pending();
    publication.release();
    let failure = if independent.is_ok() {
        multi_watch(waiting).await
    } else {
        drop(waiting);
        Err("B terminal failure not observed independently")
    };
    let b_joined = if independent.is_ok() {
        multi_watch(observer.wait_joined(1)).await
    } else {
        Err("B join not observed independently")
    };
    let while_a_active = observer.snapshot();
    let a_healthy = multi_watch(a.query::<Number>("SELECT 1 AS n", vec![])).await;
    let a_finished = multi_watch(a.finish(Finish::Rollback)).await;
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let done = observer.snapshot();
    assert!(
        independent.is_ok() && b_joined.is_ok(),
        "B failure was hidden behind A"
    );
    assert!(waiting_before_failure);
    let error = failure.unwrap().unwrap_err();
    assert_eq!(error.kind, Kind::Worker);
    assert!(error.cleanup.is_some() && error.retired);
    assert_eq!(while_a_active.joined, 1);
    assert_eq!(while_a_active.created, 2, "failure started replacement");
    assert!(while_a_active.closing);
    assert!(matches!(a_healthy, Ok(Ok(Some(Number(1))))));
    assert!(matches!(a_finished, Ok(Ok(()))));
    let close_error = closed.unwrap().unwrap_err();
    assert!(close_error.cleanup.is_some());
    assert_eq!(done.created, 2);
    assert_eq!(done.native_started, 2);
    assert_eq!(done.joined, 2);
    assert_eq!(
        done.native_closed, 2,
        "real native close succeeded for A and B"
    );
    assert_eq!(done.start_failed, 0);
    assert_eq!(done.pending_workers, 0);
}

#[tokio::test]
async fn multi_close_during_b_startup_preserves_a_and_all_join_responsibility() {
    let mut file = MultiFile::new();
    let startup = Arc::new(Gate::new());
    let adapter = Adapter::with_capacity(
        file.config(),
        AdapterSeams {
            target_worker: Some(2),
            startup: Some(Arc::clone(&startup)),
            ..AdapterSeams::default()
        },
        2,
    );
    file.observe(&adapter);
    let observer = adapter.observer();
    let a = multi_setup(
        &adapter,
        multi_watch(adapter.begin()).await,
        &mut None,
        &[&startup],
    )
    .await;
    let mut b = Box::pin(adapter.begin());
    let started = multi_watch(async {
        tokio::select! { _ = &mut b => panic!("B passed startup barrier"), _ = startup.wait() => {} }
    })
    .await;
    let mut waiting = Box::pin(adapter.begin());
    let waiting_pending = futures_util::poll!(&mut waiting).is_pending();
    let mut closing = Box::pin(adapter.close(Duration::from_secs(10)));
    let close_pending = futures_util::poll!(&mut closing).is_pending();
    drop(closing);
    let waiting_error = multi_watch(waiting).await;
    let closing_kept = observer.snapshot().closing;
    startup.release();
    let late_b = multi_watch(b).await;
    let b_joined = multi_watch(observer.wait_joined(1)).await;
    let before_a_finish = observer.snapshot();
    let close_timeout = adapter.close(Duration::ZERO).await;
    let a_healthy = multi_watch(a.query::<Number>("SELECT 1 AS n", vec![])).await;
    let a_finished = multi_watch(a.finish(Finish::Rollback)).await;
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let done = observer.snapshot();
    assert!(started.is_ok() && b_joined.is_ok());
    assert!(waiting_pending && close_pending && closing_kept);
    assert_eq!(waiting_error.unwrap().unwrap_err().kind, Kind::Closed);
    assert_eq!(late_b.unwrap().unwrap_err().kind, Kind::Closed);
    assert_eq!(close_timeout.unwrap_err().kind, Kind::CloseTimeout);
    assert_eq!(before_a_finish.joined, 1);
    assert_eq!(
        before_a_finish.admitted, 1,
        "late startup admitted user BEGIN"
    );
    assert!(matches!(a_healthy, Ok(Ok(Some(Number(1))))));
    assert!(matches!(a_finished, Ok(Ok(()))));
    assert!(matches!(closed, Ok(Ok(()))));
    assert_eq!(done.created, 2);
    assert_eq!(done.native_started, 2);
    assert_eq!(done.joined, 2);
    assert_eq!(done.native_closed, 2);
    assert_eq!(done.start_failed, 0);
    assert_eq!(done.pending_workers, 0);
}

#[tokio::test]
async fn multi_observer_spawn_failure_never_starts_or_fake_joins_native_b() {
    let mut file = MultiFile::new();
    let adapter = Adapter::with_capacity(
        file.config(),
        AdapterSeams {
            target_worker: Some(2),
            fail_observer_spawn: true,
            ..AdapterSeams::default()
        },
        2,
    );
    file.observe(&adapter);
    let observer = adapter.observer();
    let a = multi_setup(&adapter, multi_watch(adapter.begin()).await, &mut None, &[]).await;
    let failed_b = multi_watch(adapter.begin()).await;
    let after_failure = observer.snapshot();
    let later = multi_watch(adapter.begin()).await;
    let a_healthy = multi_watch(a.query::<Number>("SELECT 1 AS n", vec![])).await;
    let a_finished = multi_watch(a.finish(Finish::Rollback)).await;
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let done = observer.snapshot();
    let error = failed_b.unwrap().unwrap_err();
    assert!(error.cleanup.is_some() && error.retired);
    assert_eq!(error.outcome, Outcome::NotApplicable);
    assert_eq!(after_failure.created, 2);
    assert_eq!(after_failure.native_started, 1);
    assert_eq!(after_failure.start_failed, 1);
    assert_eq!(after_failure.joined, 0, "native B was never spawned");
    assert_eq!(after_failure.native_closed, 0);
    assert_eq!(after_failure.pending_workers, 1);
    assert_eq!(after_failure.starting, 0);
    assert!(after_failure.closing);
    assert_eq!(later.unwrap().unwrap_err().kind, Kind::Worker);
    assert!(matches!(a_healthy, Ok(Ok(Some(Number(1))))));
    assert!(matches!(a_finished, Ok(Ok(()))));
    assert!(closed.unwrap().unwrap_err().cleanup.is_some());
    assert_eq!(done.created, 2);
    assert_eq!(done.native_started, 1);
    assert_eq!(done.joined, 1);
    assert_eq!(done.native_closed, 1);
    assert_eq!(done.start_failed, 1);
    assert_eq!(done.joined + done.start_failed, done.created);
    assert_eq!(done.pending_workers, 0);
}

#[tokio::test]
async fn multi_native_spawn_failure_keeps_cause_without_native_b_or_fake_join() {
    let mut file = MultiFile::new();
    let adapter = Adapter::with_capacity(
        file.config(),
        AdapterSeams {
            target_worker: Some(2),
            fail_native_spawn: true,
            ..AdapterSeams::default()
        },
        2,
    );
    file.observe(&adapter);
    let observer = adapter.observer();
    let a = multi_setup(&adapter, multi_watch(adapter.begin()).await, &mut None, &[]).await;
    let failed_b = multi_watch(adapter.begin()).await;
    let after_failure = observer.snapshot();
    let later = multi_watch(adapter.begin()).await;
    let a_healthy = multi_watch(a.query::<Number>("SELECT 1 AS n", vec![])).await;
    let a_finished = multi_watch(a.finish(Finish::Rollback)).await;
    let closed = multi_watch(adapter.close(DEADLINE)).await;
    let done = observer.snapshot();
    let error = failed_b.unwrap().unwrap_err();
    assert!(error.cleanup.is_some() && error.retired);
    assert_eq!(error.outcome, Outcome::NotApplicable);
    assert_eq!(after_failure.created, 2);
    assert_eq!(after_failure.native_started, 1);
    assert_eq!(after_failure.start_failed, 1);
    assert_eq!(after_failure.joined, 0, "native B was never spawned");
    assert_eq!(after_failure.native_closed, 0);
    assert_eq!(after_failure.pending_workers, 1);
    assert_eq!(after_failure.starting, 0);
    assert!(after_failure.closing);
    assert_eq!(later.unwrap().unwrap_err().kind, Kind::Worker);
    assert!(matches!(a_healthy, Ok(Ok(Some(Number(1))))));
    assert!(matches!(a_finished, Ok(Ok(()))));
    assert!(closed.unwrap().unwrap_err().cleanup.is_some());
    assert_eq!(done.created, 2);
    assert_eq!(done.native_started, 1);
    assert_eq!(done.joined, 1);
    assert_eq!(done.native_closed, 1);
    assert_eq!(done.start_failed, 1);
    assert_eq!(done.joined + done.start_failed, done.created);
    assert_eq!(done.pending_workers, 0);
}
