fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    println!("{}", serde_json::json!({
        "name":"batch_future_sizes", "target":std::env::consts::ARCH,
        "receive_bytes":std::mem::size_of_val(&task_cost::receive_batch(128)),
        "discard_bytes":std::mem::size_of_val(&task_cost::discard_batch(128))
    }));
    for n in [128_i64, 1024, 8192] {
        nagi_runtime::metrics::benchmark("receive", n as usize, || {
            let value = runtime.block_on(task_cost::receive_batch(n)).unwrap();
            assert_eq!(value, n * (n - 1) / 2);
        });
        nagi_runtime::metrics::benchmark("discard", n as usize, || {
            assert_eq!(runtime.block_on(task_cost::discard_batch(n)).unwrap(), 0);
        });
    }
}
