//! Actual native core, old failure seams and real checkout owner, diagnostic cfg.
use super::*;
use std::time::Instant;
fn observer() -> Observer {
    let now = Instant::now();
    Observer::new(now, now + Duration::from_secs(10))
}
fn snapshot(o: &Observer) -> String {
    o.native_finish_for_test()
}

#[tokio::test]
async fn real_pool_commit_ack_follows_original_checkout_drop() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let directory = loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p =
            std::env::temp_dir().join(format!("native-finish-owned-{}-{n}", std::process::id()));
        match std::fs::create_dir(&p) {
            Ok(()) => break p,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => panic!("owned fixture directory unavailable"),
        }
    };
    let pool = crate::sqlite::open(
        directory.join("owned.sqlite").to_str().unwrap(),
        crate::sqlite::options(1, 2, 1000, 1000).unwrap(),
    )
    .await
    .unwrap();
    let o = observer();
    o.scope(async {
        let tx = crate::sqlite::begin(&pool, crate::sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        crate::sqlite::commit(tx).await.unwrap();
    })
    .await;
    let line = snapshot(&o);
    assert!(line.contains("native_state=7 commit_result=1 cleanup_result=0"));
    assert!(!line.contains("settled_us=60000001"));
    let tx = crate::sqlite::begin(&pool, crate::sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    crate::sqlite::rollback(tx).await.unwrap();
    crate::sqlite::close(&pool, 2000).await.unwrap();
    assert!(
        line.split(" selected_deadline_passed=").next()
            == snapshot(&o).split(" selected_deadline_passed=").next()
    );
    std::fs::remove_dir_all(directory).unwrap();
}
#[tokio::test]
async fn old_commit_gate_is_after_native_return_and_before_checkout() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        commit_gate: Some(Arc::clone(&gate)),
        ..Config::default()
    });
    let o = observer();
    let tx = o.scope(driver.begin()).await.unwrap();
    let mut finish = Box::pin(tx.finish(Finish::Commit));
    tokio::select! {result=&mut finish=>panic!("finish completed before old gate: {}",result.is_ok()),_=gate.wait()=>{}}
    let line = snapshot(&o);
    assert!(line.contains("native_state=2 commit_result=1"));
    assert!(line.contains("settled_us=60000001"));
    assert!(line.contains("commits=1"));
    gate.release();
    finish.await.unwrap();
    assert!(snapshot(&o).contains("native_state=7 commit_result=1"));
    driver.close(Duration::from_secs(2)).await.unwrap();
}
#[tokio::test]
async fn denied_commit_records_err_then_actual_cleanup_ok_or_err() {
    for deny_rollback in [false, true] {
        let driver = Driver::open(Config {
            deny_commit: true,
            deny_rollback,
            ..Config::default()
        });
        let o = observer();
        let tx = o.scope(driver.begin()).await.unwrap();
        let error = tx.finish(Finish::Commit).await.unwrap_err();
        assert_eq!(error.kind, Kind::Sql);
        assert_eq!(
            error.outcome,
            if deny_rollback {
                Outcome::Unknown
            } else {
                Outcome::RolledBack
            }
        );
        assert_eq!(error.retired, deny_rollback);
        let line = snapshot(&o);
        assert!(line.contains("native_state=7 commit_result=2"));
        assert!(line.contains(if deny_rollback {
            "cleanup_result=3 cleanup_reason=2"
        } else {
            "cleanup_result=2 cleanup_reason=2"
        }));
        assert!(!line.contains("commit_return_us=60000001"));
        let closed = driver.close(Duration::from_secs(2)).await;
        assert_eq!(closed.is_err(), deny_rollback);
        assert!(driver.stats().joined && driver.stats().native_closed);
    }
}
#[tokio::test]
async fn eof_cleanup_retains_reason_and_retirement_without_retry() {
    for deny_rollback in [false, true] {
        let driver = Driver::open(Config {
            deny_rollback,
            ..Config::default()
        });
        let o = observer();
        let tx = o.scope(driver.begin()).await.unwrap();
        drop(tx);
        let closed = driver.close(Duration::from_secs(2)).await;
        let line = snapshot(&o);
        assert!(line.contains("native_state=7 commit_result=0"));
        assert!(line.contains(if deny_rollback {
            "cleanup_result=3 cleanup_reason=1"
        } else {
            "cleanup_result=2 cleanup_reason=1"
        }));
        assert_eq!(closed.is_err(), deny_rollback);
        assert_eq!(driver.stats().rollback_attempts, 1);
        assert!(driver.stats().joined && driver.stats().native_closed);
    }
}
#[tokio::test]
async fn old_management_panic_has_unseen_commit_and_after_finish_cleanup() {
    let driver = Driver::open(Config {
        panic_management: true,
        ..Config::default()
    });
    let o = observer();
    let tx = o.scope(driver.begin()).await.unwrap();
    let error = tx.finish(Finish::Commit).await.unwrap_err();
    assert_eq!(error.kind, Kind::Worker);
    assert!(error.retired);
    let line = snapshot(&o);
    assert!(line.contains("native_state=7 commit_result=0 cleanup_result=2 cleanup_reason=3"));
    assert!(line.contains("commit_enter_us=60000001 commit_return_us=60000001"));
    assert!(driver.close(Duration::from_secs(2)).await.is_err());
    assert!(driver.stats().joined && driver.stats().native_closed);
}
#[tokio::test]
async fn precommit_expired_observer_never_invents_commit_entry() {
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        begin_gate: Some(Arc::clone(&gate)),
        ..Config::default()
    });
    let now = Instant::now();
    let o = Observer::new(now, now);
    let mut begin = Box::pin(o.scope(driver.begin()));
    tokio::select! {result=&mut begin=>panic!("begin completed before old gate: {}",result.is_ok()),_=gate.wait()=>{}}
    let line = snapshot(&o);
    assert!(line.contains("native_state=0 commit_result=0"));
    assert!(line.contains("commit_enter_us=60000001"));
    assert!(line.contains("selected_deadline_passed=1"));
    gate.release();
    let tx = begin.await.unwrap();
    tx.finish(Finish::Rollback).await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
}

#[tokio::test]
async fn actual_native_call_return_is_visible_before_record_mutex_publication() {
    let before = Arc::new(Gate::new());
    let returned = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        native_call_gate: Some(Arc::clone(&before)),
        native_return_gate: Some(Arc::clone(&returned)),
        ..Config::default()
    });
    let o = observer();
    let tx = o.scope(driver.begin()).await.unwrap();
    let mut finish = Box::pin(tx.finish(Finish::Commit));
    tokio::select! { result=&mut finish=>panic!("finish completed before native call: {}",result.is_ok()), _=before.wait()=>{} }
    let (held, held_wait) = std::sync::mpsc::sync_channel(1);
    let (release, release_wait) = std::sync::mpsc::channel();
    let owner = o.clone();
    let lock_thread =
        std::thread::spawn(move || owner.hold_native_record_for_test(held, release_wait));
    held_wait.recv_timeout(WATCHDOG).unwrap();
    before.release();
    // No async future holds the record guard. Native entry/return are atomic
    // and must become visible while the independent owner thread holds it.
    tokio::select! { result=&mut finish=>panic!("finish completed before publication gate: {}",result.is_ok()), _=returned.wait()=>{} }
    let independent_return = o.native_call_return_seen_for_test();
    release.send(()).unwrap();
    lock_thread.join().unwrap();
    let lag = o.native_call_for_test();
    returned.release();
    finish.await.unwrap();
    let published = o.native_call_for_test();
    driver.close(Duration::from_secs(2)).await.unwrap();
    assert!(driver.stats().joined && driver.stats().native_closed);
    assert!(
        independent_return,
        "return stamp must precede the observer record lock"
    );
    assert!(lag.contains("diagnostic_valid=1 native_call_state=3 generation=1"));
    assert!(lag.contains("native_state=1 commit_result=0 settled_us=60000001"));
    assert!(published.contains("native_call_state=4"));
}

#[tokio::test]
async fn actual_reader_holds_native_commit_but_old_postcommit_gate_has_returned_stamp() {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let directory = loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "native-call-reader-owned-{}-{n}",
            std::process::id()
        ));
        match std::fs::create_dir(&p) {
            Ok(()) => break p,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => panic!("owned native reader fixture unavailable"),
        }
    };
    let filename = directory.join("owned.sqlite");
    let reader = Connection::open(&filename).unwrap();
    let journal: String = reader
        .query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal, "delete");
    reader
        .execute_batch(
            "CREATE TABLE owned_value(value INTEGER); INSERT INTO owned_value VALUES(1);",
        )
        .unwrap();
    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        path: Some(filename),
        busy: Duration::from_secs(5),
        native_call_gate: Some(Arc::clone(&gate)),
        ..Config::default()
    });
    let admitted = Instant::now();
    let until = admitted + Duration::from_millis(1000);
    let o = Observer::new(admitted, until);
    let tx = o
        .scope(async {
            let sender = driver
                .begin_sender
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .clone();
            let (reply, receive) = oneshot::channel();
            let mut request = BeginRequest::owned(reply, Box::new(()));
            request.mode = super::super::BeginMode::Immediate;
            sender.send(request).await.unwrap();
            receive.await.unwrap().unwrap()
        })
        .await;
    tx.exec("UPDATE owned_value SET value=2", vec![])
        .await
        .unwrap();
    reader.execute_batch("BEGIN DEFERRED;").unwrap();
    assert_eq!(
        reader
            .query_row("SELECT value FROM owned_value", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let mut finish = Box::pin(tx.finish(Finish::Commit));
    tokio::select! { result=&mut finish=>panic!("finish completed before native call: {}",result.is_ok()), _=gate.wait()=>{} }
    gate.release();
    o.wait_native_call_for_test(false).await;
    tokio::select! { result=&mut finish=>panic!("COMMIT completed while explicit reader retained: {}",result.is_ok()), _=tokio::time::sleep_until(until.into())=>{} }
    let terminal = o.native_call_for_test();
    reader.execute_batch("ROLLBACK;").unwrap();
    drop(reader);
    finish.await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
    assert!(driver.stats().joined && driver.stats().native_closed);
    let eventual = o.native_call_for_test();
    std::fs::remove_dir_all(directory).unwrap();

    let gate = Arc::new(Gate::new());
    let driver = Driver::open(Config {
        commit_gate: Some(Arc::clone(&gate)),
        ..Config::default()
    });
    let o = observer();
    let tx = o.scope(driver.begin()).await.unwrap();
    let mut finish = Box::pin(tx.finish(Finish::Commit));
    tokio::select! { result=&mut finish=>panic!("finish completed before old gate: {}",result.is_ok()), _=gate.wait()=>{} }
    let after = o.native_call_for_test();
    gate.release();
    finish.await.unwrap();
    driver.close(Duration::from_secs(2)).await.unwrap();
    assert!(driver.stats().joined && driver.stats().native_closed);
    assert!(terminal.contains("diagnostic_valid=1 native_call_state=2 generation=1"));
    assert!(terminal.contains("call_return_us=60000001 native_state=1 commit_result=0"));
    assert!(terminal.ends_with("selected_deadline_passed=1"));
    assert!(eventual.contains("native_call_state=4"));
    assert!(after.contains("diagnostic_valid=1 native_call_state=4 generation=1"));
    assert!(after.contains("native_state=2 commit_result=1 settled_us=60000001"));
    assert!(!after.contains("call_return_us=60000001"));
}
