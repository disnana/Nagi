use super::Fixture;

#[test]
fn discarded_child_keeps_working_until_parent_finish_is_dropped() {
    Fixture::new().run_three("task-finish-drop", HIGH, LOW, ADAPTER, ASSERTIONS);
}

const HIGH: &str = r#"
from std.task import discard
@rust("native::work")
extern async def work() -> i64
async def exercise() -> Result[unit, Error]:
    async with scope:
        handle = spawn work()
        discard(handle)
    return ok(assert_true(True))
"#;

const LOW: &str = r#"
from std.task import discard;
@rust("native::work")
extern async fn work() -> i64;
async fn exercise() -> Result[unit, Error] {
    scope {
        let handle = spawn work();
        discard(handle);
    }
    return ok(assert_true(true));
}
"#;

const ADAPTER: &str = r#"
mod native {
    use std::future::pending;
    use tokio::sync::Notify;

    pub static STARTED: Notify = Notify::const_new();
    pub static WORK: Notify = Notify::const_new();
    pub static WORKED: Notify = Notify::const_new();
    pub static CLEANED: Notify = Notify::const_new();
    pub static DID_CLEANUP: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            DID_CLEANUP.store(true, std::sync::atomic::Ordering::SeqCst);
            CLEANED.notify_one();
        }
    }

    pub async fn work() -> i64 {
        let _cleanup = Cleanup;
        STARTED.notify_one();
        WORK.notified().await;
        WORKED.notify_one();
        pending().await
    }
}
"#;

const ASSERTIONS: &str = r#"
#[test]
fn finish_wait_and_owner_drop() {
    use std::{future::{poll_fn, Future}, sync::atomic::Ordering, task::Poll, time::Duration};
    let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let mut parent = Box::pin(exercise());
    let deadline = Duration::from_secs(5);
    runtime.block_on(async {
        // The checked body contains only spawn and discard. Its first
        // suspension is the generated scope join.
        poll_fn(|cx| {
            assert!(parent.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        }).await;
        tokio::time::timeout(deadline, native::STARTED.notified()).await.unwrap();
        native::WORK.notify_one();
        tokio::time::timeout(deadline, native::WORKED.notified()).await.unwrap();
        poll_fn(|cx| {
            assert!(parent.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        }).await;
    });
    drop(parent);
    assert!(!native::DID_CLEANUP.load(Ordering::SeqCst));
    // The same live executor can observe cleanup after the parent has exited.
    runtime.block_on(async {
        tokio::time::timeout(deadline, native::CLEANED.notified()).await.unwrap();
    });
}
"#;
