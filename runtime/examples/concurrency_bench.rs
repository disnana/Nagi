use nagi_runtime as rt;
use serde_json::json;
use std::time::Instant;

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .map(str::to_owned)
        })
        .and_then(|l| l.split_whitespace().nth(1).and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    for n in [1000, 10000, 50000, 100000] {
        for repeat in 0..3 {
            let before = rss_kib();
            let t = Instant::now();
            let stats = rt::task_test(n).await.unwrap();
            let elapsed = t.elapsed();
            println!(
                "{}",
                json!({"name":"task_create_join","n":n,"repeat":repeat,"completed":stats.completed,"unfinished":n-stats.completed,"max_pending":stats.max_pending,"pending_rss_kib":stats.pending_rss_kib,"elapsed_ms":elapsed.as_secs_f64()*1000.,"rss_before_kib":before,"rss_after_kib":rss_kib()})
            );
        }
    }
    for repeat in 0..5 {
        let n = 20000;
        let t = Instant::now();
        let done = rt::actor_demo(n).await.unwrap();
        let elapsed = t.elapsed();
        println!(
            "{}",
            json!({"name":"actor_rpc","repeat":repeat,"messages":n,"final":done,"messages_s":n as f64/elapsed.as_secs_f64(),"mean_roundtrip_us":elapsed.as_secs_f64()*1e6/n as f64,"mailbox_capacity":64,"rss_kib":rss_kib()})
        );
    }
    let fault = rt::supervision_test(3, 5).await.unwrap();
    for repeat in 0..5 {
        let (actor, task) = rt::CounterActor::start(64);
        let n = 20000;
        let t = Instant::now();
        let done = actor.add_pipelined(n, 32).await.unwrap();
        let elapsed = t.elapsed();
        drop(actor);
        task.await.unwrap();
        println!(
            "{}",
            json!({"name":"actor_pipelined","repeat":repeat,"messages":n,"final":done,"batch":32,"messages_s":n as f64/elapsed.as_secs_f64(),"mean_completion_us":elapsed.as_secs_f64()*1e6/n as f64,"mailbox_capacity":64,"rss_kib":rss_kib()})
        );
    }
    println!("{}", json!({"name":"supervisor_recovery","stats":fault}));
    let looped = rt::supervision_test(100, 2).await.unwrap();
    println!("{}", json!({"name":"supervisor_crash_loop","stats":looped}));
    println!(
        "{}",
        json!({"name":"queue","stats":rt::queue_test(1000).await.unwrap(),"rss_kib":rss_kib()})
    );
    // 非同期DB workerの生成と破棄を反復し、native thread/connectionを残さない。
    let baseline = rt::Db::live_workers();
    for _ in 0..100 {
        let db = rt::Db::open(":memory:").await.unwrap();
        db.exec("SELECT 1").await.unwrap();
        drop(db);
    }
    println!(
        "{}",
        json!({"name":"db_worker_lifecycle","opened_closed":100,"before":baseline,"after":rt::Db::live_workers()})
    );
}
