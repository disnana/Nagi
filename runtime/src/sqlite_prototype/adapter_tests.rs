// Q004のgeneric deadpool比較試作。公開Poolの完成テストではない。
use super::adapter::{Adapter, AdapterSeams, AdmissionGate};
use super::{Config, Finish, Gate, Kind, Outcome};
use crate::FromRow;
use rusqlite::{types::Value, Row};
use std::{sync::Arc, time::Duration};

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
async fn deadpool_adapter_reuses_same_native_worker_and_sql_session_core() {
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
async fn cancelled_tx_keeps_deadpool_checkout_until_native_cleanup_completes() {
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
async fn acquired_deadpool_object_after_close_is_rejected_before_user_begin() {
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
async fn native_cleanup_failure_closes_deadpool_and_never_creates_replacement() {
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
async fn deadpool_recycle_error_stops_create_instead_of_default_replacement() {
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
async fn native_close_failure_is_kept_after_deadpool_objects_and_workers_are_gone() {
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
