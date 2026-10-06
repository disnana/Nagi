//! 一接続の試験driverと、wrapperに依存しないlexical native session。
//! pool選択・acquire policy・Nagi capability・公開APIはここでは実装しない。
use crate::{database::indices, FromRow};
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    params_from_iter,
    types::Value,
    Connection, DropBehavior, Transaction, TransactionBehavior,
};
use std::{
    cell::RefCell,
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc as blocking_channel, Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, Notify};
const WATCHDOG: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Kind {
    Sql,
    Bind,
    Decode,
    Aborted,
    Cleanup,
    Worker,
    Closed,
    CloseTimeout,
    ReplyLost,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Outcome {
    NotApplicable,
    Active,
    Committed,
    RolledBack,
    Unknown,
}
#[derive(Clone, Debug)]
pub(super) struct Failure {
    pub kind: Kind,
    pub outcome: Outcome,
    pub primary: Option<String>,
    pub cleanup: Option<String>,
    pub retired: bool,
}
impl Failure {
    pub(super) fn primary(kind: Kind, outcome: Outcome, message: impl Into<String>) -> Self {
        Self {
            kind,
            outcome,
            primary: Some(message.into()),
            cleanup: None,
            retired: false,
        }
    }
    pub(super) fn cleanup(outcome: Outcome, message: impl Into<String>) -> Self {
        Self {
            kind: Kind::Cleanup,
            outcome,
            primary: None,
            cleanup: Some(message.into()),
            retired: true,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Finish {
    Commit,
    Rollback,
}

// 常設hookの管理権限はprivate hardcoded native終端のlexical guardだけが持つ。
// user SQLのprepare/step/finalizeとFromRowはこのguardを作らない。
thread_local! {
    static CURRENT_MODE: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}
pub(super) fn management_active() -> bool {
    CURRENT_MODE.with(|mode| {
        mode.borrow()
            .as_ref()
            .is_some_and(|mode| mode.load(Ordering::SeqCst))
    })
}
struct Management<'a>(&'a AtomicBool);
impl<'a> Management<'a> {
    fn enter(mode: &'a AtomicBool) -> Self {
        assert!(!mode.swap(true, Ordering::SeqCst));
        Self(mode)
    }
}
impl Drop for Management<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn install_authorizer(
    conn: &Connection,
    mode: &Arc<AtomicBool>,
    state: &Arc<State>,
    deny_rollback: bool,
    deny_commit: bool,
) -> rusqlite::Result<()> {
    let mode = Arc::clone(mode);
    let state = Arc::clone(state);
    conn.authorizer(Some(move |ctx: AuthContext<'_>| {
        let management = mode.load(Ordering::SeqCst);
        if let AuthAction::Transaction { operation } = ctx.action {
            if management {
                if matches!(operation, rusqlite::hooks::TransactionOperation::Rollback) {
                    state.update(|stats| stats.rollback_attempts += 1);
                    if deny_rollback {
                        return Authorization::Deny;
                    }
                }
                // rusqlite 0.40.2 maps COMMIT to Unknown, not a Commit variant.
                if deny_commit
                    && matches!(operation, rusqlite::hooks::TransactionOperation::Unknown)
                {
                    return Authorization::Deny;
                }
                return Authorization::Allow;
            }
            return Authorization::Deny;
        }
        match ctx.action {
            AuthAction::Pragma { .. } => {
                state.update(|stats| stats.denied_pragmas += 1);
                Authorization::Deny
            }
            AuthAction::CreateIndex { .. }
            | AuthAction::CreateTable { .. }
            | AuthAction::CreateTempIndex { .. }
            | AuthAction::CreateTempTable { .. }
            | AuthAction::CreateTempTrigger { .. }
            | AuthAction::CreateTempView { .. }
            | AuthAction::CreateTrigger { .. }
            | AuthAction::CreateView { .. }
            | AuthAction::Delete { .. }
            | AuthAction::DropIndex { .. }
            | AuthAction::DropTable { .. }
            | AuthAction::DropTempIndex { .. }
            | AuthAction::DropTempTable { .. }
            | AuthAction::DropTempTrigger { .. }
            | AuthAction::DropTempView { .. }
            | AuthAction::DropTrigger { .. }
            | AuthAction::DropView { .. }
            | AuthAction::Insert { .. }
            | AuthAction::Read { .. }
            | AuthAction::Select
            | AuthAction::Update { .. }
            | AuthAction::AlterTable { .. }
            | AuthAction::Reindex { .. }
            | AuthAction::Recursive => Authorization::Allow,
            AuthAction::Function { function_name }
                if !function_name.eq_ignore_ascii_case("load_extension") =>
            {
                Authorization::Allow
            }
            // Savepoint/Pragma/Attach/Detach/Unknown/Vtableと将来追加actionはDeny。
            _ => Authorization::Deny,
        }
    }))
}

/// sleepを成功oracleにしない、一回だけのworker barrier。
pub(super) struct Gate {
    entered: AtomicBool,
    changed: Notify,
    sender: Mutex<Option<blocking_channel::SyncSender<()>>>,
    receiver: Mutex<Option<blocking_channel::Receiver<()>>>,
}
impl Gate {
    pub fn new() -> Self {
        let (sender, receiver) = blocking_channel::sync_channel(1);
        Self {
            entered: AtomicBool::new(false),
            changed: Notify::new(),
            sender: Mutex::new(Some(sender)),
            receiver: Mutex::new(Some(receiver)),
        }
    }
    pub(super) fn block_once(&self) {
        let receiver = self.receiver.lock().unwrap().take();
        if let Some(receiver) = receiver {
            self.entered.store(true, Ordering::SeqCst);
            self.changed.notify_waiters();
            receiver
                .recv_timeout(WATCHDOG)
                .expect("test barrier watchdog expired or owner disappeared");
        }
    }
    pub async fn wait(&self) {
        tokio::time::timeout(WATCHDOG, async {
            loop {
                let changed = self.changed.notified();
                if self.entered.load(Ordering::SeqCst) {
                    return;
                }
                changed.await;
            }
        })
        .await
        .expect("test barrier watchdog expired");
    }
    pub fn release(&self) {
        self.sender
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(())
            .unwrap();
    }
}
#[derive(Default)]
pub(super) struct Config {
    // private multi-connection fixtureのみ。公開path/Options validationの実装ではない。
    pub path: Option<std::path::PathBuf>,
    pub seed: &'static str,
    pub begin_gate: Option<Arc<Gate>>,
    pub command_gate: Option<Arc<Gate>>,
    pub cleanup_gate: Option<Arc<Gate>>,
    pub commit_gate: Option<Arc<Gate>>,
    pub exit_gate: Option<Arc<Gate>>,
    pub deny_rollback: bool,
    pub deny_commit: bool,
    pub panic_management: bool,
    pub fail_restore_check: bool,
    pub leak_statement_at_close: bool,
}
#[derive(Clone, Default)]
pub(super) struct Stats {
    pub admitted: usize,
    pub begun: usize,
    pub executed: usize,
    // 完了したcleanup観測の件数。成功したrollback/再利用の件数ではない。
    pub settled: usize,
    pub committed: usize,
    pub rollback_attempts: usize,
    pub denied_pragmas: usize,
    pub management: bool,
    pub closing: bool,
    pub native_closed: bool,
    pub joined: bool,
}
#[derive(Default)]
pub(super) struct State {
    stats: Mutex<Stats>,
    failure: Mutex<Option<Failure>>,
    changed: Notify,
    mode: Arc<AtomicBool>,
}
impl State {
    pub(super) fn update(&self, change: impl FnOnce(&mut Stats)) {
        change(&mut self.stats.lock().unwrap());
        self.changed.notify_waiters();
    }
    pub(super) fn snapshot(&self) -> Stats {
        let mut stats = self.stats.lock().unwrap().clone();
        stats.management = self.mode.load(Ordering::SeqCst);
        stats
    }
    pub(super) fn fail(&self, error: Failure) {
        let mut failed = self.failure.lock().unwrap();
        if failed.is_none() {
            *failed = Some(error);
        } else if let Some(previous) = &mut *failed {
            // worker panic等のprimaryを残し、後続native closeのcleanup causeも保持する。
            if previous.cleanup.is_none() {
                previous.cleanup = error.cleanup;
            }
            previous.retired |= error.retired;
        }
        drop(failed);
        self.changed.notify_waiters();
    }
    pub(super) fn error(&self) -> Option<Failure> {
        self.failure.lock().unwrap().clone()
    }
    pub(super) fn worker_error(&self) -> Failure {
        let message = self
            .failure
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|error| error.primary.clone().or_else(|| error.cleanup.clone()))
            .unwrap_or_else(|| "worker stopped".into());
        let mut error = Failure::primary(Kind::Worker, Outcome::NotApplicable, message);
        error.retired = true;
        error
    }
    async fn until(&self, predicate: impl Fn(&Stats) -> bool) {
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
        .expect("test observation watchdog expired");
    }
}

pub(super) struct BeginRequest {
    reply: oneshot::Sender<Result<Tx, Failure>>,
    // lexical native cleanupが返るまで保持。session senderの強参照は入れない。
    _owner: Option<Box<dyn Send>>,
}
impl BeginRequest {
    pub(super) fn owned(reply: oneshot::Sender<Result<Tx, Failure>>, owner: Box<dyn Send>) -> Self {
        Self {
            reply,
            _owner: Some(owner),
        }
    }
}
pub(super) struct Driver {
    // これは単一connectionの試験dispatch。pool/取得期限/容量制御の実装ではない。
    // begin senderのclone→sendとcloseの全raceを調停するadmission adapterも未実装。
    begin_sender: Mutex<Option<mpsc::Sender<BeginRequest>>>,
    state: Arc<State>,
}
impl Driver {
    pub fn open(config: Config) -> Self {
        let (sender, receiver) = mpsc::channel::<BeginRequest>(1);
        let state = Arc::new(State::default());
        let worker_state = Arc::clone(&state);
        let worker = std::thread::spawn(move || {
            native_worker(Arc::new(config), worker_state, receiver, None, None)
        });
        // worker自身へJoinHandleの所有者を持たせない。通知はjoinの後にだけ出す。
        let join_state = Arc::clone(&state);
        std::thread::spawn(move || {
            if worker.join().is_err() {
                join_state.fail(join_state.worker_error());
            }
            join_state.update(|stats| stats.joined = true);
        });
        Self {
            begin_sender: Mutex::new(Some(sender)),
            state,
        }
    }
    pub async fn begin(&self) -> Result<Tx, Failure> {
        if self.state.failure.lock().unwrap().is_some() {
            return Err(self.state.worker_error());
        }
        let sender = self
            .begin_sender
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| Failure::primary(Kind::Closed, Outcome::NotApplicable, "closing"))?;
        let (reply, receiver) = oneshot::channel();
        sender
            .send(BeginRequest {
                reply,
                _owner: None,
            })
            .await
            .map_err(|_| self.state.worker_error())?;
        self.state.update(|stats| stats.admitted += 1);
        // 此のroot senderはsession senderではない。session EOFには影響しない。
        drop(sender);
        receiver.await.map_err(|_| self.state.worker_error())?
    }
    pub fn stats(&self) -> Stats {
        self.state.snapshot()
    }
    pub async fn admit<F: Future<Output = Result<Tx, Failure>>>(&self, future: &mut Pin<Box<F>>) {
        let target = self.stats().admitted + 1;
        tokio::select! {
            _ = future => panic!("session completed before cleanup barrier"),
            _ = self.state.until(|stats| stats.admitted >= target) => {}
        }
    }
    pub async fn wait_settled(&self, count: usize) {
        self.state.until(|stats| stats.settled >= count).await;
    }
    pub async fn wait_closing(&self) {
        self.state.until(|stats| stats.closing).await;
    }
    fn request_close(&self) {
        self.state.update(|stats| stats.closing = true);
        self.begin_sender.lock().unwrap().take();
    }
    pub async fn close(&self, timeout: Duration) -> Result<(), Failure> {
        self.request_close();
        // 0msでも、終了確認済みなら成功する。timeoutは終了そのものではない。
        if !self.stats().joined
            && tokio::time::timeout(timeout, self.state.until(|stats| stats.joined))
                .await
                .is_err()
        {
            return Err(Failure::primary(
                Kind::CloseTimeout,
                Outcome::NotApplicable,
                "worker not joined",
            ));
        }
        if let Some(error) = self.state.failure.lock().unwrap().clone() {
            return Err(error);
        }
        assert!(self.stats().native_closed);
        Ok(())
    }
    pub async fn reprepare_probe(&self) -> Result<bool, Failure> {
        // isolated native SQLite probe。公開path/URI policyやPool取得には関与しない。
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE items(n); CREATE VIEW v AS SELECT n FROM items")
            .unwrap();
        let mode = Arc::new(AtomicBool::new(false));
        install_authorizer(&conn, &mode, &self.state, false, false).unwrap();
        let mut statement = conn.prepare("SELECT n FROM v").unwrap();
        conn.execute_batch(
            "DROP VIEW v; CREATE VIEW v AS SELECT cid AS n FROM pragma_table_info('items')",
        )
        .unwrap();
        let denied = statement
            .query([])
            .and_then(|mut rows| rows.next().map(|_| ()))
            .is_err();
        // hookは再prepareとrows/statement破棄まで登録したまま。
        statement.finalize().unwrap();
        conn.close().unwrap();
        Ok(denied)
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.request_close();
    }
}

// direct driverとdeadpool adapterが同じnative開始/SQL/cleanup/closeを呼ぶ。
// checkoutの返却責任はBeginRequestのowned fieldにあり、Tokio taskへ逃がさない。
pub(super) fn native_worker(
    config: Arc<Config>,
    state: Arc<State>,
    mut receiver: mpsc::Receiver<BeginRequest>,
    mut ready: Option<oneshot::Sender<Result<(), Failure>>>,
    startup: Option<Arc<Gate>>,
) {
    CURRENT_MODE.with(|mode| *mode.borrow_mut() = Some(Arc::clone(&state.mode)));
    let result = catch_unwind(AssertUnwindSafe(|| {
        if let Some(gate) = startup {
            gate.block_once();
        }
        let mut conn = match &config.path {
            Some(path) => Connection::open(path).unwrap(),
            None => Connection::open_in_memory().unwrap(),
        };
        conn.busy_timeout(Duration::ZERO).unwrap();
        conn.execute_batch(config.seed).unwrap(); // trusted fixtureのみ。
        install_authorizer(
            &conn,
            &state.mode,
            &state,
            config.deny_rollback,
            config.deny_commit,
        )
        .unwrap();
        if let Some(ready) = ready.take() {
            let _ = ready.send(Ok(()));
        }
        while let Some(request) = receiver.blocking_recv() {
            if state.error().is_some() {
                let _ = request.reply.send(Err(state.worker_error()));
                break;
            }
            if !run_session(&mut conn, request, &config, &state) {
                break;
            }
        }
        if config.leak_statement_at_close {
            std::mem::forget(conn.prepare("SELECT 1").unwrap());
        }
        match conn.close() {
            Ok(()) => state.update(|stats| stats.native_closed = true),
            Err((_conn, error)) => {
                state.fail(Failure::cleanup(Outcome::NotApplicable, error.to_string()))
            }
        }
        if let Some(gate) = &config.exit_gate {
            gate.block_once();
        }
    }));
    if result.is_err() {
        state.fail(state.worker_error());
    }
    if let Some(ready) = ready {
        let _ = ready.send(Err(state.worker_error()));
    }
    CURRENT_MODE.with(|mode| mode.borrow_mut().take());
}

// 任意closureへConnectionやmanagement guardを渡さない。
// typed generic queryのowned replyだけを閉じ込めたprivate executor。
type ReadCommand = Box<dyn FnOnce(&Transaction<'_>) -> Option<Failure> + Send>;
enum Command {
    Exec {
        sql: String,
        values: Vec<Value>,
        reply: oneshot::Sender<Result<i64, Failure>>,
    },
    Read(ReadCommand),
    Finish {
        operation: Finish,
        reply: oneshot::Sender<Result<(), Failure>>,
    },
}
pub(super) struct Tx {
    sender: mpsc::Sender<Command>,
}
impl std::fmt::Debug for Tx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestTx").finish_non_exhaustive()
    }
}
fn reply_lost() -> Failure {
    Failure::primary(Kind::ReplyLost, Outcome::Unknown, "session reply lost")
}
impl Tx {
    pub async fn exec(&self, sql: &str, values: Vec<Value>) -> Result<i64, Failure> {
        let sql = sql.to_owned();
        let (reply, receiver) = oneshot::channel();
        self.sender
            .send(Command::Exec { sql, values, reply })
            .await
            .map_err(|_| reply_lost())?;
        receiver.await.map_err(|_| reply_lost())?
    }
    pub async fn query<T: FromRow>(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<Option<T>, Failure> {
        let mut result = self.read::<T>(sql, values, true).await?;
        Ok(result.pop())
    }
    pub async fn all<T: FromRow>(&self, sql: &str, values: Vec<Value>) -> Result<Vec<T>, Failure> {
        self.read::<T>(sql, values, false).await
    }
    async fn read<T: FromRow>(
        &self,
        sql: &str,
        values: Vec<Value>,
        first: bool,
    ) -> Result<Vec<T>, Failure> {
        let sql = sql.to_owned();
        let (reply, receiver) = oneshot::channel();
        let command = Command::Read(Box::new(move |native| {
            let result = read_rows::<T>(native, &sql, values, first);
            let retired = result.as_ref().err().filter(|error| error.retired).cloned();
            let _ = reply.send(result);
            retired
        }));
        self.sender.send(command).await.map_err(|_| reply_lost())?;
        receiver.await.map_err(|_| reply_lost())?
    }
    // async fnのowned引数はFuture生成時から所有され、未poll DropもEOF cleanupへ進む。
    pub async fn finish(self, operation: Finish) -> Result<(), Failure> {
        let (reply, receiver) = oneshot::channel();
        self.sender
            .send(Command::Finish { operation, reply })
            .await
            .map_err(|_| reply_lost())?;
        receiver.await.map_err(|_| reply_lost())?
    }
    pub fn available_capacity(&self) -> usize {
        self.sender.capacity()
    }
    pub async fn enqueue_without_reply(&self, sql: &str) {
        let (reply, receiver) = oneshot::channel();
        drop(receiver);
        self.sender
            .send(Command::Exec {
                sql: sql.into(),
                values: vec![],
                reply,
            })
            .await
            .unwrap();
    }
}

fn current_failure(conn: &Connection, kind: Kind, message: impl Into<String>) -> Failure {
    if conn.is_autocommit() {
        Failure::primary(Kind::Aborted, Outcome::RolledBack, message)
    } else {
        Failure::primary(kind, Outcome::Active, message)
    }
}
fn prepare<'a>(
    conn: &'a Connection,
    sql: &str,
    values: &[Value],
    read: bool,
) -> Result<rusqlite::Statement<'a>, Failure> {
    if conn.is_autocommit() {
        return Err(current_failure(
            conn,
            Kind::Aborted,
            "native transaction already aborted",
        ));
    }
    assert!(!management_active());
    let statement = conn
        .prepare(sql)
        .map_err(|error| current_failure(conn, Kind::Sql, error.to_string()))?;
    if statement.parameter_count() != values.len()
        || (1..=statement.parameter_count()).any(|index| statement.parameter_name(index).is_some())
    {
        return Err(current_failure(
            conn,
            Kind::Bind,
            "anonymous parameter count/name mismatch",
        ));
    }
    if if read {
        !statement.readonly() || statement.column_count() == 0
    } else {
        statement.column_count() != 0
    } {
        return Err(current_failure(
            conn,
            Kind::Sql,
            "operation statement shape mismatch",
        ));
    }
    Ok(statement)
}
fn execute(conn: &Connection, sql: &str, values: Vec<Value>) -> Result<i64, Failure> {
    let mut statement = prepare(conn, sql, &values, false)?;
    let result = statement
        .execute(params_from_iter(values))
        .map_err(|error| current_failure(conn, Kind::Sql, error.to_string()));
    let finalized = statement.finalize();
    let count = observe_finalization(conn, result, finalized)?;
    i64::try_from(count).map_err(|error| current_failure(conn, Kind::Sql, error.to_string()))
}
fn read_rows<T: FromRow>(
    conn: &Connection,
    sql: &str,
    values: Vec<Value>,
    first: bool,
) -> Result<Vec<T>, Failure> {
    let mut statement = prepare(conn, sql, &values, true)?;
    let result = (|| {
        let indices = indices::<T>(&statement)
            .map_err(|error| current_failure(conn, Kind::Decode, error.to_string()))?;
        let mut rows = statement
            .query(params_from_iter(values))
            .map_err(|error| current_failure(conn, Kind::Bind, error.to_string()))?;
        let mut output = vec![];
        while let Some(row) = rows
            .next()
            .map_err(|error| current_failure(conn, Kind::Sql, error.to_string()))?
        {
            assert!(!management_active());
            output.push(
                T::read(row, &indices)
                    .map_err(|error| current_failure(conn, Kind::Decode, error.to_string()))?,
            );
            if first {
                break;
            }
        }
        Ok(output)
    })();
    let finalized = statement.finalize();
    observe_finalization(conn, result, finalized)
}

fn observe_finalization<T>(
    conn: &Connection,
    result: Result<T, Failure>,
    finalized: rusqlite::Result<()>,
) -> Result<T, Failure> {
    match finalized {
        Ok(()) => result,
        Err(error) => {
            let mut failure = result.err().unwrap_or_else(|| {
                Failure::cleanup(
                    if conn.is_autocommit() {
                        Outcome::RolledBack
                    } else {
                        Outcome::Active
                    },
                    "statement finalize failed",
                )
            });
            failure.cleanup = Some(error.to_string());
            failure.retired = true;
            Err(failure)
        }
    }
}

fn run_session(
    conn: &mut Connection,
    request: BeginRequest,
    config: &Config,
    state: &Arc<State>,
) -> bool {
    let mode = Arc::clone(&state.mode);
    let (result, mut outcome, terminal_reply) = {
        let begun = {
            let _management = Management::enter(&mode);
            conn.transaction_with_behavior(TransactionBehavior::Deferred)
        };
        let mut native = match begun {
            Ok(native) => native,
            Err(error) => {
                let _ = request.reply.send(Err(Failure::primary(
                    Kind::Sql,
                    Outcome::NotApplicable,
                    error.to_string(),
                )));
                return true;
            }
        };
        // native Dropはfinish結果を捨てる。ここでは隠れたfallback/retryを禁止し、
        // native終端のResultと消費後Connectionの状態を明示観測する。
        // panic unwind後にも外側scopeが明示rollbackを所有する。
        native.set_drop_behavior(DropBehavior::Ignore);
        state.update(|stats| stats.begun += 1);
        let (sender, mut inbox) = mpsc::channel(1);
        if let Some(gate) = &config.begin_gate {
            gate.block_once();
        }
        let _ = request.reply.send(Ok(Tx { sender }));
        let mut terminal_reply = None;
        let mut outcome = Outcome::Active;
        let result = catch_unwind(AssertUnwindSafe(|| {
            while let Some(command) = inbox.blocking_recv() {
                match command {
                    Command::Exec { sql, values, reply } => {
                        if let Some(gate) = &config.command_gate {
                            gate.block_once();
                        }
                        let result = execute(&native, &sql, values);
                        state.update(|stats| stats.executed += usize::from(result.is_ok()));
                        let retired = result.as_ref().err().filter(|error| error.retired).cloned();
                        let _ = reply.send(result);
                        if let Some(error) = retired {
                            return Err(error);
                        }
                    }
                    Command::Read(read) => {
                        if let Some(error) = read(&native) {
                            return Err(error);
                        }
                    }
                    Command::Finish { operation, reply } => {
                        terminal_reply = Some(reply);
                        if native.is_autocommit() {
                            outcome = Outcome::RolledBack;
                            return match operation {
                                Finish::Rollback => Ok(()),
                                Finish::Commit => Err(Failure::primary(
                                    Kind::Aborted,
                                    outcome,
                                    "native transaction already aborted",
                                )),
                            };
                        }
                        let result = {
                            let _management = Management::enter(&mode);
                            assert!(!config.panic_management, "private management panic seam");
                            match operation {
                                Finish::Commit => native.commit(),
                                Finish::Rollback => native.rollback(),
                            }
                        };
                        if result.is_ok() {
                            outcome = match operation {
                                Finish::Commit => Outcome::Committed,
                                Finish::Rollback => Outcome::RolledBack,
                            };
                            if matches!(operation, Finish::Commit) {
                                state.update(|stats| stats.committed += 1);
                                if let Some(gate) = &config.commit_gate {
                                    gate.block_once();
                                }
                            }
                        } else {
                            outcome = Outcome::Unknown;
                        }
                        return result.map_err(|error| match operation {
                            Finish::Commit => {
                                Failure::primary(Kind::Sql, Outcome::Unknown, error.to_string())
                            }
                            Finish::Rollback => {
                                Failure::cleanup(Outcome::Unknown, error.to_string())
                            }
                        });
                    }
                }
            }
            if let Some(gate) = &config.cleanup_gate {
                gate.block_once();
            }
            if native.is_autocommit() {
                outcome = Outcome::RolledBack;
                return Ok(());
            }
            let result = {
                let _management = Management::enter(&mode);
                native.rollback()
            };
            if result.is_ok() {
                outcome = Outcome::RolledBack;
            } else {
                outcome = Outcome::Unknown;
            }
            result.map_err(|error| Failure::cleanup(Outcome::Unknown, error.to_string()))
        }));
        (result, outcome, terminal_reply)
    };
    // native Transactionと全Statement/Rowsは上のclosure終了で破棄済み。
    let mut result = match result {
        Ok(result) => result,
        Err(_) => {
            let mut error =
                Failure::primary(Kind::Worker, Outcome::Unknown, "session callback panicked");
            error.retired = true;
            Err(error)
        }
    };
    // COMMITエラーまたはpanicでnative Txがactiveなら、safe hardcoded cleanupだけを実行。
    // 明示ROLLBACKの失敗はretireするため、自動retryをしない。
    if conn.is_autocommit() {
        if outcome == Outcome::Active {
            outcome = Outcome::RolledBack;
        }
    } else if !matches!(&result, Err(error) if error.kind == Kind::Cleanup) {
        let cleanup = {
            let _management = Management::enter(&mode);
            conn.execute_batch("ROLLBACK")
        };
        match cleanup {
            Ok(()) => outcome = Outcome::RolledBack,
            Err(error) => {
                let mut failure = result
                    .err()
                    .unwrap_or_else(|| Failure::cleanup(outcome, "transaction remained active"));
                failure.cleanup = Some(error.to_string());
                failure.retired = true;
                result = Err(failure);
            }
        }
    }
    if !conn.is_autocommit() || mode.load(Ordering::SeqCst) || config.fail_restore_check {
        let mut failure = result
            .err()
            .unwrap_or_else(|| Failure::cleanup(outcome, "cleanup state could not be verified"));
        failure
            .cleanup
            .get_or_insert_with(|| "cleanup state could not be verified".into());
        failure.retired = true;
        result = Err(failure);
    }
    if let Err(error) = &mut result {
        error.outcome = outcome;
        if error.retired {
            state.fail(error.clone());
        }
    }
    state.update(|stats| stats.settled += 1);
    let reusable = state.failure.lock().unwrap().is_none();
    if let Some(reply) = terminal_reply {
        let _ = reply.send(result);
    }
    reusable
}
