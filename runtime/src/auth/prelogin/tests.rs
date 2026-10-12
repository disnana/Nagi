//! Prospective real SQLite purpose oracles. No source/definition failure is RED.
use super::*;
use super::{close_native as close, open_bound as open};
use crate::auth::{LeaseOwner, VerifiedIdentity};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    dir: std::path::PathBuf,
    auth_pool: sqlite::Pool,
    store: session::Store,
}
impl Fixture {
    async fn new() -> Self {
        let dir = loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("sf03-owned-{}-{n}", std::process::id()));
            match std::fs::create_dir(&p) {
                Ok(()) => break p,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("owned fixture unavailable"),
            }
        };
        let auth_pool = sqlite::open(
            dir.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(1, 4, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        let store = session::open(
            &auth_pool,
            session::options(2, 3, 72, 1, 10000, 100000, 5000, 2).unwrap(),
            session::cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        )
        .await
        .unwrap();
        Self {
            dir,
            auth_pool,
            store,
        }
    }
    async fn open(&self, live: i64, stored: i64, bytes: i64) -> Result<Pool, Failure> {
        open(
            &self.store,
            sqlite::options(1, 2, 1000, 1000).unwrap(),
            options(live, stored, bytes, 1, 1000, 2, 8, 10000, 4, 16).unwrap(),
            cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
            "https://example.invalid",
        )
        .await
    }
    async fn stop(self) {
        sqlite::close(&self.auth_pool, 2000).await.unwrap();
        std::fs::remove_dir_all(self.dir).unwrap();
    }
}
fn owner() -> RequestOwner {
    RequestOwner::new(LeaseOwner::new(Instant::now() + Duration::from_secs(10)).unwrap())
}
fn presented(owner: &RequestOwner) -> ClaimInput {
    let c = owner.cell.lock().unwrap();
    let pair = c.pair.as_ref().unwrap();
    ClaimInput::parse(&pair.handle, &pair.token).unwrap()
}
async fn count(pool: &Pool) -> i64 {
    let tx = sqlite::begin(&pool.native, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Count>(
        &tx,
        sqlite::literal("SELECT count(*) AS n FROM __nagi_prelogin_ready"),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    n
}
#[test]
fn finite_options_and_logical_bytes_do_not_allocate_capacity() {
    assert!(options(2, 3, 144, 1, 1000, 2, 8, 10000, 4, 16).is_ok());
    for (live, stored, bytes, ttl) in [
        (0, 3, 144, 1000),
        (4, 3, 144, 1000),
        (2, 3, 71, 1000),
        (2, 3, 144, 0),
        (2, i64::MAX, 144, 1000),
    ] {
        assert!(options(live, stored, bytes, 1, ttl, 2, 8, 10000, 4, 16).is_err());
    }
    assert!(cookie_options("__Host-prelogin", session::SameSite::None).is_err());
}
#[tokio::test]
async fn separate_native_ledger_and_cookie_name_are_checked_before_ddl() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    assert!(!p.native.same_private_queue(&f.auth_pool));
    let collision = open(
        &f.store,
        sqlite::options(1, 2, 1000, 1000).unwrap(),
        options(2, 3, 144, 1, 1000, 2, 8, 10000, 4, 16).unwrap(),
        cookie_options("__Host-auth", session::SameSite::Strict).unwrap(),
        "https://example.invalid",
    )
    .await;
    assert!(collision.is_err());
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn logical_bytes_capacity_and_full_finish_control_ready() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 72).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    let first = owner();
    p.bootstrap(&first, None).await.unwrap();
    assert!(count(&p).await == 1 && first.is_ready());
    let second = owner();
    assert!(p.bootstrap(&second, None).await.is_err());
    assert!(count(&p).await == 1 && !second.is_ready());
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn one_use_claim_actual_delete_and_full_commit_precede_verifier() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let input = presented(&bootstrap);
    let retry = presented(&bootstrap);
    let request = owner();
    let witness = p.claim(&request, input).await.unwrap();
    assert!(count(&p).await == 0);
    let calls = AtomicUsize::new(0);
    let original = Instant::now() + Duration::from_secs(60);
    let identity = witness
        .verify(&request.binding(), || async {
            calls.fetch_add(1, Ordering::SeqCst);
            VerifiedIdentity::from_verified(7, original)
        })
        .await
        .unwrap();
    assert!(
        calls.load(Ordering::SeqCst) == 1 && identity.credential.original_expires_at == original
    );
    assert!(p.claim(&owner(), retry).await.is_err());
    assert!(calls.load(Ordering::SeqCst) == 1);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn wrong_token_and_owner_end_have_zero_verifier_and_keep_ready() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let mut input = presented(&bootstrap);
    input.token = [0; 32];
    assert!(p.claim(&owner(), input).await.is_err());
    assert!(count(&p).await == 1);
    let request = owner();
    request.lease.lease.gate.lock().unwrap().active = false;
    assert!(p.claim(&request, presented(&bootstrap)).await.is_err());
    assert!(count(&p).await == 1);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn expiry_cleanup_reclaims_only_a_finite_batch_without_ttl_reset() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    let first = owner();
    p.bootstrap(&first, None).await.unwrap();
    let stale = presented(&first);
    let tx = sqlite::begin(&p.native, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    sqlite::exec(
        &tx,
        sqlite::literal("UPDATE __nagi_prelogin_ready SET expiry_ms=0"),
        sqlite::parameters(),
    )
    .await
    .unwrap();
    sqlite::commit(tx).await.unwrap();
    assert!(p.claim(&owner(), stale).await.is_err());
    let next = owner();
    p.bootstrap(&next, None).await.unwrap();
    assert!(count(&p).await == 1);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn independent_close_does_not_close_auth_pool() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    close(&p, 2000).await.unwrap();
    let tx = sqlite::begin(&f.auth_pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    sqlite::rollback(tx).await.unwrap();
    assert!(p.bootstrap(&owner(), None).await.is_err());
    f.stop().await;
}

// Later prospective groups; not part of the seven pre-core fixture snapshot.
#[test]
fn canonical_material_has_no_padding_or_alternate_length() {
    let (pair, input) = fresh_pair().unwrap();
    let parsed = ClaimInput::parse(&pair.handle, &pair.token).unwrap();
    assert!(
        bool::from(parsed.handle.ct_eq(&input.handle))
            && bool::from(parsed.token.ct_eq(&input.token))
    );
    assert!(pair.handle.len() == 43 && pair.token.len() == 43);
    assert!(ClaimInput::parse("ordinary", &pair.token).is_err());
    let padded = format!("{}=", pair.handle);
    assert!(ClaimInput::parse(&padded, &pair.token).is_err());
}
#[tokio::test]
async fn two_actual_claims_have_exactly_one_committed_witness() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let a = presented(&bootstrap);
    let b = presented(&bootstrap);
    let one = owner();
    let two = owner();
    let barrier = tokio::sync::Barrier::new(2);
    let (a, b) = tokio::join!(
        async {
            barrier.wait().await;
            p.claim(&one, a).await
        },
        async {
            barrier.wait().await;
            p.claim(&two, b).await
        }
    );
    assert!(a.is_ok() != b.is_ok());
    assert!(count(&p).await == 0);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn issuance_window_survives_reopen_and_claim_deletion() {
    let f = Fixture::new().await;
    let result = open(
        &f.store,
        sqlite::options(1, 1, 1000, 1000).unwrap(),
        options(2, 3, 144, 1, 1000, 2, 1, 10000, 4, 16).unwrap(),
        cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
        "https://example.invalid",
    )
    .await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    let first = owner();
    p.bootstrap(&first, None).await.unwrap();
    p.claim(&owner(), presented(&first)).await.unwrap();
    assert!(count(&p).await == 0);
    close(&p, 2000).await.unwrap();
    let p = open(
        &f.store,
        sqlite::options(1, 1, 1000, 1000).unwrap(),
        options(2, 3, 144, 1, 1000, 2, 1, 10000, 4, 16).unwrap(),
        cookie_options("__Host-prelogin", session::SameSite::Strict).unwrap(),
        "https://example.invalid",
    )
    .await
    .unwrap();
    assert!(p.bootstrap(&owner(), None).await.is_err());
    assert!(count(&p).await == 0);
    close(&p, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test]
async fn unknown_and_committed_cleanup_never_create_a_verifier_witness() {
    for fault in [
        sqlite::LogoutTestFault::Unknown,
        sqlite::LogoutTestFault::CommittedCleanup,
    ] {
        let f = Fixture::new().await;
        let result = f.open(2, 3, 144).await;
        if !cfg!(any(unix, windows)) {
            assert!(result.is_err());
            f.stop().await;
            return;
        }
        let mut p = result.unwrap();
        let bootstrap = owner();
        p.bootstrap(&bootstrap, None).await.unwrap();
        let input = presented(&bootstrap);
        let broken = sqlite::logout_test_pool(f.dir.join("owned.sqlite"), fault);
        let old = std::mem::replace(&mut p.native, broken);
        sqlite::close(&old, 2000).await.unwrap();
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
                panic!("failed native finish returned witness");
            }
        };
        assert!(calls.load(Ordering::SeqCst) == 0);
        assert!(
            e.outcome
                == match fault {
                    sqlite::LogoutTestFault::Unknown => sqlite::Outcome::Unknown,
                    sqlite::LogoutTestFault::CommittedCleanup => sqlite::Outcome::Committed,
                }
        );
        let _ = close(&p, 2000).await;
        f.stop().await;
    }
}
#[tokio::test]
async fn dropping_owned_pending_bootstrap_retires_material_and_join_is_separate() {
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    let lock = sqlite::begin(&f.auth_pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let request = owner();
    let mut future = Box::pin(p.bootstrap(&request, None));
    let pending =
        std::future::poll_fn(|cx| std::task::Poll::Ready(future.as_mut().poll(cx).is_pending()))
            .await;
    assert!(pending);
    drop(future);
    assert!(request.cell.lock().unwrap().pair.is_none());
    assert!(!request.is_ready());
    sqlite::rollback(lock).await.unwrap();
    close(&p, 2000).await.unwrap();
    let tx = sqlite::begin(&f.auth_pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Count>(
        &tx,
        sqlite::literal("SELECT count(*) AS n FROM __nagi_prelogin_ready"),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    assert!(n == 0);
    f.stop().await;
}
#[tokio::test]
async fn consumed_bearer_identity_can_issue_session_without_replacing_original_ceiling() {
    #[cfg(all(test, nagi_auth_stage_diag))]
    let observer = crate::auth::stage_observer::Guard::new(2);
    let f = Fixture::new().await;
    let result = f.open(2, 3, 144).await;
    if !cfg!(any(unix, windows)) {
        assert!(result.is_err());
        f.stop().await;
        return;
    }
    let p = result.unwrap();
    #[cfg(all(test, nagi_auth_stage_diag))]
    observer.attach(&p.stage_slot);
    let bootstrap = owner();
    p.bootstrap(&bootstrap, None).await.unwrap();
    let request = owner();
    let original = Instant::now() + Duration::from_secs(60);
    let identity = p
        .claim(&request, presented(&bootstrap))
        .await
        .unwrap()
        .verify(&request.binding(), || async {
            VerifiedIdentity::from_verified(7, original)
        })
        .await
        .unwrap();
    assert!(identity.credential.original_expires_at == original);
    let scope = super::super::AuthScope::bind(identity, Arc::clone(&request.lease.lease)).unwrap();
    let intent = session::issue(&f.store, scope).await.unwrap();
    drop(intent);
    let tx = sqlite::begin(&f.auth_pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Count>(
        &tx,
        sqlite::literal("SELECT absolute_ms-idle_ms AS n FROM __nagi_session_rows WHERE subject=7"),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    assert!(n > 30000);
    assert!(count(&p).await == 0);
    close(&p, 2000).await.unwrap();
    f.stop().await;
    #[cfg(all(test, nagi_auth_stage_diag))]
    observer.emit();
}

#[tokio::test]
async fn changed_session_incarnation_is_rejected_before_prelogin_ddl() {
    let f = Fixture::new().await;
    let tx = sqlite::begin(&f.auth_pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    sqlite::exec(
        &tx,
        sqlite::literal("UPDATE __nagi_session_meta SET incarnation=zeroblob(32) WHERE id=1"),
        sqlite::parameters(),
    )
    .await
    .unwrap();
    sqlite::commit(tx).await.unwrap();
    assert!(f.open(2, 3, 144).await.is_err());
    let tx = sqlite::begin(&f.auth_pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Count>(
        &tx,
        sqlite::literal(
            "SELECT count(*) AS n FROM sqlite_schema WHERE name GLOB '__nagi_prelogin_*'",
        ),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    assert!(n == 0);
    f.stop().await;
}

#[path = "clock_review_tests.rs"]
mod clock_review_tests;

#[path = "expiry_review_tests.rs"]
mod expiry_review_tests;
