//! Small owned SQLite/actual owner oracles. Secret bytes are never assertion output.
use super::*;
use crate::auth::session::CsrfBinding;
use crate::{
    auth::{AuthScope, VerifiedIdentity},
    http_server as http,
};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    dir: std::path::PathBuf,
    pool: sqlite::Pool,
    store: session::Store,
    inner: Inner,
    cookie: hyper::HeaderMap,
}
fn checked() -> http::Options {
    http::authority(
        http::options(256, 1000, 1000, 1000).unwrap(),
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap()
}
fn owner() -> LeaseOwner {
    LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap()
}
impl Fixture {
    async fn new(count: i64, bytes: i64) -> Self {
        Self::with_limits(count, bytes, 10000, 100000, 1000).await
    }
    async fn with_limits(count: i64, bytes: i64, idle: i64, absolute: i64, ttl: i64) -> Self {
        let dir = loop {
            let p = std::env::temp_dir().join(format!(
                "b1-native-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&p) {
                Ok(()) => break p,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("owned directory unavailable"),
            }
        };
        let pool = sqlite::open(
            dir.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(1, 2, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let store = session::open(
            &pool,
            session::options(4, 5, 72, 1, idle, absolute, idle / 2, 2).unwrap(),
            session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        )
        .await
        .unwrap();
        let data = open(
            &store,
            options(count, bytes, 1, ttl, 2).unwrap(),
            &checked(),
        )
        .await
        .unwrap();
        let inner = match Arc::try_unwrap(data.inner) {
            Ok(i) => i,
            Err(_) => panic!("unexpected data handle copy"),
        };
        let lease = owner();
        let scope = AuthScope::bind(
            VerifiedIdentity::from_verified(7, Instant::now() + Duration::from_secs(60)).unwrap(),
            Arc::clone(&lease.lease),
        )
        .unwrap();
        let intent = session::issue(&store, scope).await.unwrap();
        let response = session::apply(http::empty(http::Status::OK), intent).unwrap();
        let (status, headers) = http::session_delivery_headers_for_test(response, &lease);
        assert!(status.as_u16() == 200);
        let value = headers
            .get(hyper::header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap();
        let mut cookie = hyper::HeaderMap::new();
        cookie.insert(
            hyper::header::COOKIE,
            hyper::header::HeaderValue::from_str(value).unwrap(),
        );
        Self {
            dir,
            pool,
            store,
            inner,
            cookie,
        }
    }
    async fn binding(&self, lease: &Arc<Lease>) -> CsrfBinding {
        let identity = self.store.lookup_http_identity(&self.cookie).await.unwrap();
        let b = CsrfBinding::from_identity(&identity, lease).unwrap();
        let _scope = AuthScope::bind(identity, Arc::clone(lease)).unwrap();
        b
    }
    async fn issue(&self) -> (TokenOwner, CsrfBinding, [u8; 32], i64) {
        let o = TokenOwner::new(owner());
        let b = self.binding(&o.lease.lease).await;
        self.inner.issue(&o, &b).await.unwrap();
        let c = o.cell.lock().unwrap();
        let token = c.token.as_ref().unwrap();
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            "x-csrf-token",
            hyper::header::HeaderValue::from_str(token).unwrap(),
        );
        let digest = token_digest(&headers).unwrap();
        let expiry = c.expiry.unwrap();
        drop(c);
        (o, b, digest, expiry)
    }
    // Establish the real binding and both persisted clock premises before
    // fixing UTC. Setting UTC before lookup would itself create rollback.
    async fn issue_at_tracked_wall(&mut self) -> (TokenOwner, CsrfBinding, [u8; 32], i64) {
        let o = TokenOwner::new(owner());
        let b = self.binding(&o.lease.lease).await;
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .expect("expiry fixture premise: begin");
        let mut wall = None;
        self.inner
            .check(&tx, &mut wall)
            .await
            .expect("expiry fixture premise: existing Session and CSRF clocks");
        sqlite::rollback(tx)
            .await
            .expect("expiry fixture premise: rollback");
        let wall = wall.expect("expiry fixture premise: tracked UTC");
        self.inner.context.set_test_clock_sequence(vec![wall; 16]);
        self.inner
            .issue(&o, &b)
            .await
            .expect("expiry fixture premise: original one-second issue");
        let c = o.cell.lock().unwrap();
        let token = c.token.as_ref().unwrap();
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            "x-csrf-token",
            hyper::header::HeaderValue::from_str(token).unwrap(),
        );
        let digest = token_digest(&headers).unwrap();
        let expiry = c.expiry.unwrap();
        assert_eq!(expiry, wall.checked_add(1000).unwrap());
        drop(c);
        (o, b, digest, expiry)
    }
    async fn count(&self) -> i64 {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        let n = sqlite::query::<Count>(
            &tx,
            sqlite::literal("SELECT count(*) AS n FROM __nagi_csrf_tokens"),
            sqlite::parameters(),
        )
        .await
        .unwrap()
        .unwrap()
        .0;
        sqlite::rollback(tx).await.unwrap();
        n
    }
    async fn assert_original_token_row(&self, digest: [u8; 32], expiry: i64) {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        let rows = sqlite::all::<TokenRow>(
            &tx,
            sqlite::literal("SELECT digest,expiry_ms FROM __nagi_csrf_tokens"),
            sqlite::parameters(),
        )
        .await
        .unwrap();
        assert!(
            rows.len() == 1 && rows[0].digest.as_slice() == digest && rows[0].expiry_ms == expiry,
            "same_queue_fixture phase=OriginalRowUnchanged",
        );
        sqlite::rollback(tx).await.unwrap();
    }
    async fn persisted_walls(&self) -> (i64, i64) {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        let session_wall = sqlite::query::<Count>(
            &tx,
            sqlite::literal("SELECT last_wall AS n FROM __nagi_session_meta WHERE id=1"),
            sqlite::parameters(),
        )
        .await
        .unwrap()
        .unwrap()
        .0;
        let csrf_wall = metadata(&tx).await.unwrap().last_wall;
        sqlite::rollback(tx).await.unwrap();
        (session_wall, csrf_wall)
    }
    async fn stop(self) {
        sqlite::close(&self.pool, 2000).await.unwrap();
        std::fs::remove_dir_all(self.dir).unwrap();
    }
}

#[tokio::test]
async fn same_queue_reopen_reuses_data_context_without_refreshing_token_expiry() {
    #[cfg(all(test, nagi_auth_stage_diag))]
    let observer = crate::auth::stage_observer::Guard::new(1);
    let mut f = Fixture::new(2, 176).await;
    #[cfg(all(test, nagi_auth_stage_diag))]
    observer.attach(&f.inner.stage_slot);
    assert!(f.inner.native.same_private_queue(&f.pool));
    // Startup is real and completes before issue: this branch proves compatible
    // reopened-context acceptance while UTC is controlled, not real-wall speed.
    let reopened = session::open(
        &f.pool,
        session::options(4, 5, 72, 1, 10000, 100000, 5000, 2).unwrap(),
        session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
    )
    .await
    .unwrap();
    let data = open(&reopened, options(2, 176, 1, 1000, 2).unwrap(), &checked())
        .await
        .unwrap();
    assert!(
        data.matches_store(&f.store),
        "same_queue_fixture phase=CompatibleContext"
    );
    let mut inner = match Arc::try_unwrap(data.inner) {
        Ok(inner) => inner,
        Err(_) => panic!("same_queue_fixture phase=UnsharedReopenedContext"),
    };
    assert!(
        inner.native.same_private_queue(&f.pool),
        "same_queue_fixture phase=SameQueue"
    );
    #[cfg(all(test, nagi_auth_stage_diag))]
    observer.attach(&inner.stage_slot);
    let o = TokenOwner::new(owner());
    let b = f.binding(&o.lease.lease).await;
    // No further Session lookup/request may advance persisted wall after W.
    let (session_wall, csrf_wall) = f.persisted_walls().await;
    let sampled = f.inner.context.sample().unwrap();
    let wall = sampled.ceil_ms;
    assert!(
        wall >= session_wall && wall >= csrf_wall,
        "same_queue_fixture phase=WallPremise"
    );
    assert!(
        b.check_clock(&sampled, &o.lease.lease).is_ok() && o.lease.lease.validate().is_ok(),
        "same_queue_fixture phase=InitialAuthorityLive"
    );
    f.inner.context.set_test_clock_sequence(vec![wall; 16]);
    inner.context.set_test_clock_sequence(vec![wall; 16]);
    f.inner.issue(&o, &b).await.unwrap();
    let (digest, expiry, deadline) = {
        let cell = o.cell.lock().unwrap();
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            "x-csrf-token",
            hyper::header::HeaderValue::from_str(cell.token.as_ref().unwrap()).unwrap(),
        );
        (
            token_digest(&headers).unwrap(),
            cell.expiry.unwrap(),
            cell.deadline.unwrap(),
        )
    };
    assert!(
        expiry == wall.checked_add(1000).unwrap(),
        "same_queue_fixture phase=OriginalOneSecondTTL"
    );
    let probe = Arc::new(same_queue_probe::Probe::new(expiry));
    inner.checkpoint_probe = Some(Arc::clone(&probe));
    let result = inner.checkpoint(&o.lease.lease, &b, digest).await;
    if let Err(error) = &result {
        let observed = probe.failure();
        eprintln!(
            "same_queue_checkpoint_diag phase={:?} refusal={:?} matched={:?} original_expiry_same={:?} sample_last_expired={:?} lease_last_valid={:?} actual_expired={:?} kind={:?} outcome={:?}",
            observed.phase, observed.refusal, observed.matched, observed.original_expiry,
            observed.sample_expired, observed.lease_valid, observed.actual_expired,
            kind(error), outcome(error),
        );
    }
    assert!(
        result.is_ok(),
        "same_queue_fixture phase=PositiveCheckpoint kind={:?} outcome={:?}",
        result.as_ref().err().map(kind),
        result.as_ref().err().map(outcome)
    );
    f.assert_original_token_row(digest, expiry).await;
    assert!(
        {
            let cell = o.cell.lock().unwrap();
            cell.expiry == Some(expiry) && cell.deadline == Some(deadline)
        },
        "same_queue_fixture phase=PositiveOriginalDeadline"
    );
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    let sampled = f.inner.context.sample().unwrap();
    assert!(
        sampled.ceil_ms == wall && wall < expiry && Instant::now() >= deadline,
        "same_queue_fixture phase=OriginalDeliveryDeadlineReached"
    );
    assert!(
        b.check_clock(&sampled, &o.lease.lease).is_ok() && o.lease.lease.validate().is_ok(),
        "same_queue_fixture phase=PreDeliveryAuthorityLive"
    );
    assert!(
        {
            let cell = o.cell.lock().unwrap();
            cell.expiry == Some(expiry) && cell.deadline == Some(deadline)
        },
        "same_queue_fixture phase=DeliveryOriginalDeadline"
    );
    let seal = match o.seal() {
        Ok(seal) => seal,
        Err(_) => panic!("same_queue_fixture phase=SealBeforeDelivery"),
    };
    let result = f.inner.take_wire(&o, seal, &b);
    assert!(
        result.is_err_and(|error| kind(&error) == FailureKind::Denied),
        "same_queue_fixture phase=OriginalDeliveryDeadlineDenied"
    );
    // take_wire refusal itself does not promise automatic material retirement.
    o.retire();
    assert!(
        o.cell.lock().unwrap().token.is_none(),
        "same_queue_fixture phase=MaterialRetired"
    );
    let sampled = f.inner.context.sample().unwrap();
    assert!(
        b.check_clock(&sampled, &o.lease.lease).is_ok() && o.lease.lease.validate().is_ok(),
        "same_queue_fixture phase=PostDeliveryAuthorityLive"
    );
    f.assert_original_token_row(digest, expiry).await;
    drop(o);
    f.stop().await;
    #[cfg(all(test, nagi_auth_stage_diag))]
    observer.emit();
}

#[tokio::test]
async fn reopened_context_preserves_original_token_expiry_and_rejects_after_deadline() {
    let f = Fixture::new(2, 176).await;
    assert!(f.inner.native.same_private_queue(&f.pool));
    // This fresh fixture keeps the original real-clock issue -> reopen order.
    let (o, b, digest, expiry) = f.issue().await;
    let deadline = o.cell.lock().unwrap().deadline.unwrap();
    f.assert_original_token_row(digest, expiry).await;
    let reopened = session::open(
        &f.pool,
        session::options(4, 5, 72, 1, 10000, 100000, 5000, 2).unwrap(),
        session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
    )
    .await
    .unwrap();
    let data = open(&reopened, options(2, 176, 1, 1000, 2).unwrap(), &checked())
        .await
        .unwrap();
    assert!(
        data.matches_store(&f.store),
        "same_queue_fixture phase=RealCompatibleContext"
    );
    assert!(
        data.inner.native.same_private_queue(&f.pool),
        "same_queue_fixture phase=RealSameQueue"
    );
    // Opening must not refresh an existing row, even if startup exceeds its TTL.
    f.assert_original_token_row(digest, expiry).await;
    assert!(
        {
            let cell = o.cell.lock().unwrap();
            cell.expiry == Some(expiry) && cell.deadline == Some(deadline)
        },
        "same_queue_fixture phase=ReopenOriginalDeadline"
    );
    tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    let sampled = data.inner.context.sample().unwrap();
    assert!(
        sampled.ceil_ms >= expiry && Instant::now() >= deadline,
        "same_queue_fixture phase=RealOriginalExpiryReached"
    );
    assert!(
        b.check_clock(&sampled, &o.lease.lease).is_ok() && o.lease.lease.validate().is_ok(),
        "same_queue_fixture phase=PreExpiredCheckpointAuthorityLive"
    );
    let result = data.inner.checkpoint(&o.lease.lease, &b, digest).await;
    assert!(
        result.is_err_and(|error| kind(&error) == FailureKind::Denied
            && outcome(&error) == sqlite::Outcome::RolledBack),
        "same_queue_fixture phase=ExpiredCheckpointDeniedRolledBack"
    );
    let sampled = data.inner.context.sample().unwrap();
    assert!(
        b.check_clock(&sampled, &o.lease.lease).is_ok() && o.lease.lease.validate().is_ok(),
        "same_queue_fixture phase=PostExpiredCheckpointAuthorityLive"
    );
    assert!(
        f.count().await == 0,
        "same_queue_fixture phase=ExpiredRowCleaned"
    );
    drop(o);
    f.stop().await;
}
#[tokio::test]
async fn bytes_limit_is_independent_and_expired_cleanup_spends_one_batch() {
    let mut f = Fixture::new(2, 88).await;
    let o = TokenOwner::new(owner());
    let first_binding = f.binding(&o.lease.lease).await;
    let other = TokenOwner::new(owner());
    let b = f.binding(&other.lease.lease).await;
    // Finish both real Session lookups before fixing CSRF's private UTC samples:
    // Session's persisted wall cannot advance past the controlled capacity premise.
    let live_wall = f.inner.context.sample().unwrap().ceil_ms;
    f.inner.context.set_test_clock_sequence(vec![live_wall; 16]);
    f.inner.issue(&o, &first_binding).await.unwrap();
    let original_expiry = o.cell.lock().unwrap().expiry.unwrap();
    assert!(live_wall < original_expiry);
    let before = f.inner.insert_enqueues.load(Ordering::SeqCst);
    assert!(f.inner.issue(&other, &b).await.is_err());
    assert!(f.count().await == 1 && other.cell.lock().unwrap().token.is_none());
    assert!(f.inner.insert_enqueues.load(Ordering::SeqCst) == before);
    assert!(o.cell.lock().unwrap().expiry == Some(original_expiry));
    drop(o);
    drop(other);
    f.stop().await;
    let f = Fixture::new(4, 352).await;
    let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    for n in 1u8..=3 {
        let p = sqlite::bind_bytes(sqlite::parameters(), vec![n; 32]);
        let p = f.inner.context.bind_incarnation(p);
        sqlite::exec(&tx,sqlite::literal("INSERT INTO __nagi_csrf_tokens(digest,session_incarnation,lineage,generation,expiry_ms) VALUES(?,?,1,0,1)"),p).await.unwrap();
    }
    sqlite::commit(tx).await.unwrap();
    let (o, _, _, _) = f.issue().await;
    assert!(f.count().await == 3);
    drop(o);
    f.stop().await;
}
#[tokio::test]
async fn expired_token_reclaims_byte_capacity_without_relaxing_live_token_cap() {
    let mut f = Fixture::new(2, 88).await;
    let first = TokenOwner::new(owner());
    let first_binding = f.binding(&first.lease.lease).await;
    let second = TokenOwner::new(owner());
    let second_binding = f.binding(&second.lease.lease).await;
    let live_wall = f.inner.context.sample().unwrap().ceil_ms;
    f.inner.context.set_test_clock_sequence(vec![live_wall; 16]);
    f.inner.issue(&first, &first_binding).await.unwrap();
    let expiry = first.cell.lock().unwrap().expiry.unwrap();
    assert!(live_wall < expiry);
    let original_digest = {
        let cell = first.cell.lock().unwrap();
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            "x-csrf-token",
            hyper::header::HeaderValue::from_str(cell.token.as_ref().unwrap()).unwrap(),
        );
        token_digest(&headers).unwrap()
    };
    let before = f.inner.insert_enqueues.load(Ordering::SeqCst);
    // At the original expiry, one cleanup batch frees the one-row byte cap.
    f.inner.context.set_test_clock_sequence(vec![expiry; 16]);
    f.inner.issue(&second, &second_binding).await.unwrap();
    assert!(f.count().await == 1);
    assert!(f.inner.insert_enqueues.load(Ordering::SeqCst) == before + 1);
    assert!(second.cell.lock().unwrap().token.is_some());
    let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let p = sqlite::bind_bytes(sqlite::parameters(), original_digest.to_vec());
    let old_rows = sqlite::query::<Count>(
        &tx,
        sqlite::literal("SELECT count(*) AS n FROM __nagi_csrf_tokens WHERE digest=?"),
        p,
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    assert!(old_rows == 0);
    drop(first);
    drop(second);
    f.stop().await;
}

#[tokio::test]
async fn checkpoint_accepts_before_and_refuses_at_or_after_original_token_expiry() {
    // Final comparison after a real commit/restore. Deliberately project from
    // a past monotonic sample: using real now instead of the paired comparison
    // makes the positive oracle fail, independent of machine speed.
    for (delta, mono_delta) in [(-1i64, -1i64), (0, -1), (1, -1), (-500, 0), (-500, 1)] {
        let mut f = Fixture::new(2, 176).await;
        let (o, b, digest, expiry) = f.issue_at_tracked_wall().await;
        let probe = Arc::new(same_queue_probe::Probe::new(expiry));
        f.inner.checkpoint_probe = Some(Arc::clone(&probe));
        let sampled = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
        let deadline = sampled + Duration::from_millis(500);
        let comparison = if mono_delta < 0 {
            deadline.checked_sub(Duration::from_nanos(1)).unwrap()
        } else {
            deadline + Duration::from_nanos(mono_delta as u64)
        };
        assert_eq!(comparison.cmp(&deadline), mono_delta.cmp(&0));
        let post = sampled + Duration::from_nanos(1);
        let restore = sampled + Duration::from_nanos(2);
        let final_sample = sampled + Duration::from_nanos(3);
        assert!(sampled < post && post < restore && restore < final_sample);
        assert!(final_sample < deadline);
        assert!(
            deadline < Instant::now(),
            "paired-clock calibration premise"
        );
        f.inner.checkpoint_comparison_now = Some(comparison);
        // Later samples advance by nanoseconds so reprojecting D after the
        // real commit/restore cannot silently reproduce the original D.
        let values = vec![
            (sampled, expiry - 500),
            (sampled, expiry - 500),
            (sampled, expiry - 500),
            (post, expiry - 500),
            (restore, expiry - 500),
            (final_sample, expiry + delta),
        ];
        let consumed = f.inner.context.set_test_paired_clock_sequence(values);
        let result = f.inner.checkpoint(&o.lease.lease, &b, digest).await;
        assert_eq!(consumed.load(Ordering::SeqCst), 6);
        if let Err(error) = &result {
            let observed = probe.failure();
            eprintln!(
                "csrf_expiry_boundary_diag delta={} phase={:?} refusal={:?} matched={:?} original_expiry_same={:?} sample_last_expired={:?} lease_last_valid={:?} actual_expired={:?} kind={:?} outcome={:?}",
                delta,
                observed.phase,
                observed.refusal,
                observed.matched,
                observed.original_expiry,
                observed.sample_expired,
                observed.lease_valid,
                observed.actual_expired,
                kind(error),
                outcome(error),
            );
        }
        if delta == -1 && mono_delta == -1 {
            assert!(
                result.is_ok(),
                "csrf_checkpoint_boundary phase=PositiveCheckpoint kind={:?} outcome={:?}",
                result.as_ref().err().map(kind),
                result.as_ref().err().map(outcome),
            );
        } else {
            assert!(result
                .is_err_and(|e| kind(&e) == FailureKind::Denied
                    && outcome(&e) == sqlite::Outcome::Committed));
        }
        let observed = probe.failure();
        assert!(observed.matched == Some(true));
        assert!(observed.original_expiry == Some(true));
        assert!(observed.lease_valid == Some(true));
        assert!(matches!(
            observed.phase,
            same_queue_probe::Phase::Done | same_queue_probe::Phase::FinalExpiry
        ));
        assert!(f.count().await == 1);
        f.assert_original_token_row(digest, expiry).await;
        drop(o);
        f.stop().await;
    }
    // Keep the original early-expiry cleanup path: refusal precedes a token
    // match and expired rows are removed, unlike the post-commit matrix above.
    for delta in [0i64, 1] {
        let mut f = Fixture::new(2, 176).await;
        let (o, b, digest, expiry) = f.issue_at_tracked_wall().await;
        f.inner
            .context
            .set_test_clock_sequence(vec![expiry + delta; 16]);
        let result = f.inner.checkpoint(&o.lease.lease, &b, digest).await;
        assert!(result.is_err_and(|e| kind(&e) == FailureKind::Denied));
        assert!(f.count().await == 0);
        drop(o);
        f.stop().await;
    }
}
#[tokio::test]
async fn owner_only_seal_is_destructive_foreign_and_replay_are_refused() {
    let f = Fixture::new(2, 176).await;
    let (o, b, _, _) = f.issue().await;
    let seal = o.seal().unwrap();
    let foreign = TokenOwner::new(owner());
    let fake = TokenSeal {
        request: seal.request,
        cell: seal.cell.clone(),
        context: seal.context,
    };
    assert!(f.inner.take_wire(&foreign, fake, &b).is_err());
    let replay = TokenSeal {
        request: seal.request,
        cell: seal.cell.clone(),
        context: seal.context,
    };
    let body = f.inner.take_wire(&o, seal, &b).unwrap();
    assert!(body.len() == 60 && o.cell.lock().unwrap().token.is_none());
    assert!(f.inner.take_wire(&o, replay, &b).is_err());
    assert!(f.count().await == 1);
    drop(o);
    drop(foreign);
    f.stop().await;
}
#[tokio::test]
async fn unpolled_and_dropped_pending_issue_do_not_deliver_and_native_is_joined() {
    let f = Fixture::new(2, 176).await;
    let o = TokenOwner::new(owner());
    let b = f.binding(&o.lease.lease).await;
    drop(f.inner.issue(&o, &b));
    assert!(o.cell.lock().unwrap().token.is_none() && f.count().await == 0);
    let held = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let mut pending = Box::pin(f.inner.issue(&o, &b));
    assert!(futures_util::poll!(&mut pending).is_pending());
    assert!(o.cell.lock().unwrap().token.is_some());
    drop(pending);
    assert!(o.cell.lock().unwrap().token.is_none());
    sqlite::rollback(held).await.unwrap();
    drop(o);
    f.stop().await;
}
#[tokio::test]
async fn actual_unknown_and_committed_cleanup_never_make_ready() {
    for fault in [
        sqlite::LogoutTestFault::Unknown,
        sqlite::LogoutTestFault::CommittedCleanup,
    ] {
        let mut f = Fixture::new(2, 176).await;
        let o = TokenOwner::new(owner());
        let b = f.binding(&o.lease.lease).await;
        let broken = sqlite::logout_test_pool(f.dir.join("owned.sqlite"), fault);
        let original = std::mem::replace(&mut f.inner.native, broken);
        drop(original);
        let e = f
            .inner
            .issue(&o, &b)
            .await
            .expect_err("failed native finish made Ready");
        let expected = match fault {
            sqlite::LogoutTestFault::Unknown => sqlite::Outcome::Unknown,
            sqlite::LogoutTestFault::CommittedCleanup => sqlite::Outcome::Committed,
        };
        assert!(
            outcome(&e) == expected && o.cell.lock().unwrap().token.is_none() && o.seal().is_err()
        );
        let _ = sqlite::close(&f.inner.native, 2000).await;
        drop(o);
        f.stop().await;
    }
}

#[tokio::test]
async fn actual_reserve_wait_rechecks_live_gate_before_any_business_enqueue() {
    let mut f = Fixture::new(2, 176).await;
    let wait = Arc::new(ReserveWait::new());
    f.inner.reserve_wait = Some(Arc::clone(&wait));
    let observer = sqlite::open(
        f.dir.join("owned.sqlite").to_str().unwrap(),
        sqlite::options(1, 2, 1000, 40).unwrap(),
    )
    .await
    .unwrap();
    let o = TokenOwner::new(owner());
    let b = f.binding(&o.lease.lease).await;
    let mut operation = Box::pin(f.inner.issue(&o, &b));
    tokio::time::timeout(Duration::from_secs(2),async { tokio::select! { _=wait.arrived.notified()=>{},_= &mut operation=>panic!("issue ended before actual reserve wait") } }).await.unwrap();
    assert!(
        f.inner.insert_enqueues.load(Ordering::SeqCst) == 0
            && o.cell.lock().unwrap().token.is_some()
    );
    // A second actual worker cannot acquire SQLite's Immediate writer while
    // the binding-check transaction owns it across the proven reserve wait.
    let contender = tokio::time::timeout(
        Duration::from_secs(1),
        sqlite::begin(&observer, sqlite::BeginMode::Immediate),
    )
    .await
    .unwrap();
    assert!(contender.is_err());
    let until = Instant::now() + Duration::from_millis(10);
    constrain(&o.lease.lease, until).unwrap();
    tokio::time::sleep_until(tokio::time::Instant::from_std(until)).await;
    assert!(o.lease.lease.validate().is_err());
    wait.release.notify_one();
    let error = tokio::time::timeout(Duration::from_secs(2), &mut operation)
        .await
        .unwrap()
        .expect_err("expired gate made Ready");
    assert!(
        error.http_status() == http::Status::GATEWAY_TIMEOUT
            && f.inner.insert_enqueues.load(Ordering::SeqCst) == 0
    );
    assert!(o.cell.lock().unwrap().token.is_none());
    drop(operation);
    assert!(f.count().await == 0);
    let released = sqlite::begin(&observer, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let changed = sqlite::exec(
        &released,
        sqlite::literal("UPDATE __nagi_session_rows SET generation=generation+1"),
        sqlite::parameters(),
    )
    .await
    .unwrap();
    assert!(changed == 1);
    sqlite::commit(released).await.unwrap();
    sqlite::close(&observer, 2000).await.unwrap();
    drop(o);
    f.stop().await;
}

#[tokio::test]
async fn partial_schema_and_wrong_actual_source_are_refused_without_adoption() {
    let mut f = Fixture::new(2, 176).await;
    let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    sqlite::exec(
        &tx,
        sqlite::literal("CREATE TABLE __nagi_csrf_extra(n INTEGER) STRICT"),
        sqlite::parameters(),
    )
    .await
    .unwrap();
    sqlite::commit(tx).await.unwrap();
    let e = open(&f.store, options(2, 176, 1, 1000, 2).unwrap(), &checked())
        .await
        .expect_err("extra schema adopted");
    assert!(kind(&e) == FailureKind::Unavailable && outcome(&e) == sqlite::Outcome::RolledBack);
    let o = TokenOwner::new(owner());
    let b = f.binding(&o.lease.lease).await;
    let other = sqlite::open(
        f.dir.join("other.sqlite").to_str().unwrap(),
        sqlite::options(1, 2, 1000, 1000).unwrap(),
    )
    .await
    .unwrap();
    let original = std::mem::replace(&mut f.inner.native, sqlite::clone_pool(&other));
    drop(original);
    let e = f
        .inner
        .issue(&o, &b)
        .await
        .expect_err("foreign main admitted");
    assert!(kind(&e) == FailureKind::Unavailable && o.cell.lock().unwrap().token.is_none());
    sqlite::close(&other, 2000).await.unwrap();
    drop(o);
    f.stop().await;
}

#[tokio::test]
async fn aggregate_declared_budgets_are_checked_in_both_open_orders() {
    use crate::auth::prelogin;
    // Large declared integers only; three schemas and zero anonymous/token rows.
    let stored = (i64::MAX - 400) / 72;
    for prelogin_first in [false, true] {
        let f = Fixture::new(2, 176).await;
        if prelogin_first {
            let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
                .await
                .unwrap();
            for sql in [
                "DROP INDEX __nagi_csrf_expiry",
                "DROP TABLE __nagi_csrf_tokens",
                "DROP TABLE __nagi_csrf_meta",
            ] {
                sqlite::exec(&tx, sqlite::literal(sql), sqlite::parameters())
                    .await
                    .unwrap();
            }
            sqlite::commit(tx).await.unwrap();
        }
        let p = prelogin::open(
            &f.store,
            sqlite::options(1, 2, 1000, 1000).unwrap(),
            prelogin::options(1, stored, 72, 1, 1000, 2, 2, 1000, 2, 4).unwrap(),
            prelogin::cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
            &checked(),
        )
        .await;
        if prelogin_first {
            let p = p.unwrap();
            let e = open(&f.store, options(2, 176, 1, 1000, 2).unwrap(), &checked())
                .await
                .expect_err("overflow adopted B1");
            assert!(kind(&e) == FailureKind::InvalidRequest);
            let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
                .await
                .unwrap();
            assert!(schema(&tx).await.unwrap().is_empty());
            sqlite::rollback(tx).await.unwrap();
            prelogin::close(&p, 2000).await.unwrap();
        } else {
            assert!(p.is_err());
        }
        f.stop().await;
    }
}

#[tokio::test]
async fn cancelled_after_actual_business_commit_retires_material_and_preserves_row() {
    let mut f = Fixture::new(2, 176).await;
    let wait = Arc::new(ReserveWait::new());
    f.inner.postcommit_wait = Some(Arc::clone(&wait));
    let o = TokenOwner::new(owner());
    let b = f.binding(&o.lease.lease).await;
    let mut operation = Box::pin(f.inner.issue(&o, &b));
    tokio::time::timeout(Duration::from_secs(2),async { tokio::select! { _=wait.arrived.notified()=>{},_= &mut operation=>panic!("issue finished before postcommit wait") } }).await.unwrap();
    assert!(
        f.inner.insert_enqueues.load(Ordering::SeqCst) == 1
            && f.count().await == 1
            && o.cell.lock().unwrap().token.is_some()
    );
    drop(operation);
    assert!(o.cell.lock().unwrap().token.is_none() && o.seal().is_err());
    assert!(f.count().await == 1);
    drop(o);
    f.stop().await;
}
#[tokio::test]
async fn issuance_caps_original_absolute_and_checkpoint_keeps_original_idle_boundary() {
    let mut f = Fixture::with_limits(2, 176, 2000, 3000, 10000).await;
    let (o, b, digest, expiry) = f.issue().await;
    assert!(expiry == b.absolute_ms());
    let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let idle = sqlite::query::<Count>(
        &tx,
        sqlite::literal("SELECT idle_ms AS n FROM __nagi_session_rows"),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    f.inner.context.set_test_clock_sequence(vec![idle; 16]);
    let e = f
        .inner
        .checkpoint(&o.lease.lease, &b, digest)
        .await
        .expect_err("original idle equality accepted");
    assert!(kind(&e) == FailureKind::Denied && f.count().await == 1);
    drop(o);
    f.stop().await;
}
#[tokio::test]
async fn nondecreasing_clock_refusal_preserves_live_row_and_never_delivers() {
    let mut f = Fixture::new(2, 176).await;
    let (o, b, _, _) = f.issue().await;
    let wall = o.cell.lock().unwrap().wall.unwrap();
    let seal = o.seal().unwrap();
    f.inner.context.set_test_clock_sequence(vec![wall - 1; 16]);
    let e = match f.inner.take_wire(&o, seal, &b) {
        Err(e) => e,
        Ok(_) => panic!("backward clock delivered token"),
    };
    assert!(kind(&e) == FailureKind::Unavailable);
    o.retire();
    assert!(o.cell.lock().unwrap().token.is_none() && f.count().await == 1);
    drop(o);
    f.stop().await;
}

#[path = "fixed_wire_owner_tests.rs"]
mod fixed_wire_owner_tests;
