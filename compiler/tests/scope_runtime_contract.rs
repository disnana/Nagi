use nagic::{check, emit, modules, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
static NATIVE_RUN: Mutex<()> = Mutex::new(());

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi scope runtime contract {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn checked(&self, name: &str) -> source::Sources {
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        modules::validate(&loaded.program).unwrap();
        loaded
    }

    fn roundtrip(&self) {
        let high = self.checked("main.nagi");
        let low = emit::low(&high.program);
        let mut independent = parser::parse(&low, false).unwrap();
        check::check(&mut independent).unwrap_or_else(|error| panic!("{error}\n{low}"));
        modules::validate(&independent).unwrap();
        self.write("saved.low", &low);
        self.checked("saved.low");
        self.write("handwritten.low", HANDWRITTEN_LOW);
        self.checked("handwritten.low");
    }

    fn build(&self, source: &str, output_dir: &str, target: &Path) -> PathBuf {
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&self.0)
            .args([
                "build",
                source,
                "--rust",
                "native.rs",
                "--out",
                output_dir,
                "--no-project",
            ])
            .env("NAGI_NATIVE_TARGET_DIR", target)
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap_or_else(|error| panic!("could not build {source}: {error}"));
        successful(&output, source);
        let stderr = String::from_utf8(output.stderr).unwrap();
        let binary = stderr
            .lines()
            .find_map(|line| line.strip_prefix("native: "))
            .unwrap_or_else(|| panic!("build omitted native path for {source}: {stderr}"));
        let binary = PathBuf::from(binary);
        assert!(binary.is_file(), "{}", binary.display());
        binary
    }

    fn run_bounded(&self, binary: &Path, source: &str) -> Output {
        let mut child = Command::new(binary)
            .current_dir(&self.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("could not launch {source}: {error}"));
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "native scope contract timed out for {source}: stdout={} stderr={}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        successful(&output, source);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "scope runtime contract preserved",
            "{source}"
        );
        output
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn successful(output: &Output, source: &str) {
    assert!(
        output.status.success(),
        "{source}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const HIGH: &str = r#"
@rust("native::reset_contract")
extern def contract_reset() -> unit
@rust("native::wait_sibling_started")
extern async def contract_wait_sibling_started() -> unit
@rust("native::begin_failure_wait")
extern def contract_begin_failure_wait() -> unit
@rust("native::wait_failure_ready")
extern async def contract_wait_failure_ready() -> unit
@rust("native::mark_body_completed")
extern def contract_mark_body_completed() -> unit
@rust("native::body_completed")
extern def contract_body_completed() -> bool
@rust("native::failure_ready")
extern def contract_failure_ready() -> bool
@rust("native::sibling_started")
extern def contract_sibling_started() -> bool
@rust("native::sibling_dropped")
extern def contract_sibling_dropped() -> bool
@rust("native::error_child")
extern async def contract_error_child() -> Result[unit, Error]
@rust("native::panic_child")
extern async def contract_panic_child() -> Result[unit, Error]
@rust("native::held_sibling")
extern async def contract_held_sibling() -> unit
@rust("native::body_error")
extern async def contract_body_error() -> Result[i64, Error]

async def error_child_scope() -> Result[unit, Error]:
    contract_reset()
    async with scope:
        spawn contract_error_child()
        spawn contract_held_sibling()
        await contract_wait_sibling_started()
        contract_begin_failure_wait()
        await contract_wait_failure_ready()
        contract_mark_body_completed()
    return ok(assert_true(True))

async def panic_child_scope() -> Result[unit, Error]:
    contract_reset()
    async with scope:
        spawn contract_panic_child()
        spawn contract_held_sibling()
        await contract_wait_sibling_started()
        contract_begin_failure_wait()
        await contract_wait_failure_ready()
        contract_mark_body_completed()
    return ok(assert_true(True))

async def body_error_scope() -> Result[unit, Error]:
    contract_reset()
    async with scope:
        spawn contract_held_sibling()
        await contract_wait_sibling_started()
        value = try await contract_body_error()
        assert_true(value == 1)
    return ok(assert_true(True))

async def verify_error_child() -> Result[unit, Error]:
    match await error_child_scope():
        case Ok(_):
            return error("child Error was lost")
        case Err(problem):
            assert_true(error_message(problem) == "controlled child Error")
    assert_true(contract_failure_ready())
    assert_true(contract_body_completed())
    assert_true(contract_sibling_started() and contract_sibling_dropped())
    return ok(assert_true(True))

async def verify_panic_child() -> Result[unit, Error]:
    match await panic_child_scope():
        case Ok(_):
            return error("child panic was lost")
        case Err(problem):
            assert_true(error_kind(problem) == "internal")
    assert_true(contract_failure_ready())
    assert_true(contract_body_completed())
    assert_true(contract_sibling_started() and contract_sibling_dropped())
    return ok(assert_true(True))

async def verify_body_error() -> Result[unit, Error]:
    match await body_error_scope():
        case Ok(_):
            return error("body try failure was lost")
        case Err(problem):
            assert_true(error_message(problem) == "controlled body Error")
    assert_true(contract_sibling_started() and contract_sibling_dropped())
    return ok(assert_true(True))

async def main() -> Result[unit, Error]:
    try await verify_error_child()
    try await verify_panic_child()
    try await verify_body_error()
    return ok(print("scope runtime contract preserved"))
"#;

const HANDWRITTEN_LOW: &str = r#"
@rust("native::reset_contract")
extern fn contract_reset() -> unit;
@rust("native::wait_sibling_started")
extern async fn contract_wait_sibling_started() -> unit;
@rust("native::begin_failure_wait")
extern fn contract_begin_failure_wait() -> unit;
@rust("native::wait_failure_ready")
extern async fn contract_wait_failure_ready() -> unit;
@rust("native::mark_body_completed")
extern fn contract_mark_body_completed() -> unit;
@rust("native::body_completed")
extern fn contract_body_completed() -> bool;
@rust("native::failure_ready")
extern fn contract_failure_ready() -> bool;
@rust("native::sibling_started")
extern fn contract_sibling_started() -> bool;
@rust("native::sibling_dropped")
extern fn contract_sibling_dropped() -> bool;
@rust("native::error_child")
extern async fn contract_error_child() -> Result[unit, Error];
@rust("native::panic_child")
extern async fn contract_panic_child() -> Result[unit, Error];
@rust("native::held_sibling")
extern async fn contract_held_sibling() -> unit;
@rust("native::body_error")
extern async fn contract_body_error() -> Result[i64, Error];

async fn error_child_scope() -> Result[unit, Error] {
    contract_reset();
    scope {
        spawn contract_error_child();
        spawn contract_held_sibling();
        await contract_wait_sibling_started();
        contract_begin_failure_wait();
        await contract_wait_failure_ready();
        contract_mark_body_completed();
    }
    return ok(assert_true(true));
}

async fn panic_child_scope() -> Result[unit, Error] {
    contract_reset();
    scope {
        spawn contract_panic_child();
        spawn contract_held_sibling();
        await contract_wait_sibling_started();
        contract_begin_failure_wait();
        await contract_wait_failure_ready();
        contract_mark_body_completed();
    }
    return ok(assert_true(true));
}

async fn body_error_scope() -> Result[unit, Error] {
    contract_reset();
    scope {
        spawn contract_held_sibling();
        await contract_wait_sibling_started();
        let value = try await contract_body_error();
        assert_true(value == 1);
    }
    return ok(assert_true(true));
}

async fn verify_error_child() -> Result[unit, Error] {
    match await error_child_scope() {
        case Ok(_) { return error("child Error was lost"); }
        case Err(problem) { assert_true(error_message(problem) == "controlled child Error"); }
    }
    assert_true(contract_failure_ready());
    assert_true(contract_body_completed());
    assert_true(contract_sibling_started() and contract_sibling_dropped());
    return ok(assert_true(true));
}

async fn verify_panic_child() -> Result[unit, Error] {
    match await panic_child_scope() {
        case Ok(_) { return error("child panic was lost"); }
        case Err(problem) { assert_true(error_kind(problem) == "internal"); }
    }
    assert_true(contract_failure_ready());
    assert_true(contract_body_completed());
    assert_true(contract_sibling_started() and contract_sibling_dropped());
    return ok(assert_true(true));
}

async fn verify_body_error() -> Result[unit, Error] {
    match await body_error_scope() {
        case Ok(_) { return error("body try failure was lost"); }
        case Err(problem) { assert_true(error_message(problem) == "controlled body Error"); }
    }
    assert_true(contract_sibling_started() and contract_sibling_dropped());
    return ok(assert_true(true));
}

async fn main() -> Result[unit, Error] {
    try await verify_error_child();
    try await verify_panic_child();
    try await verify_body_error();
    return ok(print("scope runtime contract preserved"));
}
"#;

const NATIVE: &str = r#"
use nagi_runtime::Error;
use std::{
    future::{pending, Future},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    task::{Context, Poll, Waker},
};

static BODY_ACTIVE: AtomicBool = AtomicBool::new(false);
static FAILURE_READY: AtomicBool = AtomicBool::new(false);
static BODY_COMPLETED: AtomicBool = AtomicBool::new(false);
static SIBLING_STARTED: AtomicBool = AtomicBool::new(false);
static SIBLING_DROPPED: AtomicBool = AtomicBool::new(false);
static WAITERS: Mutex<Vec<Waker>> = Mutex::new(Vec::new());

struct FlagWait(&'static AtomicBool);

impl Future for FlagWait {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let flag = self.get_mut().0;
        if flag.load(Ordering::Acquire) {
            return Poll::Ready(());
        }
        let mut waiters = WAITERS.lock().unwrap();
        if flag.load(Ordering::Acquire) {
            return Poll::Ready(());
        }
        if !waiters.iter().any(|waker| waker.will_wake(context.waker())) {
            waiters.push(context.waker().clone());
        }
        Poll::Pending
    }
}

fn signal(flag: &'static AtomicBool) {
    flag.store(true, Ordering::Release);
    let waiters = std::mem::take(&mut *WAITERS.lock().unwrap());
    for waker in waiters {
        waker.wake();
    }
}

struct SignalOnDrop(&'static AtomicBool);

impl Drop for SignalOnDrop {
    fn drop(&mut self) {
        signal(self.0);
    }
}

pub fn reset_contract() {
    BODY_ACTIVE.store(false, Ordering::Release);
    FAILURE_READY.store(false, Ordering::Release);
    BODY_COMPLETED.store(false, Ordering::Release);
    SIBLING_STARTED.store(false, Ordering::Release);
    SIBLING_DROPPED.store(false, Ordering::Release);
    WAITERS.lock().unwrap().clear();
}

pub async fn wait_sibling_started() {
    FlagWait(&SIBLING_STARTED).await;
}

pub fn begin_failure_wait() {
    signal(&BODY_ACTIVE);
}

pub async fn wait_failure_ready() {
    FlagWait(&FAILURE_READY).await;
}

pub fn mark_body_completed() {
    BODY_COMPLETED.store(true, Ordering::Release);
}

pub fn body_completed() -> bool {
    BODY_COMPLETED.load(Ordering::Acquire)
}

pub fn failure_ready() -> bool {
    FAILURE_READY.load(Ordering::Acquire)
}

pub fn sibling_started() -> bool {
    SIBLING_STARTED.load(Ordering::Acquire)
}

pub fn sibling_dropped() -> bool {
    SIBLING_DROPPED.load(Ordering::Acquire)
}

pub async fn error_child() -> Result<(), Error> {
    FlagWait(&BODY_ACTIVE).await;
    // This marks the helper's failure exit, not observation by Scope's JoinSet.
    let _ready = SignalOnDrop(&FAILURE_READY);
    Err(Error::invalid("controlled child Error"))
}

pub async fn panic_child() -> Result<(), Error> {
    FlagWait(&BODY_ACTIVE).await;
    // During unwinding, this wakes the active scope body to finish its own work.
    let _ready = SignalOnDrop(&FAILURE_READY);
    panic!("controlled scope child panic");
}

pub async fn held_sibling() {
    let _dropped = SignalOnDrop(&SIBLING_DROPPED);
    signal(&SIBLING_STARTED);
    pending::<()>().await;
}

pub async fn body_error() -> Result<i64, Error> {
    Err(Error::invalid("controlled body Error"))
}
"#;

#[test]
fn generated_scope_failure_and_cancellation_contract_runs_with_the_real_runtime() {
    let _native_run = NATIVE_RUN
        .lock()
        .expect("the shared native build/run lock must remain unpoisoned");
    let fixture = Fixture::new();
    fixture.write("main.nagi", HIGH);
    fixture.write("native.rs", NATIVE);
    fixture.roundtrip();

    let native_target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    let sources = [
        ("main.nagi", "build/high"),
        ("saved.low", "build/saved-low"),
        ("handwritten.low", "build/handwritten-low"),
    ];
    for (source, output_dir) in sources {
        let binary = fixture.build(source, output_dir, &native_target);
        fixture.run_bounded(&binary, source);
        if source == "main.nagi" {
            // The saved and handwritten Low runs must not depend on the High file.
            fs::remove_file(fixture.0.join("main.nagi")).unwrap();
        }
    }
}
