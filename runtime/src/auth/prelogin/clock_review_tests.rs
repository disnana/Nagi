//! IR01 finite trusted-clock sequence oracles. Prospective, not executed RED.
use super::*;
fn sequence(pool: &mut Pool, values: Vec<i64>) {
    pool.context.set_test_clock_sequence(values);
}
async fn stored_wall(pool: &Pool) -> i64 {
    let tx = sqlite::begin(&pool.native, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let wall = super::super::metadata(&tx).await.unwrap().last_wall;
    sqlite::rollback(tx).await.unwrap();
    wall
}
fn no_material(owner: &RequestOwner) {
    assert!(!owner.is_ready());
    assert!(owner.cell.lock().unwrap().pair.is_none());
}
async fn checked_fixture() -> Option<(Fixture, Pool)> {
    let f = Fixture::new().await;
    let p = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(p.is_err());
        f.stop().await;
        return None;
    }
    Some((f, p.unwrap()))
}
#[tokio::test]
async fn bootstrap_reserve_backstep_rolls_back_without_business_enqueue() {
    let Some((f, mut p)) = checked_fixture().await else {
        return;
    };
    let b = stored_wall(&p).await;
    sequence(&mut p, vec![b + 10, b + 20, b + 15, b + 20, b + 20]);
    let request = owner();
    let e = p.bootstrap(&request, None).await.err().unwrap();
    assert!(e.kind == FailureKind::Unavailable);
    assert!(e.outcome == sqlite::Outcome::RolledBack && !e.business_committed);
    assert!(p.business_enqueues.load(Ordering::SeqCst) == 0);
    assert!(count(&p).await == 0);
    assert!(stored_wall(&p).await >= b + 20);
    no_material(&request);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn session_context_sample_is_part_of_the_same_operation_bound() {
    let Some((f, mut p)) = checked_fixture().await else {
        return;
    };
    let b = stored_wall(&p).await;
    sequence(&mut p, vec![b + 30, b + 20, b + 30, b + 30]);
    let request = owner();
    let e = p.bootstrap(&request, None).await.err().unwrap();
    assert!(e.kind == FailureKind::Unavailable && e.outcome == sqlite::Outcome::RolledBack);
    assert!(p.business_enqueues.load(Ordering::SeqCst) == 0);
    assert!(count(&p).await == 0);
    assert!(stored_wall(&p).await >= b + 30);
    no_material(&request);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn claim_reserve_backstep_has_no_delete_consumed_or_verifier() {
    let Some((f, mut p)) = checked_fixture().await else {
        return;
    };
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let input = presented(&bootstrap);
    let b = stored_wall(&p).await;
    p.business_enqueues.store(0, Ordering::SeqCst);
    sequence(&mut p, vec![b + 10, b + 20, b + 15, b + 20, b + 20]);
    let calls = AtomicUsize::new(0);
    let request = owner();
    let e = match p.claim(&request, input).await {
        Err(e) => e,
        Ok(witness) => {
            let _ = witness
                .verify(&request.binding(), || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
                })
                .await;
            panic!("backstep returned witness");
        }
    };
    assert!(
        e.kind == FailureKind::Unavailable
            && e.outcome == sqlite::Outcome::RolledBack
            && !e.business_committed
    );
    assert!(calls.load(Ordering::SeqCst) == 0 && p.business_enqueues.load(Ordering::SeqCst) == 0);
    assert!(count(&p).await == 1);
    assert!(stored_wall(&p).await >= b + 20);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn bootstrap_precommit_peak_blocks_lower_postcommit_ready() {
    let Some((f, mut p)) = checked_fixture().await else {
        return;
    };
    let b = stored_wall(&p).await;
    sequence(
        &mut p,
        vec![b + 10, b + 20, b + 20, b + 40, b + 30, b + 40, b + 40],
    );
    let request = owner();
    let e = p.bootstrap(&request, None).await.err().unwrap();
    assert!(
        e.kind == FailureKind::Unavailable
            && e.outcome == sqlite::Outcome::Committed
            && e.business_committed
    );
    assert!(p.business_enqueues.load(Ordering::SeqCst) == 1);
    assert!(count(&p).await == 1);
    assert!(stored_wall(&p).await >= b + 40);
    no_material(&request);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn claim_postcommit_backstep_keeps_row_commit_without_verifier() {
    let Some((f, mut p)) = checked_fixture().await else {
        return;
    };
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let input = presented(&bootstrap);
    let b = stored_wall(&p).await;
    p.business_enqueues.store(0, Ordering::SeqCst);
    sequence(
        &mut p,
        vec![b + 10, b + 20, b + 20, b + 40, b + 30, b + 40, b + 40],
    );
    let request = owner();
    let calls = AtomicUsize::new(0);
    let e = match p.claim(&request, input).await {
        Err(e) => e,
        Ok(witness) => {
            let _ = witness
                .verify(&request.binding(), || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60))
                })
                .await;
            panic!("backstep returned witness");
        }
    };
    assert!(
        e.kind == FailureKind::Unavailable
            && e.outcome == sqlite::Outcome::Committed
            && e.business_committed
    );
    assert!(calls.load(Ordering::SeqCst) == 0 && p.business_enqueues.load(Ordering::SeqCst) == 1);
    assert!(count(&p).await == 0);
    assert!(stored_wall(&p).await >= b + 40);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[test]
fn body_commit_marker_does_not_relabel_native_maintenance_outcome() {
    // Classification oracle only; this is not a native fault/cleanup execution.
    for outcome in [sqlite::Outcome::Unknown, sqlite::Outcome::RolledBack] {
        let e = Failure {
            kind: FailureKind::Unavailable,
            outcome,
            origin: FailureOrigin::Core,
            business_committed: false,
        }
        .after_commit();
        assert!(e.business_committed && e.outcome == outcome);
    }
}

// Added after the source fix: prospective restoration-boundary oracles, not RED.
#[tokio::test]
async fn rollback_restoration_backstep_refuses_without_promising_durable_peak() {
    let Some((f, mut p)) = checked_fixture().await else {
        return;
    };
    let b = stored_wall(&p).await;
    sequence(&mut p, vec![b + 10, b + 20, b + 15, b + 15]);
    let request = owner();
    let e = p.bootstrap(&request, None).await.err().unwrap();
    assert!(e.kind == FailureKind::Unavailable && e.outcome == sqlite::Outcome::RolledBack);
    assert!(!e.business_committed && p.business_enqueues.load(Ordering::SeqCst) == 0);
    assert!(count(&p).await == 0 && stored_wall(&p).await == b);
    no_material(&request);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn postcommit_maintenance_backstep_keeps_both_commit_and_rollback_facts() {
    let Some((f, mut p)) = checked_fixture().await else {
        return;
    };
    let b = stored_wall(&p).await;
    sequence(&mut p, vec![b + 10, b + 20, b + 20, b + 40, b + 50, b + 45]);
    let request = owner();
    let e = p.bootstrap(&request, None).await.err().unwrap();
    assert!(e.kind == FailureKind::Unavailable && e.outcome == sqlite::Outcome::RolledBack);
    assert!(e.business_committed && p.business_enqueues.load(Ordering::SeqCst) == 1);
    assert!(count(&p).await == 1 && stored_wall(&p).await == b + 40);
    no_material(&request);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
