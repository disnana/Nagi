#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, source};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn try_checked(text: &str, high: bool) -> Result<nagic::ast::Program, String> {
    let fixture = loop {
        let path = std::env::temp_dir().join(format!(
            "nagi-view-foundation-check-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => break Fixture(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("exclusive check fixture: {error}"),
        }
    };
    let path = fixture
        .0
        .join(if high { "input.nagi" } else { "input.low" });
    fs::write(&path, text).unwrap();
    let mut program = source::load(&path, high)?.program;
    check::check(&mut program)?;
    Ok(program)
}
fn checked(text: &str, high: bool) -> nagic::ast::Program {
    try_checked(text, high).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

#[test]
fn return_observers_and_match_edges_compile_and_run_in_all_source_forms() {
    let high = checked(HIGH, true);
    let saved = checked(&emit::low(&high), false);
    let hand = checked(LOW, false);
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "nagi-view-foundation-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    for (name, program) in [("high", high), ("saved", saved), ("hand", hand)] {
        let rust = emit::rust(&checked_emission::seal(&program)).unwrap();
        for function in [
            "alias_return",
            "result_return",
            "option_return",
            "nested_return",
            "indexed_return",
            "call_return",
            "three_edges",
            "terminal_edge",
            "pattern_restore",
            "scrutinee_move",
            "condition_move",
            "cross_move",
            "nested_binding",
            "list_views",
            "nested_list_views",
            "result_elements",
            "nested_append",
            "unrelated_loops",
            "mutation_input",
            "scalar_observation",
        ] {
            let symbol = &program.modules.resolve_root_path(function).unwrap().symbol;
            let body = rust
                .split(&format!("pub fn {symbol}<"))
                .nth(1)
                .unwrap()
                .split("\n}")
                .next()
                .unwrap();
            assert!(
                body.contains("__nagi_view_flow_"),
                "{name}: {function} did not use the flow plan"
            );
        }
        assert!(!rust.contains("unsafe"));
        assert!(!rust.contains(".clone()"));
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
        let result = Command::new(&binary).output().unwrap();
        assert!(
            result.status.success(),
            "{name}: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn observer_lowering_does_not_accept_local_owner_escapes() {
    let cases = [
        ("from std.ownership import move\ndef bad() -> Result[List[view[str]], i64]:\n    local = \"inner\"\n    parts = [view(local)]\n    alias = move(parts)\n    return ok(alias)\n", true),
        ("from std.ownership import move;\nfn bad() -> Result[List[view[str]], i64] { let local: str = \"inner\"; let parts: List[view[str]] = [view(local)]; let alias: List[view[str]] = move(parts); return ok(alias); }", false),
        ("def bad(value: Option[i64]) -> List[view[str]]:\n    match value:\n        case Some(number):\n            local = \"inner\"\n            return [view(local)]\n        case None:\n            return []\n", true),
        ("fn bad(value: Option[i64]) -> List[view[str]] { match value { case Some(number) { let local: str = \"inner\"; return [view(local)]; } case None { return []; } } }", false),
        ("def bad() -> List[List[view[str]]]:\n    local = \"inner\"\n    nested = [[view(local)]]\n    return nested\n", true),
        ("fn bad() -> List[view[i64]] { let local: List[i64] = [1]; let parts: List[view[i64]] = [view(local)]; return parts; }", false),
    ];
    for (source, high) in cases {
        let error = try_checked(source, high).expect_err(source);
        assert!(
            error.contains("所有値") || error.contains("escapes its lifetime"),
            "{error}"
        );
    }
}

const HIGH: &str = r#"from std.ownership import move
enum Three:
    First
    Second
    Third

def consume(parts: List[view[str]]) -> bool:
    return len(parts) > 0

def marker(ignored: bool) -> Option[i64]:
    return some(1)

def select(parts: List[view[str]]) -> List[view[str]]:
    return parts

def alias_return(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    first = move(parts)
    alias = move(first)
    return alias

def result_return(part: view[str]) -> Result[List[view[str]], i64]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    alias = move(parts)
    return ok(alias)

def option_return(part: view[str]) -> Option[List[view[str]]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    return some(parts)

def nested_return(part: view[str]) -> List[List[view[str]]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    return [parts]

def indexed_return(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    return [parts[0]]

def call_return(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    return select(parts)

def three_edges(value: Three, part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [part]
    match value:
        case Three.First:
            parts = [view(local)]
            parts = [part]
        case Three.Second:
            append(parts, view(local))
            parts = [part]
        case Three.Third:
            parts = [part]
    return parts

def terminal_edge(value: Option[i64], part: view[str]) -> List[view[str]]:
    parts = [part]
    match value:
        case Some(number):
            local = "inner"
            parts = [view(local)]
            parts = [part]
            return parts
        case None:
            parts = [part]
    return parts

def pattern_restore(value: Option[List[view[str]]], part: view[str]) -> List[view[str]]:
    match value:
        case Some(type):
            local = "inner"
            type = [view(local)]
            type = [part]
            alias = move(type)
            return alias
        case None:
            return [part]

def scrutinee_move(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    match some(parts):
        case Some(alias):
            return alias
        case None:
            return []

def condition_move(flag: bool, part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    match marker(flag and consume(parts)):
        case Some(number):
            parts = [part]
        case None:
            parts = [part]
    return parts

def cross_move(value: Three, choose: bool, part: view[str]) -> List[view[str]]:
    local = "inner"
    first = [view(local)]
    first = [part]
    second = [view(local)]
    second = [part]
    match value:
        case Three.First:
            first = move(second)
        case Three.Second:
            second = move(first)
        case Three.Third:
            second = [part]
    first = [part]
    second = [part]
    if choose:
        return first
    return second

def nested_binding(part: view[str]) -> List[List[view[str]]]:
    local = "inner"
    nested = [[view(local)]]
    nested = [[part]]
    return nested

def list_views(part: view[i64]) -> List[view[i64]]:
    local = [1]
    parts = [view(local)]
    parts = [part]
    return parts

def nested_list_views(part: view[List[i64]]) -> List[view[List[i64]]]:
    local = [[1]]
    parts = [view(local)]
    parts = [part]
    return parts

def result_elements(part: view[str]) -> List[Result[view[str], str]]:
    local = "inner"
    parts: List[Result[view[str], str]] = [ok(view(local))]
    parts = [ok(part)]
    return parts

def nested_append(flag: bool, part: view[str]) -> List[List[view[str]]]:
    local = "inner"
    nested = [[part]]
    if flag:
        append(nested, [view(local)])
        nested = [[part]]
    return nested

def unrelated_loops(flag: bool, part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    total = 0
    if flag:
        for number in range(3):
            total += number
        while total < 6:
            total += 1
    else:
        for other in range(2):
            total += other
    return parts

def mutation_input(part: view[str]) -> List[List[view[str]]]:
    local = "inner"
    item = [view(local)]
    item = [part]
    parts = [[part]]
    append(parts, item)
    return parts

def inspect(left: i64, right: i64):
    return

def scalar_observation(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    item = [view(local)]
    inspect(len(parts), len(item))
    for number in range(2):
        length = len(item)
    return parts
"#;

const LOW: &str = r#"from std.ownership import move
enum Three { First; Second; Third; }
fn consume(parts: List[view[str]]) -> bool { return len(parts) > 0; }
fn marker(ignored: bool) -> Option[i64] { return some(1); }
fn select(parts: List[view[str]]) -> List[view[str]] { return parts; }
fn alias_return(part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part];
    let first: List[view[str]] = move(parts); let alias: List[view[str]] = move(first); return alias;
}
fn result_return(part: view[str]) -> Result[List[view[str]], i64] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part];
    let alias: List[view[str]] = move(parts); return ok(alias);
}
fn option_return(part: view[str]) -> Option[List[view[str]]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; return some(parts);
}
fn nested_return(part: view[str]) -> List[List[view[str]]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; return [parts];
}
fn indexed_return(part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; return [parts[0]];
}
fn call_return(part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part]; return select(parts);
}
fn three_edges(value: Three, part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let parts: List[view[str]] = [part];
    match value {
        case Three.First { parts = [view(local)]; parts = [part]; }
        case Three.Second { append(parts, view(local)); parts = [part]; }
        case Three.Third { parts = [part]; }
    }
    return parts;
}
fn terminal_edge(value: Option[i64], part: view[str]) -> List[view[str]] {
    let parts: List[view[str]] = [part];
    match value {
        case Some(number) { let local: str = "inner"; parts = [view(local)]; parts = [part]; return parts; }
        case None { parts = [part]; }
    }
    return parts;
}
fn pattern_restore(value: Option[List[view[str]]], part: view[str]) -> List[view[str]] {
    match value {
        case Some(type) { let local: str = "inner"; type = [view(local)]; type = [part]; let alias: List[view[str]] = move(type); return alias; }
        case None { return [part]; }
    }
}
fn scrutinee_move(part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part];
    match some(parts) { case Some(alias) { return alias; } case None { return []; } }
}
fn condition_move(flag: bool, part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part];
    match marker(flag and consume(parts)) { case Some(number) { parts = [part]; } case None { parts = [part]; } }
    return parts;
}
fn cross_move(value: Three, choose: bool, part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let first: List[view[str]] = [view(local)]; first = [part];
    let second: List[view[str]] = [view(local)]; second = [part];
    match value { case Three.First { first = move(second); } case Three.Second { second = move(first); } case Three.Third { second = [part]; } }
    first = [part]; second = [part]; if choose { return first; } return second;
}

fn nested_binding(part: view[str]) -> List[List[view[str]]] {
    let local: str = "inner"; let nested: List[List[view[str]]] = [[view(local)]];
    nested = [[part]]; return nested;
}
fn list_views(part: view[i64]) -> List[view[i64]] {
    let local: List[i64] = [1]; let parts: List[view[i64]] = [view(local)]; parts = [part]; return parts;
}
fn nested_list_views(part: view[List[i64]]) -> List[view[List[i64]]] {
    let local: List[List[i64]] = [[1]]; let parts: List[view[List[i64]]] = [view(local)]; parts = [part]; return parts;
}
fn result_elements(part: view[str]) -> List[Result[view[str], str]] {
    let local: str = "inner"; let parts: List[Result[view[str], str]] = [ok(view(local))]; parts = [ok(part)]; return parts;
}
fn nested_append(flag: bool, part: view[str]) -> List[List[view[str]]] {
    let local: str = "inner"; let nested: List[List[view[str]]] = [[part]];
    if flag { append(nested, [view(local)]); nested = [[part]]; } return nested;
}

fn unrelated_loops(flag: bool, part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part];
    let total: i64 = 0;
    if flag { for number in range(3) { total += number; } while total < 6 { total += 1; } }
    else { for other in range(2) { total += other; } }
    return parts;
}

fn mutation_input(part: view[str]) -> List[List[view[str]]] {
    let local: str = "inner"; let item: List[view[str]] = [view(local)]; item = [part];
    let parts: List[List[view[str]]] = [[part]]; append(parts, item); return parts;
}
fn inspect(left: i64, right: i64) -> unit { return; }
fn scalar_observation(part: view[str]) -> List[view[str]] {
    let local: str = "inner"; let parts: List[view[str]] = [view(local)]; parts = [part];
    let item: List[view[str]] = [view(local)]; inspect(len(parts), len(item));
    for number in range(2) { let length: i64 = len(item); } return parts;
}
"#;

const ASSERTIONS: &str = r#"
#[test] fn observers() {
    let owner = String::from("caller"); let part = owner.as_str();
    assert_eq!(alias_return(part), vec![part]);
    assert_eq!(result_return(part), Ok(vec![part]));
    assert_eq!(option_return(part), Some(vec![part]));
    assert_eq!(nested_return(part), vec![vec![part]]);
    assert_eq!(indexed_return(part), vec![part]);
    assert_eq!(call_return(part), vec![part]);
    assert_eq!(scrutinee_move(part), vec![part]);
    assert_eq!(nested_binding(part), vec![vec![part]]);
    assert_eq!(result_elements(part), vec![Ok(part)]);
    assert_eq!(mutation_input(part), vec![vec![part], vec![part]]);
    assert_eq!(scalar_observation(part), vec![part]);
    let numbers = vec![1, 2, 3];
    assert_eq!(list_views(&numbers), vec![numbers.as_slice()]);
    let lists = vec![numbers];
    assert_eq!(nested_list_views(&lists), vec![lists.as_slice()]);
    for flag in [false, true] { assert_eq!(nested_append(flag, part), vec![vec![part]]); assert_eq!(unrelated_loops(flag, part), vec![part]); }
}
#[test] fn branches() {
    let owner = String::from("caller"); let part = owner.as_str();
    for value in [Three::First, Three::Second, Three::Third] { assert_eq!(three_edges(value, part), vec![part]); }
    for value in [None, Some(0)] { assert_eq!(terminal_edge(value, part), vec![part]); }
    assert_eq!(pattern_restore(Some(vec!["previous"]), part), vec![part]);
    assert_eq!(pattern_restore(None, part), vec![part]);
    for flag in [false, true] { assert_eq!(condition_move(flag, part), vec![part]); }
    for choose in [false, true] {
        for value in [Three::First, Three::Second, Three::Third] { assert_eq!(cross_move(value, choose, part), vec![part]); }
    }
}
"#;
