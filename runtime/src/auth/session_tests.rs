//! Tiny real SQLite behavior oracles for the private production Store foundation.
use super::*;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);
struct Fixture {
    directory: std::path::PathBuf,
    pool: sqlite::Pool,
    wall: Arc<AtomicI64>,
}
impl Fixture {
    async fn new() -> Self {
        let directory = loop {
            let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let p =
                std::env::temp_dir().join(format!("nagi-sf02-store-{}-{id}", std::process::id()));
            match std::fs::create_dir(&p) {
                Ok(()) => break p,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("owned fixture directory: {e}"),
            }
        };
        let pool = sqlite::open(
            directory.join("owned.sqlite").to_str().unwrap(),
            sqlite::options(2, 4, 1000, 1000).unwrap(),
        )
        .await
        .unwrap();
        Self {
            directory,
            pool,
            wall: Arc::new(AtomicI64::new(1000)),
        }
    }
    fn clock(&self) -> Clock {
        let wall = Arc::clone(&self.wall);
        Arc::new(move || {
            let mono = Instant::now();
            let wall = wall.load(Ordering::SeqCst);
            Ok(ClockSample {
                monotonic: mono,
                monotonic_after: mono,
                floor_ms: wall,
                ceil_ms: wall,
            })
        })
    }
    async fn store(&self) -> Foundation {
        Foundation::initialize(
            &self.pool,
            settings(),
            "__Host-example",
            1,
            [3; 32],
            self.clock(),
        )
        .await
        .unwrap()
    }
    async fn sql(&self, sql: &'static str) {
        let tx = sqlite::begin(&self.pool, sqlite::BeginMode::Immediate)
            .await
            .unwrap();
        sqlite::exec(&tx, sqlite::literal(sql), sqlite::parameters())
            .await
            .unwrap();
        sqlite::commit(tx).await.unwrap();
    }
    async fn stop(self) {
        sqlite::close(&self.pool, 2000).await.unwrap();
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}
fn settings() -> Options {
    options(2, 3, 72, 1, 10000, 100000, 5000, 2).unwrap()
}
fn authority(ms: u64) -> Instant {
    Instant::now() + Duration::from_millis(ms)
}
async fn count(store: &Foundation) -> i64 {
    let tx = sqlite::begin(&store.pool, sqlite::BeginMode::Immediate)
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

#[test]
fn fixed_options_validate_without_allocating_capacity() {
    assert!(options(2, 3, 72, 1, 10, 100, 5, 2).is_ok());
    let clock = wall_clock().unwrap();
    assert!(clock.ceil_ms >= clock.floor_ms && clock.ceil_ms - clock.floor_ms <= 1);
    assert!(clock.monotonic <= clock.monotonic_after);
    assert!(Foundation::validate_clock(&clock).is_ok());
    let earlier = Instant::now()
        .checked_sub(Duration::from_millis(100))
        .unwrap();
    let later = earlier.checked_add(Duration::from_millis(50)).unwrap();
    for invalid in [
        ClockSample {
            monotonic: later,
            monotonic_after: earlier,
            floor_ms: 1000,
            ceil_ms: 1000,
        },
        ClockSample {
            monotonic: earlier,
            monotonic_after: later,
            floor_ms: -1,
            ceil_ms: 0,
        },
        ClockSample {
            monotonic: earlier,
            monotonic_after: later,
            floor_ms: 1000,
            ceil_ms: 1002,
        },
        ClockSample {
            monotonic: earlier,
            monotonic_after: Instant::now().checked_add(Duration::from_secs(1)).unwrap(),
            floor_ms: 1000,
            ceil_ms: 1000,
        },
    ] {
        assert_eq!(
            Foundation::validate_clock(&invalid).err().unwrap().kind,
            FailureKind::Unavailable
        );
    }

    for args in [
        (0, 3, 72, 1, 10, 100, 5, 2),
        (4, 3, 72, 1, 10, 100, 5, 2),
        (2, 3, 71, 1, 10, 100, 5, 2),
        (2, 3, 72, 4, 10, 100, 5, 2),
        (2, 3, 72, 1, 10, 100, 11, 2),
        (2, 3, 72, 1, 10, 100, 5, 0),
    ] {
        assert_eq!(
            options(args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7)
                .err()
                .unwrap()
                .kind,
            FailureKind::InvalidRequest
        );
    }
}
#[tokio::test]
async fn persistent_sqlite_startup_has_session_schema() {
    let f = Fixture::new().await;
    let store = f.store().await;
    let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n=sqlite::query::<Count>(&tx,sqlite::literal("SELECT count(*) AS n FROM sqlite_schema WHERE type='table' AND name IN ('__nagi_session_meta','__nagi_session_rows')"),sqlite::parameters()).await.unwrap().unwrap().0;
    sqlite::rollback(tx).await.unwrap();
    assert_eq!(n, 2);
    assert!(
        Foundation::initialize(&f.pool, settings(), "__Host-example", 1, [4; 32], f.clock())
            .await
            .is_ok()
    );
    assert_eq!(
        Foundation::initialize(&f.pool, settings(), "__Host-other", 1, [4; 32], f.clock())
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::InvalidRequest
    );
    assert_eq!(
        Foundation::initialize(
            &f.pool,
            options(1, 3, 72, 1, 10000, 100000, 5000, 2).unwrap(),
            "__Host-example",
            1,
            [4; 32],
            f.clock()
        )
        .await
        .err()
        .unwrap()
        .kind,
        FailureKind::InvalidRequest
    );
    f.sql("UPDATE __nagi_session_meta SET cookie_name='__Host-other' WHERE id=1")
        .await;
    assert_eq!(
        store.lookup([1; 32]).await.err().unwrap().kind,
        FailureKind::Unavailable
    );
    f.sql("UPDATE __nagi_session_meta SET cookie_name='__Host-example',same_site=2 WHERE id=1")
        .await;
    assert_eq!(
        store.lookup([1; 32]).await.err().unwrap().kind,
        FailureKind::Unavailable
    );
    f.sql("UPDATE __nagi_session_meta SET same_site=1 WHERE id=1")
        .await;
    f.sql("UPDATE __nagi_session_meta SET version=2 WHERE id=1")
        .await;
    assert_eq!(
        Foundation::initialize(&f.pool, settings(), "__Host-example", 1, [4; 32], f.clock())
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::InvalidRequest
    );
    drop(store);
    f.stop().await;
}
#[tokio::test]
async fn reserved_schema_conflict_rejects_without_adopting_it() {
    let f = Fixture::new().await;
    f.sql("CREATE TABLE __nagi_session_meta(unrelated TEXT)")
        .await;
    assert_eq!(
        Foundation::initialize(&f.pool, settings(), "__Host-example", 1, [4; 32], f.clock())
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::InvalidRequest
    );
    f.stop().await;
}
#[tokio::test]
async fn capacity_cleanup_and_snapshot_expiry_are_separate() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let first = s.insert(7, [1; 32], authority(1000000)).await.unwrap();
    let second = s.insert(8, [2; 32], authority(1000000)).await.unwrap();
    assert_eq!(
        s.insert(9, [5; 32], authority(1000000))
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::Unavailable
    );
    assert_eq!(count(&s).await, 2);
    f.wall.store(6000, Ordering::SeqCst);
    let snapshot = s.lookup([1; 32]).await.unwrap();
    assert_eq!(snapshot.idle_ms, first.idle_ms);
    assert!(snapshot.expires_at <= Instant::now() + Duration::from_millis(5000));
    f.wall.store(11000, Ordering::SeqCst);
    assert!(s.lookup([2; 32]).await.is_err());
    let after_touch = s.lookup([1; 32]).await.unwrap();
    assert_eq!(after_touch.idle_ms, 16000);
    assert_eq!(snapshot.idle_ms, first.idle_ms);
    // That successful lookup refreshed the *next* lookup deadline to 21000.
    f.wall.store(21000, Ordering::SeqCst);
    assert!(s.lookup([1; 32]).await.is_err());
    assert_eq!(second.absolute_ms, first.absolute_ms);
    // Four tiny administratively seeded expired rows exercise the independent
    // stored-row cap even though ordinary inserts never exceed configured caps.
    let tx = sqlite::begin(&f.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    for value in 10..14 {
        let p = sqlite::bind_bytes(
            sqlite::bind_i64(sqlite::parameters(), value),
            vec![value as u8; 32],
        );
        sqlite::exec(
            &tx,
            sqlite::literal("INSERT INTO __nagi_session_rows VALUES(?,7,?,0,1,1000000)"),
            p,
        )
        .await
        .unwrap();
    }
    sqlite::commit(tx).await.unwrap();
    assert_eq!(
        s.insert(9, [15; 32], authority(1000000))
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::Unavailable
    );
    assert_eq!(
        count(&s).await,
        3,
        "cleanup removes at most one expired row"
    );
    s.insert(9, [16; 32], authority(1000000)).await.unwrap();
    assert_eq!(count(&s).await, 3);
    drop(s);
    f.stop().await;
}
#[tokio::test]
async fn credential_ceiling_clock_rollback_and_reopen_fail_closed() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let short = s.insert(7, [1; 32], authority(30000)).await.unwrap();
    assert!(short.absolute_ms <= 31000);
    assert!(short.absolute_ms > 1000);
    f.wall.store(short.absolute_ms, Ordering::SeqCst);
    assert_eq!(
        s.lookup([1; 32]).await.err().unwrap().kind,
        FailureKind::InvalidCredential
    );
    f.wall.store(999, Ordering::SeqCst);
    assert_eq!(
        s.insert(7, [2; 32], authority(1000000))
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::Unavailable
    );
    assert_eq!(
        Foundation::initialize(&f.pool, settings(), "__Host-example", 1, [5; 32], f.clock())
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::Unavailable
    );
    f.wall.store(short.absolute_ms, Ordering::SeqCst);
    assert!(
        Foundation::initialize(&f.pool, settings(), "__Host-example", 1, [5; 32], f.clock())
            .await
            .is_ok()
    );
    let continued = s.insert(7, [6; 32], authority(1000000)).await.unwrap();
    drop(s);
    sqlite::close(&f.pool, 2000).await.unwrap();
    let reopened = sqlite::open(
        f.directory.join("owned.sqlite").to_str().unwrap(),
        sqlite::options(1, 4, 1000, 1000).unwrap(),
    )
    .await
    .unwrap();
    f.wall.store(short.absolute_ms + 1, Ordering::SeqCst);
    let recovered = Foundation::initialize(
        &reopened,
        settings(),
        "__Host-example",
        1,
        [9; 32],
        f.clock(),
    )
    .await
    .unwrap();
    assert_eq!(
        recovered.lookup([6; 32]).await.unwrap().absolute_ms,
        continued.absolute_ms
    );
    drop(recovered);
    sqlite::close(&reopened, 2000).await.unwrap();
    f.stop().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_party_rotation_has_one_cas_winner_and_old_digest_dies() {
    let f = Fixture::new().await;
    let s = Arc::new(f.store().await);
    let snapshot = s.insert(7, [1; 32], authority(1000000)).await.unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let mut tasks = Vec::new();
    for digest in [[2; 32], [4; 32]] {
        let store = Arc::clone(&s);
        let barrier = Arc::clone(&barrier);
        let old = snapshot.private_copy();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            store.rotate(&old, digest).await
        }));
    }
    barrier.wait().await;
    let mut winners = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(new) => {
                assert_eq!(new.absolute_ms, snapshot.absolute_ms);
                winners += 1
            }
            Err(e) => assert_eq!(e.kind, FailureKind::Denied),
        }
    }
    assert_eq!(winners, 1);
    assert!(s.lookup([1; 32]).await.is_err());
    assert_eq!(count(&s).await, 1);
    f.sql("UPDATE __nagi_session_rows SET generation=9223372036854775807")
        .await;
    let current = if let Ok(row) = s.lookup([2; 32]).await {
        row
    } else {
        s.lookup([4; 32]).await.unwrap()
    };
    assert_eq!(
        s.rotate(&current, [6; 32]).await.err().unwrap().kind,
        FailureKind::Unavailable
    );
    f.sql("UPDATE __nagi_session_meta SET next_lineage=9223372036854775807 WHERE id=1")
        .await;
    assert_eq!(
        s.insert(8, [7; 32], authority(1000000))
            .await
            .err()
            .unwrap()
            .kind,
        FailureKind::Unavailable
    );
    assert_eq!(count(&s).await, 1);
    drop(s);
    f.stop().await;
}
#[tokio::test]
async fn lineage_logout_revokes_rotation_winner_but_not_other_lineage() {
    let f = Fixture::new().await;
    let s = f.store().await;
    let old = s.insert(7, [1; 32], authority(1000000)).await.unwrap();
    s.insert(7, [8; 32], authority(1000000)).await.unwrap();
    s.rotate(&old, [2; 32]).await.unwrap();
    s.logout(&old).await.unwrap();
    assert!(s.lookup([2; 32]).await.is_err());
    assert!(s.lookup([8; 32]).await.is_ok());
    s.logout(&old).await.unwrap();
    assert_eq!(count(&s).await, 1);
    let remaining = s.lookup([8; 32]).await.unwrap();
    s.logout(&remaining).await.unwrap();
    assert_eq!(
        s.rotate(&remaining, [9; 32]).await.err().unwrap().kind,
        FailureKind::Denied
    );
    drop(s);
    f.stop().await;
}
#[tokio::test]
async fn original_pool_close_stops_foundation_and_failures_are_redacted() {
    let f = Fixture::new().await;
    let s = f.store().await;
    sqlite::close(&f.pool, 2000).await.unwrap();
    let error = s.lookup([1; 32]).await.err().unwrap();
    assert_eq!(error.kind, FailureKind::Unavailable);
    assert_eq!(error.outcome, sqlite::Outcome::NotApplicable);
    assert_eq!(error.to_string(), "security service unavailable");
    assert!(!format!("{s:?}").contains("owned.sqlite"));
    drop(s);
    f.stop().await;
}

#[path = "session_review_tests.rs"]
mod session_review_tests;

#[tokio::test]
async fn memory_pool_refused_before_session_ddl() {
    let pool = sqlite::open(":memory:", sqlite::options(1, 4, 1000, 1000).unwrap())
        .await
        .unwrap();
    let error = Foundation::initialize(
        &pool,
        settings(),
        "__Host-example",
        1,
        [41; 32],
        Arc::new(wall_clock),
    )
    .await
    .err();
    let tx = sqlite::begin(&pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let n = sqlite::query::<Count>(
        &tx,
        sqlite::literal(
            "SELECT count(*) AS n FROM sqlite_schema WHERE name GLOB '__nagi_session_*'",
        ),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    sqlite::close(&pool, 2000).await.unwrap();
    assert!(
        error.is_some(),
        "memory Pool must be refused before session DDL"
    );
    assert_eq!(error.unwrap().kind, FailureKind::InvalidRequest);
    assert_eq!(n, 0);
}

#[tokio::test]
async fn checked_session_snapshot_retains_absolute_and_idle_through_grant_and_touch() {
    use crate::auth::{AuthScope, CredentialSource, Grant, LeaseOwner};
    let fixture = Fixture::new().await;
    let store = fixture.store().await;
    // Synthetic digest belongs to this private DB oracle, not a browser credential.
    let snapshot = store.insert(7, [5; 32], authority(60000)).await.unwrap();
    let original = snapshot.absolute_expires_at;
    let effective = snapshot.expires_at;
    let row = (
        snapshot.incarnation,
        snapshot.lineage,
        snapshot.digest,
        snapshot.generation,
        snapshot.idle_ms,
        snapshot.absolute_ms,
    );
    assert!(original > effective);
    let identity = snapshot.into_verified_identity().unwrap();
    assert!(identity.expires_at == effective);
    assert!(identity.credential.original_expires_at == original);
    let request = Instant::now() + Duration::from_secs(5);
    let owner = LeaseOwner::new(request).unwrap();
    let scope = AuthScope::bind(identity, Arc::clone(&owner.lease)).unwrap();
    assert!(scope.lease.checked_gate().unwrap().expires_at == request.min(effective));
    let grant = Grant::<()>::from_authorized(scope, 9).unwrap();
    fixture.wall.store(8000, Ordering::SeqCst);
    let _touched = store.lookup(row.2).await.unwrap();
    let tx = sqlite::begin(&fixture.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let changed = sqlite::query::<Count>(
        &tx,
        sqlite::literal("SELECT count(*) AS n FROM __nagi_session_rows WHERE idle_ms=18000"),
        sqlite::parameters(),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    sqlite::rollback(tx).await.unwrap();
    assert!(changed == 1);
    assert!(grant.credential.original_expires_at == original);
    match &grant.credential.source {
        CredentialSource::Session(held) => {
            assert!(
                (
                    held.incarnation,
                    held.lineage,
                    held.digest,
                    held.generation,
                    held.idle_ms,
                    held.absolute_ms
                ) == row
            );
            assert!(held.expires_at == effective && held.absolute_expires_at == original);
        }
        CredentialSource::Bearer => panic!("credential source lost"),
    }
    assert!(
        grant
            .submit(17, |subject, target, value| (subject, target, value))
            .unwrap()
            == (7, 9, 17)
    );
    drop(owner);
    drop(store);
    fixture.stop().await;
}

#[path = "session/mutation_tests.rs"]
mod mutation_tests;

#[path = "session/mutation_write_tests.rs"]
mod mutation_write_tests;
