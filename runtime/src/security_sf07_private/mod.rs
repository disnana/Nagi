//! Private SF07 quota ownership; other core/fixture modules remain test-only.
#[cfg(test)]
mod indexed;
#[cfg(test)]
mod legacy_rate;
pub(crate) mod quota;
#[cfg(test)]
mod startup_rate;

#[cfg(test)]
fn allocations_in<T>(f: impl FnOnce() -> T) -> (T, u64) {
    let (value, counts) = crate::metrics::measure(f);
    (value, counts.allocations)
}

#[cfg(test)]
fn calibrate_allocation_meter() {
    // The runtime's existing allocator counts only the calling thread.
    // Exercise simultaneous zero and positive controls without serializing tests.
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let allocating = scope.spawn(|| {
            barrier.wait();
            let (data, count) = allocations_in(|| {
                let data = Vec::<u8>::with_capacity(std::hint::black_box(128));
                barrier.wait();
                data
            });
            std::hint::black_box(&data);
            assert!(count > 0);
        });
        barrier.wait();
        let (answer, count) = allocations_in(|| {
            barrier.wait();
            std::hint::black_box(7)
        });
        assert_eq!(answer, 7);
        assert_eq!(count, 0);
        allocating.join().unwrap();
    });
}
