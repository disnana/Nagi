use serde::Serialize;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    time::Instant,
};

#[derive(Clone, Copy, Default, Debug, Serialize)]
pub struct Allocations {
    pub allocations: u64,
    pub reallocations: u64,
    pub allocated_bytes: u64,
    pub deallocations: u64,
}
thread_local! {static ENABLED:Cell<bool>=const{Cell::new(false)};static COUNTS:Cell<Allocations>=const{Cell::new(Allocations{allocations:0,reallocations:0,allocated_bytes:0,deallocations:0})};}
struct CountingAllocator;
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
fn count(size: usize, alloc: bool, realloc: bool) {
    if ENABLED.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNTS.try_with(|c| {
            let mut v = c.get();
            if alloc {
                v.allocations += 1;
                v.allocated_bytes += size as u64;
            } else {
                v.deallocations += 1;
            }
            if realloc {
                v.reallocations += 1;
            }
            c.set(v);
        });
    }
}
// Systemへ同じLayout/pointerを転送するだけ。観測コード自体ではallocationしない。
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            count(l.size(), true, false);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(l);
        if !p.is_null() {
            count(l.size(), true, false);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        count(l.size(), false, false);
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = System.realloc(p, l, n);
        if !q.is_null() {
            count(n, true, true);
        }
        q
    }
}
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, Allocations) {
    struct Disable;
    impl Drop for Disable {
        fn drop(&mut self) {
            ENABLED.with(|e| e.set(false));
        }
    }
    assert!(!ENABLED.with(Cell::get), "nested allocation measurement");
    COUNTS.with(|c| c.set(Allocations::default()));
    ENABLED.with(|e| e.set(true));
    let guard = Disable;
    let value = f();
    let c = COUNTS.with(Cell::get);
    drop(guard);
    (value, c)
}
pub fn benchmark<T>(name: &str, items: usize, mut f: impl FnMut() -> T) {
    for _ in 0..5 {
        std::hint::black_box(f());
    }
    let mut ns = vec![];
    let pilot = Instant::now();
    std::hint::black_box(f());
    let pilot_ns = pilot.elapsed().as_nanos().max(1) as usize;
    let loops = std::env::var("NAGI_BENCH_LOOPS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(|| (10_000_000 / pilot_ns).clamp(25, 100_000))
        .max(1);
    for _ in 0..7 {
        let t = Instant::now();
        for _ in 0..loops {
            std::hint::black_box(f());
        }
        ns.push(t.elapsed().as_nanos() as f64 / loops as f64);
    }
    let (value, counts) = measure(&mut f);
    std::hint::black_box(value);
    let mut sorted = ns.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    println!(
        "{}",
        serde_json::json!({"name":name,"items":items,"loops_per_repetition":loops,"ns_per_op":median,"ns_per_item":median/items.max(1) as f64,"throughput_items_s":items as f64*1e9/median,"raw_ns":ns,"allocation_counter_thread":"calling thread only","allocation":counts})
    );
}
