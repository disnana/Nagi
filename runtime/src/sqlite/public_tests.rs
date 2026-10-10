use super::*;
use crate::FromRow;
use rusqlite::Row;
use std::time::Duration;

#[derive(Debug, PartialEq)]
struct Payload {
    n: i64,
    text: String,
    bytes: Vec<u8>,
    nullable: Option<i64>,
    real: f64,
}
impl FromRow for Payload {
    fn columns() -> &'static [&'static str] {
        &["n", "text", "bytes", "nullable", "real"]
    }
    fn read(row: &Row<'_>, ix: &[usize]) -> rusqlite::Result<Self> {
        assert!(!session::management_active());
        Ok(Self {
            n: row.get(ix[0])?,
            text: row.get(ix[1])?,
            bytes: row.get(ix[2])?,
            nullable: row.get(ix[3])?,
            real: row.get(ix[4])?,
        })
    }
}
async fn memory() -> Pool {
    open(":memory:", options(1, 3, 0, 0).unwrap())
        .await
        .unwrap()
}

#[test]
fn options_enforce_native_ranges_without_reserving_slots() {
    for (connections, queue, acquire, busy) in [
        (0, 1, 0, 0),
        (-1, 1, 0, 0),
        (1, 0, 0, 0),
        (1, -1, 0, 0),
        (1, 1, -1, 0),
        (1, 1, 0, -1),
        (1, 1, 0, i64::from(i32::MAX) + 1),
        (i64::MAX, 1, 0, 0),
        (1, i64::MAX, 0, 0),
    ] {
        let error = options(connections, queue, acquire, busy).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Invalid));
    }
    assert!(options(2, 3, 0, i64::from(i32::MAX)).is_ok());
    // Options is scalar validation only: this does not construct a semaphore,
    // start native workers, or attempt capacity-sized resource allocation.
    let limit = i64::try_from(tokio::sync::Semaphore::MAX_PERMITS).unwrap();
    assert!(options(limit, limit, 0, 0).is_ok());
    // Native Instant range differs across targets; no arbitrary millisecond cap.
    assert!(bind_f64(parameters(), f64::NAN).is_err());
    assert!(bind_f64(parameters(), f64::INFINITY).is_err());
}

#[tokio::test]
async fn path_validation_and_owned_open_future() {
    for (path, connections) in [("", 1), ("file:shared?mode=memory", 1), (":memory:", 2)] {
        let error = open(path, options(connections, 1, 0, 0).unwrap())
            .await
            .unwrap_err();
        assert_eq!(error.kind, FailureKind::Invalid);
        assert_eq!(error.outcome, Outcome::NotApplicable);
        assert!(!error.retired);
    }
    let future = {
        let path = String::from(":memory:");
        open(&path, options(1, 1, 0, 0).unwrap())
    };
    let pool = future.await.unwrap();
    let observer = pool.adapter.observer();
    assert_eq!(observer.snapshot().created, 0);
    close(&pool, 0).await.unwrap();
}

#[tokio::test]
async fn public_owned_parameters_literal_query_round_trip() {
    let pool = memory().await;
    let tx = begin(&pool, BeginMode::Deferred).await.unwrap();
    exec(
        &tx,
        literal("CREATE TABLE data(n,text,bytes,nullable,real)"),
        parameters(),
    )
    .await
    .unwrap();
    let text = "日本語\0tail".to_owned();
    let bytes = vec![0, 1, 255];
    let params = bind_f64(
        bind_null(bind_bytes(
            bind_text(bind_i64(parameters(), 9), text.clone()),
            bytes.clone(),
        )),
        1.5,
    )
    .unwrap();
    assert_eq!(
        exec(&tx, literal("INSERT INTO data VALUES (?,?,?,?,?)"), params)
            .await
            .unwrap(),
        1
    );
    let value = Payload {
        n: 9,
        text,
        bytes,
        nullable: None,
        real: 1.5,
    };
    assert_eq!(
        query::<Payload>(&tx, literal("SELECT * FROM data"), parameters())
            .await
            .unwrap(),
        Some(value)
    );
    assert_eq!(
        all::<Payload>(&tx, literal("SELECT * FROM data WHERE n=0"), parameters())
            .await
            .unwrap(),
        vec![]
    );
    assert_eq!(
        query::<Payload>(&tx, literal("SELECT * FROM data WHERE n=0"), parameters())
            .await
            .unwrap(),
        None
    );
    commit(tx).await.unwrap();
    close(&pool, 2_000).await.unwrap();
}

#[tokio::test]
async fn public_failures_preserve_causes_and_metadata_debug() {
    let pool = memory().await;
    let tx = begin(&pool, BeginMode::Deferred).await.unwrap();
    let bind = query::<Payload>(&tx, literal("SELECT ? AS n"), parameters())
        .await
        .unwrap_err();
    assert_eq!(bind.kind, FailureKind::Bind);
    assert!(matches!(
        copy_primary_error(&bind).unwrap().kind,
        ErrorKind::Invalid
    ));
    assert!(copy_cleanup_error(&bind).is_none());
    let sql = exec(
        &tx,
        literal("INSERT INTO secret_table VALUES(42)"),
        parameters(),
    )
    .await
    .unwrap_err();
    assert_eq!(sql.kind, FailureKind::Sql);
    let copied = copy_primary_error(&sql).unwrap();
    assert!(matches!(copied.kind, ErrorKind::Database));
    assert_eq!(copied.message, sql.message);
    let debug = format!("{sql:?}");
    assert!(!debug.contains("secret_table") && !debug.contains(&sql.message));
    assert!(debug.contains("Active"));
    assert!(!format!("{pool:?}").contains(":memory:"));
    rollback(tx).await.unwrap();
    close(&pool, 2_000).await.unwrap();
}

#[tokio::test]
async fn public_close_and_clone_share_stop_but_active_tx_can_finish() {
    let pool = memory().await;
    let cloned = clone_pool(&pool);
    let tx = begin(&pool, BeginMode::Deferred).await.unwrap();
    assert_eq!(
        close(&cloned, 0).await.unwrap_err().kind,
        FailureKind::CloseTimeout
    );
    assert_eq!(
        begin(&pool, BeginMode::Deferred).await.unwrap_err().kind,
        FailureKind::Closed
    );
    assert!(close(&pool, -1).await.is_err());
    commit(tx).await.unwrap();
    close(&pool, 2_000).await.unwrap();
    close(&cloned, 0).await.unwrap();
}

#[tokio::test]
async fn failed_lazy_record_reservation_starts_no_worker() {
    let adapter = adapter::Adapter::new(
        session::Config::default(),
        adapter::AdapterSeams {
            fail_native_reserve: true,
            ..Default::default()
        },
    );
    let pool = Pool {
        adapter,
        acquire: Duration::ZERO,
    };
    let error = begin(&pool, BeginMode::Deferred).await.unwrap_err();
    assert_eq!(error.kind, FailureKind::Allocation);
    assert_eq!(error.outcome, Outcome::NotApplicable);
    assert!(!error.retired);
    let stats = pool.adapter.observer().snapshot();
    assert_eq!(
        (stats.created, stats.native_started, stats.pending_workers),
        (0, 0, 0)
    );
    close(&pool, 0).await.unwrap();
}

#[tokio::test]
async fn failed_idle_return_records_cause_and_keeps_actual_join_owner() {
    let adapter = adapter::Adapter::new(
        session::Config::default(),
        adapter::AdapterSeams {
            fail_idle_reserve: true,
            ..Default::default()
        },
    );
    let pool = Pool {
        adapter,
        acquire: Duration::ZERO,
    };
    let observer = pool.adapter.observer();
    let tx = begin(&pool, BeginMode::Deferred).await.unwrap();
    rollback(tx).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), observer.wait_joined(1))
        .await
        .unwrap();
    let error = begin(&pool, BeginMode::Deferred).await.unwrap_err();
    assert_eq!(error.kind, FailureKind::Allocation);
    assert!(error.retired && copy_primary_error(&error).is_some());
    let close_error = close(&pool, 0).await.unwrap_err();
    assert_eq!(close_error.kind, FailureKind::Allocation);
    let stats = observer.snapshot();
    assert_eq!(
        (stats.native_closed, stats.joined, stats.pending_workers),
        (1, 1, 0)
    );
}

#[tokio::test]
async fn unpolled_public_finish_keeps_native_cleanup_and_last_pool_drop() {
    let pool = memory().await;
    let observer = pool.adapter.observer();
    let tx = begin(&pool, BeginMode::Deferred).await.unwrap();
    let finish = commit(tx);
    drop(pool);
    drop(finish);
    tokio::time::timeout(Duration::from_secs(2), observer.wait_joined(1))
        .await
        .unwrap();
    let stats = observer.snapshot();
    assert_eq!(
        (stats.native_closed, stats.joined, stats.pending_workers),
        (1, 1, 0)
    );
}

#[tokio::test]
async fn native_open_failure_is_structured_worker_failure_and_joined() {
    let mut file = adapter_tests::MultiFile::new();
    let missing = file.path.parent().unwrap().join("missing-child/data.db");
    let pool = open(missing.to_str().unwrap(), options(1, 1, 0, 0).unwrap())
        .await
        .unwrap();
    file.observe(&pool.adapter);
    let failure = begin(&pool, BeginMode::Deferred).await.unwrap_err();
    assert_eq!(failure.kind, FailureKind::Worker);
    assert!(matches!(
        copy_primary_error(&failure).unwrap().kind,
        ErrorKind::Database
    ));
    assert!(failure.retired);
    assert!(close(&pool, 2_000).await.is_err());
    let stats = pool.adapter.observer().snapshot();
    assert_eq!((stats.joined, stats.pending_workers), (1, 0));
}

#[tokio::test]
async fn begin_modes_use_native_busy_and_preserve_busy_error_kind() {
    let mut file = adapter_tests::MultiFile::new();
    let pool = open(file.path.to_str().unwrap(), options(2, 1, 0, 0).unwrap())
        .await
        .unwrap();
    file.observe(&pool.adapter);
    let first = begin(&pool, BeginMode::Immediate).await.unwrap();
    let failure = begin(&pool, BeginMode::Exclusive).await.unwrap_err();
    assert_eq!(failure.kind, FailureKind::Busy);
    assert_eq!(failure.outcome, Outcome::NotApplicable);
    assert!(!failure.retired);
    assert!(matches!(
        copy_primary_error(&failure).unwrap().kind,
        ErrorKind::Busy
    ));
    rollback(first).await.unwrap();
    // A 0ms acquire never waits for logical checkout return. ROLLBACK's
    // completion reply precedes the callback owner's final Drop, so observe
    // both returns explicitly before testing a new idle acquisition. This
    // test concerns native BEGIN/busy, not the zero-budget admission race.
    tokio::time::timeout(
        Duration::from_secs(2),
        pool.adapter.observer().wait_returned(2),
    )
    .await
    .unwrap();
    let second = begin(&pool, BeginMode::Exclusive).await.unwrap();
    rollback(second).await.unwrap();
    close(&pool, 2_000).await.unwrap();
}

#[tokio::test]
async fn queued_acquires_keep_tokio_fifo_and_cancelled_waiter_releases_its_place() {
    let pool = open(":memory:", options(1, 1, 2_000, 0).unwrap())
        .await
        .unwrap();
    let held = begin(&pool, BeginMode::Deferred).await.unwrap();
    let mut first = Box::pin(begin(&pool, BeginMode::Deferred));
    let mut second = Box::pin(begin(&pool, BeginMode::Deferred));
    assert!(futures_util::poll!(&mut first).is_pending());
    assert!(futures_util::poll!(&mut second).is_pending());
    rollback(held).await.unwrap();
    let first_tx = first.await.unwrap();
    assert!(futures_util::poll!(&mut second).is_pending());
    rollback(first_tx).await.unwrap();
    rollback(second.await.unwrap()).await.unwrap();
    let held = begin(&pool, BeginMode::Deferred).await.unwrap();
    let mut cancelled = Box::pin(begin(&pool, BeginMode::Deferred));
    let mut waiting = Box::pin(begin(&pool, BeginMode::Deferred));
    assert!(futures_util::poll!(&mut cancelled).is_pending());
    assert!(futures_util::poll!(&mut waiting).is_pending());
    drop(cancelled);
    rollback(held).await.unwrap();
    rollback(waiting.await.unwrap()).await.unwrap();
    close(&pool, 2_000).await.unwrap();
}
