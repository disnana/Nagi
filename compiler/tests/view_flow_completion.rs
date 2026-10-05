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

#[test]
fn scopes_keep_lexical_storage_and_route_nested_errors() {
    let high = checked(
        r#"async def failure() -> Result[unit, Error]:
    return error("failure")
async def restore(part: view[str]) -> Result[List[view[str]], Error]:
    local = "local"
    parts = [part]
    async with scope:
        parts = [view(local)]
        async with scope:
            parts = [part]
            try await failure()
        parts = [part]
    return ok(parts)
async def ordinary() -> Result[unit, Error]:
    async with scope:
        try await failure()
    return ok(print("done"))
"#,
        true,
    );
    let saved = checked(&emit::low(&high), false);
    for program in [high, saved] {
        let rust = emit::rust(&checked_emission::seal(&program)).unwrap();
        let restore = rust
            .split("pub async fn restore<")
            .nth(1)
            .unwrap()
            .split("\n}")
            .next()
            .unwrap();
        assert!(restore.contains("'__nagi_scope_body_1: {"));
        assert!(restore.contains("'__nagi_scope_body_2: {"));
        assert!(restore.contains("break '__nagi_scope_body_2"));
        assert!(restore.contains("__scope.cancel().await; break '__nagi_scope_body_1"));
        assert!(restore.contains("match (__scope.join().await)"));
        assert!(restore.contains("::std::convert::From::from(__nagi_try_error)"));
        assert!(!restore.contains("= async {"));
        let ordinary = rust
            .split("pub async fn ordinary(")
            .nth(1)
            .unwrap()
            .split("\n}")
            .next()
            .unwrap();
        // Scope has the same lexical/error model without optional storage.
        // The former nested async boundary rejected body-local views assigned
        // temporarily to an outer container even when the result was scalar.
        assert!(ordinary.contains("'__nagi_scope_body_1: {"));
        assert!(ordinary.contains("(__scope.join().await)?"));
        assert!(ordinary.contains("__scope.cancel().await; return ::std::result::Result::Err"));
        assert!(!ordinary.contains("= async {"));
        assert!(!ordinary.contains("__nagi_view_flow_"));
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn checked(source: &str, high: bool) -> nagic::ast::Program {
    let mut program =
        parser::parse(source, high).unwrap_or_else(|error| panic!("{error}\n{source}"));
    check::check(&mut program).unwrap_or_else(|error| panic!("{error}\n{source}"));
    program
}
#[test]
fn owned_storage_loops_and_async_compile_and_run_in_all_source_forms() {
    let high = checked(HIGH, true);
    let saved = checked(&emit::low(&high), false);
    let hand = checked(LOW, false);
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-view-completion-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    for (name, program) in [("high", high), ("saved", saved), ("hand", hand)] {
        let rust = emit::rust(&checked_emission::seal(&program)).unwrap();
        assert!(!rust.contains("unsafe"));
        assert!(!rust.contains(".clone()"));
        for function in [
            "result_binding",
            "result_variants",
            "result_loop",
            "option_loop",
            "for_elements",
            "two_phase_append",
            "option_binding",
            "nested_wrapper",
            "for_restore",
            "while_restore",
            "nested_loops",
            "for_read",
            "loop_append",
            "loop_branch",
            "loop_terminal",
            "cross_move",
            "condition_mutation",
            "condition_move",
            "pattern_loop",
            "async_no_await",
            "async_await",
            "async_loop",
        ] {
            let marker = if function.starts_with("async_") {
                format!("pub async fn {function}<")
            } else {
                format!("pub fn {function}<")
            };
            let body = rust
                .split(&marker)
                .nth(1)
                .unwrap()
                .split("\n}")
                .next()
                .unwrap();
            assert!(
                body.contains("__nagi_view_flow_"),
                "{name}: {function} did not use owning-view lowering"
            );
        }
        let source = fixture.0.join(format!("{name}.rs"));
        let binary = fixture
            .0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::write(&source, rust + ASSERTIONS).unwrap();
        let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = Command::new(&binary)
            .env("NAGI_VIEW_COMPLETION_ENV_PRESENT", "present")
            .env_remove("NAGI_VIEW_COMPLETION_ENV_MISSING")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
#[test]
fn completion_keeps_local_and_iterator_owner_escape_rejections() {
    for (source,high) in [
        ("def bad() -> Result[List[view[str]], i64]:\n    local = \"inner\"\n    result: Result[List[view[str]], i64] = ok([view(local)])\n    return result\n",true),
        ("async def bad() -> List[view[str]]:\n    local = \"inner\"\n    parts = [view(local)]\n    return parts\n",true),
        ("def bad(part: view[str]) -> List[view[str]]:\n    parts = [part]\n    for i in range(2):\n        local = \"inner\"\n        parts = [view(local)]\n    return parts\n",true),
        ("fn bad(part: view[str]) -> List[view[str]] { let parts: List[view[str]] = [part]; for i in range(2) { let local: str = \"inner\"; parts = [view(local)]; } return parts; }",false),
        ("def bad(part: view[str]) -> List[view[str]]:\n    parts = [part]\n    for item in parts:\n        parts = [part]\n    return parts\n",true),
    ] { let mut program = parser::parse(source,high).unwrap(); assert!(check::check(&mut program).is_err(),"{source}"); }
}
const HIGH: &str = r#"def consume(parts: List[view[str]]) -> bool:
    return len(parts) > 0

def mark(unit: unit) -> bool:
    return False

def env_failure() -> Result[str, i64]:
    return fail(7)

def env_try(key: str) -> Result[str, i64]:
    value = env(key, try env_failure())
    return ok(value)

async def env_async_failure() -> Result[str, i64]:
    return fail(9)

async def env_await(key: str) -> Result[str, i64]:
    value = env(key, try await env_async_failure())
    return ok(value)

def result_binding(part: view[str]) -> Result[List[view[str]], i64]:
    local = "inner"
    result: Result[List[view[str]], i64] = ok([view(local)])
    result = ok([part])
    return result

def result_variants(part: view[str], flag: bool) -> Result[List[view[str]], i64]:
    local = "inner"
    value: Result[List[view[str]], i64] = ok([view(local)])
    if flag:
        value = fail(7)
    else:
        value = ok([part])
    return value

def result_loop(part: view[str], count: i64) -> Result[List[view[str]], i64]:
    value: Result[List[view[str]], i64] = ok([part])
    for i in range(count):
        local = "inner"
        value = ok([view(local)])
        value = fail(7)
        value = ok([part])
    return value

def option_loop(part: view[str], count: i64) -> Option[List[view[str]]]:
    value: Option[List[view[str]]] = some([part])
    for i in range(count):
        local = "inner"
        value = some([view(local)])
        value = None
        value = some([part])
    return value

def two_phase_append(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    append(parts, parts[0])
    return parts

def for_elements(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    for item in parts:
        length = len(item)
    return parts

def option_binding(part: view[str], flag: bool) -> Option[List[view[str]]]:
    local = "inner"
    value: Option[List[view[str]]] = some([view(local)])
    if flag:
        value = some([part])
    else:
        value = None
    return value

def nested_wrapper(part: view[str]) -> Result[Option[List[view[str]]], i64]:
    local = "inner"
    value: Result[Option[List[view[str]]], i64] = ok(some([view(local)]))
    value = ok(some([part]))
    return value

def for_restore(part: view[str], count: i64) -> List[view[str]]:
    parts = [part]
    for i in range(count):
        local = "inner"
        parts = [view(local)]
        parts = [part]
    return parts

def while_restore(part: view[str], count: i64) -> List[view[str]]:
    parts = [part]
    i = 0
    while i < count:
        local = "inner"
        parts = [view(local)]
        parts = [part]
        i += 1
    return parts

def nested_loops(part: view[str], count: i64) -> List[view[str]]:
    parts = [part]
    for i in range(count):
        j = 0
        while j < count:
            local = "inner"
            parts = [view(local)]
            parts = [part]
            j += 1
    return parts

def for_read(part: view[str], count: i64) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    for i in range(count):
        length = len(parts)
        selected = parts[0]
        borrowed = view(parts)
    return parts

def loop_append(part: view[str], count: i64) -> List[view[str]]:
    parts = [part]
    for i in range(count):
        local = "inner"
        append(parts, view(local))
        parts = [part]
    return parts

def loop_branch(part: view[str], count: i64, flag: bool) -> List[view[str]]:
    parts = [part]
    for i in range(count):
        local = "inner"
        if flag:
            parts = [view(local)]
        else:
            append(parts, view(local))
        parts = [part]
    return parts

def loop_terminal(part: view[str], count: i64) -> List[view[str]]:
    parts = [part]
    for i in range(count):
        local = "inner"
        parts = [view(local)]
        parts = [part]
        return parts
    return parts

def cross_move(part: view[str], count: i64, flag: bool) -> List[view[str]]:
    first = [part]
    second = [part]
    for i in range(count):
        local = "inner"
        first = [view(local)]
        second = [view(local)]
        first = [part]
        second = [part]
        if flag:
            first = second
            second = [part]
        else:
            second = first
            first = [part]
    return first

def condition_mutation(part: view[str], count: i64) -> List[view[str]]:
    local = "inner"
    parts = [part]
    i = 0
    while i < count and not mark(append(parts, view(local))):
        parts = [part]
        i += 1
    parts = [part]
    return parts

def condition_move(part: view[str], count: i64, flag: bool) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    i = 0
    while i < count and flag and consume(parts):
        parts = [part]
        i += 1
    parts = [part]
    return parts

def pattern_loop(value: Option[List[view[str]]], part: view[str], count: i64) -> List[view[str]]:
    match value:
        case Some(parts):
            for i in range(count):
                local = "inner"
                parts = [view(local)]
                parts = [part]
            return parts
        case None:
            return [part]

async def pause() -> i64:
    return 1

async def async_no_await(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    return parts

async def async_await(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    number = await pause()
    return parts

async def async_loop(part: view[str], count: i64) -> List[view[str]]:
    parts = [part]
    for i in range(count):
        local = "inner"
        parts = [view(local)]
        number = await pause()
        parts = [part]
    return parts
"#;
const LOW: &str = r#"fn consume(parts: List[view[str]]) -> bool { return len(parts) > 0; }
fn mark(unit: unit) -> bool { return false; }
fn env_failure() -> Result[str, i64] { return fail(7); }
fn env_try(key: str) -> Result[str, i64] { let value: str = env(key, try env_failure()); return ok(value); }
async fn env_async_failure() -> Result[str, i64] { return fail(9); }
async fn env_await(key: str) -> Result[str, i64] { let value: str = env(key, try await env_async_failure()); return ok(value); }
fn result_binding(part: view[str]) -> Result[List[view[str]], i64] { let local: str = "inner"; let result: Result[List[view[str]], i64] = ok([view(local)]); result = ok([part]); return result; }
fn result_variants(part: view[str], flag: bool) -> Result[List[view[str]], i64] { let local: str = "inner"; let value: Result[List[view[str]], i64] = ok([view(local)]); if flag { value = fail(7); } else { value = ok([part]); } return value; }
fn result_loop(part: view[str], count: i64) -> Result[List[view[str]], i64] { let value: Result[List[view[str]], i64] = ok([part]); for i in range(count) { let local: str = "inner"; value = ok([view(local)]); value = fail(7); value = ok([part]); } return value; }
fn option_loop(part: view[str], count: i64) -> Option[List[view[str]]] { let value: Option[List[view[str]]] = some([part]); for i in range(count) { let local: str = "inner"; value = some([view(local)]); value = None; value = some([part]); } return value; }
fn two_phase_append(part: view[str]) -> List[view[str]] { let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; append(parts, parts[0]); return parts; }
fn for_elements(part: view[str]) -> List[view[str]] { let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; for item in parts { let length: i64 = len(item); } return parts; }
fn option_binding(part: view[str], flag: bool) -> Option[List[view[str]]] { let local: str = "inner"; let value: Option[List[view[str]]] = some([view(local)]); if flag { value = some([part]); } else { value = None; } return value; }
fn nested_wrapper(part: view[str]) -> Result[Option[List[view[str]]], i64] { let local: str = "inner"; let value: Result[Option[List[view[str]]], i64] = ok(some([view(local)])); value = ok(some([part])); return value; }
fn for_restore(part: view[str], count: i64) -> List[view[str]] { let parts: List[view[str]] = [part]; for i in range(count) { let local: str = "inner"; parts = [view(local)]; parts = [part]; } return parts; }
fn while_restore(part: view[str], count: i64) -> List[view[str]] { let parts: List[view[str]] = [part]; let i: i64 = 0; while i < count { let local: str = "inner"; parts = [view(local)]; parts = [part]; i += 1; } return parts; }
fn nested_loops(part: view[str], count: i64) -> List[view[str]] { let parts: List[view[str]] = [part]; for i in range(count) { let j: i64 = 0; while j < count { let local: str = "inner"; parts = [view(local)]; parts = [part]; j += 1; } } return parts; }
fn for_read(part: view[str], count: i64) -> List[view[str]] { let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; for i in range(count) { let length: i64 = len(parts); let selected: view[str] = parts[0]; let borrowed: view[view[str]] = view(parts); } return parts; }
fn loop_append(part: view[str], count: i64) -> List[view[str]] { let parts: List[view[str]] = [part]; for i in range(count) { let local: str = "inner"; append(parts, view(local)); parts = [part]; } return parts; }
fn loop_branch(part: view[str], count: i64, flag: bool) -> List[view[str]] { let parts: List[view[str]] = [part]; for i in range(count) { let local: str = "inner"; if flag { parts = [view(local)]; } else { append(parts, view(local)); } parts = [part]; } return parts; }
fn loop_terminal(part: view[str], count: i64) -> List[view[str]] { let parts: List[view[str]] = [part]; for i in range(count) { let local: str = "inner"; parts = [view(local)]; parts = [part]; return parts; } return parts; }
fn cross_move(part: view[str], count: i64, flag: bool) -> List[view[str]] { let first: List[view[str]] = [part]; let second: List[view[str]] = [part]; for i in range(count) { let local: str = "inner"; first = [view(local)]; second = [view(local)]; first = [part]; second = [part]; if flag { first = second; second = [part]; } else { second = first; first = [part]; } } return first; }
fn condition_mutation(part: view[str], count: i64) -> List[view[str]] { let local: str = "inner"; let parts: List[view[str]] = [part]; let i: i64 = 0; while i < count and not mark(append(parts, view(local))) { parts = [part]; i += 1; } parts = [part]; return parts; }
fn condition_move(part: view[str], count: i64, flag: bool) -> List[view[str]] { let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; let i: i64 = 0; while i < count and flag and consume(parts) { parts = [part]; i += 1; } parts = [part]; return parts; }
fn pattern_loop(value: Option[List[view[str]]], part: view[str], count: i64) -> List[view[str]] { match value { case Some(parts) { for i in range(count) { let local: str = "inner"; parts = [view(local)]; parts = [part]; } return parts; } case None { return [part]; } } }
async fn pause() -> i64 { return 1; }
async fn async_no_await(part: view[str]) -> List[view[str]] { let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; return parts; }
async fn async_await(part: view[str]) -> List[view[str]] { let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; let number: i64 = await pause(); return parts; }
async fn async_loop(part: view[str], count: i64) -> List[view[str]] { let parts: List[view[str]] = [part]; for i in range(count) { let local: str = "inner"; parts = [view(local)]; let number: i64 = await pause(); parts = [part]; } return parts; }
"#;
const ASSERTIONS: &str = r#"
fn ready<F: std::future::Future>(future: F) -> F::Output {
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut cx) { std::task::Poll::Ready(value) => value, std::task::Poll::Pending => panic!("unexpected suspension") }
}
#[test] fn environment_fallback_uses_the_source_error_and_async_context() {
    assert_eq!(env_try(String::from("NAGI_VIEW_COMPLETION_ENV_PRESENT")), Ok(String::from("present")));
    assert_eq!(env_try(String::from("NAGI_VIEW_COMPLETION_ENV_MISSING")), Err(7));
    assert_eq!(ready(env_await(String::from("NAGI_VIEW_COMPLETION_ENV_PRESENT"))), Ok(String::from("present")));
    assert_eq!(ready(env_await(String::from("NAGI_VIEW_COMPLETION_ENV_MISSING"))), Err(9));
}
#[test] fn storage() {
    let owner=String::from("caller");let part=owner.as_str();
    assert_eq!(result_binding(part),Ok(vec![part]));
    assert_eq!(nested_wrapper(part),Ok(Some(vec![part])));
    assert_eq!(for_elements(part),vec![part]);
    assert_eq!(two_phase_append(part),vec![part,part]);
    for flag in [false,true] { assert_eq!(result_variants(part,flag),if flag {Err(7)}else{Ok(vec![part])}); }
    for flag in [false,true] { assert_eq!(option_binding(part,flag),if flag {Some(vec![part])}else{None}); }
}
#[test] fn loops() {
    let owner=String::from("caller");let part=owner.as_str();
    for count in [0,1,3,10] {
        assert_eq!(for_restore(part,count),vec![part]);
        assert_eq!(result_loop(part,count),Ok(vec![part]));
        assert_eq!(option_loop(part,count),Some(vec![part]));
        assert_eq!(while_restore(part,count),vec![part]);
        assert_eq!(nested_loops(part,count),vec![part]);
        assert_eq!(for_read(part,count),vec![part]);
        assert_eq!(loop_append(part,count),vec![part]);
        assert_eq!(loop_terminal(part,count),vec![part]);
        assert_eq!(condition_mutation(part,count),vec![part]);
        assert_eq!(pattern_loop(Some(vec![part]),part,count),vec![part]);
        assert_eq!(pattern_loop(None,part,count),vec![part]);
        for flag in [false,true] {
            assert_eq!(loop_branch(part,count,flag),vec![part]);
            assert_eq!(cross_move(part,count,flag),vec![part]);
            assert_eq!(condition_move(part,count,flag),vec![part]);
        }
    }
}
#[test] fn asynchronous() {
    let owner=String::from("caller");let part=owner.as_str();
    assert_eq!(ready(async_no_await(part)),vec![part]);
    assert_eq!(ready(async_await(part)),vec![part]);
    for count in [0,1,3,10] { assert_eq!(ready(async_loop(part,count)),vec![part]); }
}
"#;
