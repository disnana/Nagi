use nagic::{ast::DefKind, check, emit, modules, parser, source};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
// Keep native integration workloads bounded while fixtures share a Cargo cache.
static NATIVE_RUN: Mutex<()> = Mutex::new(());

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi HTTP stdlib 凪 {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn checked(&self, name: &str) -> source::Sources {
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        loaded
    }

    fn roundtrip(&self) -> source::Sources {
        let loaded = self.checked("main.nagi");
        let identities = |program: &nagic::ast::Program| {
            program
                .modules
                .definitions
                .iter()
                .map(|definition| (definition.id.clone(), definition.symbol.clone()))
                .collect::<Vec<_>>()
        };
        let expected = identities(&loaded.program);
        let low = emit::low(&loaded.program);
        let mut independent = parser::parse(&low, false).unwrap();
        check::check(&mut independent)
            .unwrap_or_else(|error| panic!("independent Low: {error}\n{low}"));
        assert_eq!(identities(&independent), expected);
        modules::validate(&independent).unwrap();
        self.write("saved.low", &low);
        let saved = self.checked("saved.low");
        assert_eq!(identities(&saved.program), expected);
        saved
    }

    fn rejected(&self, text: &str, line: usize) {
        self.write("main.nagi", text);
        let error = match source::load(&self.0.join("main.nagi"), true) {
            Err(error) => error,
            Ok(mut loaded) => {
                let error = check::check(&mut loaded.program)
                    .expect_err("invalid standard HTTP program passed check");
                loaded.diagnostic(&error)
            }
        };
        assert!(
            error.contains(&format!("main.nagi:{line}")),
            "{text}\n{error}"
        );
        // Rejection must precede backend invocation, even on a machine without
        // Rust or an installed runtime.
        for command in ["check", "build"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&self.0)
                .args([command, "main.nagi", "--no-project"])
                .env("PATH", "")
                .env("NAGI_ROOT", self.0.join("missing-runtime"))
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{command} accepted {text}");
            assert!(stderr.contains(&format!("main.nagi:{line}")), "{stderr}");
            assert!(
                !stderr.contains("Rust backend rejected")
                    && !stderr.contains("failed to execute Cargo"),
                "{stderr}"
            );
            assert!(!self.0.join("build/main/src/main.rs").exists());
        }
    }

    fn run_high_and_saved_low(&self, expected: &str) {
        let _guard = NATIVE_RUN.lock().unwrap();
        let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("native-target")
            });
        for source in ["main.nagi", "saved.low"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&self.0)
                .args(["run", source, "--out", "build", "--no-project"])
                .env("NAGI_NATIVE_TARGET_DIR", &target)
                .env("CARGO_NET_OFFLINE", "true")
                .output()
                .unwrap();
            let stderr = String::from_utf8(output.stderr.clone()).unwrap();
            let launcher = stderr
                .lines()
                .find(|line| line.starts_with("native: "))
                .unwrap();
            let application = successful(output);
            let executable = launcher
                .trim_end_matches('\r')
                .strip_prefix("native: ")
                .unwrap();
            assert!(Path::new(executable).is_file(), "{launcher}");
            assert_eq!(application.trim(), expected, "{source}");
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn successful(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

const APP: &str = r#"import std.http.server as http
from std.http.server import Status as Code
from std.http.server import Request as Input, Response as Output
class State:
    label: str
    count: i64
class StoredRequest:
    input: Input
class StoredResponse:
    output: Output
enum Failure:
    Denied
    Unavailable
def default_error(problem: Failure) -> Output:
    match problem:
        case Failure.Denied:
            return http.text(Code.UNAUTHORIZED, "denied")
        case Failure.Unavailable:
            return http.empty(Code.SERVICE_UNAVAILABLE)
def route_error(problem: str) -> Output:
    return http.text(Code.FORBIDDEN, "overridden")
def store(request: Input) -> StoredRequest:
    return StoredRequest(input=request)
def direct_phrase() -> view[str]:
    return Code.OK.phrase
def bound_phrase() -> view[str]:
    phrase = Code.OK.phrase
    return phrase
def copied_owned_status(code: view[owned[Code]]) -> owned[Code]:
    return copy(code)
def owned_status_value(code: view[owned[Code]]) -> i64:
    return copied_owned_status(code).value
async def handler(request: Input, state: shared[State], authority: unit) -> Result[Output, Failure]:
    await sleep(1)
    assert_true(request.method == http.Method.GET)
    assert_true(request.is_get)
    return ok(http.text(Code.OK, view(state.label)))
def json_request(request: view[Input]) -> Result[bool, Error]:
    return http.is_json_content_type(request)
def json_then_store(request: Input) -> Result[StoredRequest, Error]:
    matches = try http.is_json_content_type(view(request))
    print(matches)
    return ok(StoredRequest(input=request))
async def independent_error(request: Input, state: shared[State], authority: unit) -> Result[Output, str]:
    return fail("private error")
async def main() -> Result[unit, Error]:
    app = http.app[State, Failure](State(label="first", count=7), default_error)
    app = try http.route(app, http.Method.GET, "/", http.public_policy[State](), handler)
    selected = independent_error
    app = try http.route_mapped(app, http.Method.POST, "/special", http.public_policy[State](), selected, route_error)
    other = http.app_default[State](State(label="second", count=9))
    return ok(print("HTTP application checked"))
def constant_method_name() -> view[str]:
    name = http.method_name(http.Method.GET)
    return name
def viewed_constant_method_name() -> view[str]:
    return http.method_name(view(http.Method.POST))
"#;

#[test]
fn authority_configuration_preserves_option_move_and_list_types_before_backend() {
    let f = Fixture::new();
    f.rejected("import std.http.server as http\ndef setup() -> Result[http.Options, Error]:\n    limits = http.default_options()\n    checked = try http.authority(limits, \"https://localhost\", [\"localhost\"], 2, 128)\n    return http.trusted_proxy(limits, [\"127.0.0.1\"])\n", 5);
    f.rejected("import std.http.server as http\ndef setup() -> Result[http.Options, Error]:\n    return http.authority(http.default_options(), \"https://localhost\", [1], 2, 128)\n", 3);
    f.rejected("import std.http.server as http\ndef setup() -> Result[http.Options, Error]:\n    return http.trusted_proxy(http.default_options(), [1])\n", 3);
}

#[test]
fn standard_aliases_are_registry_resources_and_roundtrip_independently() {
    let f = Fixture::new();
    f.write("main.nagi", APP);
    let high = f.checked("main.nagi");
    for alias in ["http.Request", "Input", "http.Response", "Output", "Code"] {
        let definition = high.program.modules.resolve_root_path(alias).unwrap();
        assert_eq!(definition.id.module.0, "stdlib:std.http.server");
        assert_eq!(definition.id.kind, DefKind::Resource);
        assert!(!high
            .program
            .classes
            .iter()
            .any(|class| class.name == definition.symbol));
    }
    assert_eq!(
        high.program
            .modules
            .resolve_root_path("http.Status")
            .unwrap()
            .id,
        high.program.modules.resolve_root_path("Code").unwrap().id
    );
    modules::validate(&high.program).unwrap();
    let saved = f.roundtrip();
    assert_eq!(
        saved.program.modules.resolve_root_path("Input").unwrap().id,
        high.program.modules.resolve_root_path("Input").unwrap().id
    );
}

#[test]
fn standard_resolution_ignores_cwd_files_and_unsaved_lookalikes() {
    let f = Fixture::new();
    f.write("std/http/server.nagi", "this is not a language module\n");
    f.write("std.http.server.nagi", "class Request:\n    forged: i64\n");
    f.write("main.nagi", APP);
    let mut overlays = HashMap::new();
    overlays.insert(
        fs::canonicalize(f.0.join("std/http/server.nagi")).unwrap(),
        "class Request:\n    spoofed: str\n".into(),
    );
    let mut loaded = source::load_with_overlays(&f.0.join("main.nagi"), true, &overlays).unwrap();
    check::check(&mut loaded.program).unwrap();
    assert_eq!(
        loaded
            .program
            .modules
            .resolve_root_path("Input")
            .unwrap()
            .id
            .module
            .0,
        "stdlib:std.http.server"
    );
    assert!(!loaded.program.classes.iter().any(|class| class
        .fields
        .iter()
        .any(|(name, _)| name == "spoofed" || name == "forged")));
    for import in [
        "import std.http.unknown as http\n",
        "from std.http.server import Imaginary\n",
    ] {
        f.rejected(import, 1);
    }
}

#[test]
fn ordinary_classes_and_local_shadows_cannot_impersonate_standard_resources() {
    let f = Fixture::new();
    f.rejected("from std.http.server import Request as Input\nclass Request:\n    value: i64\ndef wrong(request: Request) -> Input:\n    return request\n", 5);
    f.rejected("import std.http.server as http\ndef main():\n    http = 7\n    response = http.empty(http.Status.OK)\n", 4);
    f.rejected("from std.http.server import Status as Code\ndef main():\n    Code = 7\n    status = Code.OK\n", 4);
    f.write("main.nagi", "from std.http.server import Status as Code\nclass Status:\n    value: i64\ndef read(code: Code) -> i64:\n    return code.value\ndef main():\n    ordinary = Status(value=7)\n    print(read(Code.OK) + ordinary.value)\n");
    f.roundtrip();
}

#[test]
fn saved_low_cannot_change_registry_identity_or_define_resource_implementations() {
    let f = Fixture::new();
    f.write("main.nagi", APP);
    let high = f.checked("main.nagi");
    let low = emit::low(&high.program);
    let header = low
        .lines()
        .find_map(|line| line.strip_prefix("# nagi-modules-v1 "))
        .unwrap();
    let metadata: serde_json::Value = serde_json::from_str(header).unwrap();
    let index = metadata["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|definition| {
            definition["id"]["module"] == "stdlib:std.http.server"
                && definition["id"]["name"] == "Request"
        })
        .unwrap();
    for (field, value) in [
        ("name", "ForgedRequest"),
        ("kind", "Class"),
        ("module", "stdlib:std.http.evil"),
    ] {
        let mut corrupt = metadata.clone();
        corrupt["definitions"][index]["id"][field] = value.into();
        let changed = low
            .lines()
            .map(|line| {
                if line.starts_with("# nagi-modules-v1 ") {
                    format!("# nagi-modules-v1 {corrupt}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        f.write("corrupt.low", &changed);
        assert!(
            source::load(&f.0.join("corrupt.low"), false).is_err(),
            "forged {field} passed the Low loader"
        );
    }
    let request = high.program.modules.resolve_root_path("Input").unwrap();
    f.write(
        "corrupt.low",
        &format!("{low}\nclass {} {{ forged: i64; }}\n", request.symbol),
    );
    assert!(source::load(&f.0.join("corrupt.low"), false).is_err());
}

#[test]
fn status_method_and_db_free_app_execute_from_high_and_saved_low() {
    let f = Fixture::new();
    let source = APP.replace(
        "    return ok(print(\"HTTP application checked\"))",
        r#"    status = try http.status(599)
    assert_true(status.value == 599)
    assert_true(status.phrase == "")
    assert_true(status.is_server_error)
    assert_true(not status.is_success)
    assert_true(Code.OK.value == 200)
    assert_true(direct_phrase() == "OK")
    assert_true(bound_phrase() == "OK")
    assert_true(constant_method_name() == "GET")
    assert_true(viewed_constant_method_name() == "POST")
    assert_true(Code.CREATED.is_success)
    assert_true(Code.NOT_MODIFIED.is_redirection)
    assert_true(Code.UNAUTHORIZED.is_client_error)
    assert_true(Code.INTERNAL_SERVER_ERROR.is_server_error)
    assert_true(Code.CONTENT_TOO_LARGE == Code.PAYLOAD_TOO_LARGE)
    assert_true(Code.UNPROCESSABLE_CONTENT == Code.UNPROCESSABLE_ENTITY)
    codes = [Code.OK, Code.CREATED, Code.NOT_FOUND]
    first = codes[0]
    assert_true(first.value == 200)
    total = 0
    for code in codes:
        response = http.empty(code)
        total += response.status.value
    assert_true(total == 805)
    named = try http.method("PROPFIND")
    assert_true(http.method_name(view(named)) == "PROPFIND")
    response = http.text(Code.CREATED, "created")
    assert_true(response.status.value == 201)
    assert_true(len(response.body) == 7)
    stored = StoredResponse(output=http.empty(Code.NO_CONTENT))
    assert_true(stored.output.status.value == 204)
    encoded = try http.json[i64](Code.OK, 42)
    assert_true(len(encoded.body) == 2)
    opts = try http.options(1024, 1000, 500, 1000)
    opts = try http.capacity(opts, 32, 8)
    opts = try http.header_timeout(opts, 200)
    for invalid in [0, 99, 100, 101, 199, 600, 9223372036854775807]:
        match http.status(invalid):
            case Ok(_):
                assert_true(False)
            case Err(_):
                print("invalid status rejected")
    return ok(print("HTTP application checked"))"#,
    );
    f.write("main.nagi", &source);
    f.roundtrip();
    f.run_high_and_saved_low("invalid status rejected\ninvalid status rejected\ninvalid status rejected\ninvalid status rejected\ninvalid status rejected\ninvalid status rejected\ninvalid status rejected\nHTTP application checked");
}

#[test]
fn handlers_require_exact_async_request_shared_state_and_error_contracts() {
    let f = Fixture::new();
    for (changed, expected) in [
        (
            APP.replace("async def handler(", "def handler("),
            "名前付きasync関数",
        ),
        (
            APP.replace(
                "request: Input, state: shared[State]",
                "request: Input, state: State",
            ),
            "expected shared[",
        ),
        (
            APP.replace(
                "request: Input, state: shared[State]",
                "request: view[Input], state: shared[State]",
            ),
            "got view[stdlib:std.http.server::Request]",
        ),
        (
            APP.replace(
                "http.Method.GET, \"/\", http.public_policy[State](), handler",
                "http.Method.GET, \"/\", http.public_policy[State](), independent_error",
            ),
            "got str",
        ),
        (
            APP.replace("selected, route_error", "selected, default_error"),
            "expected fn[str, stdlib:std.http.server::Response]",
        ),
    ] {
        // A sync function must not fail merely because its body still awaits.
        let changed = if changed.contains("def handler(") && !changed.contains("async def handler(")
        {
            changed.replace("    await sleep(1)\n", "")
        } else {
            changed
        };
        f.write("main.nagi", &changed);
        let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
        let error = check::check(&mut loaded.program).expect_err("accepted wrong handler");
        assert!(error.contains(expected), "{error}\n{changed}");
    }
}

#[test]
fn apps_validate_generic_arity_owned_state_and_mapper_error_type() {
    let f = Fixture::new();
    for changed in [
        APP.replace("http.app[State, Failure]", "http.app[State]"),
        APP.replace("http.app[State, Failure]", "http.app[State, Failure, i64]"),
        APP.replace(
            "State(label=\"first\", count=7), default_error",
            "7, default_error",
        ),
        APP.replace(
            "State(label=\"first\", count=7), default_error",
            "State(label=\"first\", count=7), route_error",
        ),
        APP.replace(
            "app = try http.route(app,",
            "used = app\n    app = try http.route(app,",
        ),
    ] {
        f.write("main.nagi", &changed);
        let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
        assert!(
            check::check(&mut loaded.program).is_err(),
            "accepted invalid app:\n{changed}"
        );
    }
    f.rejected(
        "import std.http.server as http\ndef bad(value: http.App[i64]) -> i64:\n    return 0\n",
        2,
    );
    f.rejected(
        "import std.http.server as http\ndef bad(value: http.Request[i64]) -> i64:\n    return 0\n",
        2,
    );
    f.rejected(
        "import std.http.server as http\ndef mapped(problem: Error) -> http.Response:\n    return http.empty(http.Status.BAD_REQUEST)\ndef main():\n    state = \"borrowed\"\n    app = http.app[view[str], Error](view(state), mapped)\n",
        6,
    );
    f.rejected(
        "import std.http.server as http\ndef bad(app: http.App[view[str], Error]):\n    print(0)\n",
        2,
    );
}

#[test]
fn borrowed_option_temporaries_allow_arm_local_use_but_cannot_escape() {
    let f = Fixture::new();
    f.write("main.nagi", "def make() -> str:\n    return \"temporary\"\ndef render() -> str:\n    match some(view(make())):\n        case Some(part):\n            alias = part\n            return copy(alias)\n        case None:\n            return \"absent\"\ndef main():\n    assert_true(render() == \"temporary\")\n    print(\"temporary match copied\")\n");
    f.roundtrip();
    f.run_high_and_saved_low("temporary match copied");
    for source in [
        "def make() -> str:\n    return \"temporary\"\ndef escape(fallback: view[str]) -> view[str]:\n    match some(view(make())):\n        case Some(part):\n            return part\n        case None:\n            return fallback\n",
        "def make() -> str:\n    return \"temporary\"\ndef main():\n    original = \"owned\"\n    saved = view(original)\n    match some(view(make())):\n        case Some(part):\n            saved = part\n        case None:\n            print(0)\n    print(saved)\n",
        "def make() -> str:\n    return \"temporary\"\ndef main():\n    saved: List[view[str]] = []\n    match some(view(make())):\n        case Some(part):\n            append(saved, part)\n        case None:\n            print(0)\n    print(saved[0])\n",
    ] {
        f.write("main.nagi", source);
        let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
        let error = check::check(&mut loaded.program).expect_err("temporary match view escaped its arm");
        let diagnostic = loaded.diagnostic(&error);
        assert!(diagnostic.contains("main.nagi:") && diagnostic.contains("view"), "{diagnostic}");
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&f.0).args(["check", "main.nagi", "--no-project"])
            .env("PATH", "").output().unwrap();
        assert!(!output.status.success(), "CLI accepted an escaping Option view");
    }
}

#[test]
fn resource_construction_serialization_and_shared_field_moves_fail_before_build() {
    let f = Fixture::new();
    for (source, line) in [
        ("import std.http.server as http\ndef main():\n    request = http.Request()\n", 3),
        ("import std.http.server as http\ndef main():\n    code = http.Status(value=200)\n", 3),
        ("import std.http.server as http\ndef main() -> Result[unit, Error]:\n    encoded = try json_encode(http.empty(http.Status.OK))\n    return ok(print(encoded))\n", 3),
        ("import std.http.server as http\nclass Wrapped:\n    output: http.Response\ndef main() -> Result[unit, Error]:\n    encoded = try json_encode(Wrapped(output=http.empty(http.Status.OK)))\n    return ok(print(encoded))\n", 5),
        ("import std.http.server as http\nclass Wrapped:\n    output: http.Response\ndef main() -> Result[unit, Error]:\n    encoded = try http.json[Wrapped](http.Status.OK, Wrapped(output=http.empty(http.Status.OK)))\n    return ok(print(encoded.status.value))\n", 5),
        ("class State:\n    label: str\ndef take(state: shared[State]) -> str:\n    return state.label\n", 4),
        ("import std.http.server as http\ndef bad(response: http.Response) -> Result[http.Response, Error]:\n    return http.append_header(response, \"X-Demo\", response.body)\n", 3),
        ("import std.http.server as http\ndef bad(response: http.Response) -> Result[http.Response, Error]:\n    return http.append_header_text(response, \"X-Demo\", response.status.phrase)\n", 3),
    ] {
        f.rejected(source, line);
    }
}

#[test]
fn resource_references_do_not_acquire_legacy_slice_operations() {
    let f = Fixture::new();
    for (text, line) in [
        ("import std.http.server as http\ndef bad(value: view[http.Status]) -> i64:\n    return len(value)\n", 3),
        ("import std.http.server as http\ndef bad(value: view[http.Status]) -> http.Status:\n    return value[0]\n", 3),
        ("import std.http.server as http\ndef bad(value: view[http.Status]):\n    for code in value:\n        print(code.value)\n", 3),
        ("import std.http.server as http\ndef bad(value: view[http.Method]) -> Result[view[http.Method], Error]:\n    return slice(value, 0, 1)\n", 3),
        ("import std.http.server as http\ndef main():\n    codes = [http.Status.OK]\n    values = view(codes)\n", 4),
        ("import std.http.server as http\ndef bad(value: view[owned[http.Status]]) -> i64:\n    return len(value)\n", 3),
        ("import std.http.server as http\ndef bad(value: view[owned[http.Status]]) -> owned[http.Status]:\n    return value[0]\n", 3),
        ("import std.http.server as http\ndef bad(value: view[owned[http.Status]]):\n    for code in value:\n        print(code.value)\n", 3),
        ("import std.http.server as http\ndef bad(value: view[owned[http.Method]]) -> Result[view[owned[http.Method]], Error]:\n    return slice(value, 0, 1)\n", 3),
    ] {
        f.rejected(text, line);
    }
}

#[test]
fn json_content_type_requires_a_request_and_has_no_generic_arguments() {
    let f = Fixture::new();
    for source in [
        "import std.http.server as http\ndef bad() -> Result[bool, Error]:\n    return http.is_json_content_type(42)\n",
        "import std.http.server as http\ndef bad(request: http.Request) -> Result[bool, Error]:\n    return http.is_json_content_type[i64](view(request))\n",
        "import std.http.server as http\ndef bad(request: http.Request) -> Result[bool, Error]:\n    return http.is_json_content_type(view(request), \"application/json\")\n",
    ] {
        f.rejected(source, 3);
    }
}

#[test]
fn native_method_comparisons_borrow_but_assignment_tracks_request_partial_moves() {
    let f = Fixture::new();
    f.write("main.nagi", "import std.http.server as http\ndef read(request: http.Request) -> bool:\n    same = request.method == http.Method.GET\n    name = http.method_name(view(request.method))\n    path = request.path\n    return same and name == \"GET\" and path == \"/\"\n");
    f.roundtrip();
    for getter in [
        "request.path",
        "request.query",
        "request.body",
        "request.is_get",
        "http.header_text(view(request), \"Authorization\")",
    ] {
        f.rejected(&format!("import std.http.server as http\ndef read(request: http.Request):\n    method = request.method\n    value = {getter}\n"), 4);
    }
    f.rejected("import std.http.server as http\ndef take(request: http.Request):\n    print(0)\ndef read(request: http.Request):\n    path = request.path\n    take(request)\n    print(path)\n", 6);
}

#[test]
fn header_views_depend_on_request_owner_and_not_the_name_temporary() {
    let f = Fixture::new();
    f.write("main.nagi", "import std.http.server as http\ndef read(request: view[http.Request]) -> Result[List[view[bytes]], Error]:\n    return http.headers(request, \"Authorization\")\ndef inspect(request: http.Request) -> Result[unit, Error]:\n    name = \"Authorization\"\n    values = try http.headers(view(request), view(name))\n    name = \"changed\"\n    print(len(view(values)))\n    return ok(print(request.path))\n");
    f.roundtrip();
    f.rejected("import std.http.server as http\ndef take(request: http.Request):\n    print(0)\ndef inspect(request: http.Request) -> Result[unit, Error]:\n    values = try http.headers(view(request), \"Authorization\")\n    take(request)\n    return ok(print(len(view(values))))\n", 6);
    f.rejected("import std.http.server as http\ndef escape(request: http.Request) -> view[str]:\n    return request.path\n", 3);
}

#[test]
fn method_name_views_can_return_from_borrowed_request_parameters() {
    let f = Fixture::new();
    f.write("main.nagi", "import std.http.server as http\ndef name(request: view[http.Request]) -> view[str]:\n    return http.method_name(view(request.method))\ndef direct_name(request: view[http.Request]) -> view[str]:\n    return http.method_name(request.method)\n");
    f.roundtrip();
    for body in [
        "http.method_name(view(request.method))",
        "http.method_name(request.method)",
    ] {
        f.rejected(&format!("import std.http.server as http\ndef name(request: http.Request) -> view[str]:\n    return {body}\n"), 3);
    }
    f.rejected("import std.http.server as http\ndef name() -> view[str]:\n    method = http.Method.GET\n    return http.method_name(view(method))\n", 4);
}

#[test]
fn optional_header_and_query_views_keep_the_request_borrow_through_match() {
    let f = Fixture::new();
    f.write("main.nagi", "import std.http.server as http\ndef inspect(request: http.Request) -> Result[unit, Error]:\n    name = \"Authorization\"\n    header = try http.header_text(view(request), view(name))\n    name = \"changed\"\n    match header:\n        case Some(value):\n            print(value)\n        case None:\n            print(\"absent\")\n    match request.query:\n        case Some(value):\n            print(value)\n        case None:\n            print(\"no query\")\n    return ok(print(request.path))\n");
    f.roundtrip();
    f.rejected("import std.http.server as http\ndef take(request: http.Request):\n    print(0)\ndef inspect(request: http.Request) -> Result[unit, Error]:\n    header = try http.header_text(view(request), \"Authorization\")\n    match header:\n        case Some(value):\n            take(request)\n            print(value)\n        case None:\n            print(\"absent\")\n    return ok(print(\"done\"))\n", 8);
    f.rejected("import std.http.server as http\ndef take(request: http.Request):\n    print(0)\ndef inspect(request: http.Request):\n    match request.query:\n        case Some(value):\n            take(request)\n            print(value)\n        case None:\n            print(\"absent\")\n", 7);
}
