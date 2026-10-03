use crate::Error;
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicI64, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::{AbortHandle, JoinHandle, JoinSet},
};

struct AbortOnDrop(AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub struct Scope {
    tasks: JoinSet<Result<(), Error>>,
}
impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}
impl Scope {
    pub fn new() -> Self {
        Self {
            tasks: JoinSet::new(),
        }
    }
    pub fn spawn(
        &mut self,
        f: impl std::future::Future<Output = Result<(), Error>> + Send + 'static,
    ) {
        self.tasks.spawn(f);
    }
    pub async fn join(&mut self) -> Result<(), Error> {
        while let Some(r) = self.tasks.join_next().await {
            match r {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    self.tasks.shutdown().await;
                    return Err(e);
                }
                Err(e) => {
                    self.tasks.shutdown().await;
                    return Err(Error::internal(e.to_string()));
                }
            }
        }
        Ok(())
    }
    pub async fn cancel(&mut self) {
        self.tasks.shutdown().await;
    }
    pub fn pending(&self) -> usize {
        self.tasks.len()
    }
}
// DropはJoinSetのabort。正常/エラーの出口ではjoin/cancelをawaitし、実際の破棄完了まで待つ。
enum CounterMessage {
    Add(i64, oneshot::Sender<i64>),
}
#[derive(Clone)]
pub struct CounterActor {
    tx: mpsc::Sender<CounterMessage>,
}
impl CounterActor {
    pub fn start(capacity: usize) -> (Self, JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<CounterMessage>(capacity);
        let task = tokio::spawn(async move {
            let mut state = 0;
            while let Some(CounterMessage::Add(n, reply)) = rx.recv().await {
                state += n;
                let _ = reply.send(state);
            }
        });
        (Self { tx }, task)
    }
    pub async fn add(&self, n: i64) -> Result<i64, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(CounterMessage::Add(n, tx))
            .await
            .map_err(|_| Error::internal("actor stopped"))?;
        rx.await.map_err(|_| Error::internal("actor reply lost"))
    }
    pub fn mailbox_remaining(&self) -> usize {
        self.tx.capacity()
    }
    pub async fn add_pipelined(&self, n: usize, batch: usize) -> Result<i64, Error> {
        let batch = batch.clamp(1, 1024);
        let mut replies = Vec::with_capacity(batch);
        let mut last = 0;
        for start in (0..n).step_by(batch) {
            for _ in start..(start + batch).min(n) {
                let (tx, rx) = oneshot::channel();
                self.tx
                    .send(CounterMessage::Add(1, tx))
                    .await
                    .map_err(|_| Error::internal("actor stopped"))?;
                replies.push(rx);
            }
            // 一件ずつreplyを待つ往復を減らす。全メッセージのreply自体は確認する。
            for rx in replies.drain(..) {
                last = rx.await.map_err(|_| Error::internal("actor reply lost"))?;
            }
        }
        Ok(last)
    }
}
pub async fn actor_demo(n: i64) -> Result<i64, Error> {
    let (actor, task) = CounterActor::start(64);
    let mut r = 0;
    for _ in 0..n.max(0) {
        r = actor.add(1).await?;
    }
    drop(actor);
    task.await.map_err(|e| Error::internal(e.to_string()))?;
    Ok(r)
}
pub async fn actor_pair_demo(n: i64) -> Result<i64, Error> {
    let (counter, counter_task) = CounterActor::start(64);
    let (tx, mut rx) = mpsc::channel::<CounterMessage>(64);
    let forwarder = tokio::spawn(async move {
        while let Some(CounterMessage::Add(n, reply)) = rx.recv().await {
            let value = counter.add(n).await?;
            let _ = reply.send(value);
        }
        Ok::<(), Error>(())
    });
    let mut last = 0;
    for _ in 0..n.max(0) {
        let (reply, receive) = oneshot::channel();
        tx.send(CounterMessage::Add(1, reply))
            .await
            .map_err(|_| Error::internal("forwarder stopped"))?;
        last = receive
            .await
            .map_err(|_| Error::internal("forwarder reply lost"))?;
    }
    drop(tx);
    forwarder
        .await
        .map_err(|e| Error::internal(e.to_string()))??;
    counter_task
        .await
        .map_err(|e| Error::internal(e.to_string()))?;
    Ok(last)
}

#[derive(Debug, Serialize)]
pub struct SupervisorStats {
    pub restarts: usize,
    pub stopped_by_intensity: bool,
    pub restart_latency_ns: Vec<u64>,
    pub unaffected_iterations: i64,
}
pub async fn supervision_test(
    crashes: usize,
    max_restarts: usize,
) -> Result<SupervisorStats, Error> {
    let unaffected = Arc::new(AtomicI64::new(0));
    let a = unaffected.clone();
    let other = tokio::spawn(async move {
        loop {
            a.fetch_add(1, Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });
    // Keep cancellation ownership even while awaiting startup or a child result.
    // Drop only requests abort; normal exits below still abort and join explicitly.
    let _other_guard = AbortOnDrop(other.abort_handle());
    let mut history = VecDeque::new();
    let mut restarts = 0;
    let mut latencies = vec![];
    let mut stopped = false;
    let mut previous_crash: Option<Instant> = None;
    loop {
        let attempt = restarts;
        let (ready_tx, ready_rx) = oneshot::channel();
        let child = tokio::spawn(async move {
            let _ = ready_tx.send(Instant::now());
            tokio::time::sleep(Duration::from_millis(3)).await;
            if attempt < crashes {
                panic!("intentional supervised worker crash {attempt}");
            }
        });
        let _child_guard = AbortOnDrop(child.abort_handle());
        let ready = ready_rx
            .await
            .map_err(|_| Error::internal("child did not start"))?;
        if let Some(crash) = previous_crash.take() {
            latencies.push(ready.duration_since(crash).as_nanos() as u64);
        }
        match child.await {
            Ok(()) => break,
            Err(e) if e.is_panic() => {
                let detected = Instant::now();
                while history
                    .front()
                    .is_some_and(|t: &Instant| t.elapsed() > Duration::from_secs(1))
                {
                    history.pop_front();
                }
                if history.len() >= max_restarts {
                    stopped = true;
                    break;
                }
                history.push_back(Instant::now());
                restarts += 1;
                tokio::time::sleep(Duration::from_millis(1)).await;
                previous_crash = Some(detected);
            }
            Err(e) => {
                other.abort();
                let _ = other.await;
                return Err(Error::internal(e.to_string()));
            }
        }
    }
    other.abort();
    let _ = other.await;
    Ok(SupervisorStats {
        restarts,
        stopped_by_intensity: stopped,
        restart_latency_ns: latencies,
        unaffected_iterations: unaffected.load(Ordering::Relaxed),
    })
}
pub async fn supervisor_demo() -> Result<i64, Error> {
    let s = supervision_test(3, 5).await?;
    Ok(s.restarts as i64)
}
#[derive(Debug, Serialize)]
pub struct TaskStats {
    pub completed: i64,
    pub max_pending: usize,
    pub pending_rss_kib: u64,
}

pub async fn task_test(n: i64) -> Result<TaskStats, Error> {
    struct Gate {
        ready: AtomicUsize,
        release: tokio::sync::Semaphore,
    }
    let n = n.max(0) as usize;
    let gate = Arc::new(Gate {
        ready: AtomicUsize::new(0),
        release: tokio::sync::Semaphore::new(0),
    });
    let mut tasks = JoinSet::new();
    for i in 0..n {
        let gate = gate.clone();
        tasks.spawn(async move {
            gate.ready.fetch_add(1, Ordering::Release);
            gate.release
                .acquire()
                .await
                .map_err(|_| Error::internal("task gate closed"))?
                .forget();
            Ok::<i64, Error>(1)
        });
        if i % 1024 == 0 {
            tokio::task::yield_now().await;
        }
    }
    // 全taskが最初のpollへ到達し、1件も完了できない状態でメモリを測る。
    while gate.ready.load(Ordering::Acquire) != n {
        tokio::task::yield_now().await;
    }
    let pending_rss_kib = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0);
    gate.release.add_permits(n);
    let mut completed = 0;
    while let Some(r) = tasks.join_next().await {
        completed += r.map_err(|e| Error::internal(e.to_string()))??;
    }
    Ok(TaskStats {
        completed,
        max_pending: n,
        pending_rss_kib,
    })
}
pub async fn task_demo(n: i64) -> Result<i64, Error> {
    Ok(task_test(n).await?.completed)
}
pub async fn cpu_sum(n: i64) -> Result<i64, Error> {
    static CPU_LIMIT: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    let permit = CPU_LIMIT
        .get_or_init(|| tokio::sync::Semaphore::new(4))
        .acquire()
        .await
        .map_err(|_| Error::internal("CPU pool closed"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let data = crate::make_ints(n);
        data.iter().sum()
    })
    .await
    .map_err(|e| Error::internal(e.to_string()))
}

#[derive(Debug, Serialize)]
pub struct QueueStats {
    pub completed: usize,
    pub dead_letter: usize,
    pub retries: usize,
    pub peak_in_flight: usize,
}
pub async fn queue_test(n: usize) -> Result<QueueStats, Error> {
    // bounded ingress + 上限8のworker。再試行は同じworker内、終了時に全taskをjoinする。
    let (tx, mut rx) = mpsc::channel(64);
    let producer = tokio::spawn(async move {
        for id in 0..n {
            if tx.send(id).await.is_err() {
                break;
            }
        }
    });
    let mut workers = JoinSet::new();
    let mut stats = QueueStats {
        completed: 0,
        dead_letter: 0,
        retries: 0,
        peak_in_flight: 0,
    };
    fn account(
        stats: &mut QueueStats,
        r: Option<Result<(bool, usize), tokio::task::JoinError>>,
    ) -> Result<(), Error> {
        let (success, retries) = r
            .ok_or_else(|| Error::internal("missing queue worker"))?
            .map_err(|e| Error::internal(e.to_string()))?;
        if success {
            stats.completed += 1
        } else {
            stats.dead_letter += 1
        }
        stats.retries += retries;
        Ok(())
    }
    while let Some(id) = rx.recv().await {
        if workers.len() >= 8 {
            account(&mut stats, workers.join_next().await)?;
        }
        workers.spawn(async move {
            let mut retry = 0;
            loop {
                let fails = id % 11 == 0 || id % 7 == 0 && retry == 0;
                if !fails {
                    return (true, retry);
                }
                if retry == 2 {
                    return (false, retry);
                }
                tokio::time::sleep(Duration::from_millis(1 << retry)).await;
                retry += 1;
            }
        });
        stats.peak_in_flight = stats.peak_in_flight.max(workers.len());
    }
    while !workers.is_empty() {
        account(&mut stats, workers.join_next().await)?;
    }
    producer.await.map_err(|e| Error::internal(e.to_string()))?;
    Ok(stats)
}
pub async fn queue_demo(n: i64) -> Result<i64, Error> {
    Ok(queue_test(n.max(0) as usize).await?.completed as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn actor_pair_and_pipeline() {
        assert_eq!(actor_pair_demo(100).await.unwrap(), 100);
        let (a, t) = CounterActor::start(8);
        assert_eq!(a.add_pipelined(1000, 32).await.unwrap(), 1000);
        assert_eq!(a.add(1).await.unwrap(), 1001);
        drop(a);
        t.await.unwrap();
    }
    #[tokio::test]
    async fn scope_finishes() {
        let mut s = Scope::new();
        for _ in 0..100 {
            s.spawn(async { Ok(()) });
        }
        s.join().await.unwrap();
        assert_eq!(s.pending(), 0);
    }
    #[tokio::test]
    async fn scope_cancel_drops_all() {
        let count = Arc::new(AtomicI64::new(0));
        struct Guard(Arc<AtomicI64>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let mut s = Scope::new();
        for _ in 0..100 {
            let c = count.clone();
            s.spawn(async move {
                c.fetch_add(1, Ordering::SeqCst);
                let _g = Guard(c);
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(())
            });
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        s.cancel().await;
        assert_eq!(s.pending(), 0);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn sibling_error_cancelled() {
        let mut s = Scope::new();
        s.spawn(async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        });
        s.spawn(async { Err(Error::invalid("test")) });
        assert!(s.join().await.is_err());
        assert_eq!(s.pending(), 0);
    }
    #[tokio::test]
    async fn panic_isolated() {
        let mut s = Scope::new();
        s.spawn(async {
            panic!("test child panic");
            #[allow(unreachable_code)]
            Ok(())
        });
        assert!(s.join().await.is_err());
        assert_eq!(actor_demo(5).await.unwrap(), 5);
    }
    #[tokio::test]
    async fn actors_isolated() {
        let (a, ta) = CounterActor::start(2);
        let (b, tb) = CounterActor::start(2);
        assert_eq!(a.add(3).await.unwrap(), 3);
        assert_eq!(b.add(5).await.unwrap(), 5);
        assert_eq!(a.add(1).await.unwrap(), 4);
        drop(a);
        drop(b);
        ta.await.unwrap();
        tb.await.unwrap();
    }
    #[tokio::test]
    async fn supervisor_restart() {
        let s = supervision_test(2, 4).await.unwrap();
        assert_eq!(s.restarts, 2);
        assert!(!s.stopped_by_intensity);
        assert!(s.unaffected_iterations > 0);
    }
    #[tokio::test]
    async fn supervisor_intensity() {
        let s = supervision_test(100, 2).await.unwrap();
        assert_eq!(s.restarts, 2);
        assert!(s.stopped_by_intensity);
    }
    fn assert_supervisor_drop_cancels_tasks(after_worker_start: bool) {
        use std::future::{poll_fn, Future};
        use std::task::Poll;

        // A separate runtime makes its live-task count independent of other tests.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let metrics = runtime.metrics();
        runtime.block_on(async {
            let mut parent = Box::pin(supervision_test(usize::MAX, usize::MAX));
            poll_fn(|cx| {
                assert!(parent.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            // Both children have been registered; neither has been polled yet.
            assert_eq!(metrics.num_alive_tasks(), 2);
            if after_worker_start {
                tokio::task::yield_now().await;
                poll_fn(|cx| {
                    assert!(parent.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
                assert!(metrics.num_alive_tasks() > 0);
            }
            drop(parent);
            // Drop requests cancellation; the runtime still has to poll cleanup.
            for _ in 0..16 {
                tokio::task::yield_now().await;
            }
            assert_eq!(metrics.num_alive_tasks(), 0);
        });
    }
    #[test]
    fn supervisor_drop_while_waiting_for_worker_start() {
        assert_supervisor_drop_cancels_tasks(false);
    }
    #[test]
    fn supervisor_drop_after_worker_start() {
        assert_supervisor_drop_cancels_tasks(true);
    }
    #[tokio::test]
    async fn queue_retries_and_dead_letter() {
        let s = queue_test(40).await.unwrap();
        assert_eq!(s.completed + s.dead_letter, 40);
        assert_eq!(s.dead_letter, 4);
        assert!(s.retries > 0);
        assert!(s.peak_in_flight <= 8);
    }
    #[tokio::test]
    async fn tasks_and_cpu() {
        let s = task_test(1000).await.unwrap();
        assert_eq!(s.completed, 1000);
        assert_eq!(s.max_pending, 1000);
        assert_eq!(
            cpu_sum(100).await.unwrap(),
            crate::make_ints(100).iter().sum::<i64>()
        );
    }
}
