use nagic::{check, emit, source};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir();
        let path = (0u64..)
            .find_map(|n| {
                let p = base.join(format!("nagi-sf01-{}-{n}", std::process::id()));
                match fs::create_dir(&p) {
                    Ok(()) => Some(p),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(e) => panic!("{e}"),
                }
            })
            .unwrap();
        Self(path)
    }
    fn reject(&self, text: &str, expected: &str, line: usize, low_anchor: &str) {
        let high = self.0.join("source.nagi");
        fs::write(&high, text).unwrap();
        // Loading must succeed before the purpose-specific checker rejection.
        let loaded = source::load(&high, true).unwrap();
        let low = emit::low(&loaded.program);
        let low_line = low
            .lines()
            .position(|s| s.contains(low_anchor))
            .unwrap_or_else(|| panic!("expected Low origin {low_anchor}: {low}"))
            + 1;
        for (name, text, high) in [
            ("source.nagi", text.to_owned(), true),
            ("saved.low", low, false),
        ] {
            let path = self.0.join(name);
            fs::write(&path, &text).unwrap();
            let mut loaded = source::load(&path, high).unwrap();
            let error =
                check::check(&mut loaded.program).expect_err("legacy security entry accepted");
            assert!(error.contains(expected), "{error}");
            let d = loaded.diagnostic(&error);
            assert!(d.contains(name), "{d}");
            let expected_line = if high { line } else { low_line };
            assert!(d.contains(&format!("{name}:{expected_line}\n")), "{d}");
            assert!(!d.contains("internal compiler error"), "{d}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn legacy_decorator_is_a_checker_migration_diagnostic() {
    Fixture::new().reject(
        "@get(\"/answer\")\nasync def health() -> Result[i64, Error]:\n    return ok(7)\n",
        "SF01 migration",
        2,
        "async fn ",
    );
}
#[test]
fn legacy_global_serve_is_a_checker_migration_diagnostic() {
    Fixture::new().reject("async def main() -> Result[unit, Error]:\n    db = try await db_open(\":memory:\")\n    try await serve(db, 0)\n    return ok(print(0))\n", "SF01 migration",3,"await serve");
}
#[test]
fn legacy_principal_is_a_checker_migration_diagnostic() {
    Fixture::new().reject(
        "import std.auth as auth\n@rust(\"native::old\")\nextern def old() -> auth.Principal\n",
        "SF01 migration",
        3,
        "extern fn ",
    );
}
#[test]
fn policy_free_route_is_a_checker_migration_diagnostic() {
    Fixture::new().reject("import std.http.server as http\nclass State:\n    count: i64\nasync def handler(request: http.Request, state: shared[State]) -> Result[http.Response, Error]:\n    return ok(http.empty(http.Status.OK))\ndef setup() -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(count=7))\n    return http.route(app, http.Method.GET, \"/\", handler)\n", "SF01 migration",8,"_f_726f757465(");
}

#[test]
fn handwritten_low_cannot_use_retired_or_policy_free_entrypoints() {
    for (text,line) in [
  ("@get(\"/answer\")\nasync fn handler() -> Result[i64, Error] { return ok(7); }\n",2),
  ("import std.auth as auth;\n@rust(\"native::old\")\nextern fn old() -> auth.Principal;\n",3),
  ("async fn main() -> Result[unit, Error] {\n let db = try await db_open(\":memory:\");\n try await serve(db,0);\n return ok(print(0));\n}\n",3),
  ("import std.http.server as http;\nasync fn handler(r: http.Request, s: shared[i64]) -> Result[http.Response, Error] { return ok(http.empty(http.Status.OK)); }\nfn setup() -> Result[http.App[i64, Error], Error] {\n let app = http.app_default[i64](7);\n return http.route(app,http.Method.GET,\"/\",handler);\n}\n",5),
 ] {
  let fixture=Fixture::new();let path=fixture.0.join("manual.low");fs::write(&path,text).unwrap();let mut loaded=source::load(&path,false).unwrap();let e=check::check(&mut loaded.program).expect_err(text);assert!(e.contains("SF01 migration"),"{e}");let d=loaded.diagnostic(&e);assert!(d.contains(&format!("manual.low:{line}\n")),"{d}");
 }
}
#[test]
fn policy_output_state_and_callback_contracts_are_checked() {
    let fixture = Fixture::new();
    let baseline = r#"import std.auth as auth
import std.http.server as http
class State:
    value: i64
@rust("native::verify")
extern async def verify(r: http.Request, s: shared[State]) -> Result[auth.VerifiedIdentity, auth.Failure]
async def handler(r: http.Request, s: shared[State], a: unit) -> Result[http.Response, Error]:
    return ok(http.empty(http.Status.OK))
def setup() -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](State(value=1))
    return http.route(app, http.Method.GET, "/", http.public_policy[State](), handler)
"#;
    for (text, reason) in [
        (
            baseline.replace(
                "http.public_policy[State]()",
                "http.authenticated_policy[State](verify)",
            ),
            "型",
        ),
        (
            baseline.replace("http.public_policy[State]()", "http.public_policy[i64]()"),
            "型",
        ),
        (
            baseline
                .replace(
                    "Result[auth.VerifiedIdentity, auth.Failure]",
                    "Result[auth.VerifiedIdentity, Error]",
                )
                .replace(
                    "http.public_policy[State]()",
                    "http.authenticated_policy[State](verify)",
                ),
            "型",
        ),
        (
            baseline.replace(
                "http.public_policy[State]()",
                "http.authenticated_policy[State](handler)",
            ),
            "型",
        ),
        (baseline.replace("a: unit", "a: auth.AuthScope"), "型"),
    ] {
        let high = fixture.0.join("bad.nagi");
        fs::write(&high, &text).unwrap();
        let loaded = source::load(&high, true).unwrap();
        let saved = emit::low(&loaded.program);
        for (name, text, high) in [("bad.nagi", text, true), ("bad.low", saved, false)] {
            let path = fixture.0.join(name);
            fs::write(&path, text).unwrap();
            let mut loaded = source::load(&path, high).unwrap();
            let e =
                check::check(&mut loaded.program).expect_err("invalid policy contract accepted");
            assert!(e.contains(reason), "{e}");
            assert!(!e.contains("internal compiler error"), "{e}");
        }
    }
}
#[test]
fn native_replacements_and_aliases_cannot_downgrade_security_contracts() {
    use std::process::Command;
    let fixture = Fixture::new();
    let high = r#"import std.http.server as http
import std.auth as auth
from std.auth import AuthScope as Identity
async def handler(r: http.Request, s: shared[i64], a: unit) -> Result[http.Response, Error]:
    return ok(http.empty(http.Status.OK))
def setup() -> Result[http.App[i64, Error], Error]:
    app = http.app_default[i64](7)
    return http.route(app,http.Method.GET,"/",http.public_policy[i64](),handler)
async def use_scope(value: Identity):
    print(auth.subject(view(value)))
async def guarded(value: Identity) -> Result[unit, Error]:
    await use_scope(value)
    return ok(print(0))
"#;
    let high_path = fixture.0.join("main.nagi");
    fs::write(&high_path, high).unwrap();
    let mut sources = source::load(&high_path, true).unwrap();
    check::check(&mut sources.program).unwrap();
    let saved = emit::low(&sources.program);
    fs::write(fixture.0.join("saved.low"), saved).unwrap();
    for (replacement,reason,line) in [
  ("@replace generated::setup\nfn changed() -> Result[http.App[i64, Error], Error] {\n let app = http.app_default[i64](7);\n return http.route(app,http.Method.GET,\"/\",handler);\n}\n","SF01 migration",4),
  ("@replace generated::handler\nasync fn changed(r: http.Request, s: shared[i64], a: auth.AuthScope) -> Result[http.Response, Error] { return ok(http.empty(http.Status.OK)); }\n","一致しません",2),
  ("@replace generated::guarded\nasync fn changed(value: Identity) -> Result[unit, Error] {\n scope { spawn use_scope(value); }\n return ok(print(0));\n}\n","SameTask",3),
 ] {
  fs::write(fixture.0.join("replace.low"),replacement).unwrap();
  for source in ["main.nagi","saved.low"] {for command in ["check","build"] {
   let output=Command::new(env!("CARGO_BIN_EXE_nagic")).current_dir(&fixture.0).args([command,source,"--no-project","--native","replace.low"]).env("PATH","").output().unwrap();let error=String::from_utf8_lossy(&output.stderr);
   assert!(!output.status.success(),"{replacement}");assert!(error.contains(reason),"{source}/{command}: {error}");assert!(error.contains(&format!("replace.low:{line}\n")),"{error}");assert!(!error.contains("Cargoが見つかりません") && !error.contains("internal compiler error"),"{error}");
  }}
 }
}
