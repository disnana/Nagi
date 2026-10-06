//! debug profileのprivate比較入口。測定値に合否thresholdやproduction性能保証を設けない。
use super::{
    adapter::{Adapter, AdapterSeams},
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
async fn direct_and_deadpool_warm_native_session_comparison() {
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
                    "backend": if use_adapter { "deadpool" } else { "direct" }, "elapsed_ns": elapsed }));
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
        "direct": percentiles(&direct_samples), "deadpool": percentiles(&adapter_samples),
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
