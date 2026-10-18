//! Helper semantics only; actual native boundaries have separate SQLite cases.
use super::*;
use std::time::{Duration, Instant};
fn observer() -> Observer {
    let now = Instant::now();
    Observer::new(now, now + Duration::from_secs(10))
}
fn record(o: &Observer) -> Record {
    o.0.record.lock().unwrap().native
}

#[test]
fn unseen_sentinel_is_distinct_from_zero_and_clamped_elapsed() {
    let o = observer();
    let n = record(&o);
    assert_eq!(n.commit_enter_us, UNSEEN_US);
    assert_eq!(n.settled_us, UNSEEN_US);
    assert!(n.valid);
    assert_eq!(n.stage, Stage::Unseen);
    assert_eq!(bounded_us(Duration::ZERO), 0);
    assert_eq!(bounded_us(Duration::from_secs(61)), LIMIT_US);
    let guard = o.native_finish();
    guard.commit_enter();
    guard.commit_return(true);
    assert!(record(&o).commit_enter_us <= LIMIT_US);
    assert!(record(&o).commit_return_us <= LIMIT_US);
    assert!(LIMIT_US < UNSEEN_US);
}
#[test]
fn result_reason_and_checkout_are_closed_independent_boundaries() {
    for (commit_ok, cleanup_ok) in [(true, true), (false, true), (false, false)] {
        let o = observer();
        let guard = o.native_finish();
        guard.commit_enter();
        guard.commit_return(commit_ok);
        assert_eq!(record(&o).commit_result, if commit_ok { 1 } else { 2 });
        assert_eq!(record(&o).settled_us, UNSEEN_US);
        if !commit_ok {
            guard.cleanup_after_finish_enter();
            assert_eq!(record(&o).cleanup_reason, 2);
            guard.cleanup_return(cleanup_ok);
            assert_eq!(record(&o).cleanup_result, if cleanup_ok { 2 } else { 3 });
        }
        guard.checkout_returned();
        assert_eq!(record(&o).stage, Stage::CheckoutReturned);
        assert_ne!(record(&o).settled_us, UNSEEN_US);
    }
    let o = observer();
    let guard = o.native_finish();
    guard.cleanup_eof_enter();
    guard.cleanup_return(true);
    guard.checkout_returned();
    assert_eq!(record(&o).cleanup_reason, 1);
    assert_eq!(record(&o).commit_enter_us, UNSEEN_US);
}
#[test]
fn serial_tx_resets_and_overlapping_or_saturated_tx_is_invalid() {
    let o = observer();
    {
        let guard = o.native_finish();
        guard.commit_enter();
        guard.commit_return(true);
        o.db(Db::CommitDone);
        guard.checkout_returned();
    }
    let first = record(&o);
    assert_eq!(first.commit_result, 1);
    let second = o.native_finish();
    assert_eq!(record(&o).commit_enter_us, UNSEEN_US);
    assert_eq!(o.0.record.lock().unwrap().commits, 1);
    let overlap = o.native_finish();
    assert!(!record(&o).valid);
    second.commit_enter();
    drop(overlap);
    drop(second);
    let later = o.native_finish();
    assert!(!record(&o).valid);
    drop(later);
    let other = observer();
    let prior = other.native_finish();
    prior.checkout_returned();
    let next = other.native_finish();
    drop(prior);
    next.commit_enter();
    next.commit_return(true);
    next.checkout_returned();
    assert!(record(&other).valid);
    let o = observer();
    o.update(|r| r.begins = 1023);
    let _guard = o.native_finish();
    assert!(!record(&o).valid);
}
#[test]
fn duplicate_or_out_of_order_marker_never_fabricates_completion() {
    let o = observer();
    let guard = o.native_finish();
    guard.commit_return(true);
    assert!(!record(&o).valid);
    let o = observer();
    let guard = o.native_finish();
    guard.cleanup_return(true);
    assert!(!record(&o).valid);
    let o = observer();
    let guard = o.native_finish();
    guard.checkout_returned();
    guard.checkout_returned();
    assert!(!record(&o).valid);
    let o = observer();
    let guard = o.native_finish();
    guard.commit_enter();
    o.update(|r| r.native.commit_enter_us = LIMIT_US);
    guard.commit_return(true);
    assert!(!record(&o).valid);
}
#[tokio::test]
async fn delayed_worker_updates_only_original_owner_and_not_copied_terminal() {
    let first = observer();
    let second = observer();
    let native = first
        .scope(async { super::super::current().unwrap() })
        .await;
    let (entered, entry) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let guard = native.native_finish();
        guard.commit_enter();
        entered.send(()).unwrap();
        wait.recv().unwrap();
        guard.commit_return(true);
        native.db(Db::CommitDone);
        guard.checkout_returned();
    });
    entry.recv().unwrap();
    let terminal = first.closed(504).unwrap();
    let copied = Closed::new(terminal, Origin::Session, 0).to_string();
    assert!(copied.contains("native_state=1 commit_result=0"));
    assert!(copied.contains("commit_return_us=60000001"));
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(first
        .native_finish_for_test()
        .contains("native_state=7 commit_result=1"));
    assert_eq!(record(&second).stage, Stage::Unseen);
    assert_eq!(second.0.record.lock().unwrap().begins, 0);
    assert_eq!(
        copied,
        Closed::new(terminal, Origin::Session, 0).to_string()
    );
    assert!(super::super::current().is_none());
}
#[test]
fn closed_numeric_line_and_poisoned_record_cannot_expose_payload() {
    let o = observer();
    let line = o.native_finish_for_test();
    assert!(line.len() < 512);
    assert!(line
        .strip_prefix("session_native_finish_diag ")
        .unwrap()
        .split(' ')
        .all(|f| f
            .split_once('=')
            .is_some_and(|(_, v)| v.bytes().all(|b| b.is_ascii_digit()))));
    let poisoned = o.clone();
    assert!(std::thread::spawn(move || {
        let _held = poisoned.0.record.lock().unwrap();
        panic!("private observer poison seam");
    })
    .join()
    .is_err());
    assert!(o.closed(504).is_none());
}
