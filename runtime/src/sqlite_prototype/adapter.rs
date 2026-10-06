//! Q004のprivate generic deadpool比較。slot待機/公平性/recycle algorithmはdeadpool。
//! native session/closeはsession.rs、ここはowned checkoutと終了観測のadapterだけ。
use super::session::{
    native_worker, BeginRequest, Config, Failure, Gate, Kind, Outcome, State, Tx,
};
use deadpool::managed::{self, Manager, Metrics, RecycleError, RecycleResult};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::time::Instant;

const WATCHDOG: Duration = Duration::from_secs(10);
type NativePool = managed::Pool<NativeManager>;
type Checkout = managed::Object<NativeManager>;
type ReadyReply = Arc<Mutex<Option<oneshot::Sender<Result<(), Failure>>>>>;

tokio::task_local! {
    // Manager.createの署名に取得引数がないため、stock getのpoll中だけ設定する。
    static CURRENT_ACQUIRE_BUDGET: AcquireBudget;
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
    pub detaching: Option<Arc<Gate>>,
    pub fail_terminal_result: bool,
    pub fail_observer_spawn: bool,
    pub fail_native_spawn: bool,
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
    pub async fn wait(&self) {
        self.wait_for(&self.entered).await;
    }
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
    pool: Mutex<Option<managed::WeakPool<NativeManager>>>,
    changed: Notify,
}
impl Ledger {
    fn error(&self) -> Option<Failure> {
        let records = self.records.lock().unwrap();
        if records.failure.is_some() {
            let mut error = records.failure.clone().unwrap();
            error.kind = Kind::Worker;
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
        // deadpool.close drops WorkerHandle inside its own lock。ledger lockを保持しない。
        let pool = self
            .pool
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|pool| pool.upgrade());
        if let Some(pool) = pool {
            pool.close();
            // Pending waiterへのpermit予約や返却途中ではresize(0)がidle Objectを
            // 残し得る。closed/max_size=0の後にqueueを退役し、late返却はstock
            // detachへ渡す。active checkoutには触れず、ledger lock外でDropする。
            let removed = pool.retain(|_, _| false).removed;
            drop(removed);
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
        tokio::time::timeout(WATCHDOG, async {
            loop {
                let changed = self.changed.notified();
                if predicate(&self.snapshot()) {
                    return;
                }
                changed.await;
            }
        })
        .await
        .expect("adapter observation watchdog expired");
    }
}
#[derive(Clone)]
pub(super) struct Observer(Arc<Ledger>);
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
    state.fail(error.clone());
    ledger.fail(error.clone());
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

impl Manager for NativeManager {
    type Type = WorkerHandle;
    type Error = Failure;
    async fn create(&self) -> Result<WorkerHandle, Failure> {
        // 最初のpollで取得ごとの値をcopyし、同じ絶対期限を全native待機へ使う。
        // scopeなしは従来のprivate begin/checkout比較用。公開defaultではない。
        let budget = CURRENT_ACQUIRE_BUDGET.try_with(|budget| *budget).ok();
        let state = Arc::new(State::default());
        let ordinal = loop {
            let changed = self.ledger.changed.notified();
            let registered = {
                let mut records = self.ledger.records.lock().unwrap();
                if let Some(error) = &records.failure {
                    let mut error = error.clone();
                    error.kind = Kind::Worker;
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
                // slotの選択/待機順序/公平性はstock deadpoolが引き続き所有する。
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
                fail_start(&self.ledger, &state, &ready, error.clone());
                return Err(error);
            }
        }
        let ready = ready_receiver.await.map_err(|_| state.worker_error())?;
        if let Err(error) = ready {
            self.ledger.fail(error.clone());
            return Err(error);
        }
        if self.seams.applies(ordinal) {
            if let Some(gate) = &self.seams.created {
                gate.pause().await;
            }
        }
        startup.handoff()
    }
    async fn recycle(
        &self,
        worker: &mut WorkerHandle,
        _metrics: &Metrics,
    ) -> RecycleResult<Failure> {
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
            self.ledger.fail(error.clone());
            return Err(RecycleError::Backend(error));
        }
        let stats = worker.state.snapshot();
        if stats.joined || stats.begun != stats.settled || stats.management {
            let error = Failure::cleanup(
                Outcome::NotApplicable,
                "worker/session cleanup not verified",
            );
            self.ledger.fail(error.clone());
            return Err(RecycleError::Backend(error));
        }
        Ok(())
    }
    fn detach(&self, worker: &mut WorkerHandle) {
        if self.seams.applies(worker.ordinal) {
            if let Some(gate) = &self.seams.detaching {
                gate.block_once();
            }
        }
        self.ledger.change(|stats| stats.detached += 1);
    }
}

// Objectはnative workerのBeginRequest内へ移動し、cleanupのscope末尾でだけ返す。
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
        let pool = NativePool::builder(manager)
            .max_size(capacity)
            .runtime(deadpool::Runtime::Tokio1)
            .build()
            .unwrap();
        *ledger.pool.lock().unwrap() = Some(pool.weak());
        Self(Arc::new(Inner {
            pool,
            ledger,
            seams,
        }))
    }
    pub fn clone_handle(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
    pub fn observer(&self) -> Observer {
        Observer(Arc::clone(&self.0.ledger))
    }
    pub fn available(&self) -> usize {
        self.0.pool.status().available
    }
    pub fn resize_zero(&self) {
        self.0.pool.resize(0);
    }
    pub async fn begin(&self) -> Result<Tx, Failure> {
        // 従来のprivate無期限比較入口。公開Options/defaultの保証ではない。
        self.begin_inner(None).await
    }
    pub async fn begin_with_budget(&self, budget: AcquireBudget) -> Result<Tx, Failure> {
        self.begin_inner(Some(budget)).await
    }
    fn get_failure(&self, error: managed::PoolError<Failure>) -> Failure {
        self.0.ledger.error().unwrap_or_else(|| match error {
            managed::PoolError::Timeout(managed::TimeoutType::Wait) => Failure::primary(
                Kind::AcquireTimeout,
                Outcome::NotApplicable,
                "logical slot reservation timed out",
            ),
            managed::PoolError::Backend(error) => error,
            managed::PoolError::Closed => {
                Failure::primary(Kind::Closed, Outcome::NotApplicable, "deadpool closed")
            }
            error => Failure::primary(
                Kind::Worker,
                Outcome::NotApplicable,
                format!("deadpool get: {error:?}"),
            ),
        })
    }
    async fn begin_inner(&self, budget: Option<AcquireBudget>) -> Result<Tx, Failure> {
        if let Some(error) = self.0.ledger.error() {
            return Err(error);
        }
        let object = match budget {
            Some(budget) => {
                let timeouts = managed::Timeouts {
                    wait: Some(budget.remaining_at(Instant::now())),
                    create: None,
                    recycle: None,
                };
                // scopeはstock取得Futureだけ。task-localはpoll/Drop後に元の値へ戻る。
                // native threadやcheckout後のsession admission/返信には渡さない。
                CURRENT_ACQUIRE_BUDGET
                    .scope(budget, self.0.pool.timeout_get(&timeouts))
                    .await
            }
            None => self.0.pool.get().await,
        }
        .map_err(|error| self.get_failure(error))?;
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
        // admissionとclosingは同じledgerで直列化。try_sendはworker inboxのみで、
        // slot待機/queue fairnessはdeadpool.getへ委譲する。
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
        if let Some(error) = self.0.ledger.records.lock().unwrap().failure.clone() {
            return Err(error);
        }
        let stats = self.0.ledger.snapshot();
        assert_eq!(stats.native_closed, stats.created);
        Ok(())
    }
    pub async fn checkout_without_begin(&self) -> Result<HeldCheckout, Failure> {
        // privateの無期限pool所有権fixture。公開取得入口ではない。
        let object = self
            .0
            .pool
            .get()
            .await
            .map_err(|_| self.0.ledger.error().unwrap())?;
        Ok(HeldCheckout(object))
    }
}
pub(super) struct HeldCheckout(Checkout);
impl HeldCheckout {
    pub fn take_and_drop(self) {
        drop(Checkout::take(self.0));
    }
}
