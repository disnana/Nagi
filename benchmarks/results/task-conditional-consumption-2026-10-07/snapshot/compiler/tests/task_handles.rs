#[path = "support/native_triple.rs"]
mod native_triple;
#[path = "support/task_conditional_consumption.rs"]
mod task_conditional_consumption;
#[path = "support/task_service.rs"]
mod task_service;
#[path = "support/task_service_s2.rs"]
mod task_service_s2;
use nagic::{check, emit, source};
use native_triple::Fixture;

#[test]
fn positive_contracts_build_and_run_high_saved_and_handwritten_low() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/task-handles");
    let cases: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(root.join("contracts.json")).unwrap()).unwrap();
    for case in cases
        .iter()
        .filter(|case| case["high"] == true && case["expected"] == "check-pass")
    {
        let name = case["source"].as_str().unwrap();
        let high = std::fs::read_to_string(root.join(name)).unwrap();
        let low = std::fs::read_to_string(root.join(name.replace(".nagi", ".low"))).unwrap();
        let assertion = if high.contains("async def exercise(") {
            "#[test] fn run_contract(){ let rt=tokio::runtime::Runtime::new().unwrap(); rt.block_on(async {exercise(false).await.unwrap();exercise(true).await.unwrap();}); }"
        } else {
            "#[test] fn run_contract(){ main(); }"
        };
        Fixture::new().run_three(name, &high, &low, "", assertion);
    }
}

#[test]
fn canonical_task_metadata_survives_saved_low_and_user_task_names() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", "class Task:\n    count: i64\nasync def work() -> i64:\n    return 3\nasync def main() -> Result[unit, Error]:\n    local = Task(count=1)\n    async with scope:\n        task = spawn work()\n        received = await task\n    return ok(print(local.count))\n");
    let high = fixture.checked("main.nagi").unwrap();
    assert!(high
        .modules
        .definitions
        .iter()
        .any(|d| d.id == nagic::stdlib::resource_id(nagic::stdlib::Resource::Task)));
    fixture.write("saved.low", &emit::low(&high));
    let saved = fixture.checked("saved.low").unwrap();
    assert!(saved
        .modules
        .definitions
        .iter()
        .any(|d| d.id == nagic::stdlib::resource_id(nagic::stdlib::Resource::Task)));
    let mut raw = nagic::parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut raw).unwrap();
    nagic::modules::validate(&raw).unwrap();
    // A textual bare Task remains the user's record; only SpawnBind creates
    // the canonical scope-local handle type without an import binding.
    assert!(high
        .classes
        .iter()
        .any(|c| high
            .modules
            .definition(&c.name)
            .is_some_and(|d| d.id.name == "Task" && d.id.kind == nagic::ast::DefKind::Class)));
    let _ = source::load(&fixture.0.join("saved.low"), false).unwrap();
}

#[test]
fn raw_check_registers_contextual_task_and_preserves_obligation() {
    for high in [true, false] {
        let (positive, negative, line) = if high {
            ("async def work() -> i64:\n    return 7\nasync def main() -> Result[unit, Error]:\n    async with scope:\n        task = spawn work()\n        received = await task\n    return ok(print(0))\n", "async def work() -> i64:\n    return 7\nasync def main() -> Result[unit, Error]:\n    async with scope:\n        task = spawn work()\n    return ok(print(0))\n", 5)
        } else {
            ("async fn work() -> i64 { return 7; }\nasync fn main() -> Result[unit,Error] { scope {\nlet task = spawn work();\nlet received = await task; } return ok(print(0)); }\n", "async fn work() -> i64 { return 7; }\nasync fn main() -> Result[unit,Error] { scope {\nlet task = spawn work();\n} return ok(print(0)); }\n", 3)
        };
        let mut bad = nagic::parser::parse(negative, high).unwrap();
        let error = check::check(&mut bad).unwrap_err();
        assert!(error.starts_with(&format!("line {line}:")), "{error}");
        assert!(error.contains("未受取Task"), "{error}");
        let mut good = nagic::parser::parse(positive, high).unwrap();
        check::check(&mut good).unwrap();
        let saved = nagic::parser::parse(&emit::low(&good), false).unwrap();
        let sealed = check::finalize(
            saved,
            Default::default(),
            nagic::source::SourceProvenance::user_low_unmapped(),
        )
        .unwrap();
        assert!(emit::rust(&sealed).unwrap().contains("TaskScope"));
    }
}

#[test]
fn native_business_fault_cleanup_and_branch_contracts() {
    Fixture::new().run_three("task-lifecycle", HIGH, LOW, ADAPTER, ASSERTIONS);
}

#[test]
fn bounded_generated_task_paths_reach_native() {
    // Fixed seed, bounded combinations of receive/discard, move regeneration,
    // branch joins, loop backedges, and a nested legacy-only scope. This oracle
    // asserts language behavior rather than generated Rust spelling.
    let seed = 0x1234_5678_u64;
    let mut state = seed;
    let mut high = String::from("from std.task import discard\nfrom std.ownership import move\nasync def work() -> i64:\n    return 7\n");
    let mut low = String::from("from std.task import discard;\nfrom std.ownership import move;\nasync fn work() -> i64 { return 7; }\n");
    let mut assertions = String::from("#[test] fn generated_paths(){ let rt=tokio::runtime::Runtime::new().unwrap();rt.block_on(async {\n");
    for index in 0..16 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let receive = "received = await task";
        let discard = "discard(task)";
        let (yes, no) = if state & 1 == 0 {
            (receive, discard)
        } else {
            (discard, receive)
        };
        let iterations = 1 + state % 4;
        high.push_str(&format!("async def generated_{index}(flag: bool) -> Result[unit, Error]:\n    async with scope:\n        task = spawn work()\n        if flag:\n            {yes}\n        else:\n            {no}\n        task = spawn work()\n        alias = move(task)\n        received = await alias\n        for number in range({iterations}):\n            task = spawn work()\n            if flag:\n                {no}\n            else:\n                {yes}\n        async with scope:\n            spawn sleep(0)\n        task = spawn work()\n        received = await task\n    return ok(print(0))\n"));
        let low_arm = |arm: &str| {
            if arm == receive {
                "let received = await task;"
            } else {
                "discard(task);"
            }
        };
        low.push_str(&format!("async fn generated_{index}(flag: bool) -> Result[unit,Error] {{ scope {{\nlet task = spawn work(); if flag {{ {} }} else {{ {} }}\ntask = spawn work(); let alias = move(task); let received = await alias;\nfor number in range({iterations}) {{ task = spawn work(); if flag {{ {} }} else {{ {} }} }}\nscope {{ spawn sleep(0); }} task = spawn work(); received = await task;\n}} return ok(print(0)); }}\n",low_arm(yes),low_arm(no),low_arm(no).replace("let ",""),low_arm(yes).replace("let ","")));
        assertions.push_str(&format!(
            "generated_{index}(false).await.unwrap(); generated_{index}(true).await.unwrap();\n"
        ));
    }
    assertions.push_str("});}\n");
    Fixture::new().run_three("task-generated-305419896", &high, &low, "", &assertions);
}

const HIGH: &str = r#"from std.task import discard, kind, message, TaskFailureKind
from std.ownership import move
class Marker:
    text: str
@rust("native::held")
extern async def held() -> unit
@rust("native::wait_started")
extern async def wait_started() -> unit
@rust("native::business")
extern async def business() -> Result[i64, i64]
@rust("native::crash")
extern async def crash() -> i64
@rust("native::legacy")
extern async def legacy() -> Result[unit, Error]
@rust("native::body_error")
extern def body_error() -> Result[i64, Error]
@rust("native::release")
extern def release() -> unit
@rust("native::continued")
extern def continued() -> unit
@rust("native::message_seen")
extern def message_seen(value: view[str]) -> unit
async def value() -> i64:
    return 17
async def normal() -> Result[unit, Error]:
    async with scope:
        child = spawn business()
        sibling = spawn held()
        received = await child
        match received:
            case Ok(inner):
                match inner:
                    case Ok(value):
                        assert_true(False)
                    case Err(value):
                        assert_true(value == 22)
            case Err(failure):
                assert_true(False)
        release()
        done = await sibling
    return ok(print(0))
async def fault() -> Result[unit, Error]:
    async with scope:
        child = spawn crash()
        sibling = spawn held()
        received = await child
        match received:
            case Ok(value):
                assert_true(False)
            case Err(failure):
                assert_true(kind(failure) == TaskFailureKind.Panicked)
                message_seen(message(failure))
        discard(sibling)
        continued()
    return ok(print(0))
async def mixed() -> Result[unit, Error]:
    async with scope:
        sibling = spawn held()
        spawn legacy()
        received = await sibling
        match received:
            case Ok(value):
                assert_true(False)
            case Err(failure):
                assert_true(kind(failure) == TaskFailureKind.LegacyError)
    return ok(print(0))
async def body() -> Result[unit, Error]:
    async with scope:
        sibling = spawn held()
        discard(sibling)
        await wait_started()
        marker = Marker(text="body")
        value = try body_error()
    return ok(print(0))
async def branch(flag: bool) -> Result[unit, Error]:
    async with scope:
        task = spawn value()
        if flag:
            received = await task
            task = spawn value()
        else:
            received = await task
            task = spawn value()
        alias = move(task)
        received = await alias
        for i in range(2):
            inner = spawn value()
            discard(inner)
        async with scope:
            nested = spawn value()
            received = await nested
    return ok(print(0))
async def pending() -> Result[unit, Error]:
    async with scope:
        child = spawn held()
        received = await child
    return ok(print(0))
"#;

const LOW: &str = r#"from std.task import discard, kind, message, TaskFailureKind;
from std.ownership import move;
record Marker { text: str; }
@rust("native::held")
extern async fn held() -> unit;
@rust("native::wait_started")
extern async fn wait_started() -> unit;
@rust("native::business")
extern async fn business() -> Result[i64,i64];
@rust("native::crash")
extern async fn crash() -> i64;
@rust("native::legacy")
extern async fn legacy() -> Result[unit,Error];
@rust("native::body_error")
extern fn body_error() -> Result[i64,Error];
@rust("native::release")
extern fn release() -> unit;
@rust("native::continued")
extern fn continued() -> unit;
@rust("native::message_seen")
extern fn message_seen(value: view[str]) -> unit;
async fn value() -> i64 { return 17; }
async fn normal() -> Result[unit,Error] {
    scope {
        let child = spawn business(); let sibling = spawn held(); let received = await child;
        match received {
            case Ok(inner) { match inner { case Ok(value) { assert_true(False); } case Err(value) { assert_true(value == 22); } } }
            case Err(failure) { assert_true(False); }
        }
        release(); let done = await sibling;
    }
    return ok(print(0));
}
async fn fault() -> Result[unit,Error] {
    scope {
        let child = spawn crash(); let sibling = spawn held(); let received = await child;
        match received {
            case Ok(value) { assert_true(False); }
            case Err(failure) { assert_true(kind(failure) == TaskFailureKind.Panicked); message_seen(message(failure)); }
        }
        discard(sibling); continued();
    }
    return ok(print(0));
}
async fn mixed() -> Result[unit,Error] {
    scope {
        let sibling = spawn held(); spawn legacy(); let received = await sibling;
        match received { case Ok(value) { assert_true(False); } case Err(failure) { assert_true(kind(failure) == TaskFailureKind.LegacyError); } }
    }
    return ok(print(0));
}
async fn body() -> Result[unit,Error] {
    scope {
        let sibling = spawn held(); discard(sibling); await wait_started();
        let marker = Marker(text="body"); let value = try body_error();
    }
    return ok(print(0));
}
async fn branch(flag: bool) -> Result[unit,Error] {
    scope {
        let task = spawn value();
        if flag { let received = await task; task = spawn value(); } else { let received = await task; task = spawn value(); }
        let alias = move(task); let received = await alias;
        for i in range(2) { let inner = spawn value(); discard(inner); }
        scope { let nested = spawn value(); received = await nested; }
    }
    return ok(print(0));
}
async fn pending() -> Result[unit,Error] { scope { let child = spawn held(); let received = await child; } return ok(print(0)); }
"#;

const ADAPTER: &str = r#"
mod native {
    use tokio::sync::Notify;
    use std::sync::{Mutex,atomic::{AtomicBool,Ordering}};
    pub static START: Notify=Notify::const_new();
    pub static STOP: Notify=Notify::const_new();
    pub static DROPPED: Notify=Notify::const_new();
    pub static STARTED: AtomicBool=AtomicBool::new(false);
    pub static CONTINUED: AtomicBool=AtomicBool::new(false);
    pub static MESSAGE: AtomicBool=AtomicBool::new(false);
    pub static EVENTS: Mutex<Vec<i64>>=Mutex::new(Vec::new());
    pub fn reset(){ STARTED.store(false,Ordering::SeqCst); CONTINUED.store(false,Ordering::SeqCst); MESSAGE.store(false,Ordering::SeqCst); EVENTS.lock().unwrap().clear(); }
    struct Child;
    impl Drop for Child { fn drop(&mut self){ EVENTS.lock().unwrap().push(20); DROPPED.notify_one(); } }
    pub async fn held(){ let _guard=Child; STARTED.store(true,Ordering::SeqCst); START.notify_one(); STOP.notified().await; EVENTS.lock().unwrap().push(30); }
    pub async fn wait_started(){ while !STARTED.load(Ordering::SeqCst){ START.notified().await; } }
    pub async fn business()->Result<i64,i64>{ wait_started().await; Err(22) }
    pub async fn crash()->i64{ wait_started().await; panic!("task panic sentinel"); }
    pub async fn legacy()->Result<(),nagi_runtime::Error>{ wait_started().await; Err(nagi_runtime::Error::invalid("legacy sentinel")) }
    pub fn body_error()->Result<i64,nagi_runtime::Error>{ Err(nagi_runtime::Error::invalid("body sentinel")) }
    pub fn release(){ assert!(STARTED.load(Ordering::SeqCst)); assert!(!EVENTS.lock().unwrap().contains(&20)); STOP.notify_one(); }
    pub fn continued(){ assert!(EVENTS.lock().unwrap().contains(&20)); CONTINUED.store(true,Ordering::SeqCst); }
    pub fn message_seen(value:&str){ assert!(value == "child panicked"); MESSAGE.store(true,Ordering::SeqCst); }
}
impl Drop for Marker { fn drop(&mut self){ native::EVENTS.lock().unwrap().push(10); } }
"#;

const ASSERTIONS: &str = r#"
#[test]
fn actual_task_contracts(){
    use std::future::Future;
    use std::sync::atomic::Ordering;
    let rt=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        native::reset(); normal().await.unwrap(); assert_eq!(*native::EVENTS.lock().unwrap(),vec![30,20]);
        native::reset(); let failure=fault().await.unwrap_err(); assert!(failure.message == "child panicked"); assert!(native::CONTINUED.load(Ordering::SeqCst)); assert!(native::MESSAGE.load(Ordering::SeqCst)); assert_eq!(*native::EVENTS.lock().unwrap(),vec![20]);
        native::reset(); let failure=mixed().await.unwrap_err(); assert!(matches!(failure.kind,nagi_runtime::ErrorKind::Invalid)); assert_eq!(failure.message,"legacy sentinel"); assert_eq!(*native::EVENTS.lock().unwrap(),vec![20]);
        native::reset(); let failure=body().await.unwrap_err(); assert!(matches!(failure.kind,nagi_runtime::ErrorKind::Invalid)); assert_eq!(failure.message,"body sentinel"); assert_eq!(*native::EVENTS.lock().unwrap(),vec![10,20]);
        branch(false).await.unwrap(); branch(true).await.unwrap();
    });
    // Destroy the owning parent, then observe the child's Drop separately.
    rt.block_on(async {
        native::reset(); let mut future=Box::pin(pending());
        std::future::poll_fn(|cx|{ assert!(future.as_mut().poll(cx).is_pending()); std::task::Poll::Ready(()) }).await;
        native::wait_started().await; native::EVENTS.lock().unwrap().clear(); drop(future);
        tokio::time::timeout(std::time::Duration::from_secs(5),async {
            while !native::EVENTS.lock().unwrap().contains(&20) { native::DROPPED.notified().await; }
        }).await.unwrap();
        assert_eq!(*native::EVENTS.lock().unwrap(),vec![20]);
    });
}
"#;
