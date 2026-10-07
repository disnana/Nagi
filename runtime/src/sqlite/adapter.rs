//! Public SQLite checkout adapter. Tokio owns FIFO slot waiting and cancellation.
//! native session/closeはsession.rs、ここはowned checkoutと終了観測のadapterだけ。
use super::session::{
    native_worker, BeginRequest, Config, Failure, Gate, Kind, Outcome, State, Tx,
};
use std::collections::VecDeque;
use std::ops::Deref;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, Notify, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

const WATCHDOG: Duration = Duration::from_secs(10);
type ReadyReply = Arc<Mutex<Option<oneshot::Sender<Result<(), Failure>>>>>;

// Tokio owns FIFO waiting, permit cancellation, and closing wakeups. This small
// adapter owns only lazy idle handles and cleanup-before-return checkout ownership.
struct LogicalPool {
    manager: NativeManager,
    idle: Mutex<VecDeque<WorkerHandle>>,
    semaphore: Arc<Semaphore>,
}
struct NativePool(Arc<LogicalPool>);
struct Checkout {
    worker: Option<WorkerHandle>,
    pool: Weak<LogicalPool>,
    _permit: Option<OwnedSemaphorePermit>,
}
impl Deref for Checkout {
    type Target = WorkerHandle;
    fn deref(&self) -> &WorkerHandle {
        self.worker.as_ref().unwrap()
    }
}
impl Checkout {
    #[cfg(test)]
    fn take(mut this: Self) -> WorkerHandle {
        let mut worker = this.worker.take().unwrap();
        this._permit.take(); // fixture preserves the pre-detach permit gap oracle.
        if let Some(pool) = this.pool.upgrade() {
            pool.manager.detach(&mut worker);
        }
        worker
    }
}
impl Drop for Checkout {
    fn drop(&mut self) {
        let Some(worker) = self.worker.take() else {
            return;
        };
        let Some(pool) = self.pool.upgrade() else {
            drop(worker);
            return;
        };
        let mut worker = Some(worker);
        let error = {
            let mut idle = pool.idle.lock().unwrap();
            if pool.semaphore.is_closed() {
                None
            } else if pool.manager.seams.fail_idle_reserve || idle.try_reserve(1).is_err() {
                Some(Failure::allocation("SQLite idle handle reservation failed"))
            } else {
                idle.push_back(worker.take().unwrap());
                None
            }
        };
        if let Some(mut error) = error {
            error.retired = true;
            pool.manager.ledger.fail(error); // cause -> shared stop/drain before native Drop/join.
        }
        drop(worker); // lock-free closure request; ledger keeps actual join responsibility.
    }
}
impl NativePool {
    fn new(manager: NativeManager, capacity: usize) -> Self {
        Self(Arc::new(LogicalPool {
            manager,
            idle: Mutex::new(VecDeque::new()),
            semaphore: Arc::new(Semaphore::new(capacity)),
        }))
    }
    fn close(&self) {
        self.0.semaphore.close();
        let idle = std::mem::take(&mut *self.0.idle.lock().unwrap());
        drop(idle); // never while holding the queue or native ledger locks.
    }
    #[cfg(test)]
    fn available(&self) -> usize {
        self.0.idle.lock().unwrap().len()
    }
    async fn get(&self, budget: Option<AcquireBudget>) -> Result<Checkout, Failure> {
        if let Some(error) = self.0.manager.ledger.error() {
            return Err(error);
        }
        let semaphore = Arc::clone(&self.0.semaphore);
        let permit = match budget {
            Some(AcquireBudget::Immediate) => {
                semaphore.try_acquire_owned().map_err(|error| match error {
                    tokio::sync::TryAcquireError::Closed => {
                        Failure::primary(Kind::Closed, Outcome::NotApplicable, "SQLite pool closed")
                    }
                    tokio::sync::TryAcquireError::NoPermits => Failure::primary(
                        Kind::AcquireTimeout,
                        Outcome::NotApplicable,
                        "logical slot not immediately available",
                    ),
                })
            }
            Some(AcquireBudget::Deadline(deadline)) => {
                match tokio::time::timeout_at(deadline, semaphore.acquire_owned()).await {
                    Ok(result) => result.map_err(|_| {
                        Failure::primary(Kind::Closed, Outcome::NotApplicable, "SQLite pool closed")
                    }),
                    Err(_) => Err(Failure::primary(
                        Kind::AcquireTimeout,
                        Outcome::NotApplicable,
                        "logical slot reservation timed out",
                    )),
                }
            }
            None => semaphore.acquire_owned().await.map_err(|_| {
                Failure::primary(Kind::Closed, Outcome::NotApplicable, "SQLite pool closed")
            }),
        }
        .map_err(|error| self.0.manager.ledger.error().unwrap_or(error))?;
        if let Some(error) = self.0.manager.ledger.error() {
            return Err(error);
        }
        let worker = self.0.idle.lock().unwrap().pop_front();
        let result = if let Some(mut worker) = worker {
            self.0.manager.recycle(&mut worker).await.map(|()| worker)
        } else {
            self.0.manager.create(budget).await
        };
        let worker = result.map_err(|error| self.0.manager.ledger.error().unwrap_or(error))?;
        Ok(Checkout {
            worker: Some(worker),
            pool: Arc::downgrade(&self.0),
            _permit: Some(permit),
        })
    }
}

// private一取得の不変予算。Immediateは期限切れDeadlineとは別に扱う。
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum AcquireBudget {
    Immediate,
    Deadline(Instant),
}
impl AcquireBudget {
    pub fn after_at(now: Instant, duration: Duration) -> Option<Self> {
        if duration.is_zero() {
            Some(Self::Immediate)
        } else {
            now.checked_add(duration).map(Self::Deadline)
        }
    }
    #[cfg(test)]
    pub fn remaining_at(self, now: Instant) -> Duration {
        match self {
            Self::Immediate => Duration::ZERO,
            Self::Deadline(deadline) => deadline.saturating_duration_since(now),
        }
    }
}

#[derive(Default)]
pub(super) struct AdapterSeams {
    pub target_worker: Option<usize>,
    pub startup: Option<Arc<Gate>>,
    pub created: Option<Arc<AdmissionGate>>,
    pub checkout: Option<Arc<AdmissionGate>>,
    pub fail_recycle: bool,
    pub publication: Option<Arc<Gate>>,
    #[cfg(test)]
    pub detaching: Option<Arc<Gate>>,
    pub fail_terminal_result: bool,
    pub fail_observer_spawn: bool,
    pub fail_native_spawn: bool,
    pub fail_native_reserve: bool,
    pub fail_idle_reserve: bool,
}
impl AdapterSeams {
    fn applies(&self, ordinal: usize) -> bool {
        self.target_worker.is_none_or(|target| target == ordinal)
    }
}
pub(super) struct AdmissionGate {
    entered: AtomicBool,
    released: AtomicBool,
    changed: Notify,
}
impl AdmissionGate {
    #[cfg(test)]
    pub fn new() -> Self {
        Self {
            entered: AtomicBool::new(false),
            released: AtomicBool::new(false),
            changed: Notify::new(),
        }
    }
    async fn pause(&self) {
        if self.entered.swap(true, Ordering::SeqCst) {
            return;
        }
        self.changed.notify_waiters();
        self.wait_for(&self.released).await;
    }
    async fn wait_for(&self, flag: &AtomicBool) {
        tokio::time::timeout(WATCHDOG, async {
            loop {
                let changed = self.changed.notified();
                if flag.load(Ordering::SeqCst) {
                    return;
                }
                changed.await;
            }
        })
        .await
        .expect("adapter barrier watchdog expired");
    }
    #[cfg(test)]
    pub async fn wait(&self) {
        self.wait_for(&self.entered).await;
    }
    #[cfg(test)]
    pub fn release(&self) {
        self.released.store(true, Ordering::SeqCst);
        self.changed.notify_waiters();
    }
}

#[derive(Clone, Default)]
pub(super) struct AdapterStats {
    pub created: usize,
    pub native_started: usize,
    pub start_failed: usize,
    pub starting: usize,
    pub admitted: usize,
    pub returned: usize,
    #[cfg(test)]
    pub detached: usize,
    pub handle_drops: usize,
    pub native_closed: usize,
    pub joined: usize,
    pub pending_workers: usize,
    pub closing: bool,
}
#[derive(Default)]
struct Records {
    stats: AdapterStats,
    // 完了したStateは保持せず、join/native closeの累積counterだけ残す。
    workers: Vec<Arc<State>>,
    failure: Option<Failure>,
}
struct Ledger {
    records: Mutex<Records>,
    pool: Mutex<Option<Weak<LogicalPool>>>,
    changed: Notify,
}
impl Ledger {
    fn error(&self) -> Option<Failure> {
        let records = self.records.lock().unwrap();
        if let Some(failure) = &records.failure {
            let mut error = failure.duplicate();
            if error.kind != Kind::Allocation {
                error.kind = Kind::Worker;
            }
            error.outcome = Outcome::NotApplicable;
            return Some(error);
        }
        if records.stats.closing {
            Some(Failure::primary(
                Kind::Closed,
                Outcome::NotApplicable,
                "adapter closing",
            ))
        } else {
            None
        }
    }
    fn change(&self, update: impl FnOnce(&mut AdapterStats)) {
        update(&mut self.records.lock().unwrap().stats);
        self.changed.notify_waiters();
    }
    fn request_close(&self) {
        self.change(|stats| stats.closing = true);
        // idle handlesのDropはledgerを触るためledger lockを保持しない。
        let pool = self.pool.lock().unwrap().as_ref().and_then(Weak::upgrade);
        if let Some(pool) = pool {
            NativePool(pool).close();
        }
    }

    fn fail(&self, error: Failure) {
        {
            let mut records = self.records.lock().unwrap();
            if records.failure.is_none() {
                records.failure = Some(error);
            } else if let Some(previous) = &mut records.failure {
                if previous.cleanup.is_none() {
                    previous.cleanup = error.cleanup;
                }
                previous.retired |= error.retired;
            }
        }
        self.request_close();
    }
    fn snapshot(&self) -> AdapterStats {
        let records = self.records.lock().unwrap();
        let mut stats = records.stats.clone();
        stats.native_closed += records
            .workers
            .iter()
            .filter(|worker| worker.snapshot().native_closed)
            .count();
        stats.pending_workers = records.workers.len();
        stats
    }
    fn completed(&self, state: &Arc<State>) {
        let mut records = self.records.lock().unwrap();
        let index = records
            .workers
            .iter()
            .position(|worker| Arc::ptr_eq(worker, state))
            .expect("worker completion without registration");
        state.update(|stats| stats.joined = true);
        records.stats.joined += 1;
        records.stats.native_closed += usize::from(state.snapshot().native_closed);
        records.workers.swap_remove(index);
        drop(records);
        self.changed.notify_waiters();
    }
    fn start_failed(&self, state: &Arc<State>) {
        let mut records = self.records.lock().unwrap();
        let index = records
            .workers
            .iter()
            .position(|worker| Arc::ptr_eq(worker, state))
            .expect("failed start without registration");
        // native未起動。State.joined/native_closedも成功counterも変更しない。
        records.stats.start_failed += 1;
        records.workers.swap_remove(index);
        drop(records);
        self.changed.notify_waiters();
    }
    fn done(&self) -> bool {
        let stats = self.snapshot();
        stats.starting == 0 && stats.joined + stats.start_failed == stats.created
    }
    async fn until(&self, predicate: impl Fn(&AdapterStats) -> bool) {
        loop {
            let changed = self.changed.notified();
            if predicate(&self.snapshot()) {
                return;
            }
            changed.await;
        }
    }
}
#[derive(Clone)]
#[cfg(test)]
pub(super) struct Observer(Arc<Ledger>);
#[cfg(test)]
impl Observer {
    pub fn snapshot(&self) -> AdapterStats {
        self.0.snapshot()
    }
    pub async fn wait_returned(&self, count: usize) {
        self.0.until(|stats| stats.returned >= count).await;
    }
    pub async fn wait_joined(&self, count: usize) {
        self.0.until(|stats| stats.joined >= count).await;
    }
}

struct WorkerHandle {
    ordinal: usize,
    sender: Option<mpsc::Sender<BeginRequest>>,
    state: Arc<State>,
    ledger: Arc<Ledger>,
}
impl Drop for WorkerHandle {
    fn drop(&mut self) {
        // Dropは閉鎖要求だけ。自分のworkerのjoinやasync cleanupをここでしない。
        self.sender.take();
        self.ledger.change(|stats| stats.handle_drops += 1);
    }
}
struct Startup {
    handle: Option<WorkerHandle>,
    ledger: Arc<Ledger>,
    registered: bool,
}
impl Startup {
    fn handoff(mut self) -> Result<WorkerHandle, Failure> {
        if let Some(error) = self.ledger.error() {
            return Err(error);
        }
        // handoffもclosing/startup登録と同じledgerで確定。
        let result = {
            let mut records = self.ledger.records.lock().unwrap();
            if records.stats.closing {
                Err(Failure::primary(
                    Kind::Closed,
                    Outcome::NotApplicable,
                    "create finished after close",
                ))
            } else {
                records.stats.starting -= 1;
                self.registered = false;
                Ok(self.handle.take().unwrap())
            }
        };
        self.ledger.changed.notify_waiters();
        result
    }
}
impl Drop for Startup {
    fn drop(&mut self) {
        self.handle.take(); // cancelled create Futureもnative close要求を残す。
        if self.registered {
            self.ledger.change(|stats| stats.starting -= 1);
        }
    }
}

struct NativeManager {
    config: Arc<Config>,
    seams: Arc<AdapterSeams>,
    ledger: Arc<Ledger>,
    capacity: usize,
}

// replyの喪失より先に起動失敗を公開。native未起動をfake joinへ変えない。
fn fail_start(ledger: &Ledger, state: &Arc<State>, ready: &ReadyReply, error: Failure) {
    state.fail(error.duplicate());
    ledger.fail(error.duplicate());
    ledger.start_failed(state);
    let reply = ready.lock().unwrap().take();
    if let Some(reply) = reply {
        let _ = reply.send(Err(error));
    }
}

fn observe_native(
    ordinal: usize,
    config: Arc<Config>,
    state: Arc<State>,
    ledger: Arc<Ledger>,
    seams: Arc<AdapterSeams>,
    receiver: mpsc::Receiver<BeginRequest>,
    ready: ReadyReply,
) {
    let worker_state = Arc::clone(&state);
    let worker_ledger = Arc::clone(&ledger);
    let worker_ready = Arc::clone(&ready);
    let startup = seams.startup.clone().filter(|_| seams.applies(ordinal));
    // observer自身がnativeを起動し、成功したJoinHandleをこのscopeから外へ渡さない。
    let worker = if seams.applies(ordinal) && seams.fail_native_spawn {
        Err(std::io::Error::other("private native startup failure seam"))
    } else {
        std::thread::Builder::new()
            .name("nagi-sqlite-adapter".into())
            .spawn(move || {
                worker_ledger.change(|stats| stats.native_started += 1);
                let reply = worker_ready.lock().unwrap().take();
                native_worker(config, worker_state, receiver, reply, startup);
            })
    };
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            fail_start(
                &ledger,
                &state,
                &ready,
                Failure::cleanup(Outcome::NotApplicable, error.to_string()),
            );
            return;
        }
    };
    if worker.join().is_err() {
        state.fail(state.worker_error());
    }
    if seams.applies(ordinal) {
        if seams.fail_terminal_result {
            state.fail(Failure::cleanup(
                Outcome::NotApplicable,
                "private terminal-result failure seam; native close succeeded",
            ));
        }
        if let Some(gate) = &seams.publication {
            gate.block_once();
        }
    }
    if let Some(error) = state.error() {
        ledger.fail(error);
    }
    // terminal cause公開→actual join済みcounter/live除去→通知。
    ledger.completed(&state);
    // native entry前panic等でSenderが残る場合も、failure公開後にだけ通知する。
    let reply = ready.lock().unwrap().take();
    if let Some(reply) = reply {
        let _ = reply.send(Err(state.worker_error()));
    }
}

impl NativeManager {
    async fn create(&self, budget: Option<AcquireBudget>) -> Result<WorkerHandle, Failure> {
        // 明示された同じ絶対予算をlogical permitとnative登録待ちへ使う。
        // Noneはtest-only比較入口。共有期限やtask-localは不要。
        let state = Arc::new(State::default());
        let ordinal = loop {
            let changed = self.ledger.changed.notified();
            let registered = {
                let mut records = self.ledger.records.lock().unwrap();
                if let Some(error) = &records.failure {
                    let mut error = error.duplicate();
                    if error.kind != Kind::Allocation {
                        error.kind = Kind::Worker;
                    }
                    error.outcome = Outcome::NotApplicable;
                    return Err(error);
                }
                if records.stats.closing {
                    return Err(Failure::primary(
                        Kind::Closed,
                        Outcome::NotApplicable,
                        "create after close",
                    ));
                }
                // finiteは新native登録前に失効を確認。Immediateの空き判定と区別する。
                if let Some(AcquireBudget::Deadline(deadline)) = budget {
                    if Instant::now() >= deadline {
                        return Err(Failure::primary(
                            Kind::AcquireTimeout,
                            Outcome::NotApplicable,
                            "native slot reservation deadline expired",
                        ));
                    }
                }
                // starting/healthy/取消/detachedを含むnative容量の終了fence。
                // slotの選択/待機順序/公平性はTokio Semaphoreへ委譲する。
                if records.workers.len() >= self.capacity {
                    if matches!(budget, Some(AcquireBudget::Immediate)) {
                        return Err(Failure::primary(
                            Kind::AcquireTimeout,
                            Outcome::NotApplicable,
                            "native slot is not immediately available",
                        ));
                    }
                    None
                } else {
                    // closing確認とstarting/join責任の登録はspawn前の同じcritical section。
                    if self.seams.fail_native_reserve || records.workers.try_reserve(1).is_err() {
                        return Err(Failure::allocation(
                            "SQLite native record reservation failed",
                        ));
                    }
                    records.workers.push(Arc::clone(&state));
                    records.stats.created += 1;
                    records.stats.starting += 1;
                    Some(records.stats.created)
                }
            };
            if let Some(ordinal) = registered {
                break ordinal;
            }
            match budget {
                Some(AcquireBudget::Deadline(deadline)) => {
                    if tokio::time::timeout_at(deadline, changed).await.is_err() {
                        return Err(self.ledger.error().unwrap_or_else(|| {
                            Failure::primary(
                                Kind::AcquireTimeout,
                                Outcome::NotApplicable,
                                "native slot reservation timed out",
                            )
                        }));
                    }
                }
                // Immediateは上の同lock容量判定で登録かErrへ進み、ここでは待たない。
                Some(AcquireBudget::Immediate) => {
                    unreachable!("immediate registration cannot wait")
                }
                None => changed.await,
            }
        };
        // 登録後はstartup/ready/BEGINへ取得timerを持ち越さない。
        self.ledger.changed.notify_waiters();
        let (sender, receiver) = mpsc::channel(1);
        let startup = Startup {
            handle: Some(WorkerHandle {
                ordinal,
                sender: Some(sender),
                state: Arc::clone(&state),
                ledger: Arc::clone(&self.ledger),
            }),
            ledger: Arc::clone(&self.ledger),
            registered: true,
        };
        let (ready, ready_receiver) = oneshot::channel();
        // Builder.spawn ErrはclosureをDropするが、Senderはobserver側にも残す。
        let ready = Arc::new(Mutex::new(Some(ready)));
        let config = Arc::clone(&self.config);
        let observer_state = Arc::clone(&state);
        let ledger = Arc::clone(&self.ledger);
        let seams = Arc::clone(&self.seams);
        let observer_ready = Arc::clone(&ready);
        let observer = if self.seams.applies(ordinal) && self.seams.fail_observer_spawn {
            Err(std::io::Error::other(
                "private observer startup failure seam",
            ))
        } else {
            std::thread::Builder::new()
                .name("nagi-sqlite-join-observer".into())
                .spawn(move || {
                    observe_native(
                        ordinal,
                        config,
                        observer_state,
                        ledger,
                        seams,
                        receiver,
                        observer_ready,
                    )
                })
        };
        match observer {
            Ok(observer) => drop(observer), // native JoinHandleはobserver closure内だけにある。
            Err(error) => {
                let error = Failure::cleanup(Outcome::NotApplicable, error.to_string());
                fail_start(&self.ledger, &state, &ready, error.duplicate());
                return Err(error);
            }
        }
        let ready = ready_receiver.await.map_err(|_| state.worker_error())?;
        if let Err(error) = ready {
            self.ledger.fail(error.duplicate());
            return Err(error);
        }
        if self.seams.applies(ordinal) {
            if let Some(gate) = &self.seams.created {
                gate.pause().await;
            }
        }
        startup.handoff()
    }
    async fn recycle(&self, worker: &mut WorkerHandle) -> Result<(), Failure> {
        let error = worker.state.error().or_else(|| {
            if self.seams.fail_recycle {
                Some(Failure::cleanup(
                    Outcome::NotApplicable,
                    "private recycle failure seam",
                ))
            } else {
                self.ledger.error()
            }
        });
        if let Some(error) = error {
            self.ledger.fail(error.duplicate());
            return Err(error);
        }
        let stats = worker.state.snapshot();
        if stats.joined || stats.begun != stats.settled || stats.management {
            let error = Failure::cleanup(
                Outcome::NotApplicable,
                "worker/session cleanup not verified",
            );
            self.ledger.fail(error.duplicate());
            return Err(error);
        }
        Ok(())
    }
    #[cfg(test)]
    fn detach(&self, worker: &mut WorkerHandle) {
        if self.seams.applies(worker.ordinal) {
            if let Some(gate) = &self.seams.detaching {
                gate.block_once();
            }
        }
        self.ledger.change(|stats| stats.detached += 1);
    }
}

// Checkoutはnative workerのBeginRequest内へ移動し、cleanupのscope末尾でだけ返す。
// ledgerはTx/session senderを保持しない。Tokio task Dropで早期返却する経路もない。
struct CheckoutOwner {
    object: Option<Checkout>,
    ledger: Arc<Ledger>,
    state: Arc<State>,
}
impl Drop for CheckoutOwner {
    fn drop(&mut self) {
        if let Some(error) = self.state.error() {
            self.ledger.fail(error);
        }
        self.object.take();
        self.ledger.change(|stats| stats.returned += 1);
    }
}
struct Inner {
    pool: NativePool,
    ledger: Arc<Ledger>,
    seams: Arc<AdapterSeams>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.ledger.request_close();
    }
}
pub(super) struct Adapter(Arc<Inner>);
impl Adapter {
    #[cfg(test)]
    pub fn new(config: Config, seams: AdapterSeams) -> Self {
        Self::with_capacity(config, seams, 1)
    }
    pub fn with_capacity(config: Config, seams: AdapterSeams, capacity: usize) -> Self {
        assert!(capacity > 0, "private fixture capacity must be positive");
        let ledger = Arc::new(Ledger {
            records: Mutex::new(Records::default()),
            pool: Mutex::new(None),
            changed: Notify::new(),
        });
        let seams = Arc::new(seams);
        let manager = NativeManager {
            config: Arc::new(config),
            seams: Arc::clone(&seams),
            ledger: Arc::clone(&ledger),
            capacity,
        };
        let pool = NativePool::new(manager, capacity);
        *ledger.pool.lock().unwrap() = Some(Arc::downgrade(&pool.0));
        Self(Arc::new(Inner {
            pool,
            ledger,
            seams,
        }))
    }
    pub fn is_closing(&self) -> bool {
        self.0.ledger.snapshot().closing
    }
    pub fn clone_handle(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
    #[cfg(test)]
    pub fn observer(&self) -> Observer {
        Observer(Arc::clone(&self.0.ledger))
    }
    #[cfg(test)]
    pub fn available(&self) -> usize {
        self.0.pool.available()
    }
    #[cfg(test)]
    pub fn resize_zero(&self) {
        self.0.pool.close();
    }
    #[cfg(test)]
    pub async fn begin(&self) -> Result<Tx, Failure> {
        // 従来のprivate無期限比較入口。公開Options/defaultの保証ではない。
        self.begin_inner(None).await
    }
    #[cfg(test)]
    pub async fn begin_with_budget(&self, budget: AcquireBudget) -> Result<Tx, Failure> {
        self.begin_inner(Some(budget)).await
    }
    pub async fn begin_mode_with_budget(
        &self,
        mode: super::BeginMode,
        budget: AcquireBudget,
    ) -> Result<Tx, Failure> {
        self.begin_inner_mode(Some(budget), mode).await
    }
    #[cfg(test)]
    async fn begin_inner(&self, budget: Option<AcquireBudget>) -> Result<Tx, Failure> {
        self.begin_inner_mode(budget, super::BeginMode::Deferred)
            .await
    }
    async fn begin_inner_mode(
        &self,
        budget: Option<AcquireBudget>,
        mode: super::BeginMode,
    ) -> Result<Tx, Failure> {
        if let Some(error) = self.0.ledger.error() {
            return Err(error);
        }
        let object = self.0.pool.get(budget).await?;
        if self.0.seams.applies(object.ordinal) {
            if let Some(gate) = &self.0.seams.checkout {
                gate.pause().await;
            }
        }
        let sender = object.sender.as_ref().unwrap().clone();
        let state = Arc::clone(&object.state);
        let (reply, receiver) = oneshot::channel();
        let owner = CheckoutOwner {
            object: Some(object),
            ledger: Arc::clone(&self.0.ledger),
            state,
        };
        let mut request = Some(BeginRequest::owned(reply, Box::new(owner)));
        request.as_mut().unwrap().mode = mode;
        // admissionとclosingは同じledgerで直列化。try_sendはworker inboxのみで、
        // slot待機/queue fairnessはTokio Semaphoreへ委譲する。
        let mut rejected = None;
        let result = {
            let mut records = self.0.ledger.records.lock().unwrap();
            if records.failure.is_some() {
                Err(Kind::Worker)
            } else if records.stats.closing {
                Err(Kind::Closed)
            } else {
                match sender.try_send(request.take().unwrap()) {
                    Ok(()) => {
                        records.stats.admitted += 1;
                        Ok(())
                    }
                    Err(error) => {
                        rejected = Some(error.into_inner());
                        Err(Kind::Worker)
                    }
                }
            }
        };
        // rejected owned ObjectのDropはledger lockの外側でだけ行う。
        drop(request);
        drop(rejected);
        drop(sender);
        self.0.ledger.changed.notify_waiters();
        result.map_err(|kind| {
            self.0.ledger.error().unwrap_or_else(|| {
                Failure::primary(kind, Outcome::NotApplicable, "session admission failed")
            })
        })?;
        receiver
            .await
            .map_err(|_| Failure::primary(Kind::ReplyLost, Outcome::Unknown, "begin reply lost"))?
    }
    pub async fn close(&self, timeout: Duration) -> Result<(), Failure> {
        self.0.ledger.request_close();
        if !self.0.ledger.done()
            && tokio::time::timeout(
                timeout,
                self.0.ledger.until(|stats| {
                    stats.starting == 0 && stats.joined + stats.start_failed == stats.created
                }),
            )
            .await
            .is_err()
        {
            return Err(Failure::primary(
                Kind::CloseTimeout,
                Outcome::NotApplicable,
                "adapter workers not joined",
            ));
        }
        if let Some(mut error) = self
            .0
            .ledger
            .records
            .lock()
            .unwrap()
            .failure
            .as_ref()
            .map(Failure::duplicate)
        {
            error.outcome = Outcome::NotApplicable;
            return Err(error);
        }
        let stats = self.0.ledger.snapshot();
        assert_eq!(stats.native_closed, stats.created);
        Ok(())
    }
    #[cfg(test)]
    pub async fn checkout_without_begin(&self) -> Result<HeldCheckout, Failure> {
        // privateの無期限pool所有権fixture。公開取得入口ではない。
        let object = self.0.pool.get(None).await?;
        Ok(HeldCheckout(object))
    }
}
#[cfg(test)]
pub(super) struct HeldCheckout(Checkout);
#[cfg(test)]
impl HeldCheckout {
    pub fn take_and_drop(self) {
        drop(Checkout::take(self.0));
    }
}
