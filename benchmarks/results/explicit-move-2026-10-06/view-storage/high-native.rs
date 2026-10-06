pub fn marker(id: i64) -> super::Marker {
    super::Marker { id, tag: super::Tag::Text { value: String::new() } }
}

pub fn capture(id: i64) {
    super::CAPTURE_VEC_ID.store(id as usize, ::std::sync::atomic::Ordering::Relaxed);
}

pub fn bad_index() -> i64 {
    // Clear the capture before panicking and prove the old alias Vec is still
    // alive while Rust is evaluating the replacement RHS.
    super::CAPTURE_VEC_ID.store(0, ::std::sync::atomic::Ordering::Relaxed);
    super::record(super::BAD_INDEX);
    // Vec construction has allocated the replacement before evaluating its
    // element. Both the old buffer and the incomplete RHS buffer are alive.
    assert_eq!(super::tracked_count(), 2, "both buffers must survive RHS evaluation");
    panic!("intentional bad index");
}

pub fn crash() {
    super::CAPTURE_VEC_ID.store(0, ::std::sync::atomic::Ordering::Relaxed);
    super::record(super::CRASH);
    assert_eq!(super::tracked_count(), 1, "only the replacement buffer remains");
    panic!("intentional panic after replacement");
}

pub fn consume(parts: Vec<&str>) -> bool {
    !parts.is_empty()
}

pub fn checkpoint() {
    assert_eq!(super::tracked_count(), 1, "the old buffer must be freed by the assignment");
    super::record(super::CHECKPOINT);
}

pub fn abort_mutation() {
    assert_eq!(super::tracked_count(), 1, "moving the mutation storage must not allocate");
    super::record(super::MUTATION_PANIC);
    panic!("intentional panic while evaluating append argument");
}

pub fn fail_step() -> Result<(), i64> {
    assert_eq!(super::tracked_count(), 1, "error propagation starts after old-buffer cleanup");
    super::record(super::CHECKPOINT);
    Err(7)
}

pub async fn pause() {
    ::std::future::poll_fn(|_| {
        if super::PAUSE_READY.load(::std::sync::atomic::Ordering::Relaxed) {
            ::std::task::Poll::Ready(())
        } else {
            ::std::task::Poll::Pending
        }
    }).await
}

impl Drop for super::Marker {
    fn drop(&mut self) {
        super::record(super::DROP_BASE + self.id as usize);
        if super::PANIC_MARKER_ID
            .compare_exchange(self.id as usize, 0,
                              ::std::sync::atomic::Ordering::Relaxed,
                              ::std::sync::atomic::Ordering::Relaxed)
            .is_ok()
        {
            panic!("intentional panic while destroying a replaced element");
        }
    }
}
