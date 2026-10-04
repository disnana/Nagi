use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-static-callback-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn static_callbacks_keep_their_origins_and_independent_lifetime_scopes_in_high_and_low() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", VALID);
    fixture.write("native.rs", NATIVE);
    let mut high = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut high.program).unwrap();
    let low = emit::low(&high.program);
    let mut independent = parser::parse(&low, false).unwrap();
    check::check(&mut independent).unwrap();
    fixture.write("main.low", &low);
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    for entry in ["main.nagi", "main.low"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&fixture.0)
            .args([
                "run",
                entry,
                "--rust",
                "native.rs",
                "--out",
                "build",
                "--no-project",
            ])
            .env("NAGI_NATIVE_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{entry}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().replace("\r\n", "\n").trim(),
            "OK\nOK\nOK\nOK\nOK\nOK\nOK\nowned\nOK\nOK\ninput\ninput\ninput\nOK\n0\nabsent\nnative\nnative\nnative\n1\n1\n1\n1\n1\n1\n1\n1"
        );
    }
}

fn rejects(high: &str, low: &str) {
    for (text, is_high) in [(high, true), (low, false)] {
        let mut program = parser::parse(text, is_high)
            .unwrap_or_else(|error| panic!("fixture failed to parse:\n{text}\n{error}"));
        let error = check::check(&mut program).expect_err(text);
        assert!(error.contains("view escapes"), "{text}\n{error}");
    }
}

#[test]
fn static_outputs_do_not_allow_local_or_owned_parameter_borrows() {
    rejects(
        "def bad() -> view[str]:\n    return view(\"local\")\n",
        "fn bad() -> view[str] { return view(\"local\"); }\n",
    );
    rejects(
        "def bad(value: str) -> view[str]:\n    return view(value)\n",
        "fn bad(value: str) -> view[str] { return view(value); }\n",
    );
    rejects(
        "def bad(number: i64) -> view[str]:\n    local = \"local\"\n    return view(local)\n",
        "fn bad(number: i64) -> view[str] { let local: str = \"local\"; return view(local); }\n",
    );
    rejects(
        "def bad(factory: fn[view[str]], value: str) -> view[str]:\n    return view(value)\n",
        "fn bad(factory: fn[view[str]], value: str) -> view[str] { return view(value); }\n",
    );
    rejects(
        "def bad(factory: fn[view[str], view[str]], value: str) -> view[str]:\n    return factory(view(value))\n",
        "fn bad(factory: fn[view[str], view[str]], value: str) -> view[str] { return factory(view(value)); }\n",
    );
}

#[test]
fn input_bound_callback_results_still_borrow_their_actual_inputs() {
    for (text, high) in [
        ("def echo(part: view[str]) -> view[str]:\n    return part\ndef main():\n    owner = \"before\"\n    callback: fn[view[str], view[str]] = echo\n    borrowed = callback(view(owner))\n    owner = \"after\"\n    print(borrowed)\n", true),
        ("fn echo(part: view[str]) -> view[str] { return part; }\nfn main() -> unit {\n    let owner: str = \"before\";\n    let callback: fn[view[str], view[str]] = echo;\n    let borrowed: view[str] = callback(view(owner));\n    owner = \"after\";\n    print(borrowed);\n}\n", false),
    ] {
        let mut program = parser::parse(text, high).unwrap();
        let error = check::check(&mut program).expect_err(text);
        assert!(error.contains("参照中の所有値は再代入できません"), "{text}\n{error}");
    }
}

const VALID: &str = r#"import std.http.server as http

def phrase() -> view[str]:
    return http.Status.OK.phrase

def relay() -> view[str]:
    return phrase()

def apply(factory: fn[view[str]]) -> view[str]:
    stored = factory()
    return stored

def choose() -> fn[view[str]]:
    return phrase

def nested(maker: fn[fn[view[str]]]) -> view[str]:
    factory = maker()
    return factory()

def numeric(number: i64) -> view[str]:
    return phrase()

def from_owned(value: str) -> view[str]:
    print(value)
    return phrase()

def echo(part: view[str]) -> view[str]:
    return part

def choose_bound() -> fn[view[str], view[str]]:
    return echo

def outer_static(callback: fn[view[str], view[str]]) -> view[str]:
    return phrase()

def outer_bound(callback: fn[view[str]], part: view[str]) -> view[str]:
    return part

def through_bound(callback: fn[fn[view[str]], view[str], view[str]], part: view[str]) -> view[str]:
    return callback(phrase, part)

def through_list(callbacks: view[fn[view[str]]]) -> view[str]:
    first = callbacks[0]
    return first()

def absent() -> Option[view[str]]:
    return None

def empty() -> List[view[str]]:
    return []

def relay_absent(factory: fn[Option[view[str]]]) -> Option[view[str]]:
    return factory()

def relay_empty(factory: fn[List[view[str]]]) -> List[view[str]]:
    return factory()

@rust("native::provide")
extern def provide() -> fn[view[str]]
@rust("native::provide_optional")
extern def provide_optional() -> Option[fn[view[str]]]
@rust("native::provide_list")
extern def provide_list() -> List[fn[view[str]]]
@rust("native::provide")
extern def provide_owned() -> owned[fn[view[str]]]
@rust("native::consume")
extern def consume(callback: fn[view[str]]) -> i64
@rust("native::consume")
extern def consume_owned(callback: owned[fn[view[str]]]) -> i64
@rust("native::inspect_outer_static")
extern def inspect_outer_static(callback: fn[fn[view[str], view[str]], view[str]]) -> i64
@rust("native::inspect_outer_bound")
extern def inspect_outer_bound(callback: fn[fn[view[str]], view[str], view[str]]) -> i64
@rust("native::inspect_list")
extern def inspect_list(callback: fn[view[fn[view[str]]], view[str]]) -> i64
@rust("native::inspect_optional_mixed")
extern def inspect_optional_mixed(callback: Option[fn[fn[view[str], view[str]], view[str]]]) -> i64
@rust("native::inspect_list_mixed")
extern def inspect_list_mixed(callbacks: List[fn[fn[view[str]], view[str], view[str]]]) -> i64
@rust("native::provide_owned_mixed")
extern def provide_owned_mixed() -> owned[fn[fn[view[str]], view[str], view[str]]]
@rust("native::inspect_owned_mixed")
extern def inspect_owned_mixed(callback: owned[fn[fn[view[str]], view[str], view[str]]]) -> i64

def main():
    print(phrase())
    print(relay())
    chosen = phrase
    print(chosen())
    explicit: fn[view[str]] = phrase
    stored = explicit()
    print(stored)
    print(apply(explicit))
    print(nested(choose))
    selected_number: fn[i64, view[str]] = numeric
    print(selected_number(7))
    selected_owned: fn[str, view[str]] = from_owned
    print(selected_owned("owned"))
    print(outer_static(echo))
    owner = "input"
    print(echo(view(owner)))
    selected_bound = choose_bound()
    print(selected_bound(view(owner)))
    print(through_bound(outer_bound, view(owner)))
    callbacks = [phrase]
    print(through_list(view(callbacks)))
    print(len(relay_empty(empty)))
    optional = relay_absent(absent)
    match optional:
        case Some(part):
            print(part)
        case None:
            print("absent")
    native_callback = provide()
    print(native_callback())
    native_optional = provide_optional()
    match native_optional:
        case Some(callback):
            print(callback())
        case None:
            print("missing callback")
    native_callbacks = provide_list()
    first_native = native_callbacks[0]
    print(first_native())
    print(consume(phrase))
    print(consume_owned(provide_owned()))
    print(inspect_outer_static(outer_static))
    print(inspect_outer_bound(outer_bound))
    print(inspect_list(through_list))
    print(inspect_optional_mixed(some(outer_static)))
    print(inspect_list_mixed([outer_bound]))
    print(inspect_owned_mixed(provide_owned_mixed()))
"#;

const NATIVE: &str = r#"fn label() -> &'static str { "native" }
fn identity(value: &str) -> &str { value }
pub fn provide() -> fn() -> &'static str { label }
pub fn provide_optional() -> Option<fn() -> &'static str> { Some(label) }
pub fn provide_list() -> Vec<fn() -> &'static str> { vec![label] }
pub fn consume(callback: fn() -> &'static str) -> i64 {
    assert!(["OK", "native"].contains(&callback()));
    1
}
pub fn inspect_outer_static(callback: fn(for<'a> fn(&'a str) -> &'a str) -> &'static str) -> i64 {
    assert_eq!(callback(identity), "OK");
    1
}
pub fn inspect_outer_bound(callback: for<'a> fn(fn() -> &'static str, &'a str) -> &'a str) -> i64 {
    let owned = String::from("input");
    assert_eq!(callback(label, &owned), "input");
    1
}
pub fn inspect_list(callback: for<'a> fn(&'a [fn() -> &'static str]) -> &'a str) -> i64 {
    assert_eq!(callback(&[label]), "native");
    1
}
pub fn inspect_optional_mixed(callback: Option<fn(for<'a> fn(&'a str) -> &'a str) -> &'static str>) -> i64 {
    assert_eq!(callback.unwrap()(identity), "OK");
    1
}
pub fn inspect_list_mixed(callbacks: Vec<for<'a> fn(fn() -> &'static str, &'a str) -> &'a str>) -> i64 {
    let owned = String::from("input");
    assert_eq!(callbacks[0](label, &owned), "input");
    1
}
fn bound(_: fn() -> &'static str, value: &str) -> &str { value }
pub fn provide_owned_mixed() -> for<'a> fn(fn() -> &'static str, &'a str) -> &'a str { bound }
pub fn inspect_owned_mixed(callback: for<'a> fn(fn() -> &'static str, &'a str) -> &'a str) -> i64 {
    let owned = String::from("input");
    assert_eq!(callback(label, &owned), "input");
    1
}
"#;
