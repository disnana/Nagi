#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-view-container-drop-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn compile_and_run(&self, name: &str, program: &nagic::ast::Program) {
        let adapter = self.0.join(format!("{name}-native.rs"));
        fs::write(&adapter, NATIVE_ADAPTER).unwrap();
        let rust = format!(
            "{}\n{}\n#[path = {}]\nmod native;\n{}",
            emit::rust(&checked_emission::seal(program)).unwrap(),
            RUNTIME,
            serde_json::to_string(&adapter.file_name().unwrap().to_string_lossy()).unwrap(),
            ASSERTIONS
        );
        let file = self.0.join(format!("{name}.rs"));
        fs::write(&file, rust).unwrap();
        let binary = self
            .0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test", "-C", "panic=unwind"])
            .arg(&file)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(&binary).arg("--nocapture").output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if let Some(directory) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
            let directory = PathBuf::from(directory);
            fs::create_dir_all(&directory).unwrap();
            fs::copy(&file, directory.join(format!("{name}.rs"))).unwrap();
            fs::copy(&adapter, directory.join(format!("{name}-native.rs"))).unwrap();
            fs::write(directory.join(format!("{name}.stdout")), &output.stdout).unwrap();
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked(text: &str, high: bool) -> nagic::ast::Program {
    let mut program = parser::parse(text, high).unwrap_or_else(|error| panic!("{text}\n{error}"));
    check::check(&mut program).unwrap_or_else(|error| panic!("{text}\n{error}"));
    program
}

#[test]
fn local_view_container_replacement_preserves_allocations_and_drop_order() {
    let high = checked(HIGH, true);
    let saved_low = checked(&emit::low(&high), false);
    let handwritten_low = checked(LOW, false);
    let fixture = Fixture::new();

    for (name, program) in [
        ("high", &high),
        ("saved-low", &saved_low),
        ("handwritten-low", &handwritten_low),
    ] {
        fixture.compile_and_run(name, program);
    }
}

const HIGH: &str = r#"enum Tag:
    Text(value: str)

class Marker:
    id: i64
    tag: Tag

@rust("native::marker")
extern def marker(id: i64) -> Marker
@rust("native::capture")
extern def capture(id: i64)
@rust("native::bad_index")
extern def bad_index() -> i64
@rust("native::crash")
extern def crash()
@rust("native::consume")
extern def consume(parts: List[view[str]]) -> bool
@rust("native::checkpoint")
extern def checkpoint()
@rust("native::abort_mutation")
extern def abort_mutation()
@rust("native::fail_step")
extern def fail_step() -> Result[unit, i64]
@rust("native::pause")
extern async def pause()

def mutation_crash(value: view[str]) -> view[str]:
    abort_mutation()
    return value

def replace_and_return(parts: List[view[str]]) -> List[view[str]]:
    before = marker(10)
    local = copy(parts[0])
    capture(1)
    alias = [view(local)]
    capture(2)
    alias = [parts[0]]
    after = marker(20)
    return alias

def panic_during_replacement(parts: List[view[str]]) -> List[view[str]]:
    before = marker(30)
    local = copy(parts[0])
    capture(3)
    alias = [view(local)]
    after = marker(40)
    capture(4)
    alias = [parts[bad_index()]]
    return alias

def panic_after_replacement(parts: List[view[str]]) -> List[view[str]]:
    before = marker(50)
    local = copy(parts[0])
    capture(5)
    alias = [view(local)]
    after = marker(60)
    capture(6)
    alias = [parts[0]]
    crash()
    return alias

def short_circuit_replacement(flag: bool, parts: List[view[str]]) -> List[view[str]]:
    before = marker(70)
    local = copy(parts[0])
    capture(7)
    alias = [view(local)]
    after = marker(80)
    selected = flag and consume(alias)
    capture(8)
    alias = [parts[0]]
    checkpoint()
    return alias

def branch_mutation(flag: bool, parts: List[view[str]]) -> List[view[str]]:
    before = marker(90)
    local = copy(parts[0])
    capture(9)
    alias = [parts[0]]
    after = marker(100)
    if flag:
        append(alias, view(local))
        capture(10)
        alias = [parts[0]]
    return alias

def panic_during_mutation(parts: List[view[str]]) -> List[view[str]]:
    before = marker(110)
    local = copy(parts[0])
    capture(11)
    alias = [parts[0]]
    after = marker(120)
    append(alias, mutation_crash(view(local)))
    alias = [parts[0]]
    return alias

def alias_move_then_panic(part: view[str]) -> List[view[str]]:
    before = marker(130)
    local = copy(part)
    capture(13)
    parts = [view(local)]
    after = marker(140)
    capture(14)
    parts = [part]
    selected = parts
    crash()
    return selected

def restored_list_then_error(part: view[str]) -> Result[List[view[str]], i64]:
    before = marker(150)
    local = copy(part)
    capture(15)
    parts = [view(local)]
    after = marker(160)
    capture(16)
    parts = [part]
    try fail_step()
    return ok(parts)

def pattern_list_then_panic(part: view[str], input: Result[List[view[str]], i64]) -> Result[List[view[str]], i64]:
    before = marker(170)
    match input:
        case Ok(parts):
            local = copy(part)
            capture(17)
            parts = [view(local)]
            after = marker(180)
            capture(18)
            parts = [part]
            crash()
            return ok(parts)
        case Err(code):
            return fail(code)

def list_with_owned_error_elements(part: view[str]) -> List[Result[view[str], Marker]]:
    before = marker(190)
    local = copy(part)
    capture(19)
    parts: List[Result[view[str], Marker]] = [ok(view(local)), fail(marker(200))]
    after = marker(220)
    capture(20)
    parts = [ok(part), fail(marker(210))]
    return parts

def result_error_replacement(part: view[str]) -> Result[List[view[str]], Marker]:
    before = marker(260)
    local = copy(part)
    capture(41)
    value: Result[List[view[str]], Marker] = ok([view(local)])
    after = marker(270)
    value = fail(marker(280))
    value = fail(marker(290))
    match value:
        case Ok(_):
            return ok([part])
        case Err(problem):
            return fail(problem)

def option_replacement(part: view[str]) -> Option[List[view[str]]]:
    before = marker(300)
    local = copy(part)
    capture(42)
    value: Option[List[view[str]]] = some([view(local)])
    after = marker(310)
    value = None
    capture(43)
    value = some([part])
    return value

async def async_replacement(part: view[str], panic_now: bool, error_now: bool) -> Result[List[view[str]], i64]:
    before = marker(320)
    local = copy(part)
    capture(44)
    parts = [view(local)]
    after = marker(330)
    capture(45)
    parts = [part]
    await pause()
    if panic_now:
        crash()
    if error_now:
        try fail_step()
    return ok(parts)

async def async_before_replacement(part: view[str]) -> List[view[str]]:
    before = marker(340)
    local = copy(part)
    capture(46)
    parts = [view(local)]
    after = marker(350)
    await pause()
    capture(47)
    parts = [part]
    return parts

def loop_replacement(part: view[str], count: i64) -> List[view[str]]:
    before = marker(360)
    parts = [part]
    for number in range(count):
        local = copy(part)
        capture(48)
        parts = [view(local)]
        within = marker(370)
        capture(49)
        parts = [part]
    after = marker(380)
    return parts
"#;

// This Low is written separately so the fixture checks the same return and
// lifetime contract without relying on High-to-Low serialization.
const LOW: &str = r#"enum Tag { Text(value: str) }
record Marker { id: i64; tag: Tag }

@rust("native::marker")
extern fn marker(id: i64) -> Marker;
@rust("native::capture")
extern fn capture(id: i64) -> unit;
@rust("native::bad_index")
extern fn bad_index() -> i64;
@rust("native::crash")
extern fn crash() -> unit;
@rust("native::consume")
extern fn consume(parts: List[view[str]]) -> bool;
@rust("native::checkpoint")
extern fn checkpoint() -> unit;
@rust("native::abort_mutation")
extern fn abort_mutation() -> unit;
@rust("native::fail_step")
extern fn fail_step() -> Result[unit, i64];
@rust("native::pause")
extern async fn pause() -> unit;

fn mutation_crash(value: view[str]) -> view[str] {
    abort_mutation();
    return value;
}

fn replace_and_return(parts: List[view[str]]) -> List[view[str]] {
    let before: Marker = marker(10);
    let local: str = copy(parts[0]);
    capture(1);
    let alias: List[view[str]] = [view(local)];
    capture(2);
    alias = [parts[0]];
    let after: Marker = marker(20);
    return alias;
}

fn panic_during_replacement(parts: List[view[str]]) -> List[view[str]] {
    let before: Marker = marker(30);
    let local: str = copy(parts[0]);
    capture(3);
    let alias: List[view[str]] = [view(local)];
    let after: Marker = marker(40);
    capture(4);
    alias = [parts[bad_index()]];
    return alias;
}

fn panic_after_replacement(parts: List[view[str]]) -> List[view[str]] {
    let before: Marker = marker(50);
    let local: str = copy(parts[0]);
    capture(5);
    let alias: List[view[str]] = [view(local)];
    let after: Marker = marker(60);
    capture(6);
    alias = [parts[0]];
    crash();
    return alias;
}

fn short_circuit_replacement(flag: bool, parts: List[view[str]]) -> List[view[str]] {
    let before: Marker = marker(70);
    let local: str = copy(parts[0]);
    capture(7);
    let alias: List[view[str]] = [view(local)];
    let after: Marker = marker(80);
    let selected: bool = flag and consume(alias);
    capture(8);
    alias = [parts[0]];
    checkpoint();
    return alias;
}

fn branch_mutation(flag: bool, parts: List[view[str]]) -> List[view[str]] {
    let before: Marker = marker(90);
    let local: str = copy(parts[0]);
    capture(9);
    let alias: List[view[str]] = [parts[0]];
    let after: Marker = marker(100);
    if flag {
        append(alias, view(local));
        capture(10);
        alias = [parts[0]];
    }
    return alias;
}

fn panic_during_mutation(parts: List[view[str]]) -> List[view[str]] {
    let before: Marker = marker(110);
    let local: str = copy(parts[0]);
    capture(11);
    let alias: List[view[str]] = [parts[0]];
    let after: Marker = marker(120);
    append(alias, mutation_crash(view(local)));
    alias = [parts[0]];
    return alias;
}

fn alias_move_then_panic(part: view[str]) -> List[view[str]] {
    let before: Marker = marker(130);
    let local: str = copy(part);
    capture(13);
    let parts: List[view[str]] = [view(local)];
    let after: Marker = marker(140);
    capture(14);
    parts = [part];
    let selected: List[view[str]] = parts;
    crash();
    return selected;
}

fn restored_list_then_error(part: view[str]) -> Result[List[view[str]], i64] {
    let before: Marker = marker(150);
    let local: str = copy(part);
    capture(15);
    let parts: List[view[str]] = [view(local)];
    let after: Marker = marker(160);
    capture(16);
    parts = [part];
    try fail_step();
    return ok(parts);
}

fn pattern_list_then_panic(part: view[str], input: Result[List[view[str]], i64]) -> Result[List[view[str]], i64] {
    let before: Marker = marker(170);
    match input {
        case Ok(parts) {
            let local: str = copy(part);
            capture(17);
            parts = [view(local)];
            let after: Marker = marker(180);
            capture(18);
            parts = [part];
            crash();
            return ok(parts);
        }
        case Err(code) {
            return fail(code);
        }
    }
}

fn list_with_owned_error_elements(part: view[str]) -> List[Result[view[str], Marker]] {
    let before: Marker = marker(190);
    let local: str = copy(part);
    capture(19);
    let parts: List[Result[view[str], Marker]] = [ok(view(local)), fail(marker(200))];
    let after: Marker = marker(220);
    capture(20);
    parts = [ok(part), fail(marker(210))];
    return parts;
}

fn result_error_replacement(part: view[str]) -> Result[List[view[str]], Marker] {
    let before: Marker = marker(260);
    let local: str = copy(part);
    capture(41);
    let value: Result[List[view[str]], Marker] = ok([view(local)]);
    let after: Marker = marker(270);
    value = fail(marker(280));
    value = fail(marker(290));
    match value {
        case Ok(_) { return ok([part]); }
        case Err(problem) { return fail(problem); }
    }
}
fn option_replacement(part: view[str]) -> Option[List[view[str]]] {
    let before: Marker = marker(300);
    let local: str = copy(part);
    capture(42);
    let value: Option[List[view[str]]] = some([view(local)]);
    let after: Marker = marker(310);
    value = None;
    capture(43);
    value = some([part]);
    return value;
}
async fn async_replacement(part: view[str], panic_now: bool, error_now: bool) -> Result[List[view[str]], i64] {
    let before: Marker = marker(320);
    let local: str = copy(part);
    capture(44);
    let parts: List[view[str]] = [view(local)];
    let after: Marker = marker(330);
    capture(45);
    parts = [part];
    await pause();
    if panic_now { crash(); }
    if error_now { try fail_step(); }
    return ok(parts);
}
async fn async_before_replacement(part: view[str]) -> List[view[str]] {
    let before: Marker = marker(340);
    let local: str = copy(part);
    capture(46);
    let parts: List[view[str]] = [view(local)];
    let after: Marker = marker(350);
    await pause();
    capture(47);
    parts = [part];
    return parts;
}
fn loop_replacement(part: view[str], count: i64) -> List[view[str]] {
    let before: Marker = marker(360);
    let parts: List[view[str]] = [part];
    for number in range(count) {
        let local: str = copy(part);
        capture(48);
        parts = [view(local)];
        let within: Marker = marker(370);
        capture(49);
        parts = [part];
    }
    let after: Marker = marker(380);
    return parts;
}
"#;

const RUNTIME: &str = r#"
use ::std::alloc::{GlobalAlloc, Layout, System};
use ::std::sync::atomic::{AtomicUsize, Ordering};

const EVENT_CAPACITY: usize = 128;
const TRACK_CAPACITY: usize = 16;
const ALLOC_BASE: usize = 1_000;
const FREE_BASE: usize = 2_000;
const DROP_BASE: usize = 3_000;
const BAD_INDEX: usize = 4_000;
const CAUGHT: usize = 5_000;
const CRASH: usize = 6_000;
const CHECKPOINT: usize = 7_000;
const REALLOC_BASE: usize = 8_000;
const MUTATION_PANIC: usize = 9_000;

static EVENTS: [AtomicUsize; EVENT_CAPACITY] =
    [const { AtomicUsize::new(0) }; EVENT_CAPACITY];
static EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);
static PANIC_MARKER_ID: AtomicUsize = AtomicUsize::new(0);
static PAUSE_READY: ::std::sync::atomic::AtomicBool =
    ::std::sync::atomic::AtomicBool::new(false);
static CAPTURE_VEC_ID: AtomicUsize = AtomicUsize::new(0);
static TRACKED_POINTERS: [AtomicUsize; TRACK_CAPACITY] =
    [const { AtomicUsize::new(0) }; TRACK_CAPACITY];
static TRACKED_IDS: [AtomicUsize; TRACK_CAPACITY] =
    [const { AtomicUsize::new(0) }; TRACK_CAPACITY];

fn record(event: usize) {
    let index = EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
    if index < EVENT_CAPACITY {
        EVENTS[index].store(event, Ordering::Relaxed);
    }
}

fn tracked_count() -> usize {
    TRACKED_POINTERS
        .iter()
        .filter(|pointer| pointer.load(Ordering::Relaxed) != 0)
        .count()
}

fn event_log() -> Vec<usize> {
    let count = EVENT_COUNT.load(Ordering::Relaxed).min(EVENT_CAPACITY);
    (0..count)
        .map(|index| EVENTS[index].load(Ordering::Relaxed))
        .collect()
}

fn reset_events() {
    assert_eq!(tracked_count(), 0, "a tracked view-container buffer escaped");
    EVENT_COUNT.store(0, Ordering::Relaxed);
    CAPTURE_VEC_ID.store(0, Ordering::Relaxed);
    PANIC_MARKER_ID.store(0, Ordering::Relaxed);
}

fn assert_events(expected: &[usize], tracked: usize) {
    assert_eq!(event_log(), expected);
    assert_eq!(tracked_count(), tracked);
}

struct AllocationLog;

#[global_allocator]
static ALLOCATOR: AllocationLog = AllocationLog;

unsafe impl GlobalAlloc for AllocationLog {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Safety: delegate the same allocation layout to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        let id = CAPTURE_VEC_ID.swap(0, Ordering::Relaxed);
        if !pointer.is_null() && id != 0 {
            for index in 0..TRACK_CAPACITY {
                if TRACKED_POINTERS[index]
                    .compare_exchange(
                        0,
                        pointer as usize,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    TRACKED_IDS[index].store(id, Ordering::Relaxed);
                    record(ALLOC_BASE + id);
                    break;
                }
            }
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let tracked = TRACKED_POINTERS.iter().position(|entry| {
            entry.load(Ordering::Relaxed) == pointer as usize
        });
        // Safety: forward the original pointer/layout and requested size to
        // the same allocator. A failed realloc leaves the old buffer valid.
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            if let Some(index) = tracked {
                TRACKED_POINTERS[index].store(next as usize, Ordering::Relaxed);
                record(REALLOC_BASE + TRACKED_IDS[index].load(Ordering::Relaxed));
            }
        }
        next
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let mut freed_id = 0;
        for index in 0..TRACK_CAPACITY {
            if TRACKED_POINTERS[index]
                .compare_exchange(
                    pointer as usize,
                    0,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                freed_id = TRACKED_IDS[index].swap(0, Ordering::Relaxed);
                break;
            }
        }
        // Safety: this pointer and matching layout came from `System.alloc`.
        unsafe { System.dealloc(pointer, layout) };
        if freed_id != 0 {
            record(FREE_BASE + freed_id);
        }
    }
}
"#;

const NATIVE_ADAPTER: &str = r#"pub fn marker(id: i64) -> super::Marker {
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
"#;

const ASSERTIONS: &str = r#"
// Same public arguments, resources, allocation count, await and output as
// async_replacement, with caller-only borrows that need no lifetime splitting.
async fn caller_only_reference(part: &str, panic_now: bool, error_now: bool) -> Result<Vec<&str>, i64> {
    let before = native::marker(320);
    let local = part.to_owned();
    let mut parts = vec![part];
    let after = native::marker(330);
    parts = vec![part];
    native::pause().await;
    if panic_now { native::crash(); }
    if error_now { native::fail_step()?; }
    Ok(parts)
}

// Match the generated function's local-to-caller borrow history and original
// cleanup anchors. Use the same async extern wrapper, not native::pause directly.
// These are measurement oracles; exact Future sizes are toolchain-dependent.
async fn same_history_reference(part: &str, panic_now: bool, error_now: bool) -> Result<Vec<&str>, i64> {
    let before = native::marker(320);
    let local = part.to_owned();
    let mut previous = Some(vec![local.as_str()]);
    let next;
    let after = native::marker(330);
    next = Some(vec![part]);
    previous = None;
    drop(previous);
    pause().await;
    if panic_now { native::crash(); }
    if error_now { native::fail_step()?; }
    Ok(next.expect("initialized"))
}

// Change only the await bridge, keeping the same slots and cleanup anchors.
async fn same_history_native_pause(part: &str, panic_now: bool, error_now: bool) -> Result<Vec<&str>, i64> {
    let before = native::marker(320);
    let local = part.to_owned();
    let mut previous = Some(vec![local.as_str()]);
    let next;
    let after = native::marker(330);
    next = Some(vec![part]);
    previous = None;
    drop(previous);
    native::pause().await;
    if panic_now { native::crash(); }
    if error_now { native::fail_step()?; }
    Ok(next.expect("initialized"))
}

fn poll_once<F: ::std::future::Future>(future: ::std::pin::Pin<&mut F>) -> ::std::task::Poll<F::Output> {
    let mut context = ::std::task::Context::from_waker(::std::task::Waker::noop());
    future.poll(&mut context)
}

#[test]
fn replacement_observes_rhs_then_old_free_and_scope_drop_order() {
    let source = String::from("caller-owned");
    let generated_size = ::std::mem::size_of_val(&async_replacement(source.as_str(), false, false));
    let reference_size = ::std::mem::size_of_val(&caller_only_reference(source.as_str(), false, false));
    println!("future-bytes: generated={generated_size} caller-only-reference={reference_size}");
    let same_history_size = ::std::mem::size_of_val(&same_history_reference(source.as_str(), false, false));
    println!("future-layout: same-history-same-bridge={same_history_size} same-history-native-pause={} native-pause={} extern-wrapper={}",
        ::std::mem::size_of_val(&same_history_native_pause(source.as_str(), false, false)),
        ::std::mem::size_of_val(&native::pause()), ::std::mem::size_of_val(&pause()));

    reset_events();
    let returned = replace_and_return(vec![source.as_str()]);
    assert_eq!(returned, vec!["caller-owned"]);
    assert_events(
        &[
            ALLOC_BASE + 1,
            ALLOC_BASE + 2,
            FREE_BASE + 1,
            DROP_BASE + 20,
            DROP_BASE + 10,
        ],
        1,
    );
    drop(returned);
    assert_events(
        &[
            ALLOC_BASE + 1,
            ALLOC_BASE + 2,
            FREE_BASE + 1,
            DROP_BASE + 20,
            DROP_BASE + 10,
            FREE_BASE + 2,
        ],
        0,
    );

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        panic_during_replacement(vec![source.as_str()])
    }));
    assert!(result.is_err());
    record(CAUGHT);
    assert_events(
        &[
            ALLOC_BASE + 3,
            ALLOC_BASE + 4,
            BAD_INDEX,
            FREE_BASE + 4,
            DROP_BASE + 40,
            FREE_BASE + 3,
            DROP_BASE + 30,
            CAUGHT,
        ],
        0,
    );

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        panic_after_replacement(vec![source.as_str()])
    }));
    assert!(result.is_err());
    record(CAUGHT);
    assert_events(
        &[
            ALLOC_BASE + 5,
            ALLOC_BASE + 6,
            FREE_BASE + 5,
            CRASH,
            DROP_BASE + 60,
            FREE_BASE + 6,
            DROP_BASE + 50,
            CAUGHT,
        ],
        0,
    );

    for flag in [false, true] {
        reset_events();
        let returned = short_circuit_replacement(flag, vec![source.as_str()]);
        assert_eq!(returned, vec!["caller-owned"]);
        let expected = if flag {
            vec![ALLOC_BASE + 7, FREE_BASE + 7, ALLOC_BASE + 8, CHECKPOINT,
                 DROP_BASE + 80, DROP_BASE + 70]
        } else {
            vec![ALLOC_BASE + 7, ALLOC_BASE + 8, FREE_BASE + 7, CHECKPOINT,
                 DROP_BASE + 80, DROP_BASE + 70]
        };
        assert_events(&expected, 1);
        drop(returned);
        let mut finished = expected;
        finished.push(FREE_BASE + 8);
        assert_events(&finished, 0);
    }

    for flag in [false, true] {
        reset_events();
        let returned = branch_mutation(flag, vec![source.as_str()]);
        assert_eq!(returned, vec!["caller-owned"]);
        let (expected, returned_id) = if flag {
            (vec![ALLOC_BASE + 9, REALLOC_BASE + 9, ALLOC_BASE + 10,
                  FREE_BASE + 9, DROP_BASE + 100, DROP_BASE + 90], 10)
        } else {
            (vec![ALLOC_BASE + 9, DROP_BASE + 100, DROP_BASE + 90], 9)
        };
        assert_events(&expected, 1);
        drop(returned);
        let mut finished = expected;
        finished.push(FREE_BASE + returned_id);
        assert_events(&finished, 0);
    }

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        panic_during_mutation(vec![source.as_str()])
    }));
    assert!(result.is_err());
    record(CAUGHT);
    assert_events(
        &[ALLOC_BASE + 11, MUTATION_PANIC, DROP_BASE + 120,
          FREE_BASE + 11, DROP_BASE + 110, CAUGHT],
        0,
    );

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        alias_move_then_panic(source.as_str())
    }));
    assert!(result.is_err());
    record(CAUGHT);
    // Moving into a new source binding transfers its cleanup position too.
    // `selected` is declared after the later Marker and must drop first.
    assert_events(
        &[ALLOC_BASE + 13, ALLOC_BASE + 14, FREE_BASE + 13, CRASH,
          FREE_BASE + 14, DROP_BASE + 140, DROP_BASE + 130, CAUGHT],
        0,
    );

    reset_events();
    assert_eq!(restored_list_then_error(source.as_str()), Err(7));
    assert_events(
        &[ALLOC_BASE + 15, ALLOC_BASE + 16, FREE_BASE + 15, CHECKPOINT,
          DROP_BASE + 160, FREE_BASE + 16, DROP_BASE + 150],
        0,
    );

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        pattern_list_then_panic(source.as_str(), Ok(vec![source.as_str()]))
    }));
    assert!(result.is_err());
    record(CAUGHT);
    // A pattern-bound container has the pattern's cleanup anchor, before
    // the Marker in the arm. Synthetic storage must not move that anchor.
    assert_events(
        &[ALLOC_BASE + 17, ALLOC_BASE + 18, FREE_BASE + 17, CRASH,
          DROP_BASE + 180, FREE_BASE + 18, DROP_BASE + 170, CAUGHT],
        0,
    );

    reset_events();
    let returned = list_with_owned_error_elements(source.as_str());
    assert!(matches!(&returned[0], Ok(value) if *value == "caller-owned"));
    assert!(matches!(&returned[1], Err(value) if value.id == 210));
    let expected = [ALLOC_BASE + 19, ALLOC_BASE + 20, DROP_BASE + 200,
                    FREE_BASE + 19, DROP_BASE + 220, DROP_BASE + 190];
    assert_events(&expected, 1);
    drop(returned);
    let mut expected = expected.to_vec();
    expected.extend([DROP_BASE + 210, FREE_BASE + 20]);
    assert_events(&expected, 0);

    reset_events();
    PANIC_MARKER_ID.store(200, Ordering::Relaxed);
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        list_with_owned_error_elements(source.as_str())
    }));
    assert!(result.is_err());
    record(CAUGHT);
    // Rust assignment keeps the already evaluated replacement at the source
    // binding's cleanup position even if destroying an old element panics.
    assert_events(
        &[ALLOC_BASE + 19, ALLOC_BASE + 20, DROP_BASE + 200, FREE_BASE + 19,
          DROP_BASE + 220, DROP_BASE + 210, FREE_BASE + 20, DROP_BASE + 190, CAUGHT],
        0,
    );

    reset_events();
    let returned = result_error_replacement(source.as_str());
    assert!(matches!(&returned, Err(value) if value.id == 290));
    assert_events(&[ALLOC_BASE + 41, FREE_BASE + 41, DROP_BASE + 280,
                    DROP_BASE + 270, DROP_BASE + 260], 0);
    drop(returned);
    assert_events(&[ALLOC_BASE + 41, FREE_BASE + 41, DROP_BASE + 280,
                    DROP_BASE + 270, DROP_BASE + 260, DROP_BASE + 290], 0);

    reset_events();
    PANIC_MARKER_ID.store(280, Ordering::Relaxed);
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        result_error_replacement(source.as_str())
    }));
    assert!(result.is_err());
    record(CAUGHT);
    assert_events(&[ALLOC_BASE + 41, FREE_BASE + 41, DROP_BASE + 280,
                    DROP_BASE + 270, DROP_BASE + 290, DROP_BASE + 260, CAUGHT], 0);

    reset_events();
    let returned = option_replacement(source.as_str());
    assert_eq!(returned.as_deref(), Some(&[source.as_str()][..]));
    assert_events(&[ALLOC_BASE + 42, FREE_BASE + 42, ALLOC_BASE + 43,
                    DROP_BASE + 310, DROP_BASE + 300], 1);
    drop(returned);
    assert_events(&[ALLOC_BASE + 42, FREE_BASE + 42, ALLOC_BASE + 43,
                    DROP_BASE + 310, DROP_BASE + 300, FREE_BASE + 43], 0);

    for count in [0, 1, 3, 10] {
        reset_events();
        let returned = loop_replacement(source.as_str(), count);
        assert_eq!(returned, vec![source.as_str()]);
        let mut expected = Vec::new();
        for index in 0..count {
            expected.push(ALLOC_BASE + 48);
            if index != 0 { expected.push(FREE_BASE + 49); }
            expected.extend([ALLOC_BASE + 49, FREE_BASE + 48, DROP_BASE + 370]);
        }
        expected.extend([DROP_BASE + 380, DROP_BASE + 360]);
        assert_events(&expected, usize::from(count != 0));
        drop(returned);
        if count != 0 { expected.push(FREE_BASE + 49); }
        assert_events(&expected, 0);
    }

    // A never-polled Future has not initialized any source-local resource.
    reset_events();
    drop(async_replacement(source.as_str(), false, false));
    assert_events(&[], 0);

    // Cancel before restoration: the local borrow and its owner must stay
    // together in the suspended Future, then be destroyed in source order.
    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_before_replacement(source.as_str()));
    assert!(poll_once(future.as_mut()).is_pending());
    assert_events(&[ALLOC_BASE + 46], 1);
    drop(future);
    assert_events(&[ALLOC_BASE + 46, DROP_BASE + 350, FREE_BASE + 46, DROP_BASE + 340], 0);

    // Cancel after restoration. No executor sleeps or timing races are used.
    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_replacement(source.as_str(), false, false));
    assert!(poll_once(future.as_mut()).is_pending());
    let initialized = [ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44];
    assert_events(&initialized, 1);
    drop(future);
    assert_events(&[ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    DROP_BASE + 330, FREE_BASE + 45, DROP_BASE + 320], 0);

    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_replacement(source.as_str(), false, false));
    assert!(poll_once(future.as_mut()).is_pending());
    PAUSE_READY.store(true, Ordering::Relaxed);
    let returned = match poll_once(future.as_mut()) {
        ::std::task::Poll::Ready(Ok(value)) => value,
        _ => panic!("resumed Future did not return its restored container"),
    };
    assert_eq!(returned, vec![source.as_str()]);
    let expected = [ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    DROP_BASE + 330, DROP_BASE + 320];
    assert_events(&expected, 1);
    drop(future);
    assert_events(&expected, 1);
    drop(returned);
    assert_events(&[ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    DROP_BASE + 330, DROP_BASE + 320, FREE_BASE + 45], 0);

    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_replacement(source.as_str(), false, true));
    assert!(poll_once(future.as_mut()).is_pending());
    PAUSE_READY.store(true, Ordering::Relaxed);
    assert!(matches!(poll_once(future.as_mut()), ::std::task::Poll::Ready(Err(7))));
    let expected = [ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    CHECKPOINT, DROP_BASE + 330, FREE_BASE + 45, DROP_BASE + 320];
    assert_events(&expected, 0);
    drop(future);
    assert_events(&expected, 0);

    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_replacement(source.as_str(), true, false));
    assert!(poll_once(future.as_mut()).is_pending());
    PAUSE_READY.store(true, Ordering::Relaxed);
    let caught = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        poll_once(future.as_mut())
    }));
    assert!(caught.is_err());
    record(CAUGHT);
    let expected = [ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    CRASH, DROP_BASE + 330, FREE_BASE + 45, DROP_BASE + 320, CAUGHT];
    assert_events(&expected, 0);
    drop(future);
    assert_events(&expected, 0);
}
"#;
