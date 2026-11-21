use super::Fixture;

#[test]
fn body_panic_abandons_output_and_requests_abort_without_joining() {
    Fixture::new().run_three("task-body-panic", HIGH, LOW, ADAPTER, ASSERTIONS);
}

const HIGH: &str = r#"
class Payload:
    marker: str
@rust("native::work")
extern async def work(buffer_output: bool) -> Payload
@rust("native::wait_for_child")
extern async def wait_for_child() -> unit
@rust("native::panic_body")
extern def panic_body() -> unit
async def exercise(buffer_output: bool) -> Result[unit, Error]:
    async with scope:
        handle = spawn work(buffer_output)
        await wait_for_child()
        panic_body()
        received = await handle
    return ok(assert_true(True))
"#;

const LOW: &str = r#"
record Payload { marker: str; }
@rust("native::work")
extern async fn work(buffer_output: bool) -> Payload;
@rust("native::wait_for_child")
extern async fn wait_for_child() -> unit;
@rust("native::panic_body")
extern fn panic_body() -> unit;
async fn exercise(buffer_output: bool) -> Result[unit, Error] {
    scope {
        let handle = spawn work(buffer_output);
        await wait_for_child();
        panic_body();
        let received = await handle;
    }
    return ok(assert_true(true));
}
"#;

const ADAPTER: &str = r#"
mod native {
    use std::{cell::RefCell, future::pending};
    use tokio::sync::Notify;

    #[derive(Debug, PartialEq, Eq)]
    pub enum Event {
        BodyPanic,
        OutputDropped { unwinding: bool },
        ChildCleanup,
    }

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    }
    static CHILD_READY: Notify = Notify::const_new();
    pub static CHILD_CLEANED: Notify = Notify::const_new();

    pub fn record(event: Event) {
        EVENTS.with(|events| events.borrow_mut().push(event));
    }

    pub fn take_events() -> Vec<Event> {
        EVENTS.with(|events| events.take())
    }

    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            record(Event::ChildCleanup);
            CHILD_CLEANED.notify_one();
        }
    }

    pub async fn work(buffer_output: bool) -> super::Payload {
        if buffer_output {
            // On the current-thread executor, this task sends its output
            // before the woken scope body can run. There is no further await.
            CHILD_READY.notify_one();
            return super::Payload { marker: "buffered".into() };
        }
        let _cleanup = Cleanup;
        CHILD_READY.notify_one();
        pending().await
    }

    pub async fn wait_for_child() {
        CHILD_READY.notified().await;
    }

    #[derive(Debug)]
    pub struct BodyPanic;

    pub fn panic_body() {
        record(Event::BodyPanic);
        std::panic::panic_any(BodyPanic);
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        native::record(native::Event::OutputDropped {
            unwinding: std::thread::panicking(),
        });
    }
}
"#;

const ASSERTIONS: &str = r#"
#[test]
fn observe_generated_unwind() {
    use native::Event;
    use std::{panic::{catch_unwind, AssertUnwindSafe}, time::Duration};

    let timeout = Duration::from_secs(5);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    for buffer_output in [true, false] {
        let mut future = Box::pin(exercise(buffer_output));
        let panic = catch_unwind(AssertUnwindSafe(|| {
            runtime.block_on(async {
                tokio::time::timeout(timeout, future.as_mut()).await
            })
        }))
        .expect_err("scope body panic must propagate to its caller");
        assert!(panic.is::<native::BodyPanic>(), "caller saw a different panic");

        if buffer_output {
            assert_eq!(native::take_events(), [
                Event::BodyPanic,
                Event::OutputDropped { unwinding: true },
            ]);
        } else {
            // This executor cannot poll the child while the caller is outside
            // block_on. The unwind reached the caller before child cleanup.
            assert_eq!(native::take_events(), [Event::BodyPanic]);
            // Keep the panicked parent future and runtime alive throughout.
            // Cleanup of this permanently pending child requires owner abort.
            runtime.block_on(async {
                tokio::time::timeout(timeout, native::CHILD_CLEANED.notified())
                    .await
                    .expect("body unwind must request abort before parent future is dropped");
            });
            assert_eq!(native::take_events(), [Event::ChildCleanup]);
        }
        drop(future);
    }
}
"#;
