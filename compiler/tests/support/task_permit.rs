use super::Fixture;

#[test]
fn single_child_releases_owned_permit_at_cleanup() {
    Fixture::new().run_three("task-permit", HIGH, LOW, ADAPTER, ASSERTIONS);
}

const HIGH: &str = r#"
from std.task import discard
@rust("native::work")
extern async def work(mode: i64) -> i64
async def exercise(mode: i64) -> Result[unit, Error]:
    async with scope:
        handle = spawn work(mode)
        discard(handle)
    return ok(assert_true(True))
"#;

const LOW: &str = r#"
from std.task import discard;
@rust("native::work")
extern async fn work(mode: i64) -> i64;
async fn exercise(mode: i64) -> Result[unit, Error] {
    scope {
        let handle = spawn work(mode);
        discard(handle);
    }
    return ok(assert_true(true));
}
"#;

const ADAPTER: &str = r#"
mod native {
    use std::{
        sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, mpsc, Arc, Mutex},
        time::Duration,
    };
    use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

    pub struct Round {
        pub pool: Arc<Semaphore>,
        pub acquired: Notify,
        pub finish_work: Notify,
        pub cleanup_entered: Notify,
        pub cleanup_calls: AtomicUsize,
        pub gate_failed: AtomicBool,
    }
    static NEXT: Mutex<Option<(Arc<Round>, mpsc::Receiver<()>)>> = Mutex::new(None);

    pub fn prepare() -> (Arc<Round>, mpsc::Sender<()>) {
        let round = Arc::new(Round {
            pool: Arc::new(Semaphore::new(1)),
            acquired: Notify::new(),
            finish_work: Notify::new(),
            cleanup_entered: Notify::new(),
            cleanup_calls: AtomicUsize::new(0),
            gate_failed: AtomicBool::new(false),
        });
        let (release, gate) = mpsc::channel();
        assert!(NEXT.lock().unwrap().replace((Arc::clone(&round), gate)).is_none());
        (round, release)
    }

    struct Held {
        _permit: OwnedSemaphorePermit,
        round: Arc<Round>,
        gate: mpsc::Receiver<()>,
    }

    impl Drop for Held {
        fn drop(&mut self) {
            self.round.cleanup_calls.fetch_add(1, Ordering::SeqCst);
            self.round.cleanup_entered.notify_one();
            // The permit field is dropped after the gate, including on unwind.
            if self.gate.recv_timeout(Duration::from_secs(5)).is_err() {
                self.round.gate_failed.store(true, Ordering::SeqCst);
            }
        }
    }

    pub async fn work(mode: i64) -> i64 {
        let (round, gate) = NEXT.lock().unwrap().take().unwrap();
        let _held = Held { _permit: Arc::clone(&round.pool).try_acquire_owned().unwrap(),
            round: Arc::clone(&round), gate };
        round.acquired.notify_one();
        if mode == 0 {
            nagi_runtime::sleep(86400000).await;
        } else {
            nagi_runtime::sleep(0).await;
            round.finish_work.notified().await;
            assert_ne!(mode, 2, "controlled child panic");
        }
        17
    }
}
"#;

const ASSERTIONS: &str = r#"
#[test]
fn owner_exit_cleanup_and_readmission() {
    use std::{future::{poll_fn, Future}, sync::{atomic::Ordering, Arc}, task::Poll, time::Duration};
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2)
        .enable_time().build().unwrap();
    let deadline = Duration::from_secs(5);
    for mode in [0, 1, 2] {
        let (round, release) = native::prepare();
        let mut parent = Some(Box::pin(exercise(mode)));
        runtime.block_on(async {
            poll_fn(|cx| {
                assert!(parent.as_mut().unwrap().as_mut().poll(cx).is_pending());
                Poll::Ready(())
            }).await;
            tokio::time::timeout(deadline, round.acquired.notified()).await.unwrap();
            assert!(round.pool.try_acquire().is_err());
            if mode == 0 {
                drop(parent.take());
            } else {
                round.finish_work.notify_one();
            }
            tokio::time::timeout(deadline, round.cleanup_entered.notified()).await.unwrap();
            assert!(round.pool.try_acquire().is_err(), "permit returned during cleanup");
            if let Some(parent) = parent.as_mut() {
                poll_fn(|cx| {
                    assert!(parent.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                }).await;
            }
            release.send(()).unwrap();
            let held = tokio::time::timeout(deadline, Arc::clone(&round.pool).acquire_owned())
                .await.unwrap().unwrap();
            assert!(round.pool.try_acquire().is_err(), "capacity exceeded one");
            if let Some(parent) = parent {
                let exit = tokio::time::timeout(deadline, parent).await.unwrap();
                if mode == 2 {
                    assert_eq!(exit.unwrap_err().message, "child panicked");
                } else {
                    exit.unwrap();
                }
            }
            assert_eq!(round.cleanup_calls.load(Ordering::SeqCst), 1);
            assert!(!round.gate_failed.load(Ordering::SeqCst));
            drop(held);
            assert_eq!(round.pool.available_permits(), 1);
        });
    }
}
"#;
