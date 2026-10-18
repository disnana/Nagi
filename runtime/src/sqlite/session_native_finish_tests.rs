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
