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
            "nagi-view-branch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn compile_and_run(&self, name: &str, program: &nagic::ast::Program) {
        let source = self.0.join(format!("{name}.rs"));
        let binary = self
            .0
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::write(&source, emit::rust(program).unwrap() + ASSERTIONS).unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test"])
            .arg(&source)
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
fn terminating_view_updates_compile_and_run_in_high_saved_low_and_handwritten_low() {
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

#[test]
fn branch_copies_are_synthetic_and_preserve_public_and_callback_lifetimes() {
    let high = checked(
        "def mapped(flag: bool, part: view[str], callback: fn[view[str], view[str]]) -> view[str]:\n    alias = part\n    if flag:\n        local = \"inner\"\n        alias = view(local)\n        return callback(part)\n    return alias\n",
        true,
    );
    let generated_low = emit::low_with_lines(&high);
    let mut saved_low = parser::parse(&generated_low.text, false).unwrap();
    generated_low.restore_lines(&mut saved_low).unwrap();
    check::check(&mut saved_low).unwrap();
    for program in [&high, &saved_low] {
        let generated = emit::rust_with_lines(program).unwrap();
        assert!(generated.text.contains(
            "pub fn mapped<'a>(mut flag: ::std::primitive::bool, mut part: &'a ::std::primitive::str"
        ));
        assert!(generated.text.contains(
            "mut callback: for<'nagi_fn_0> fn(&'nagi_fn_0 ::std::primitive::str) -> &'nagi_fn_0 ::std::primitive::str"
        ));
        assert!(!generated.text.contains("let mut callback:"));
        let mut branch_copies = 0;
        let mut original_statements = 0;
        for (index, line) in generated.text.lines().enumerate() {
            match line.trim() {
                "let mut alias: &::std::primitive::str = alias;" => {
                    branch_copies += 1;
                    assert_eq!(generated.line_origin(index + 1), None);
                }
                "let mut alias: &::std::primitive::str = part;" => {
                    original_statements += 1;
                    assert_eq!(generated.line_origin(index + 1), Some(2));
                }
                "alias = (local).as_str();" => {
                    original_statements += 1;
                    assert_eq!(generated.line_origin(index + 1), Some(5));
                }
                "return callback(part);" => {
                    original_statements += 1;
                    assert_eq!(generated.line_origin(index + 1), Some(6));
                }
                "return alias;" => {
                    original_statements += 1;
                    assert_eq!(generated.line_origin(index + 1), Some(7));
                }
                _ => {}
            }
        }
        assert_eq!(branch_copies, 1);
        assert_eq!(original_statements, 4);
    }

    let program = checked(
        "def callback_slice(flag: bool, callbacks: view[fn[view[str], view[str]]]) -> i64:\n    alias = callbacks\n    if flag:\n        replacements = [identity]\n        alias = view(replacements)\n        return len(alias)\n    return len(alias)\ndef identity(part: view[str]) -> view[str]:\n    return part\n",
        true,
    );
    let generated = emit::rust(&program).unwrap();
    assert!(generated.contains(
        "mut callbacks: &'a [for<'nagi_fn_1> fn(&'nagi_fn_1 ::std::primitive::str) -> &'nagi_fn_1 ::std::primitive::str]"
    ));
    assert!(generated.contains(
        "let mut alias: &[for<'nagi_fn_1> fn(&'nagi_fn_1 ::std::primitive::str) -> &'nagi_fn_1 ::std::primitive::str] = alias;"
    ));
}

#[test]
fn terminating_parent_blocks_respect_loop_and_pattern_binding_names() {
    let high = checked(
        "def scoped(flag: bool, count: i64, part: view[str]) -> view[str]:\n    alias = part\n    if flag:\n        for alias in range(count):\n            alias = 1\n            assert_true(alias == 1)\n        return part\n    return alias\n",
        true,
    );
    let saved_low = checked(&emit::low(&high), false);
    let handwritten_low = checked(
        "fn scoped(flag: bool, count: i64, part: view[str]) -> view[str] { let alias: view[str] = part; if flag { for alias in range(count) { alias = 1; assert_true(alias == 1); } return part; } return alias; }",
        false,
    );
    for program in [&high, &saved_low, &handwritten_low] {
        let generated = emit::rust(program).unwrap();
        assert!(!generated.contains("let mut alias: &::std::primitive::str = alias;"));
    }

    for (text, is_high) in [
        (
            "def bad(value: Option[view[str]], part: view[str]) -> view[str]:\n    alias = part\n    match value:\n        case Some(alias):\n            return alias\n        case None:\n            return part\n",
            true,
        ),
        (
            "fn bad(value: Option[view[str]], part: view[str]) -> view[str] { let alias: view[str] = part; match value { case Some(alias) { return alias; } case None { return part; } } }",
            false,
        ),
    ] {
        let mut program = parser::parse(text, is_high).unwrap();
        let error = check::check(&mut program).expect_err(text);
        assert!(
            error.contains("caseの変数名は外側の変数と重複できません"),
            "{text}\n{error}"
        );
    }
}

#[test]
fn local_views_still_cannot_escape_a_terminating_or_continuing_branch() {
    for (high, low, expected) in [
        (
            "def bad(flag: bool, part: view[str]) -> view[str]:\n    alias = part\n    if flag:\n        local = \"inner\"\n        alias = view(local)\n        return alias\n    return part\n",
            "fn bad(flag: bool, part: view[str]) -> view[str] { let alias: view[str] = part; if flag { let local: str = \"inner\"; alias = view(local); return alias; } return part; }",
            "view escapes",
        ),
        (
            "def bad(flag: bool, part: view[str]) -> view[str]:\n    alias = part\n    if flag:\n        local = \"inner\"\n        alias = view(local)\n    return alias\n",
            "fn bad(flag: bool, part: view[str]) -> view[str] { let alias: view[str] = part; if flag { let local: str = \"inner\"; alias = view(local); } return alias; }",
            "内側のscopeの所有値",
        ),
        (
            "def bad(flag: bool, parts: view[view[str]]) -> view[view[str]]:\n    if flag:\n        local = \"inner\"\n        values = [view(local)]\n        parts = view(values)\n        return parts\n    return parts\n",
            "fn bad(flag: bool, parts: view[view[str]]) -> view[view[str]] { if flag { let local: str = \"inner\"; let values: List[view[str]] = [view(local)]; parts = view(values); return parts; } return parts; }",
            "view escapes",
        ),
        (
            "def bad(flag: bool, parts: List[view[str]]) -> List[view[str]]:\n    if flag:\n        local = \"inner\"\n        parts = [view(local)]\n        return parts\n    return parts\n",
            "fn bad(flag: bool, parts: List[view[str]]) -> List[view[str]] { if flag { let local: str = \"inner\"; parts = [view(local)]; return parts; } return parts; }",
            "view escapes",
        ),
    ] {
        for (text, is_high) in [(high, true), (low, false)] {
            let mut program = parser::parse(text, is_high).unwrap();
            let error = check::check(&mut program).expect_err(text);
            assert!(error.contains(expected), "{text}\n{error}");
        }
    }
}

const HIGH: &str = r#"def choose(flag: bool, part: view[str]) -> view[str]:
    alias = part
    if flag:
        local = "inner"
        alias = view(local)
        assert_true(alias == "inner")
        return part
    return alias

def both(flag: bool, part: view[str]) -> view[str]:
    alias = part
    if flag:
        local = "left"
        alias = view(local)
        assert_true(alias == "left")
        return part
    else:
        local = "right"
        alias = view(local)
        assert_true(alias == "right")
        return part

def nested(flag: bool, update: bool, part: view[str]) -> view[str]:
    alias = part
    if flag:
        local = "nested"
        if update:
            alias = view(local)
        if update:
            assert_true(alias == "nested")
        else:
            assert_true(alias == part)
        return part
    return alias

def looped(flag: bool, part: view[str]) -> view[str]:
    alias = part
    while flag:
        local = "loop"
        alias = view(local)
        assert_true(alias == "loop")
        return part
    return alias

def iterated(count: i64, part: view[str]) -> view[str]:
    alias = part
    for number in range(count):
        local = "for"
        alias = view(local)
        assert_true(alias == "for")
        return part
    return alias

def iterated_join(flag: bool, part: view[str]) -> view[str]:
    alias = part
    for number in range(1):
        if flag:
            local = "joined"
            alias = view(local)
            assert_true(alias == "joined")
            return part
        else:
            return part
    return alias

def matched(flag: Option[bool], part: view[str]) -> view[str]:
    alias = part
    match flag:
        case Some(value):
            local = "match"
            alias = view(local)
            assert_true(alias == "match")
            return part
        case None:
            assert_true(alias == part)
    return alias

def result(reject: bool) -> Result[i64, i64]:
    if reject:
        return fail(1)
    return ok(0)

def propagated(flag: bool, reject: bool, part: view[str]) -> Result[view[str], i64]:
    alias = part
    if flag:
        local = "try"
        alias = view(local)
        assert_true(alias == "try")
        value = try result(reject)
        return ok(part)
    return ok(alias)

def propagated_continuing(flag: bool, reject: bool, part: view[str], replacement: view[str]) -> Result[view[str], i64]:
    alias = part
    if flag:
        alias = replacement
        value = try result(reject)
    return ok(alias)

def local_declaration(flag: bool, part: view[str]) -> view[str]:
    if flag:
        local = "first"
        fresh = view(local)
        other = "second"
        fresh = view(other)
        assert_true(fresh == "second")
        return part
    return part

def reassigned_parameter(flag: bool, part: view[str]) -> view[str]:
    saved = part
    if flag:
        local = "parameter"
        part = view(local)
        assert_true(part == "parameter")
        return saved
    return part

def continuing(flag: bool, part: view[str], replacement: view[str]) -> view[str]:
    alias = part
    if flag:
        alias = replacement
    return alias

def continuing_loop(count: i64, part: view[str], replacement: view[str]) -> view[str]:
    alias = part
    for number in range(count):
        alias = replacement
    return alias

def loop_shadow(count: i64, part: view[str]) -> view[str]:
    alias = part
    for alias in range(count):
        alias = 1
        assert_true(alias == 1)
        return part
    return alias

def view_loop_binding(parts: view[view[str]], part: view[str]) -> view[str]:
    for item in parts:
        local = "loop item"
        item = view(local)
        assert_true(item == "loop item")
        return part
    return part

def matched_binding(value: Option[view[str]], part: view[str]) -> view[str]:
    match value:
        case Some(item):
            local = "match item"
            item = view(local)
            assert_true(item == "match item")
            return part
        case None:
            return part

def nested_elements(flag: bool, parts: view[view[str]]) -> view[view[str]]:
    alias = parts
    if flag:
        local = "element"
        values = [view(local)]
        alias = view(values)
        assert_true(alias[0] == "element")
        return parts
    return alias

def consume(parts: List[view[str]]) -> i64:
    return len(parts)

def list_reinitialized(flag: bool, part: view[str]) -> i64:
    parts = [part]
    consumed = consume(parts)
    if flag:
        local = "container"
        parts = [view(local)]
        return len(parts) + consumed
    parts = [part]
    return len(parts) + consumed

def identity(part: view[str]) -> view[str]:
    return part

def invoke(callback: fn[view[str], view[str]], flag: bool, part: view[str]) -> view[str]:
    alias = part
    if flag:
        local = "callback"
        alias = view(local)
        assert_true(alias == "callback")
        return callback(part)
    return alias
"#;

// Standalone Low is deliberately handwritten rather than obtained from emit::low.
const LOW: &str = r#"fn choose(flag: bool, part: view[str]) -> view[str] {
    let alias: view[str] = part;
    if flag {
        let local: str = "inner";
        alias = view(local);
        assert_true(alias == "inner");
        return part;
    }
    return alias;
}
fn both(flag: bool, part: view[str]) -> view[str] {
    let alias: view[str] = part;
    if flag {
        let local: str = "left";
        alias = view(local);
        assert_true(alias == "left");
        return part;
    } else {
        let local: str = "right";
        alias = view(local);
        assert_true(alias == "right");
        return part;
    }
}
fn nested(flag: bool, update: bool, part: view[str]) -> view[str] {
    let alias: view[str] = part;
    if flag {
        let local: str = "nested";
        if update { alias = view(local); }
        if update { assert_true(alias == "nested"); }
        else { assert_true(alias == part); }
        return part;
    }
    return alias;
}
fn looped(flag: bool, part: view[str]) -> view[str] {
    let alias: view[str] = part;
    while flag {
        let local: str = "loop";
        alias = view(local);
        assert_true(alias == "loop");
        return part;
    }
    return alias;
}
fn iterated(count: i64, part: view[str]) -> view[str] {
    let alias: view[str] = part;
    for number in range(count) {
        let local: str = "for";
        alias = view(local);
        assert_true(alias == "for");
        return part;
    }
    return alias;
}
fn iterated_join(flag: bool, part: view[str]) -> view[str] {
    let alias: view[str] = part;
    for number in range(1) {
        if flag {
            let local: str = "joined";
            alias = view(local);
            assert_true(alias == "joined");
            return part;
        } else { return part; }
    }
    return alias;
}
fn matched(flag: Option[bool], part: view[str]) -> view[str] {
    let alias: view[str] = part;
    match flag {
        case Some(value) {
            let local: str = "match";
            alias = view(local);
            assert_true(alias == "match");
            return part;
        }
        case None { assert_true(alias == part); }
    }
    return alias;
}
fn result(reject: bool) -> Result[i64, i64] {
    if reject { return fail(1); }
    return ok(0);
}
fn propagated(flag: bool, reject: bool, part: view[str]) -> Result[view[str], i64] {
    let alias: view[str] = part;
    if flag {
        let local: str = "try";
        alias = view(local);
        assert_true(alias == "try");
        let value: i64 = try result(reject);
        return ok(part);
    }
    return ok(alias);
}
fn propagated_continuing(flag: bool, reject: bool, part: view[str], replacement: view[str]) -> Result[view[str], i64] {
    let alias: view[str] = part;
    if flag {
        alias = replacement;
        let value: i64 = try result(reject);
    }
    return ok(alias);
}
fn local_declaration(flag: bool, part: view[str]) -> view[str] {
    if flag {
        let local: str = "first";
        let fresh: view[str] = view(local);
        let other: str = "second";
        fresh = view(other);
        assert_true(fresh == "second");
        return part;
    }
    return part;
}
fn reassigned_parameter(flag: bool, part: view[str]) -> view[str] {
    let saved: view[str] = part;
    if flag {
        let local: str = "parameter";
        part = view(local);
        assert_true(part == "parameter");
        return saved;
    }
    return part;
}
fn continuing(flag: bool, part: view[str], replacement: view[str]) -> view[str] {
    let alias: view[str] = part;
    if flag { alias = replacement; }
    return alias;
}
fn continuing_loop(count: i64, part: view[str], replacement: view[str]) -> view[str] {
    let alias: view[str] = part;
    for number in range(count) { alias = replacement; }
    return alias;
}
fn loop_shadow(count: i64, part: view[str]) -> view[str] {
    let alias: view[str] = part;
    for alias in range(count) {
        alias = 1;
        assert_true(alias == 1);
        return part;
    }
    return alias;
}
fn view_loop_binding(parts: view[view[str]], part: view[str]) -> view[str] {
    for item in parts {
        let local: str = "loop item";
        item = view(local);
        assert_true(item == "loop item");
        return part;
    }
    return part;
}
fn matched_binding(value: Option[view[str]], part: view[str]) -> view[str] {
    match value {
        case Some(item) {
            let local: str = "match item";
            item = view(local);
            assert_true(item == "match item");
            return part;
        }
        case None { return part; }
    }
}
fn nested_elements(flag: bool, parts: view[view[str]]) -> view[view[str]] {
    let alias: view[view[str]] = parts;
    if flag {
        let local: str = "element";
        let values: List[view[str]] = [view(local)];
        alias = view(values);
        assert_true(alias[0] == "element");
        return parts;
    }
    return alias;
}
fn consume(parts: List[view[str]]) -> i64 { return len(parts); }
fn list_reinitialized(flag: bool, part: view[str]) -> i64 {
    let parts: List[view[str]] = [part];
    let consumed: i64 = consume(parts);
    if flag {
        let local: str = "container";
        parts = [view(local)];
        return len(parts) + consumed;
    }
    parts = [part];
    return len(parts) + consumed;
}
fn identity(part: view[str]) -> view[str] { return part; }
fn invoke(callback: fn[view[str], view[str]], flag: bool, part: view[str]) -> view[str] {
    let alias: view[str] = part;
    if flag {
        let local: str = "callback";
        alias = view(local);
        assert_true(alias == "callback");
        return callback(part);
    }
    return alias;
}
"#;

const ASSERTIONS: &str = r#"
#[test]
fn branch_values_and_borrowed_returns() {
    let original = String::from("original");
    let replacement = String::from("replacement");
    let parts = [original.as_str(), replacement.as_str()];
    for flag in [false, true] {
        assert_eq!(choose(flag, &original), "original");
        assert_eq!(both(flag, &original), "original");
        for update in [false, true] {
            assert_eq!(nested(flag, update, &original), "original");
        }
        assert_eq!(looped(flag, &original), "original");
        assert_eq!(iterated_join(flag, &original), "original");
        assert_eq!(reassigned_parameter(flag, &original), "original");
        assert_eq!(local_declaration(flag, &original), "original");
        assert_eq!(invoke(identity, flag, &original), "original");
        assert_eq!(nested_elements(flag, &parts), parts);
        assert_eq!(
            continuing(flag, &original, &replacement),
            if flag { "replacement" } else { "original" }
        );
        assert_eq!(list_reinitialized(flag, &original), 2);
        for reject in [false, true] {
            assert_eq!(propagated(flag, reject, &original),
                if flag && reject { Err(1) } else { Ok(original.as_str()) });
            assert_eq!(propagated_continuing(flag, reject, &original, &replacement),
                if flag && reject { Err(1) }
                else { Ok(if flag { replacement.as_str() } else { original.as_str() }) });
        }
    }
    for count in [0, 1, 3] {
        assert_eq!(iterated(count, &original), "original");
        assert_eq!(loop_shadow(count, &original), "original");
        assert_eq!(continuing_loop(count, &original, &replacement),
            if count == 0 { "original" } else { "replacement" });
    }
    for flag in [None, Some(false), Some(true)] {
        assert_eq!(matched(flag, &original), "original");
    }
    assert_eq!(view_loop_binding(&parts, &original), "original");
    assert_eq!(view_loop_binding(&[], &original), "original");
    assert_eq!(matched_binding(Some(&replacement), &original), "original");
    assert_eq!(matched_binding(None, &original), "original");
    assert_eq!(original, "original");
    assert_eq!(replacement, "replacement");
}
"#;
