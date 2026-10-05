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
            emit::rust(program).unwrap(),
            RUNTIME,
            serde_json::to_string(&adapter.to_string_lossy()).unwrap(),
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
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
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

impl Drop for super::Marker {
    fn drop(&mut self) {
        super::record(super::DROP_BASE + self.id as usize);
    }
}
"#;

const ASSERTIONS: &str = r#"
#[test]
fn replacement_observes_rhs_then_old_free_and_scope_drop_order() {
    let source = String::from("caller-owned");

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
}
"#;
