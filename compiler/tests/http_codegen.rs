use nagic::{ast::Program, check, emit, parser, source};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn checked(text: &str) -> (Self, Program) {
        let path = std::env::temp_dir().join(format!(
            "nagi-http-codegen-{}-{}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("main.nagi"), text).unwrap();
        let fixture = Self(path);
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        (fixture, loaded.program)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn independent_low(program: &Program) -> Program {
    let low = emit::low(program);
    let mut saved = parser::parse(&low, false).unwrap();
    check::check(&mut saved).unwrap_or_else(|error| panic!("{error}\n{low}"));
    saved
}

// This whole handler reads native request/header/body views and shared state.
// It needs no owned string construction, state clone, or adapter allocation.
// Response construction and the heterogeneous runtime route table have their
// own measured costs; this checks that code generation adds none of those.
const AUTH: &str = r#"import std.http.server as http
class State:
    expected: str
def allowed(request: view[http.Request], state: shared[State]) -> Result[bool, Error]:
    assert_true(request.method == http.Method.GET)
    assert_true(request.path == "/probe")
    assert_true(request.path < "/z")
    assert_true("/a" < request.path)
    assert_true(request.path <= state.expected)
    assert_true(state.expected >= request.path)
    assert_true(request.body <= request.body)
    assert_true(len(request.body) == 0)
    status = copy(view(http.Status.OK))
    assert_true(status.value == 200)
    match try http.header_text(request, view("Authorization")):
        case Some(value):
            return ok(value == view(state.expected))
        case None:
            return ok(False)
async def handler(request: http.Request, state: shared[State]) -> Result[http.Response, Error]:
    accepted = try allowed(view(request), state)
    if accepted:
        return ok(http.empty(http.Status.OK))
    else:
        return ok(http.empty(http.Status.UNAUTHORIZED))
def register(state: State) -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](state)
    app = try http.route(app, http.Method.GET, view("/probe"), handler)
    return ok(app)
"#;

#[test]
fn borrowed_http_inspection_and_async_registration_add_no_owned_adapter_work() {
    let (_fixture, program) = Fixture::checked(AUTH);
    let saved = independent_low(&program);
    let high_rust = emit::rust(&program).unwrap();
    let low_rust = emit::rust(&saved).unwrap();
    assert_eq!(high_rust, low_rust, "saved Low changed adapter work");

    for operation in [
        "String::from(",
        ".to_owned()",
        ".clone()",
        "Arc::clone(",
        "Arc::new(",
        "Box::",
        "BoxFuture",
        "format!(",
    ] {
        assert!(
            !high_rust.contains(operation),
            "borrowed HTTP adapter introduced {operation}:\n{high_rust}"
        );
    }
    assert!(high_rust.contains("&'a ::nagi_runtime::http_server::Request"));
    assert!(high_rust.contains("::nagi_runtime::http_server::Method::GET"));
    assert!(high_rust.contains("::nagi_runtime::http_server::Status::OK"));
    assert!(high_rust.contains("::nagi_runtime::http_server::header_text("));
    let handler = &program.modules.resolve_root_path("handler").unwrap().symbol;
    let registration = high_rust
        .lines()
        .find(|line| line.contains("::nagi_runtime::http_server::route("))
        .expect("missing native handler registration");
    assert!(registration.contains(&format!(", crate::{handler})")));
}

#[test]
fn literal_view_optimization_does_not_change_a_shadowing_owned_function() {
    let (_fixture, program) = Fixture::checked(
        "def view(value: str) -> str:\n    return value\ndef owned() -> str:\n    return view(\"owned argument\")\n",
    );
    let function = &program.modules.resolve_root_path("view").unwrap().symbol;
    for checked in [&program, &independent_low(&program)] {
        let rust = emit::rust(checked).unwrap();
        assert!(rust.contains(&format!(
            "crate::{function}(::std::string::String::from(\"owned argument\"))"
        )));
    }
}
