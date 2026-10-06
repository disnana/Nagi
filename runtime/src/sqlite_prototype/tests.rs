use super::*;
use crate::FromRow;
use rusqlite::{types::Value, Row};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, PartialEq)]
struct Number(i64);
impl FromRow for Number {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(row: &Row<'_>, indices: &[usize]) -> rusqlite::Result<Self> {
        assert!(
            !management_active(),
            "FromRow callback received management authority"
        );
        Ok(Self(row.get(indices[0])?))
    }
}

async fn count(tx: &Tx) -> i64 {
    tx.query::<Number>("SELECT count(*) AS n FROM items", vec![])
        .await
        .unwrap()
        .unwrap()
        .0
}

#[tokio::test]
async fn sql_policy_denies_control_in_every_entry_and_keeps_tx_active() {
    let driver = Driver::open(Config::default());
    let tx = driver.begin().await.unwrap();
    tx.exec("CREATE TABLE items(n INTEGER)", vec![])
        .await
        .unwrap();
    for sql in [
        "COMMIT",
        "END",
        "ROLLBACK",
        "BEGIN",
        "SAVEPOINT x",
        "RELEASE x",
        "ROLLBACK TO x",
        "PRAGMA user_version=7",
        "ATTACH ':memory:' AS other",
        "DETACH main",
        "CREATE VIRTUAL TABLE v USING fts5(n)",
        "SELECT load_extension('x')",
    ] {
        assert!(tx.exec(sql, vec![]).await.is_err(), "exec accepted {sql}");
        assert!(
            tx.query::<Number>(sql, vec![]).await.is_err(),
            "query accepted {sql}"
        );
        assert!(
            tx.all::<Number>(sql, vec![]).await.is_err(),
            "all accepted {sql}"
        );
    }
    tx.exec("INSERT INTO items VALUES (?)", vec![Value::Integer(8)])
        .await
        .unwrap();
    assert_eq!(count(&tx).await, 1);
    assert_eq!(
        tx.query::<Number>("SELECT length('COMMIT; PRAGMA') AS n", vec![])
            .await
            .unwrap(),
        Some(Number(14))
    );
    tx.finish(Finish::Commit).await.unwrap();
    let tx = driver.begin().await.unwrap();
    assert_eq!(count(&tx).await, 1);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn one_statement_anonymous_bind_and_operation_shape_are_checked_before_step() {
    let driver = Driver::open(Config::default());
    let tx = driver.begin().await.unwrap();
    tx.exec("CREATE TABLE items(n INTEGER)", vec![])
        .await
        .unwrap();
    for sql in [
        "INSERT INTO items VALUES (1); INSERT INTO items VALUES (2)",
        "INSERT INTO items VALUES (?) RETURNING n",
        "SELECT 1 AS n",
    ] {
        assert!(tx.exec(sql, vec![]).await.is_err());
    }
    assert_eq!(count(&tx).await, 0);
    for sql in [
        "SELECT ?1 AS n",
        "SELECT ?2 AS n",
        "SELECT :n AS n",
        "SELECT @n AS n",
        "SELECT $n AS n",
    ] {
        assert_eq!(
            tx.query::<Number>(sql, vec![Value::Integer(3)])
                .await
                .unwrap_err()
                .kind,
            Kind::Bind
        );
    }
    assert_eq!(
        tx.query::<Number>("SELECT ? AS n", vec![])
            .await
            .unwrap_err()
            .kind,
        Kind::Bind
    );
    assert_eq!(
        tx.query::<Number>("SELECT ? AS n", vec![Value::Integer(1), Value::Integer(2)])
            .await
            .unwrap_err()
            .kind,
        Kind::Bind
    );
    assert!(tx
        .query::<Number>("INSERT INTO items VALUES (9) RETURNING n", vec![])
        .await
        .is_err());
    assert_eq!(count(&tx).await, 0);
    assert_eq!(
        tx.all::<Number>(
            "SELECT column1 AS n FROM (VALUES (?), (?))",
            vec![Value::Integer(2), Value::Integer(5)]
        )
        .await
        .unwrap(),
        vec![Number(2), Number(5)]
    );
    assert_eq!(
        tx.exec("INSERT INTO items VALUES (7); -- trailing comment", vec![])
            .await
            .unwrap(),
        1
    );
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[derive(Debug, PartialEq)]
struct Payload {
    text: String,
    bytes: Vec<u8>,
    nullable: Option<i64>,
    n: i64,
}
impl FromRow for Payload {
    fn columns() -> &'static [&'static str] {
        &["text", "bytes", "nullable", "n"]
    }
    fn read(row: &Row<'_>, ix: &[usize]) -> rusqlite::Result<Self> {
        assert!(!management_active());
        Ok(Self {
            text: row.get(ix[0])?,
            bytes: row.get(ix[1])?,
            nullable: row.get(ix[2])?,
            n: row.get(ix[3])?,
        })
    }
}

#[tokio::test]
async fn owned_bind_values_and_decode_errors_use_native_row_contract() {
    let driver = Driver::open(Config::default());
    let tx = driver.begin().await.unwrap();
    let text = "日本語\0tail".to_owned();
    let bytes = vec![0, 1, 255];
    let result = tx
        .query::<Payload>(
            "SELECT ? AS text, ? AS bytes, ? AS nullable, ? AS n",
            vec![
                Value::Text(text.clone()),
                Value::Blob(bytes.clone()),
                Value::Null,
                Value::Integer(i64::MIN),
            ],
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result,
        Payload {
            text,
            bytes,
            nullable: None,
            n: i64::MIN
        }
    );
    assert_eq!(
        tx.query::<Number>("SELECT NULL AS n", vec![])
            .await
            .unwrap_err()
            .kind,
        Kind::Decode
    );
    assert_eq!(
        tx.query::<Number>("SELECT 'bad' AS n", vec![])
            .await
            .unwrap_err()
            .kind,
        Kind::Decode
    );
    assert_eq!(
        tx.query::<Number>("SELECT 4 AS n", vec![]).await.unwrap(),
        Some(Number(4))
    );
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn pragma_tvf_existing_views_and_schema_reprepare_keep_authorizer_installed() {
    let unrestricted = rusqlite::Connection::open_in_memory().unwrap();
    unrestricted.execute_batch("CREATE TABLE items(n INTEGER); CREATE VIEW info AS SELECT * FROM pragma_table_info('items');").unwrap();
    let driver = Driver::open(Config { seed: "CREATE TABLE items(n INTEGER); CREATE VIEW info AS SELECT * FROM pragma_table_info('items');", ..Config::default() });
    let tx = driver.begin().await.unwrap();
    for sql in [
        "SELECT cid AS n FROM pragma_table_info('items')",
        "SELECT cid AS n FROM info",
        "SELECT user_version AS n FROM pragma_user_version",
    ] {
        unrestricted
            .query_row(sql, [], |row| row.get::<_, i64>(0))
            .unwrap();
        let denied_before = driver.stats().denied_pragmas;
        assert!(
            tx.all::<Number>(sql, vec![]).await.is_err(),
            "pragma path accepted: {sql}"
        );
        assert!(
            driver.stats().denied_pragmas > denied_before,
            "failure did not come from denied AuthAction::Pragma: {sql}"
        );
    }
    tx.exec("CREATE VIEW ordinary AS SELECT n FROM items", vec![])
        .await
        .unwrap();
    tx.exec("INSERT INTO items VALUES (3)", vec![])
        .await
        .unwrap();
    assert_eq!(
        tx.all::<Number>("SELECT n FROM ordinary", vec![])
            .await
            .unwrap(),
        vec![Number(3)]
    );
    // private seam prepares an allowed view, changes its schema on the same native connection,
    // then steps the same statement to force SQLite's automatic reprepare under the hook.
    unrestricted
        .execute_batch("CREATE VIEW v AS SELECT n FROM items")
        .unwrap();
    let mut statement = unrestricted.prepare("SELECT n FROM v").unwrap();
    unrestricted
        .execute_batch(
            "DROP VIEW v; CREATE VIEW v AS SELECT cid AS n FROM pragma_table_info('items')",
        )
        .unwrap();
    assert_eq!(
        statement.query_row([], |row| row.get::<_, i64>(0)).unwrap(),
        0
    );
    statement.finalize().unwrap();
    unrestricted.close().unwrap();
    let denied_before = driver.stats().denied_pragmas;
    assert!(driver.reprepare_probe().await.unwrap());
    assert!(driver.stats().denied_pragmas > denied_before);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn existing_trigger_and_or_rollback_abort_session_and_reject_later_sql() {
    for (seed, sql) in [
        ("CREATE TABLE items(n INTEGER UNIQUE); INSERT INTO items VALUES (1)", "INSERT OR ROLLBACK INTO items VALUES (1)"),
        ("CREATE TABLE items(n INTEGER); CREATE TRIGGER abort_insert BEFORE INSERT ON items BEGIN SELECT RAISE(ROLLBACK, 'abort'); END", "INSERT INTO items VALUES (1)"),
    ] {
        let driver = Driver::open(Config { seed, ..Config::default() });
        let tx = driver.begin().await.unwrap();
        let error = tx.exec(sql, vec![]).await.unwrap_err();
        assert_eq!(error.kind, Kind::Aborted);
        assert_eq!(error.outcome, Outcome::RolledBack);
        assert_eq!(tx.exec("CREATE TABLE forbidden(n)", vec![]).await.unwrap_err().kind, Kind::Aborted);
        tx.finish(Finish::Rollback).await.unwrap();
        let tx = driver.begin().await.unwrap();
        assert!(tx.query::<Number>("SELECT n FROM forbidden", vec![]).await.is_err());
        tx.finish(Finish::Rollback).await.unwrap();
        driver.close(Duration::from_secs(2)).await.unwrap();
    }
}

#[tokio::test]
async fn commit_after_native_auto_rollback_returns_aborted_error_instead_of_success() {
    for (seed, sql) in [
        ("CREATE TABLE items(n INTEGER UNIQUE); INSERT INTO items VALUES (1)", "INSERT OR ROLLBACK INTO items VALUES (1)"),
        ("CREATE TABLE items(n INTEGER); CREATE TRIGGER abort_insert BEFORE INSERT ON items BEGIN SELECT RAISE(ROLLBACK, 'abort'); END", "INSERT INTO items VALUES (1)"),
    ] {
        let driver = Driver::open(Config { seed, ..Config::default() });
        let tx = driver.begin().await.unwrap();
        assert_eq!(tx.exec(sql, vec![]).await.unwrap_err().kind, Kind::Aborted);
        let error = tx.finish(Finish::Commit).await.unwrap_err();
        assert_eq!(error.kind, Kind::Aborted);
        assert_eq!(error.outcome, Outcome::RolledBack);
        assert!(!error.retired);
        let tx = driver.begin().await.unwrap();
        tx.finish(Finish::Rollback).await.unwrap();
        driver.close(Duration::from_secs(2)).await.unwrap();
    }
}

#[tokio::test]
async fn trigger_pragma_is_denied_and_native_prior_effects_require_explicit_rollback() {
    for (seed, expected_prior_rows) in [
        ("CREATE TABLE items(n INTEGER); CREATE TRIGGER inspect BEFORE INSERT ON items BEGIN SELECT cid FROM pragma_table_info('items'); END", 0),
        ("CREATE TABLE items(n INTEGER); CREATE TRIGGER inspect AFTER INSERT ON items BEGIN SELECT cid FROM pragma_table_info('items'); END", 1),
    ] {
        let mut unrestricted = rusqlite::Connection::open_in_memory().unwrap();
        unrestricted.execute_batch(seed).unwrap();
        assert_eq!(unrestricted.execute("INSERT INTO items VALUES (1)", []).unwrap(), 1);
        unrestricted.execute("DELETE FROM items", []).unwrap();
        unrestricted.authorizer(Some(|context: rusqlite::hooks::AuthContext<'_>| {
            if matches!(context.action, rusqlite::hooks::AuthAction::Pragma { .. }) {
                rusqlite::hooks::Authorization::Deny
            } else { rusqlite::hooks::Authorization::Allow }
        })).unwrap();
        let native = unrestricted.transaction().unwrap();
        let mut statement = native.prepare("INSERT INTO items VALUES (1)").unwrap();
        assert!(matches!(statement.execute([]), Err(rusqlite::Error::SqliteFailure(error, _)) if error.extended_code == 23));
        statement.finalize().unwrap();
        assert!(!native.is_autocommit());
        let native_prior_rows: i64 = native.query_row("SELECT count(*) FROM items", [], |row| row.get(0)).unwrap();
        assert_eq!(native_prior_rows, expected_prior_rows);
        native.rollback().unwrap();
        unrestricted.close().unwrap();
        let driver = Driver::open(Config { seed, ..Config::default() });
        let tx = driver.begin().await.unwrap();
        let denied_before = driver.stats().denied_pragmas;
        let error = tx.exec("INSERT INTO items VALUES (1)", vec![]).await.unwrap_err();
        assert_eq!(error.kind, Kind::Sql);
        assert_eq!(error.outcome, Outcome::Active);
        assert!(driver.stats().denied_pragmas > denied_before);
        assert!(!driver.stats().management);
        // 新prototype testの「SQL Errなら効果0」は未採用の原子性保証だった。
        // AUTH Err以前のDMLはSQLite同条件の観測に従い、明示rollbackを完了させる。
        assert_eq!(count(&tx).await, native_prior_rows);
        tx.finish(Finish::Rollback).await.unwrap();
        let tx = driver.begin().await.unwrap();
        assert_eq!(count(&tx).await, 0);
        tx.finish(Finish::Rollback).await.unwrap();
        driver.close(Duration::from_secs(2)).await.unwrap();
    }
}

#[tokio::test]
async fn raise_fail_preserves_active_prior_effects_until_explicit_rollback() {
    let driver = Driver::open(Config {
        seed: "CREATE TABLE items(n); CREATE TRIGGER fail AFTER INSERT ON items BEGIN SELECT RAISE(FAIL, 'ordinary failure'); END",
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let error = tx
        .exec("INSERT INTO items VALUES (1)", vec![])
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::Sql);
    assert_eq!(error.outcome, Outcome::Active);
    assert_eq!(count(&tx).await, 1);
    tx.finish(Finish::Rollback).await.unwrap();
    let tx = driver.begin().await.unwrap();
    assert_eq!(count(&tx).await, 0);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn ordinary_sql_error_does_not_abort_active_session_and_exec_counts_direct_changes() {
    let driver = Driver::open(Config { seed: "CREATE TABLE items(n INTEGER UNIQUE); CREATE TABLE audit(n); CREATE TRIGGER log AFTER INSERT ON items BEGIN INSERT INTO audit VALUES(NEW.n); END", ..Config::default() });
    let tx = driver.begin().await.unwrap();
    assert_eq!(
        tx.exec("INSERT INTO items VALUES (1)", vec![])
            .await
            .unwrap(),
        1
    );
    let error = tx
        .exec("INSERT INTO items VALUES (1)", vec![])
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::Sql);
    assert_eq!(error.outcome, Outcome::Active);
    tx.exec("INSERT INTO items VALUES (2)", vec![])
        .await
        .unwrap();
    assert_eq!(count(&tx).await, 2);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn eof_cleanup_blocks_next_session_and_unpolled_finish_still_owns_cleanup() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        cleanup_gate: Some(Arc::clone(&gate)),
        seed: "CREATE TABLE items(n)",
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    tx.exec("INSERT INTO items VALUES (7)", vec![])
        .await
        .unwrap();
    let unpolled = tx.finish(Finish::Commit);
    drop(unpolled);
    gate.wait().await;
    let mut next = Box::pin(driver.begin());
    driver.admit(&mut next).await;
    assert_eq!(
        driver.stats().begun,
        1,
        "slot reused before cleanup barrier"
    );
    gate.release();
    let tx = next.await.unwrap();
    assert_eq!(count(&tx).await, 0);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn begin_reply_cancellation_closes_only_sender_and_runs_observed_cleanup() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        begin_gate: Some(Arc::clone(&gate)),
        ..Config::default()
    });
    let mut begin = Box::pin(driver.begin());
    tokio::select! { _ = &mut begin => panic!("begin passed barrier"), _ = gate.wait() => {} }
    drop(begin);
    gate.release();
    driver.wait_settled(1).await;
    let tx = driver.begin().await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn full_inbox_and_cancelled_admitted_operation_drain_before_eof_rollback() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        command_gate: Some(Arc::clone(&gate)),
        seed: "CREATE TABLE items(n)",
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let mut first = Box::pin(tx.exec("INSERT INTO items VALUES (1)", vec![]));
    tokio::select! { _ = &mut first => panic!("operation passed barrier"), _ = gate.wait() => {} }
    tx.enqueue_without_reply("INSERT INTO items VALUES (2)")
        .await;
    assert_eq!(tx.available_capacity(), 0);
    drop(first);
    drop(tx); // EOF is reliable even though try_send(rollback) would fail with Full.
    gate.release();
    driver.wait_settled(1).await;
    assert_eq!(
        driver.stats().executed,
        2,
        "accepted commands were discarded"
    );
    let tx = driver.begin().await.unwrap();
    assert_eq!(count(&tx).await, 0);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn cancelled_commit_reply_preserves_committed_data_and_does_not_retry() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        commit_gate: Some(Arc::clone(&gate)),
        seed: "CREATE TABLE items(n)",
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    tx.exec("INSERT INTO items VALUES (9)", vec![])
        .await
        .unwrap();
    let mut commit = Box::pin(tx.finish(Finish::Commit));
    tokio::select! { _ = &mut commit => panic!("commit passed barrier"), _ = gate.wait() => {} }
    drop(commit); // caller receives no fabricated result; it cannot infer rollback.
    gate.release();
    let tx = driver.begin().await.unwrap();
    assert_eq!(count(&tx).await, 1);
    assert_eq!(driver.stats().committed, 1);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn rollback_failure_is_observed_without_native_drop_retry_and_retires_connection() {
    let driver = Driver::open(Config {
        deny_rollback: true,
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let error = tx.finish(Finish::Rollback).await.unwrap_err();
    assert_eq!(error.kind, Kind::Cleanup);
    assert_eq!(error.outcome, Outcome::Unknown);
    assert!(error.retired && error.cleanup.is_some());
    assert_eq!(
        driver.stats().rollback_attempts,
        1,
        "native Drop hid/retried a failure"
    );
    assert_eq!(driver.begin().await.unwrap_err().kind, Kind::Worker);
    assert!(driver.close(Duration::from_secs(2)).await.is_err());
    assert!(driver.stats().joined);
}

#[tokio::test]
async fn commit_failure_and_failed_cleanup_preserve_both_causes_and_unknown_outcome() {
    let driver = Driver::open(Config {
        deny_commit: true,
        deny_rollback: true,
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let error = tx.finish(Finish::Commit).await.unwrap_err();
    assert_eq!(error.kind, Kind::Sql);
    assert_eq!(error.outcome, Outcome::Unknown);
    assert!(error.primary.is_some() && error.cleanup.is_some() && error.retired);
    assert_eq!(driver.stats().rollback_attempts, 1);
    assert_eq!(driver.begin().await.unwrap_err().kind, Kind::Worker);
    assert!(driver.close(Duration::from_secs(2)).await.is_err());
}

#[tokio::test]
async fn failed_commit_with_successful_observed_rollback_can_reuse_connection() {
    let driver = Driver::open(Config {
        deny_commit: true,
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let error = tx.finish(Finish::Commit).await.unwrap_err();
    assert_eq!(error.kind, Kind::Sql);
    assert_eq!(error.outcome, Outcome::RolledBack);
    assert!(error.primary.is_some() && error.cleanup.is_none() && !error.retired);
    let tx = driver.begin().await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn management_panic_restores_raii_flag_before_observed_rollback_and_retirement() {
    let driver = Driver::open(Config {
        panic_management: true,
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let error = tx.finish(Finish::Commit).await.unwrap_err();
    assert_eq!(error.kind, Kind::Worker);
    assert!(error.retired);
    assert!(!driver.stats().management);
    assert_eq!(driver.stats().rollback_attempts, 1);
    assert!(driver.close(Duration::from_secs(2)).await.is_err());
}

#[tokio::test]
async fn cancelled_finish_before_full_inbox_admission_uses_eof_cleanup() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        command_gate: Some(Arc::clone(&gate)),
        seed: "CREATE TABLE items(n)",
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let mut first = Box::pin(tx.exec("INSERT INTO items VALUES (1)", vec![]));
    tokio::select! {
        _ = &mut first => panic!("operation passed barrier"),
        _ = gate.wait() => {}
    }
    drop(first);
    tx.enqueue_without_reply("INSERT INTO items VALUES (2)")
        .await;
    let mut finish = Box::pin(tx.finish(Finish::Commit));
    assert!(futures_util::poll!(&mut finish).is_pending());
    drop(finish);
    gate.release();
    driver.wait_settled(1).await;
    assert_eq!(driver.stats().executed, 2);
    assert_eq!(driver.stats().committed, 0);
    let tx = driver.begin().await.unwrap();
    assert_eq!(count(&tx).await, 0);
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn committed_outcome_and_cleanup_failure_are_both_preserved() {
    let driver = Driver::open(Config {
        fail_restore_check: true,
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let error = tx.finish(Finish::Commit).await.unwrap_err();
    assert_eq!(error.outcome, Outcome::Committed);
    assert!(error.retired && error.cleanup.is_some());
    assert_eq!(driver.begin().await.unwrap_err().kind, Kind::Worker);
    assert!(driver.close(Duration::from_secs(2)).await.is_err());
}

#[tokio::test]
async fn user_callback_panic_restores_management_state_and_retires_worker() {
    struct PanicRow;
    impl FromRow for PanicRow {
        fn columns() -> &'static [&'static str] {
            &["n"]
        }
        fn read(_: &Row<'_>, _: &[usize]) -> rusqlite::Result<Self> {
            assert!(!management_active());
            panic!("private test callback panic")
        }
    }
    let driver = Driver::open(Config::default());
    let tx = driver.begin().await.unwrap();
    let error = tx
        .query::<PanicRow>("SELECT 1 AS n", vec![])
        .await
        .err()
        .unwrap();
    assert!(matches!(error.kind, Kind::Worker | Kind::ReplyLost));
    drop(tx);
    driver.wait_settled(1).await;
    assert!(!driver.stats().management);
    assert_eq!(driver.begin().await.unwrap_err().kind, Kind::Worker);
    assert!(driver.close(Duration::from_secs(2)).await.is_err());
}

#[tokio::test]
async fn close_timeout_and_cancel_keep_closing_until_native_close_and_join() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        exit_gate: Some(Arc::clone(&gate)),
        ..Config::default()
    });
    let tx = driver.begin().await.unwrap();
    let mut closing = Box::pin(driver.close(Duration::from_secs(2)));
    tokio::select! { _ = &mut closing => panic!("close passed active session"), _ = driver.wait_closing() => {} }
    drop(closing);
    assert_eq!(driver.begin().await.unwrap_err().kind, Kind::Closed);
    assert_eq!(
        driver.close(Duration::ZERO).await.unwrap_err().kind,
        Kind::CloseTimeout
    );
    tx.finish(Finish::Rollback).await.unwrap();
    gate.wait().await;
    assert!(driver.stats().native_closed);
    assert!(
        !driver.stats().joined,
        "native close notification confused with worker join"
    );
    assert_eq!(
        driver.close(Duration::ZERO).await.unwrap_err().kind,
        Kind::CloseTimeout
    );
    gate.release();
    driver.close(Duration::from_secs(2)).await.unwrap();
    assert!(driver.stats().joined);
    driver.close(Duration::ZERO).await.unwrap();
}

#[tokio::test]
async fn native_close_failure_is_not_reported_as_success_even_after_join() {
    let driver = Driver::open(Config {
        leak_statement_at_close: true,
        ..Config::default()
    });
    let error = driver.close(Duration::from_secs(2)).await.unwrap_err();
    assert!(error.cleanup.is_some());
    assert!(driver.stats().joined);
    assert!(!driver.stats().native_closed);
}
