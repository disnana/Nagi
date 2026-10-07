//! debug profileのprivate比較入口。測定値に合否thresholdやproduction性能保証を設けない。
use super::{
    adapter::{AcquireBudget, Adapter, AdapterSeams},
    session::{Config, Driver, Finish, Tx},
};
use crate::FromRow;
use rusqlite::Row;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
struct Number(i64);
impl FromRow for Number {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(row: &Row<'_>, indices: &[usize]) -> rusqlite::Result<Self> {
        assert!(!super::management_active());
        Ok(Self(row.get(indices[0])?))
    }
}
async fn native_round_trip(tx: Tx) {
    assert_eq!(
        tx.query::<Number>("SELECT 1 AS n", vec![]).await.unwrap(),
        Some(Number(1))
    );
    tx.finish(Finish::Rollback).await.unwrap();
}
fn percentiles(samples: &[u128]) -> serde_json::Value {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    serde_json::json!({ "p50_ns": sorted[(sorted.len() - 1) * 50 / 100],
        "p95_ns": sorted[(sorted.len() - 1) * 95 / 100] })
}

#[tokio::test(flavor = "current_thread")]
async fn direct_and_tokio_adapter_warm_native_session_comparison() {
    const BATCHES: usize = 32;
    const PER_BATCH: usize = 128;
    const WARMUP: usize = 8;
    let direct = Driver::open(Config::default());
    let adapter = Adapter::new(Config::default(), AdapterSeams::default());
    for _ in 0..WARMUP {
        native_round_trip(direct.begin().await.unwrap()).await;
        native_round_trip(adapter.begin().await.unwrap()).await;
    }
    let mut direct_samples = Vec::with_capacity(BATCHES * PER_BATCH);
    let mut adapter_samples = Vec::with_capacity(BATCHES * PER_BATCH);
    let mut raw = Vec::with_capacity(BATCHES * PER_BATCH * 2);
    for batch in 0..BATCHES {
        for phase in 0..2 {
            let use_adapter = (batch + phase) % 2 != 0;
            for index in 0..PER_BATCH {
                let started = Instant::now();
                if use_adapter {
                    native_round_trip(adapter.begin().await.unwrap()).await;
                } else {
                    native_round_trip(direct.begin().await.unwrap()).await;
                }
                let elapsed = started.elapsed().as_nanos();
                if use_adapter {
                    adapter_samples.push(elapsed);
                } else {
                    direct_samples.push(elapsed);
                }
                raw.push(serde_json::json!({ "batch": batch, "phase": phase, "index": index,
                    "backend": if use_adapter { "tokio_adapter" } else { "direct" }, "elapsed_ns": elapsed }));
            }
        }
    }
    direct.close(Duration::from_secs(2)).await.unwrap();
    adapter.close(Duration::from_secs(2)).await.unwrap();
    assert!(direct.stats().native_closed && direct.stats().joined);
    let adapter_stats = adapter.observer().snapshot();
    assert_eq!(adapter_stats.created, 1);
    assert_eq!(adapter_stats.native_closed, 1);
    assert_eq!(adapter_stats.joined, 1);
    let summary = serde_json::json!({ "scope": "private cfg(test) debug-profile, warm serial native session",
        "batches": BATCHES, "transactions_per_batch_per_backend": PER_BATCH,
        "warmup_transactions_per_backend": WARMUP, "sql": "SELECT 1 AS n", "terminal": "rollback",
        "direct": percentiles(&direct_samples), "tokio_adapter": percentiles(&adapter_samples),
        "direct_threads": "1 native worker + 1 join observer",
        "adapter_threads": "1 native worker + 1 independent join observer per connection",
        "direct_native_health_and_join": true, "adapter_native_health_and_join": true });
    eprintln!("NAGI_SQLITE_COMPARISON {summary}");
    // この環境値はprivate測定artifactの保存先だけを指定。公開policy/defaultではない。
    if let Some(path) = std::env::var_os("NAGI_SQLITE_MEASURE_OUTPUT") {
        let report = serde_json::json!({ "summary": summary, "raw": raw });
        let file = std::fs::File::create(path).unwrap();
        serde_json::to_writer(file, &report).unwrap();
    }
}

// 同じadapter/native coreの取得予算あり/なし。数値は合否条件にしない。
#[tokio::test(flavor = "current_thread")]
async fn bounded_and_unbounded_tokio_adapter_warm_comparison() {
    const BATCHES: usize = 32;
    const PER_BATCH: usize = 128;
    const WARMUP: usize = 8;
    let plain = Adapter::new(Config::default(), AdapterSeams::default());
    let bounded = Adapter::new(Config::default(), AdapterSeams::default());
    // 10秒はこの測定fixtureだけ。待機がないwarm経路を測るための予算。
    let budget =
        || AcquireBudget::after_at(tokio::time::Instant::now(), Duration::from_secs(10)).unwrap();
    let plain_future_bytes = std::mem::size_of_val(&plain.begin());
    let bounded_future_bytes = std::mem::size_of_val(&bounded.begin_with_budget(budget()));
    for _ in 0..WARMUP {
        native_round_trip(plain.begin().await.unwrap()).await;
        native_round_trip(bounded.begin_with_budget(budget()).await.unwrap()).await;
    }
    let mut plain_samples = Vec::with_capacity(BATCHES * PER_BATCH);
    let mut bounded_samples = Vec::with_capacity(BATCHES * PER_BATCH);
    let mut raw = Vec::with_capacity(BATCHES * PER_BATCH * 2);
    for batch in 0..BATCHES {
        for phase in 0..2 {
            let use_budget = (batch + phase) % 2 != 0;
            for index in 0..PER_BATCH {
                let started = Instant::now();
                if use_budget {
                    native_round_trip(bounded.begin_with_budget(budget()).await.unwrap()).await;
                } else {
                    native_round_trip(plain.begin().await.unwrap()).await;
                }
                let elapsed = started.elapsed().as_nanos();
                if use_budget {
                    bounded_samples.push(elapsed);
                } else {
                    plain_samples.push(elapsed);
                }
                raw.push(serde_json::json!({ "batch": batch, "phase": phase, "index": index,
                    "backend": if use_budget { "bounded" } else { "unbounded_fixture" }, "elapsed_ns": elapsed }));
            }
        }
    }
    plain.close(Duration::from_secs(2)).await.unwrap();
    bounded.close(Duration::from_secs(2)).await.unwrap();
    for adapter in [&plain, &bounded] {
        let stats = adapter.observer().snapshot();
        assert_eq!(stats.created, 1);
        assert_eq!(stats.native_closed, 1);
        assert_eq!(stats.joined, 1);
        assert_eq!(stats.pending_workers, 0);
    }
    let summary = serde_json::json!({ "scope": "private cfg(test) debug-profile, cap1 warm serial, bounded vs legacy fixture",
        "batches": BATCHES, "transactions_per_batch_per_backend": PER_BATCH,
        "warmup_transactions_per_backend": WARMUP, "sql": "SELECT 1 AS n", "terminal": "rollback",
        "unbounded_fixture": percentiles(&plain_samples), "bounded": percentiles(&bounded_samples),
        "unbounded_future_bytes": plain_future_bytes, "bounded_future_bytes": bounded_future_bytes,
        "future_measurement": "size_of_val of unpolled begin future; not heap allocation or generated Nagi future",
        "native_health_and_join": true });
    eprintln!("NAGI_SQLITE_BUDGET_COMPARISON {summary}");
    if let Some(path) = std::env::var_os("NAGI_SQLITE_BUDGET_MEASURE_OUTPUT") {
        let file = std::fs::File::create(path).unwrap();
        serde_json::to_writer(file, &serde_json::json!({ "summary": summary, "raw": raw }))
            .unwrap();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn public_two_connection_native_cost_snapshot() {
    fn threads() -> Option<usize> {
        #[cfg(target_os = "linux")]
        {
            std::fs::read_dir("/proc/self/task")
                .ok()
                .map(|entries| entries.count())
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }
    let mut file = super::adapter_tests::MultiFile::new();
    let before = threads();
    let options = super::options(2, 2, 1_000, 0).unwrap();
    let opened = Instant::now();
    let pool = super::open(file.path.to_str().unwrap(), options)
        .await
        .unwrap();
    let open_ns = opened.elapsed().as_nanos();
    file.observe(&pool.adapter);
    let observer = pool.adapter.observer();
    let lazy = observer.snapshot();
    let first = super::begin(&pool, super::BeginMode::Deferred)
        .await
        .unwrap();
    let second = super::begin(&pool, super::BeginMode::Deferred)
        .await
        .unwrap();
    let active = observer.snapshot();
    let active_threads = threads();
    super::rollback(second).await.unwrap();
    super::rollback(first).await.unwrap();
    super::close(&pool, 2_000).await.unwrap();
    let joined = observer.snapshot();
    assert_eq!(
        (
            lazy.created,
            active.native_started,
            joined.joined,
            joined.pending_workers
        ),
        (0, 2, 2, 0)
    );
    let summary = serde_json::json!({"scope":"public API cap2 normal-size debug snapshot; no performance threshold",
        "open_ns":open_ns,"threads_before":before,"threads_active":active_threads,
        "native_workers":active.native_started,"native_joined":joined.joined,
        "native_pending":joined.pending_workers,"lazy_open_created":lazy.created,
        "thread_model":"one native worker and one independent join observer per started connection",
        "options_bytes":std::mem::size_of::<super::Options>(),"pool_bytes":std::mem::size_of::<super::Pool>(),
        "tx_bytes":std::mem::size_of::<super::Tx>()});
    eprintln!("NAGI_SQLITE_PUBLIC_COST {summary}");
    if let Some(path) = std::env::var_os("NAGI_SQLITE_PUBLIC_COST_OUTPUT") {
        serde_json::to_writer(std::fs::File::create(path).unwrap(), &summary).unwrap();
    }
}
