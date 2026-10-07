use super::native_triple::Fixture;

#[test]
fn always_evaluated_receipts_and_lazy_values_run_in_three_sources() {
    Fixture::new().run_three("task-conditional", HIGH, LOW, ADAPTER, ASSERTIONS);
}

const HIGH: &str = r#"from std.task import TaskFailure, discard as abandon
from std.ownership import move as transfer
@rust("native::mark")
extern def mark(code: i64) -> unit
@rust("native::probe")
extern def probe(value: bool) -> bool
@rust("native::fallback")
extern def fallback() -> str
def take(result: Result[i64, TaskFailure], value: bool) -> bool:
    mark(1)
    match result:
        case Ok(number):
            assert_true(number == 7)
            return value
        case Err(failure):
            assert_true(False)
            return False
def text(result: Result[str, TaskFailure]) -> str:
    mark(4)
    match result:
        case Ok(value):
            return value
        case Err(failure):
            assert_true(False)
            return "failed"
def truth(value: unit) -> bool:
    mark(6)
    return True
def key(value: unit) -> str:
    mark(7)
    return "NAGI_CONDITIONAL_PRESENT"
async def number() -> i64:
    return 7
async def string(value: str) -> str:
    return value
async def left(flag: bool) -> Result[unit, Error]:
    async with scope:
        task = spawn number()
        value = take(await task, flag) and probe(True)
        assert_true(value == flag)
    mark(3)
    async with scope:
        task = spawn number()
        value = take(await task, flag) or probe(False)
        assert_true(value == flag)
    mark(3)
    return ok(print("scope-ok"))
async def env_key(name: str) -> Result[str, Error]:
    value = ""
    async with scope:
        task = spawn string(name)
        value = env(text(await task), fallback())
    mark(3)
    return ok(value)
async def eager(name: str) -> Result[str, Error]:
    value = ""
    async with scope:
        task = spawn string("payload")
        received = await task
        mark(8)
        value = env(name, text(transfer(received)))
    mark(3)
    return ok(value)
async def left_discard() -> Result[unit, Error]:
    async with scope:
        task = spawn number()
        value = truth(abandon(task)) and probe(False)
        assert_true(not value)
    mark(3)
    async with scope:
        task = spawn number()
        value = env(key(abandon(task)), fallback())
        assert_true(view(value) == "present")
    mark(3)
    return ok(print("scope-ok"))
"#;

// Independent Low, rather than a checked/printed High program.
const LOW: &str = r#"from std.task import TaskFailure, discard as abandon;
from std.ownership import move as transfer;
@rust("native::mark")
extern fn mark(code: i64) -> unit;
@rust("native::probe")
extern fn probe(value: bool) -> bool;
@rust("native::fallback")
extern fn fallback() -> str;
fn take(result: Result[i64,TaskFailure], value: bool) -> bool {
    mark(1); match result {
        case Ok(number) { assert_true(number == 7); return value; }
        case Err(failure) { assert_true(False); return False; }
    }
}
fn text(result: Result[str,TaskFailure]) -> str {
    mark(4); match result {
        case Ok(value) { return value; }
        case Err(failure) { assert_true(False); return "failed"; }
    }
}
fn truth(value: unit) -> bool { mark(6); return True; }
fn key(value: unit) -> str { mark(7); return "NAGI_CONDITIONAL_PRESENT"; }
async fn number() -> i64 { return 7; }
async fn string(value: str) -> str { return value; }
async fn left(flag: bool) -> Result[unit,Error] {
    scope { let task = spawn number();
        let value = take(await task, flag) and probe(True); assert_true(value == flag);
    } mark(3);
    scope { let task = spawn number();
        let value = take(await task, flag) or probe(False); assert_true(value == flag);
    } mark(3); return ok(print("scope-ok"));
}
async fn env_key(name: str) -> Result[str,Error] {
    let value = "";
    scope { let task = spawn string(name); value = env(text(await task), fallback()); }
    mark(3); return ok(value);
}
async fn eager(name: str) -> Result[str,Error] {
    let value = "";
    scope { let task = spawn string("payload"); let received = await task; mark(8);
        value = env(name, text(transfer(received)));
    } mark(3); return ok(value);
}
async fn left_discard() -> Result[unit,Error] {
    scope { let task = spawn number();
        let value = truth(abandon(task)) and probe(False); assert_true(not value);
    } mark(3);
    scope { let task = spawn number();
        let value = env(key(abandon(task)), fallback()); assert_true(view(value) == "present");
    } mark(3); return ok(print("scope-ok"));
}
"#;

const ADAPTER: &str = r#"
mod native {
    static EVENTS: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());
    pub fn mark(code: i64) { EVENTS.lock().unwrap().push(code); }
    pub fn probe(value: bool) -> bool { mark(2); value }
    pub fn fallback() -> String { mark(5); "fallback".into() }
    pub fn expect(expected: &[i64]) {
        assert_eq!(&*EVENTS.lock().unwrap(), expected);
        EVENTS.lock().unwrap().clear();
    }
}
"#;
const ASSERTIONS: &str = r#"
#[test]
fn conditional_native_contract() {
    std::env::set_var("NAGI_CONDITIONAL_PRESENT", "present");
    std::env::remove_var("NAGI_CONDITIONAL_MISSING");
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        left(false).await.unwrap(); native::expect(&[1,3,1,2,3]);
        left(true).await.unwrap(); native::expect(&[1,2,3,1,3]);
        assert_eq!(env_key("NAGI_CONDITIONAL_PRESENT".into()).await.unwrap(), "present");
        native::expect(&[4,3]);
        assert_eq!(env_key("NAGI_CONDITIONAL_MISSING".into()).await.unwrap(), "fallback");
        native::expect(&[4,5,3]);
        assert_eq!(eager("NAGI_CONDITIONAL_PRESENT".into()).await.unwrap(), "present");
        native::expect(&[8,3]);
        assert_eq!(eager("NAGI_CONDITIONAL_MISSING".into()).await.unwrap(), "payload");
        native::expect(&[8,4,3]);
        left_discard().await.unwrap(); native::expect(&[6,2,3,7,3]);
    });
}
"#;
