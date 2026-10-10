use nagi_runtime::{metrics, sqlite as db};
use std::time::Instant;
fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (pool, open) = metrics::measure(|| {
        runtime
            .block_on(db::open(":memory:", db::options(1, 2, 1000, 0).unwrap()))
            .unwrap()
    });
    let tx = runtime
        .block_on(db::begin(&pool, db::BeginMode::Deferred))
        .unwrap();
    runtime
        .block_on(db::exec(
            &tx,
            db::literal("CREATE TABLE data(value)"),
            db::parameters(),
        ))
        .unwrap();
    println!(
        "{}",
        serde_json::json!({"name":"future_and_open", "arch":std::env::consts::ARCH,
        "literal_future_bytes":std::mem::size_of_val(&sqlite_cost::literal(&tx, 1)),
        "selected_future_bytes":std::mem::size_of_val(&sqlite_cost::selected(&tx, db::literal("INSERT INTO data VALUES (?)"), 1)),
        "lazy_open_allocation":open,"allocation_scope":"calling thread only; native SQLite, worker and observer allocations excluded"})
    );
    for mode in ["literal", "selected"] {
        for _ in 0..4 {
            let count = runtime
                .block_on(async {
                    if mode == "literal" {
                        sqlite_cost::literal(&tx, 1).await
                    } else {
                        sqlite_cost::selected(&tx, db::literal("INSERT INTO data VALUES (?)"), 1).await
                    }
                })
                .unwrap();
            assert_eq!(count, 1);
        }
        let mut samples = Vec::new();
        for _ in 0..32 {
            let started = Instant::now();
            let (count, allocation) = metrics::measure(|| {
                runtime
                    .block_on(async {
                        if mode == "literal" {
                            sqlite_cost::literal(&tx, 1).await
                        } else {
                            sqlite_cost::selected(&tx, db::literal("INSERT INTO data VALUES (?)"), 1).await
                        }
                    })
                    .unwrap()
            });
            let ns = started.elapsed().as_nanos();
            assert_eq!(count, 1);
            samples.push(serde_json::json!({"ns":ns,"allocation":allocation}));
        }
        println!(
            "{}",
            serde_json::json!({"name":mode,"samples":samples,
            "conditions":"release, one active transaction, 4 warmup + 32 measured inserts; no throughput or performance threshold"})
        );
    }
    runtime.block_on(db::rollback(tx)).unwrap();
    let (_, close) = metrics::measure(|| runtime.block_on(db::close(&pool, 2000)).unwrap());
    println!(
        "{}",
        serde_json::json!({"name":"close_after_rollback","allocation":close})
    );
}
