//! Production-layout TaskScope cost baseline. Retention internals are measured by
//! the separate explicitly invoked task::cost_tests native unit measurement.
use nagi_runtime::{metrics, Task, TaskFailure, TaskFailureKind, TaskScope};
use std::mem::{size_of, size_of_val};

async fn receive_batch(n: usize) {
    let mut scope = TaskScope::new();
    let tasks: Vec<_> = (0..n)
        .map(|value| scope.spawn_value(async move { value }))
        .collect();
    for (value, task) in tasks.into_iter().enumerate() {
        assert_eq!(scope.receive(task).await.unwrap(), value);
    }
    scope.join().await.unwrap();
}
async fn discard_batch(n: usize) {
    let mut scope = TaskScope::new();
    for value in 0..n {
        let task = scope.spawn_value(async move { value });
        scope.discard(task);
    }
    scope.join().await.unwrap();
}
fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut scope = TaskScope::new();
        let task = scope.spawn_value(async { 1_u64 });
        let receiving = scope.receive(task);
        let receive_bytes = size_of_val(&receiving);
        drop(receiving);
        let joining = scope.join();
        let join_bytes = size_of_val(&joining);
        drop(joining);
        let cancelling = scope.cancel();
        let cancel_bytes = size_of_val(&cancelling);
        drop(cancelling);
        scope.cancel().await.unwrap();
        println!(
            "{}",
            serde_json::json!({
                "name":"task_handle_production_sizes", "target":std::env::consts::ARCH,
                "scope_bytes":size_of::<TaskScope>(), "handle_u64_bytes":size_of::<Task<u64>>(),
                "failure_bytes":size_of::<TaskFailure>(), "kind_bytes":size_of::<TaskFailureKind>(),
                "receive_future_bytes":receive_bytes, "join_future_bytes":join_bytes,
                "cancel_future_bytes":cancel_bytes,
                "receive_batch_future_bytes":size_of_val(&receive_batch(128)),
                "discard_batch_future_bytes":size_of_val(&discard_batch(128))
            })
        );
    });
    for n in [128, 1024, 8192] {
        metrics::benchmark("taskscope_production_receive", n, || {
            runtime.block_on(receive_batch(n))
        });
        metrics::benchmark("taskscope_production_discard", n, || {
            runtime.block_on(discard_batch(n))
        });
    }
}
