use super::super::mutation::{complete_logout, enqueue_logout};
use super::*;
use crate::auth::{AuthScope, FailureKind, LeaseOwner};
fn session_scope(snapshot: Snapshot) -> (LeaseOwner, AuthScope) {
    let owner = LeaseOwner::new(authority(30000)).unwrap();
    let scope = AuthScope::bind(
        snapshot.into_verified_identity().unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    (owner, scope)
}
fn slot(lease: &crate::auth::Lease) -> (bool, bool, bool, bool) {
    super::super::delivery::test_slot_status(lease).unwrap()
}
async fn row_count(pool: &sqlite::Pool) -> i64 {
    let tx = sqlite::begin(pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Count>(
        &tx,
        sqlite::literal("SELECT count(*) AS n FROM __nagi_session_rows"),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    n
}
// The base fixture has a real queue of four. Pause only the test scheduling
// after actual metadata/row reads; then occupy all permits and observe the
// production reserve future waiting. No business SQL or clock is substituted.
async fn hold_queue_until_reserve<'a, F: std::future::Future>(
    tx: &'a sqlite::Tx,
    mut attempt: std::pin::Pin<&mut F>,
    reached: tokio::sync::oneshot::Receiver<()>,
    resume: tokio::sync::oneshot::Sender<()>,
) -> Vec<sqlite::ExecReservation<'a>> {
    tokio::select! { result = reached => result.unwrap(), _ = &mut attempt => panic!("attempt ended before reservation") }
    let mut holds = Vec::new();
    for _ in 0..4 {
        holds.push(tx.reserve_exec().await.unwrap());
    }
    resume.send(()).unwrap();
    assert!(futures_util::poll!(attempt.as_mut()).is_pending());
    holds
}
#[tokio::test]
async fn commit_logout_revokes_rotated_lineage_and_preserves_other_subject() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let old = store.insert(7, [1; 32], authority(90000)).await.unwrap();
    store.insert(8, [2; 32], authority(90000)).await.unwrap();
    store.rotate(&old, [4; 32]).await.unwrap();
    let (owner, scope) = session_scope(old);
    let intent = store.logout_delivery(scope).await.unwrap();
    assert!(row_count(&fixture.pool).await == 1);
    assert!(store.lookup([4; 32]).await.is_err());
    assert!(store.lookup([2; 32]).await.is_ok());
    let (_, admitted, ready, material) = slot(&owner.lease);
    assert!(!admitted && ready && material);
    drop(intent);
    let (_, admitted, ready, material) = slot(&owner.lease);
    assert!(!admitted && !ready && !material);
    fixture.stop().await;
}
#[tokio::test]
async fn real_reservation_wait_owner_end_enqueues_zero() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let (owner, scope) = session_scope(store.insert(7, [1; 32], authority(90000)).await.unwrap());
    let prepared = store.prepare_logout(scope).await.unwrap();
    let super::super::mutation::PreparedLogout {
        tx,
        mut context,
        mut progress,
    } = prepared;
    let (reached, resume) = context.pause_before_reserve();
    let result = {
        let attempt = enqueue_logout(&store, &tx, context, &mut progress);
        tokio::pin!(attempt);
        let hold = hold_queue_until_reserve(&tx, attempt.as_mut(), reached, resume).await;
        let before = tx.enqueued_execs();
        drop(owner);
        drop(hold);
        let result = attempt.await;
        assert!(result.is_err() && tx.enqueued_execs() == before);
        result
    };
    let failure = complete_logout(&store, tx, result, progress)
        .await
        .err()
        .unwrap();
    assert!(kind(&failure) == FailureKind::Expired);
    assert!(row_count(&fixture.pool).await == 1);
    fixture.stop().await;
}
#[tokio::test]
async fn drop_owned_pending_future_before_enqueue_discards_without_admission() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let (owner, scope) = session_scope(store.insert(7, [1; 32], authority(90000)).await.unwrap());
    let super::super::mutation::PreparedLogout {
        tx,
        mut context,
        mut progress,
    } = store.prepare_logout(scope).await.unwrap();
    let (reached, resume) = context.pause_before_reserve();
    {
        let attempt = enqueue_logout(&store, &tx, context, &mut progress);
        tokio::pin!(attempt);
        let hold = hold_queue_until_reserve(&tx, attempt.as_mut(), reached, resume).await;
        drop(attempt);
        drop(hold);
    }
    assert!(tx.enqueued_execs() == 0);
    let (vacant, admitted, ready, material) = slot(&owner.lease);
    assert!(vacant && !admitted && !ready && !material);
    sqlite::commit(tx).await.unwrap();
    assert!(row_count(&fixture.pool).await == 1);
    fixture.stop().await;
}
#[tokio::test]
async fn fresh_clock_after_queue_wait_rejects_immutable_lookup_expiry() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let (owner, scope) = session_scope(store.insert(7, [1; 32], authority(90000)).await.unwrap());
    let super::super::mutation::PreparedLogout {
        tx,
        mut context,
        mut progress,
    } = store.prepare_logout(scope).await.unwrap();
    let (reached, resume) = context.pause_before_reserve();
    let result = {
        let attempt = enqueue_logout(&store, &tx, context, &mut progress);
        tokio::pin!(attempt);
        let hold = hold_queue_until_reserve(&tx, attempt.as_mut(), reached, resume).await;
        fixture.wall.store(20000, Ordering::SeqCst);
        drop(hold);
        let result = attempt.await;
        assert!(result.is_err() && tx.enqueued_execs() == 0);
        result
    };
    let error = complete_logout(&store, tx, result, progress)
        .await
        .err()
        .unwrap();
    assert!(kind(&error) == FailureKind::Expired);
    assert!(!slot(&owner.lease).3);
    assert!(row_count(&fixture.pool).await == 1);
    fixture.stop().await;
}
#[tokio::test]
async fn same_tx_wall_high_water_beats_stale_transaction_snapshot() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let (owner, scope) = session_scope(store.insert(7, [1; 32], authority(90000)).await.unwrap());
    let super::super::mutation::PreparedLogout {
        tx,
        context,
        mut progress,
    } = store.prepare_logout(scope).await.unwrap();
    sqlite::exec(
        &tx,
        sqlite::literal("UPDATE __nagi_session_meta SET last_wall=1300 WHERE id=1"),
        sqlite::parameters(),
    )
    .await
    .unwrap();
    fixture.wall.store(1100, Ordering::SeqCst);
    let result = enqueue_logout(&store, &tx, context, &mut progress).await;
    assert!(result.is_err() && tx.enqueued_execs() == 0);
    let error = complete_logout(&store, tx, result, progress)
        .await
        .err()
        .unwrap();
    assert!(kind(&error) == FailureKind::Unavailable);
    let check = sqlite::begin(&fixture.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let metadata = super::super::metadata(&check).await.unwrap();
    assert!(metadata.last_wall == 1300);
    sqlite::rollback(check).await.unwrap();
    assert!(!slot(&owner.lease).3);
    fixture.stop().await;
}
#[tokio::test]
async fn actual_enqueue_is_admitted_before_reply_and_drop_is_terminal() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let (owner, scope) = session_scope(store.insert(7, [1; 32], authority(90000)).await.unwrap());
    let super::super::mutation::PreparedLogout {
        tx,
        context,
        mut progress,
    } = store.prepare_logout(scope).await.unwrap();
    let submitted = enqueue_logout(&store, &tx, context, &mut progress)
        .await
        .unwrap();
    assert!(tx.enqueued_execs() == 1);
    let (_, admitted, ready, material) = slot(&owner.lease);
    assert!(admitted && !ready && material);
    assert!(submitted.reply.await.unwrap() == submitted.expected);
    drop(submitted.admitted);
    sqlite::rollback(tx).await.unwrap();
    let (vacant, admitted, ready, material) = slot(&owner.lease);
    assert!(!vacant && !admitted && !ready && !material);
    assert!(row_count(&fixture.pool).await == 1);
    fixture.stop().await;
}
#[tokio::test]
async fn owner_end_after_enqueue_allows_commit_but_never_intent() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let (owner, scope) = session_scope(store.insert(7, [1; 32], authority(90000)).await.unwrap());
    let super::super::mutation::PreparedLogout {
        tx,
        context,
        mut progress,
    } = store.prepare_logout(scope).await.unwrap();
    let submitted = enqueue_logout(&store, &tx, context, &mut progress)
        .await
        .unwrap();
    assert!(tx.enqueued_execs() == 1);
    drop(owner);
    let error = complete_logout(&store, tx, Ok(submitted), progress)
        .await
        .err()
        .unwrap();
    assert!(kind(&error) == FailureKind::Expired && outcome(&error) == sqlite::Outcome::Committed);
    assert!(row_count(&fixture.pool).await == 0);
    fixture.stop().await;
}
#[tokio::test]
async fn foreign_slot_rejects_before_actual_send() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let (owner, scope) = session_scope(store.insert(7, [1; 32], authority(90000)).await.unwrap());
    let super::super::mutation::PreparedLogout {
        tx,
        mut context,
        mut progress,
    } = store.prepare_logout(scope).await.unwrap();
    let foreign = LeaseOwner::new(authority(30000)).unwrap();
    let material = secrets::CookiePolicy::new("__Host-example", cookie::SameSite::Strict)
        .unwrap_or_else(|_| panic!("owned policy"))
        .remove()
        .unwrap_or_else(|_| panic!("owned material"));
    context.replace_pending_for_test(
        foreign
            .checked_delivery(&foreign.lease)
            .unwrap()
            .stage(material)
            .unwrap(),
    );
    let result = enqueue_logout(&store, &tx, context, &mut progress).await;
    assert!(result.is_err() && tx.enqueued_execs() == 0);
    assert!(complete_logout(&store, tx, result, progress).await.is_err());
    assert!(!slot(&owner.lease).3 && !slot(&foreign.lease).3);
    fixture.stop().await;
}
#[tokio::test]
async fn unknown_or_committed_cleanup_error_never_confirms_delivery() {
    for fault in [
        sqlite::LogoutTestFault::Unknown,
        sqlite::LogoutTestFault::CommittedCleanup,
    ] {
        let fixture = Fixture::new().await;
        let mut store = fixture.store().await;
        let (owner, scope) =
            session_scope(store.insert(7, [1; 32], authority(90000)).await.unwrap());
        let fault_pool = sqlite::logout_test_pool(fixture.directory.join("owned.sqlite"), fault);
        store.pool = sqlite::clone_pool(&fault_pool);
        let error = store.logout_delivery(scope).await.err().unwrap();
        assert!(kind(&error) == FailureKind::Unavailable);
        assert!(
            outcome(&error)
                == match fault {
                    sqlite::LogoutTestFault::Unknown => sqlite::Outcome::Unknown,
                    sqlite::LogoutTestFault::CommittedCleanup => sqlite::Outcome::Committed,
                }
        );
        let (vacant, admitted, ready, material) = slot(&owner.lease);
        assert!(!vacant && !admitted && !ready && !material);
        assert!(sqlite::close(&fault_pool, 2000).await.is_err());
        fixture.stop().await;
    }
}
#[tokio::test]
async fn bearer_source_is_rejected_before_slot_or_session_transaction() {
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    let owner = LeaseOwner::new(authority(30000)).unwrap();
    let scope = AuthScope::bind(
        crate::auth::VerifiedIdentity::from_verified(7, authority(30000)).unwrap(),
        Arc::clone(&owner.lease),
    )
    .unwrap();
    let error = store.logout_delivery(scope).await.err().unwrap();
    assert!(kind(&error) == FailureKind::Denied);
    let (vacant, admitted, ready, material) = slot(&owner.lease);
    assert!(vacant && !admitted && !ready && !material);
    fixture.stop().await;
}
