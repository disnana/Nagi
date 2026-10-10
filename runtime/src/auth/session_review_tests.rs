//! Independent regression oracles for the two SF02 session review findings.
//! These tests deliberately use tiny owned SQLite fixtures and fixed clock
//! samples; they do not use sleep or expose credential material.
use super::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn fixed_clock(monotonic: Instant, monotonic_after: Instant, floor_ms: i64, ceil_ms: i64) -> Clock {
    Arc::new(move || {
        Ok(ClockSample {
            monotonic,
            monotonic_after,
            floor_ms,
            ceil_ms,
        })
    })
}

async fn row_by_lineage(store: &Foundation, lineage: i64) -> Option<SessionRow> {
    let tx = sqlite::begin(&store.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let row = sqlite::query::<SessionRow>(
        &tx,
        sqlite::literal(
            "SELECT lineage,subject,digest,generation,idle_ms,absolute_ms \
             FROM __nagi_session_rows WHERE lineage=?",
        ),
        sqlite::bind_i64(sqlite::parameters(), lineage),
    )
    .await
    .unwrap();
    sqlite::rollback(tx).await.unwrap();
    row
}

async fn metadata_row(store: &Foundation) -> Metadata {
    let tx = sqlite::begin(&store.pool, sqlite::BeginMode::Immediate)
        .await
        .unwrap();
    let row = metadata(&tx).await.unwrap();
    sqlite::rollback(tx).await.unwrap();
    row
}

#[tokio::test]
async fn insert_snapshot_failure_does_not_commit_row_or_lineage() {
    let f = Fixture::new().await;
    let mut store = f.store().await;

    // The later clock sample is 100ms old at invocation; the credential has
    // 20ms left at that sample and 120ms at the earlier one. Both paths reach
    // Snapshot after the writes, where the mapped expiry is now in the past.
    let later = Instant::now()
        .checked_sub(Duration::from_millis(100))
        .unwrap();
    let earlier = later.checked_sub(Duration::from_millis(100)).unwrap();
    let credential_expires = later.checked_add(Duration::from_millis(20)).unwrap();
    store.clock = fixed_clock(earlier, later, 1100, 1100);

    let error = store
        .insert(7, [31; 32], credential_expires)
        .await
        .err()
        .unwrap();
    assert_eq!(error.kind, FailureKind::InvalidCredential);
    assert_eq!(error.outcome, sqlite::Outcome::RolledBack);
    assert_eq!(count(&store).await, 0, "failed insert must leave no row");

    let meta = metadata_row(&store).await;
    assert_eq!(
        meta.next_lineage, 1,
        "failed insert must not consume a lineage"
    );
    assert_eq!(meta.last_wall, 1100, "validated wall progress is retained");
    assert_eq!(
        meta.options,
        settings(),
        "the fixture's configured options stay intact"
    );
    assert_eq!(meta.cookie_name, "__Host-example");
    assert_eq!(meta.same_site, 1);

    drop(store);
    f.stop().await;
}

#[tokio::test]
async fn rotate_snapshot_failure_preserves_original_digest_and_generation() {
    let f = Fixture::new().await;
    let mut store = f.store().await;
    let old = store
        .insert(7, [32; 32], authority(1_000_000))
        .await
        .unwrap();

    // Keep the stored row live at the fixed sample, with only 50ms left on its
    // absolute ceiling. Rotation writes the CAS update before Snapshot fails.
    f.sql("UPDATE __nagi_session_rows SET idle_ms=absolute_ms WHERE lineage=1")
        .await;
    let later = Instant::now()
        .checked_sub(Duration::from_millis(100))
        .unwrap();
    let earlier = later.checked_sub(Duration::from_millis(100)).unwrap();
    let floor_ms = old.absolute_ms - 50;
    store.clock = fixed_clock(earlier, later, floor_ms, floor_ms);

    let error = store.rotate(&old, [33; 32]).await.err().unwrap();
    assert_eq!(error.kind, FailureKind::InvalidCredential);
    assert_eq!(error.outcome, sqlite::Outcome::RolledBack);
    assert_eq!(count(&store).await, 1, "failed rotation keeps one row");

    let row = row_by_lineage(&store, old.lineage).await.unwrap();
    assert!(
        row.digest.as_slice() == [32; 32],
        "failed rotation preserves the old digest"
    );
    assert_eq!(
        row.generation, old.generation,
        "failed rotation preserves the CAS generation"
    );
    assert_eq!(
        row.idle_ms, old.absolute_ms,
        "failed rotation preserves the prior row deadline"
    );
    assert_eq!(row.absolute_ms, old.absolute_ms);

    let meta = metadata_row(&store).await;
    assert_eq!(
        meta.last_wall, floor_ms,
        "validated wall progress is retained"
    );
    assert_eq!(meta.next_lineage, 2);
    assert_eq!(meta.options, settings());
    assert_eq!(meta.cookie_name, "__Host-example");
    assert_eq!(meta.same_site, 1);

    drop(store);
    f.stop().await;
}

#[tokio::test]
async fn insert_ceiling_uses_later_monotonic_sample() {
    let f = Fixture::new().await;
    let mut store = f.store().await;

    let after = Instant::now()
        .checked_sub(Duration::from_millis(100))
        .unwrap();
    let before = after.checked_sub(Duration::from_millis(100)).unwrap();
    let credential_expires = after.checked_add(Duration::from_secs(60)).unwrap();
    let expected_ceiling = 1000 + credential_expires.duration_since(after).as_millis() as i64;
    store.clock = fixed_clock(before, after, 1000, 1000);

    let snapshot = store.insert(7, [34; 32], credential_expires).await.unwrap();
    let row = row_by_lineage(&store, snapshot.lineage).await.unwrap();
    assert!(
        row.absolute_ms <= expected_ceiling,
        "stored session deadline must not exceed the credential ceiling"
    );
    assert_eq!(snapshot.absolute_ms, row.absolute_ms);

    let meta = metadata_row(&store).await;
    assert_eq!(meta.options, settings());
    assert_eq!(meta.cookie_name, "__Host-example");
    assert_eq!(meta.same_site, 1);

    // Post-RED strengthening: bind the next snapshot to the stored absolute
    // ceiling rather than the shorter idle TTL. Both later samples are already
    // in the past; this does not sleep or invent a future clock observation.
    f.sql("UPDATE __nagi_session_rows SET idle_ms=absolute_ms WHERE lineage=1")
        .await;
    let later = after.checked_add(Duration::from_millis(50)).unwrap();
    let later_clock = fixed_clock(later, later, 1050, 1050);
    store.clock = Arc::clone(&later_clock);
    let subsequent = store.lookup([34; 32]).await.unwrap();
    assert_eq!(subsequent.absolute_ms, row.absolute_ms);
    assert!(subsequent.expires_at <= credential_expires);

    drop(store);
    sqlite::close(&f.pool, 2000).await.unwrap();
    let reopened = sqlite::open(
        f.directory.join("owned.sqlite").to_str().unwrap(),
        sqlite::options(2, 4, 1000, 1000).unwrap(),
    )
    .await
    .unwrap();
    let recovered = Foundation::initialize(
        &reopened,
        settings(),
        "__Host-example",
        1,
        [35; 32],
        later_clock,
    )
    .await
    .unwrap();
    let restored = recovered.lookup([34; 32]).await.unwrap();
    assert_eq!(restored.absolute_ms, row.absolute_ms);
    assert!(restored.expires_at <= credential_expires);
    drop(recovered);
    sqlite::close(&reopened, 2000).await.unwrap();
    f.stop().await;
}
