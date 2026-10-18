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
def default_error(context: view[http.ResponseContext], problem: Failure) -> Result[Output, Error]:
    match problem:
        case Failure.Denied:
            return http.text(context, Code.UNAUTHORIZED, "denied")
        case Failure.Unavailable:
            return http.empty(context, Code.SERVICE_UNAVAILABLE)
def route_error(context: view[http.ResponseContext], problem: str) -> Result[Output, Error]:
    return http.text(context, Code.FORBIDDEN, view(problem))
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
    context = http.response_context(request)
    match http.text(context, Code.OK, state.label):
        case Ok(response):
            return ok(response)
        case Err(_):
            return fail(Failure.Unavailable)
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
    f.rejected("import std.http.server as http\ndef shadowed(context: view[http.ResponseContext], status: http.Status):\n    http = 7\n    response = http.empty(context, status)\n", 4);
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
fn u4_response_context_fixtures_have_valid_high_and_low_syntax() {
    for (relative, high) in [
        ("http-response-context-red.nagi", true),
        ("http-response-context-red.low", false),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/resource-contract")
            .join(relative);
        let text = fs::read_to_string(&path).unwrap();
        parser::parse(&text, high).unwrap_or_else(|error| {
            panic!("{} must be syntactically valid: {error}", path.display())
        });
    }
}

#[test]
fn u4_explicit_response_limits_check_high_and_handwritten_low() {
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    for (name, high, text) in [
        ("limits.nagi", true, "import std.http.server as http\ndef setup() -> Result[http.Options, Error]:\n    limits = try http.response_limits(256, 128, 4, 512, 12, 128, 1, 131072, 1, 131072, 512, 12)\n    return http.with_response_limits(http.default_options(), limits)\n"),
        ("limits.low", false, "import std.http.server as http;\nfn setup() -> Result[http.Options, Error] {\n    let limits = try http.response_limits(256, 128, 4, 512, 12, 128, 1, 131072, 1, 131072, 512, 12);\n    return http.with_response_limits(http.default_options(), limits);\n}\n"),
    ] {
        parser::parse(text, high).expect("limits fixture syntax must precede registry checks");
        fixture.write(name, text);
        match source::load(&fixture.0.join(name), high) {
            Ok(mut loaded) => {
                if let Err(error) = check::check(&mut loaded.program) {
                    failures.push(loaded.diagnostic(&error));
                } else if high {
                    let low = emit::low(&loaded.program);
                    fixture.write("limits-saved.low", &low);
                    fixture.checked("limits-saved.low");
                }
            }
            Err(error) => failures.push(error),
        }
    }
    assert!(
        failures.is_empty(),
        "explicit response limits API: {failures:?}"
    );
}

#[test]
fn u5_explicit_rate_settings_keep_app_generic_identity_in_three_forms() {
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    for (name, high, text) in [
        ("rate-settings.nagi", true, "import std.http.server as http\ndef settings() -> Result[http.Options, Error]:\n    limits = try http.rate_limits(4, 1, 2, 131072, 4096)\n    return http.with_rate_limits(http.default_options(), limits)\ndef bind(app: http.App[i64, Error]) -> Result[http.App[i64, Error], Error]:\n    policy = try http.rate_policy(2, 1000)\n    policy = try http.rate_subject(policy, 2, 1000, 2, 1000)\n    return http.rate_route(app, http.Method.GET, \"/\", policy)\n"),
        ("rate-settings.low", false, "import std.http.server as http;\nfn settings() -> Result[http.Options, Error] {\n    let limits = try http.rate_limits(4, 1, 2, 131072, 4096);\n    return http.with_rate_limits(http.default_options(), limits);\n}\nfn bind(app: http.App[i64, Error]) -> Result[http.App[i64, Error], Error] {\n    let policy = try http.rate_policy(2, 1000);\n    policy = try http.rate_subject(policy, 2, 1000, 2, 1000);\n    return http.rate_route(app, http.Method.GET, \"/\", policy);\n}\n"),
    ] {
        parser::parse(text, high).expect("rate configuration wrappers must parse");
        fixture.write(name, text);
        match source::load(&fixture.0.join(name), high) {
            Ok(mut loaded) => match check::check(&mut loaded.program) {
                Ok(()) if high => {
                    fixture.write("rate-settings-saved.low", &emit::low(&loaded.program));
                    fixture.checked("rate-settings-saved.low");
                }
                Ok(()) => {}
                Err(error) => failures.push(loaded.diagnostic(&error)),
            },
            Err(error) => failures.push(error),
        }
    }
    assert!(failures.is_empty(), "explicit rate API: {failures:?}");
}

#[test]
fn u5_rate_attachment_rejects_spoofed_apps_wrong_errors_and_policy_reuse() {
    let fixture = Fixture::new();
    for (name, high, text, call_line, reason) in [
        ("rate-fake-app.nagi", true, "import std.http.server as http\nclass App:\n    value: i64\ndef bind(app: App, policy: http.RatePolicy) -> Result[http.App[i64, Error], Error]:\n    return http.rate_route(app, http.Method.GET, \"/\", policy)\n", 5, "app"),
        ("rate-fake-app.low", false, "import std.http.server as http;\nrecord App { value: i64; }\nfn bind(app: App, policy: http.RatePolicy) -> Result[http.App[i64, Error], Error] {\n    return http.rate_route(app, http.Method.GET, \"/\", policy);\n}\n", 4, "app"),
        ("rate-wrong-error.nagi", true, "import std.http.server as http\ndef bind(app: http.App[i64, str], policy: http.RatePolicy) -> Result[http.App[i64, Error], Error]:\n    return http.rate_route(app, http.Method.GET, \"/\", policy)\n", 3, "error"),
        ("rate-wrong-error.low", false, "import std.http.server as http;\nfn bind(app: http.App[i64, str], policy: http.RatePolicy) -> Result[http.App[i64, Error], Error] {\n    return http.rate_route(app, http.Method.GET, \"/\", policy);\n}\n", 3, "error"),
        ("rate-reused-policy.nagi", true, "import std.http.server as http\ndef bind(app: http.App[i64, Error], policy: http.RatePolicy) -> Result[http.App[i64, Error], Error]:\n    changed = try http.rate_subject(policy, 2, 1000, 2, 1000)\n    return http.rate_route(app, http.Method.GET, \"/\", policy)\n", 4, "move"),
        ("rate-reused-policy.low", false, "import std.http.server as http;\nfn bind(app: http.App[i64, Error], policy: http.RatePolicy) -> Result[http.App[i64, Error], Error] {\n    let changed = try http.rate_subject(policy, 2, 1000, 2, 1000);\n    return http.rate_route(app, http.Method.GET, \"/\", policy);\n}\n", 4, "move"),
    ] {
        parser::parse(text, high).expect("negative rate fixture must parse");
        fixture.write(name, text);
        let mut loaded = source::load(&fixture.0.join(name), high)
            .unwrap_or_else(|error| panic!("{name} must resolve before rate rejection: {error}"));
        let error = check::check(&mut loaded.program)
            .expect_err("invalid rate attachment must fail in the checker");
        let diagnostic = loaded.diagnostic(&error);
        assert!(diagnostic.contains(&format!("{name}:{call_line}")), "{diagnostic}");
        let correct_cause = match reason {
            "app" => error.contains("rate_routeには標準App[State,E]が必要です"),
            "error" => error.contains("str") && error.contains("Error") && error.contains("型"),
            "move" => error.contains("policy") && error.contains("move"),
            _ => unreachable!(),
        };
        assert!(correct_cause, "{name}: unexpected rejection: {diagnostic}");
    }
}

#[test]
fn u4_typed_html_uses_the_same_borrowed_response_context() {
    let fixture = Fixture::new();
    for (name, high, text) in [
        ("html-context.nagi", true, "import std.html as html\nimport std.http.server as http\ndef page(context: view[http.ResponseContext], document: html.HtmlDocument) -> Result[http.Response, Error]:\n    return http.html_response(context, http.Status.OK, document)\n"),
        ("html-context.low", false, "import std.html as html;\nimport std.http.server as http;\nfn page(context: view[http.ResponseContext], document: html.HtmlDocument) -> Result[http.Response, Error] {\n    return http.html_response(context, http.Status.OK, document);\n}\n"),
    ] {
        parser::parse(text, high).expect("typed HTML Context fixture must parse");
        fixture.write(name, text);
        let checked = fixture.checked(name);
        if high {
            fixture.write("html-context-saved.low", &emit::low(&checked.program));
            fixture.checked("html-context-saved.low");
        }
    }
}

#[test]
fn u4_response_limits_reject_wrong_types_arity_and_reuse_before_backend() {
    let fixture = Fixture::new();
    for (name, high, text, call_line, reason) in [
        ("wrong-type.nagi", true, "import std.http.server as http\ndef setup() -> Result[http.ResponseLimits, Error]:\n    return http.response_limits(true, 128, 4, 512, 12, 128, 1, 131072, 1, 131072, 512, 12)\n", 3, "type"),
        ("wrong-type.low", false, "import std.http.server as http;\nfn setup() -> Result[http.ResponseLimits, Error] {\n    return http.response_limits(true, 128, 4, 512, 12, 128, 1, 131072, 1, 131072, 512, 12);\n}\n", 3, "type"),
        ("missing-limit.nagi", true, "import std.http.server as http\ndef setup() -> Result[http.ResponseLimits, Error]:\n    return http.response_limits(256, 128, 4, 512, 12, 128, 1, 131072, 1, 131072, 512)\n", 3, "arity"),
        ("missing-limit.low", false, "import std.http.server as http;\nfn setup() -> Result[http.ResponseLimits, Error] {\n    return http.response_limits(256, 128, 4, 512, 12, 128, 1, 131072, 1, 131072, 512);\n}\n", 3, "arity"),
        ("reused-limit.nagi", true, "import std.http.server as http\ndef setup(limits: http.ResponseLimits) -> Result[http.Options, Error]:\n    first = try http.with_response_limits(http.default_options(), limits)\n    return http.with_response_limits(first, limits)\n", 4, "move"),
        ("reused-limit.low", false, "import std.http.server as http;\nfn setup(limits: http.ResponseLimits) -> Result[http.Options, Error] {\n    let first = try http.with_response_limits(http.default_options(), limits);\n    return http.with_response_limits(first, limits);\n}\n", 4, "move"),
    ] {
        parser::parse(text, high).expect("negative limits fixture must parse");
        fixture.write(name, text);
        let mut loaded = source::load(&fixture.0.join(name), high)
            .unwrap_or_else(|error| panic!("{name} must resolve before rejection: {error}"));
        let error = check::check(&mut loaded.program)
            .expect_err("invalid limits call must fail before backend");
        let diagnostic = loaded.diagnostic(&error);
        assert!(diagnostic.contains(&format!("{name}:{call_line}")), "{diagnostic}");
        let correct_cause = match reason {
            "type" => error.contains("i64") && error.contains("bool"),
            "arity" => error.contains("response_limitsの引数は12個です"),
            "move" => error.contains("limits") && error.contains("move"),
            _ => unreachable!(),
        };
        assert!(correct_cause, "{name}: unexpected rejection: {diagnostic}");
    }
}

#[test]
fn u4_context_first_app_mapper_is_accepted_in_high_and_low() {
    let mut failures = Vec::new();
    for (relative, high) in [
        ("http-response-context-red.nagi", true),
        ("http-response-context-red.low", false),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/resource-contract")
            .join(relative);
        match source::load(&path, high) {
            Ok(mut loaded) => {
                if let Err(error) = check::check(&mut loaded.program) {
                    failures.push(format!("{relative}: {}", loaded.diagnostic(&error)));
                }
            }
            Err(error) => failures.push(format!("{relative}: {error}")),
        }
    }
    assert!(
        failures.is_empty(),
        "U4 app mapper API must resolve after Context-first factories in High and Low:\n{}",
        failures.join("\n")
    );
}

#[test]
fn legacy_app_mapper_gets_context_migration_guidance_in_high_and_low() {
    let f = Fixture::new();
    for (name, high, text, call_line) in [
        (
            "legacy-app.nagi",
            true,
            "import std.http.server as http\nclass State:\n    count: i64\ndef old_mapper(problem: Error) -> Result[http.Response, Error]:\n    return fail(problem)\ndef setup(state: State) -> Result[http.App[State, Error], Error]:\n    return http.app[State, Error](state, old_mapper)\n",
            7,
        ),
        (
            "legacy-app.low",
            false,
            "import std.http.server as http;\nrecord State { count: i64; }\nfn old_mapper(problem: Error) -> Result[http.Response, Error] { return fail(problem); }\nfn setup(state: State) -> Result[http.App[State, Error], Error] { return http.app[State, Error](state, old_mapper); }\n",
            4,
        ),
    ] {
        f.write(name, text);
        let mut loaded = source::load(&f.0.join(name), high)
            .unwrap_or_else(|error| panic!("{name} must load before mapper rejection: {error}"));
        let error = check::check(&mut loaded.program)
            .expect_err("legacy one-argument App mapper must be rejected");
        assert!(
            error.contains("旧App mapperは単一引数")
                && error.contains("view[ResponseContext]")
                && error.contains("Result[Response, Error]"),
            "{name}: expected concrete Context-first migration guidance: {error}"
        );
        let diagnostic = loaded.diagnostic(&error);
        assert!(
            diagnostic.contains(&format!("{name}:{call_line}")),
            "{name}: expected mapper diagnostic at App call line {call_line}: {diagnostic}"
        );
    }
}

#[test]
fn route_mapped_uses_context_first_result_mapper_in_high_and_low() {
    let f = Fixture::new();
    for (name, high, text) in [
        (
            "route-mapped.nagi",
            true,
            r#"import std.http.server as http

class State:
    value: i64

async def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, str]:
    return fail("failure")

def mapper(context: view[http.ResponseContext], problem: str) -> Result[http.Response, Error]:
    return http.text(context, http.Status.INTERNAL_SERVER_ERROR, view(problem))

def register(state: State) -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](state)
    return http.route_mapped(app, http.Method.POST, "/", http.public_policy[State](), handler, mapper)
"#,
        ),
        (
            "route-mapped.low",
            false,
            r#"import std.http.server as http;
record State { value: i64; }
async fn handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, str] { return fail("failure"); }
fn mapper(context: view[http.ResponseContext], problem: str) -> Result[http.Response, Error] { return http.text(context, http.Status.INTERNAL_SERVER_ERROR, view(problem)); }
fn register(state: State) -> Result[http.App[State, Error], Error] {
    let app = http.app_default[State](state);
    return http.route_mapped(app, http.Method.POST, "/", http.public_policy[State](), handler, mapper);
}
"#,
        ),
    ] {
        f.write(name, text);
        let mut loaded = source::load(&f.0.join(name), high)
            .unwrap_or_else(|error| panic!("{name} must load before route mapper check: {error}"));
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{name}: {}", loaded.diagnostic(&error)));
    }
}

#[test]
fn legacy_route_mapped_mapper_gets_specific_context_migration_guidance_in_high_and_low() {
    let f = Fixture::new();
    for (name, high, text, call_line) in [
        (
            "legacy-route-mapped.nagi",
            true,
            r#"import std.html as html
import std.http.server as http

class State:
    value: i64

async def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, str]:
    return fail("failure")

def error_document() -> html.HtmlDocument:
    match html.policy("https://example.invalid"):
        case Ok(policy):
            match html.document(view(policy), "Error", html.empty(view(policy))):
                case Ok(document):
                    return document
                case Err(_):
                    return error_document()
        case Err(_):
            return error_document()

def legacy_mapper(problem: str) -> http.Response:
    return legacy_mapper(problem)

def register(state: State) -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](state)
    return http.route_mapped(app, http.Method.POST, "/", http.public_policy[State](), handler, legacy_mapper)
"#,
            26,
        ),
        (
            "legacy-route-mapped.low",
            false,
            r#"import std.html as html;
import std.http.server as http;
record State { value: i64; }
async fn handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, str] { return fail("failure"); }
fn error_document() -> html.HtmlDocument {
    match html.policy("https://example.invalid") {
        case Ok(policy) {
            match html.document(view(policy), "Error", html.empty(view(policy))) {
                case Ok(document) { return document; }
                case Err(_) { return error_document(); }
            }
        }
        case Err(_) { return error_document(); }
    }
}
fn legacy_mapper(problem: str) -> http.Response { return legacy_mapper(problem); }
fn register(state: State) -> Result[http.App[State, Error], Error] {
    let app = http.app_default[State](state);
    return http.route_mapped(app, http.Method.POST, "/", http.public_policy[State](), handler, legacy_mapper);
}
"#,
            19,
        ),
    ] {
        // The legacy signature needs a well-typed body without a removed
        // unbounded factory. This recursive stub is never executed: rejection
        // must happen at registration, with the mapper migration diagnostic.
        f.write(name, text);
        let mut loaded = source::load(&f.0.join(name), high).unwrap_or_else(|error| {
            panic!("{name} must load before legacy mapper rejection: {error}")
        });
        let error = check::check(&mut loaded.program)
            .expect_err("legacy single-argument route mapper must be rejected");
        assert!(
            error.contains("旧route_mapped mapper fn[F, Response]")
                && error.contains("view[ResponseContext]")
                && error.contains("Result[Response, Error]"),
            "{name}: expected route_mapped-specific migration guidance: {error}"
        );
        let diagnostic = loaded.diagnostic(&error);
        assert!(
            diagnostic.contains(&format!("{name}:{call_line}")),
            "{name}: expected mapper diagnostic at route_mapped call line {call_line}: {diagnostic}"
        );
    }
}

#[test]
fn app_and_route_mapped_accept_local_mapper_aliases_in_high_and_low() {
    let f = Fixture::new();
    let high = r#"import std.http.server as http

class State:
    value: i64

def mapper(context: view[http.ResponseContext], failure: str) -> Result[http.Response, Error]:
    return http.empty(context, http.Status.INTERNAL_SERVER_ERROR)

async def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, str]:
    return fail("failure")

def register(state: State) -> Result[http.App[State, str], Error]:
    mapper_alias = mapper
    app = http.app[State, str](state, mapper_alias)
    return http.route_mapped(app, http.Method.POST, "/", http.public_policy[State](), handler, mapper_alias)
"#;
    f.write("mapper-alias.nagi", high);
    let mut loaded = source::load(&f.0.join("mapper-alias.nagi"), true)
        .expect("High local mapper alias must load");
    check::check(&mut loaded.program)
        .unwrap_or_else(|error| panic!("High local mapper alias: {}", loaded.diagnostic(&error)));

    let low = emit::low(&loaded.program);
    f.write("mapper-alias.low", &low);
    let mut loaded = source::load(&f.0.join("mapper-alias.low"), false)
        .expect("saved Low local mapper alias must load");
    check::check(&mut loaded.program).unwrap_or_else(|error| {
        panic!(
            "saved Low local mapper alias: {}",
            loaded.diagnostic(&error)
        )
    });
}

fn assert_http_mapper_rejected_at_call(
    fixture: &Fixture,
    name: &str,
    high: bool,
    text: &str,
    call_line: usize,
    causes: &[&str],
) {
    fixture.write(name, text);
    let mut loaded = source::load(&fixture.0.join(name), high).unwrap_or_else(|error| {
        panic!("{name} must load before mapper checker rejection: {error}")
    });
    let error =
        check::check(&mut loaded.program).expect_err("invalid mapper unexpectedly passed checking");
    for cause in causes {
        assert!(
            error.contains(cause),
            "{name}: expected `{cause}` at mapper call, got: {error}"
        );
    }
    let diagnostic = loaded.diagnostic(&error);
    assert!(
        diagnostic.contains(&format!("{name}:{call_line}")),
        "{name}: expected mapper checker error at call line {call_line}, got: {diagnostic}"
    );
}

fn assert_http_mapper_rejected_in_high_and_saved_low(
    fixture: &Fixture,
    name: &str,
    high_source: &str,
    high_call_line: usize,
    call_marker: &str,
    causes: &[&str],
) {
    fixture.write(name, high_source);
    let parsed = source::load(&fixture.0.join(name), true)
        .unwrap_or_else(|error| panic!("{name} High source must load: {error}"));
    let low = emit::low(&parsed.program);
    assert_http_mapper_rejected_at_call(fixture, name, true, high_source, high_call_line, causes);
    let low_name = format!("{name}.low");
    let low_call_line = low
        .lines()
        .position(|line| line.contains(call_marker))
        .unwrap_or_else(|| panic!("{name}: generated Low is missing call marker `{call_marker}`"))
        + 1;
    assert_http_mapper_rejected_at_call(fixture, &low_name, false, &low, low_call_line, causes);
}

#[test]
fn http_mapper_borrowed_async_and_result_contracts_reject_at_call_in_high_and_low() {
    let f = Fixture::new();
    let expected = "expected fn[view[stdlib:std.http.server::ResponseContext], str, Result[stdlib:std.http.server::Response, Error]]";
    let borrowed_error = "HTTP mapperのエラー値にviewは保持できません";
    assert_http_mapper_rejected_in_high_and_saved_low(
        &f,
        "app-borrowed-e.nagi",
        r#"import std.http.server as http
class State:
    value: i64
def mapper(context: view[http.ResponseContext], problem: view[str]) -> Result[http.Response, Error]:
    return http.empty(context, http.Status.INTERNAL_SERVER_ERROR)
def register(state: State) -> Result[http.App[State, Error], Error]:
    return http.app[State, view[str]](state, mapper)
"#,
        7,
        "_f_617070[",
        &[borrowed_error],
    );
    assert_http_mapper_rejected_in_high_and_saved_low(
        &f,
        "app-async-mapper.nagi",
        r#"import std.http.server as http
class State:
    value: i64
async def mapper(context: view[http.ResponseContext], problem: str) -> Result[http.Response, Error]:
    return http.empty(context, http.Status.INTERNAL_SERVER_ERROR)
def register(state: State) -> Result[http.App[State, str], Error]:
    return http.app[State, str](state, mapper)
"#,
        7,
        "_f_617070[",
        &[
            expected,
            "Future[Result[stdlib:std.http.server::Response, Error]]",
        ],
    );
    assert_http_mapper_rejected_in_high_and_saved_low(
        &f,
        "app-wrong-result.nagi",
        r#"import std.http.server as http
class State:
    value: i64
def mapper(context: view[http.ResponseContext], problem: str) -> Result[http.Response, str]:
    return fail(problem)
def register(state: State) -> Result[http.App[State, str], Error]:
    return http.app[State, str](state, mapper)
"#,
        7,
        "_f_617070[",
        &[
            expected,
            "got fn[view[stdlib:std.http.server::ResponseContext], str, Result[stdlib:std.http.server::Response, str]]",
        ],
    );
    assert_http_mapper_rejected_in_high_and_saved_low(
        &f,
        "route-async-mapper.nagi",
        r#"import std.http.server as http
class State:
    value: i64
async def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, str]:
    return fail("failure")
async def mapper(context: view[http.ResponseContext], problem: str) -> Result[http.Response, Error]:
    return http.empty(context, http.Status.INTERNAL_SERVER_ERROR)
def register(state: State) -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](state)
    return http.route_mapped(app, http.Method.POST, "/", http.public_policy[State](), handler, mapper)
"#,
        10,
        "_f_726f7574655f6d6170706564(",
        &[
            expected,
            "Future[Result[stdlib:std.http.server::Response, Error]]",
        ],
    );
    assert_http_mapper_rejected_in_high_and_saved_low(
        &f,
        "route-wrong-result.nagi",
        r#"import std.http.server as http
class State:
    value: i64
async def handler(request: http.Request, state: shared[State], authority: unit) -> Result[http.Response, str]:
    return fail("failure")
def mapper(context: view[http.ResponseContext], problem: str) -> Result[http.Response, str]:
    return fail(problem)
def register(state: State) -> Result[http.App[State, Error], Error]:
    app = http.app_default[State](state)
    return http.route_mapped(app, http.Method.POST, "/", http.public_policy[State](), handler, mapper)
"#,
        10,
        "_f_726f7574655f6d6170706564(",
        &[
            expected,
            "got fn[view[stdlib:std.http.server::ResponseContext], str, Result[stdlib:std.http.server::Response, str]]",
        ],
    );
}

#[test]
fn response_context_lookup_preserves_request_borrow_in_high_and_low() {
    let f = Fixture::new();
    for (name, high, text) in [
        (
            "lookup.nagi",
            true,
            "import std.http.server as http\ndef borrow(request: view[http.Request]) -> view[http.ResponseContext]:\n    return http.response_context(request)\n",
        ),
        (
            "lookup.low",
            false,
            "import std.http.server as http;\nfn borrow(request: view[http.Request]) -> view[http.ResponseContext] {\n    return http.response_context(request);\n}\n",
        ),
    ] {
        f.write(name, text);
        let mut loaded = source::load(&f.0.join(name), high)
            .unwrap_or_else(|error| panic!("{name} lookup must resolve: {error}"));
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{name} borrowed lookup must type-check: {error}"));
    }
}

#[test]
fn response_context_from_owned_request_cannot_escape_in_high_and_low() {
    let f = Fixture::new();
    for (name, high, text) in [
        (
            "escape.nagi",
            true,
            "import std.http.server as http\ndef escape(request: http.Request) -> view[http.ResponseContext]:\n    return http.response_context(view(request))\n",
        ),
        (
            "escape.low",
            false,
            "import std.http.server as http;\nfn escape(request: http.Request) -> view[http.ResponseContext] {\n    return http.response_context(view(request));\n}\n",
        ),
    ] {
        f.write(name, text);
        let mut loaded = source::load(&f.0.join(name), high)
            .unwrap_or_else(|error| panic!("{name} owned-request escape negative must resolve: {error}"));
        let error = check::check(&mut loaded.program)
            .expect_err("a request-local ResponseContext view must not escape its owner");
        assert!(
            error.contains("request/local-scoped view escapes its lifetime"),
            "{name}: expected request-local borrow escape rejection, got: {error}"
        );
        let diagnostic = loaded.diagnostic(&error);
        assert!(
            diagnostic.contains(&format!("{name}:3")),
            "{name}: expected the original return line, got: {diagnostic}"
        );
    }
}

#[test]
fn response_factories_accept_context_first_arguments_in_high_and_low() {
    let f = Fixture::new();
    for (name, high, text) in [
        (
            "factories.nagi",
            true,
            "import std.http.server as http\ndef empty(context: view[http.ResponseContext]) -> Result[http.Response, Error]:\n    return http.empty(context, http.Status.OK)\ndef text(context: view[http.ResponseContext], body: view[str]) -> Result[http.Response, Error]:\n    return http.text(context, http.Status.OK, body)\ndef bytes(context: view[http.ResponseContext], body: view[bytes]) -> Result[http.Response, Error]:\n    return http.bytes(context, http.Status.OK, body)\ndef json(context: view[http.ResponseContext], payload: i64) -> Result[http.Response, Error]:\n    return http.json[i64](context, http.Status.OK, payload)\n",
        ),
        (
            "factories.low",
            false,
            "import std.http.server as http;\nfn empty(context: view[http.ResponseContext]) -> Result[http.Response, Error] { return http.empty(context, http.Status.OK); }\nfn text(context: view[http.ResponseContext], body: view[str]) -> Result[http.Response, Error] { return http.text(context, http.Status.OK, body); }\nfn bytes(context: view[http.ResponseContext], body: view[bytes]) -> Result[http.Response, Error] { return http.bytes(context, http.Status.OK, body); }\nfn json(context: view[http.ResponseContext], payload: i64) -> Result[http.Response, Error] { return http.json[i64](context, http.Status.OK, payload); }\n",
        ),
    ] {
        f.write(name, text);
        let mut loaded = source::load(&f.0.join(name), high)
            .unwrap_or_else(|error| panic!("{name} factories must resolve: {error}"));
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{name} Context-first factories: {error}"));
    }
}

#[test]
fn legacy_status_first_factories_fail_with_context_migration_guidance() {
    let f = Fixture::new();
    let calls = [
        (
            "empty",
            "http.empty(http.Status.OK)",
            "http.empty(context, status)",
        ),
        (
            "text",
            "http.text(http.Status.OK, view(\"body\"))",
            "http.text(context, status, body)",
        ),
        (
            "bytes",
            "http.bytes(http.Status.OK, body)",
            "http.bytes(context, status, body)",
        ),
        (
            "json",
            "http.json[i64](http.Status.OK, 42)",
            "http.json[T](context, status, value)",
        ),
    ];
    for (factory, call, migration) in calls {
        for (suffix, high, import, function, line) in [
            (
                "nagi",
                true,
                "import std.http.server as http\n",
                "def old_call(body: view[bytes]) -> Result[http.Response, Error]:\n    return CALL\n",
                3,
            ),
            (
                "low",
                false,
                "import std.http.server as http;\n",
                "fn old_call(body: view[bytes]) -> Result[http.Response, Error] { return CALL; }\n",
                2,
            ),
        ] {
            let name = format!("legacy-{factory}.{suffix}");
            let text = format!("{import}{}", function.replace("CALL", call));
            f.write(&name, &text);
            let mut loaded = source::load(&f.0.join(&name), high)
                .unwrap_or_else(|error| panic!("{name} must load before checker rejection: {error}"));
            let error = check::check(&mut loaded.program)
                .expect_err("legacy status-first factory call must be rejected by the checker");
            assert!(error.contains(migration), "{name}: missing migration `{migration}`: {error}");
            let diagnostic = loaded.diagnostic(&error);
            assert!(
                diagnostic.contains(&format!("{name}:{line}")),
                "{name}: expected checker diagnostic at original return line {line}: {diagnostic}"
            );
        }
    }
    // The checker points each former status-first call to its adopted Context-first form.
}

#[test]
fn context_first_json_rejects_nonserializable_request_payload_at_return_line_high_low() {
    let f = Fixture::new();
    for (name, high, text, line) in [
        (
            "json-request.nagi",
            true,
            "import std.http.server as http\ndef invalid(context: view[http.ResponseContext], request: http.Request) -> Result[http.Response, Error]:\n    return http.json[http.Request](context, http.Status.OK, request)\n",
            3,
        ),
        (
            "json-request.low",
            false,
            "import std.http.server as http;\nfn invalid(context: view[http.ResponseContext], request: http.Request) -> Result[http.Response, Error] { return http.json[http.Request](context, http.Status.OK, request); }\n",
            2,
        ),
    ] {
        f.write(name, text);
        let mut loaded = source::load(&f.0.join(name), high)
            .unwrap_or_else(|error| panic!("{name} must load before JSON checker rejection: {error}"));
        let error = check::check(&mut loaded.program)
            .expect_err("Request is not a JSON-serializable payload");
        assert!(
            error.contains("HTTP JSON builderの値はSerializeに対応していません"),
            "{name}: expected payload serializer rejection at argument 2, got: {error}"
        );
        let diagnostic = loaded.diagnostic(&error);
        assert!(
            diagnostic.contains(&format!("{name}:{line}")),
            "{name}: expected JSON checker diagnostic at original return line {line}: {diagnostic}"
        );
    }
}

#[test]
fn status_method_and_db_free_app_execute_from_high_and_saved_low() {
    let f = Fixture::new();
    let source = APP.replace("async def main() -> Result[unit, Error]:", &format!("{FACTORY_PROBE}\nasync def main() -> Result[unit, Error]:"))
    .replace(
        "    other = http.app_default[State](State(label=\"second\", count=9))",
        "    probe_app = http.app_default[State](State(label=\"probe\", count=0))\n    probe_app = try http.route(probe_app, http.Method.GET, view(\"/factory\"), http.public_policy[State](), factory_probe)\n    try await exercise(probe_app)\n    other = http.app_default[State](State(label=\"second\", count=9))",
    )
    .replace(
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
    named = try http.method("PROPFIND")
    assert_true(http.method_name(view(named)) == "PROPFIND")
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
    f.write("nagi.toml", "entry = \"main.nagi\"\n[rust.dependencies]\ntokio = { version = \"1.48\", features = [\"net\", \"io-util\", \"sync\", \"time\", \"rt\"] }\n");
    f.write("native.rs", FACTORY_PROBE_NATIVE);
    for input in ["main.nagi", "saved.low"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .arg("run")
            .arg(f.0.join(input))
            .arg("--project")
            .arg(&f.0)
            .arg("--rust")
            .arg(f.0.join("native.rs"))
            .arg("--out")
            .arg(f.0.join(format!("build-{input}")))
            .env("NAGI_NATIVE_TARGET_DIR", f.0.join("native-target"))
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap();
        let application = successful(output);
        assert_eq!(
            application.trim(),
            "invalid status rejected\ninvalid status rejected\ninvalid status rejected\ninvalid status rejected\ninvalid status rejected\ninvalid status rejected\ninvalid status rejected\nHTTP application checked",
            "{input}"
        );
    }
}

const FACTORY_PROBE: &str = r#"@rust("native::exercise")
extern async def exercise(app: http.App[State, Error]) -> Result[unit, Error]
async def factory_probe(request: Input, state: shared[State], authority: unit) -> Result[Output, Error]:
    context = http.response_context(view(request))
    codes = [Code.OK, Code.CREATED, Code.NOT_FOUND]
    first = codes[0]
    assert_true(first.value == 200)
    total = 0
    for code in codes:
        response = try http.empty(context, code)
        total += response.status.value
    assert_true(total == 805)
    response = try http.text(context, Code.CREATED, "created")
    assert_true(response.status.value == 201)
    assert_true(len(response.body) == 7)
    empty = try http.empty(context, Code.NO_CONTENT)
    stored = StoredResponse(output=empty)
    assert_true(stored.output.status.value == 204)
    encoded = try http.json[i64](context, Code.OK, 42)
    assert_true(len(encoded.body) == 2)
    return ok(response)
"#;

const FACTORY_PROBE_NATIVE: &str = r#"
pub async fn exercise(app: nagi_runtime::http_server::App<super::State, nagi_runtime::Error>) -> Result<(), nagi_runtime::Error> {
    use nagi_runtime::http_server as http;
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::oneshot};
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    let address = listener.local_addr().map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    // The probe retains text, a stored empty response and JSON simultaneously.
    // Keep exactly three finite ordinary slots; one actual socket/request.
    let app = http::rate_route(app, http::Method::GET, "/factory", http::rate_policy(10, 1000)?)?;
    let limits = http::response_limits(1024, 128, 4, 512, 12, 128, 3, 393216, 1, 131072, 512, 12)?;
    let options = http::with_response_limits(http::capacity(http::default_options(), 1, 1)?, limits)?;
    let options = http::with_rate_limits(options, http::rate_limits(1, 1, 1, 131072, 1000)?)?;
    let options = http::authority(options, "https://localhost", vec!["localhost".to_owned()], 1, 128)?;
    let (stop, shutdown) = oneshot::channel();
    let task = tokio::spawn(http::serve_listener(listener, app, options, async { let _ = shutdown.await; }));
    let mut stream = TcpStream::connect(address).await.map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    stream.write_all(b"GET /factory HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    let mut raw = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(3), stream.read_to_end(&mut raw)).await.map_err(|_| nagi_runtime::Error::internal("factory probe timed out"))?.map_err(|error| nagi_runtime::Error::internal(error.to_string()))?;
    let separator = raw.windows(4).position(|window| window == b"\r\n\r\n").ok_or_else(|| nagi_runtime::Error::internal("factory probe response missing header separator"))?;
    let head = std::str::from_utf8(&raw[..separator]).map_err(|_| nagi_runtime::Error::internal("factory probe response header invalid"))?;
    assert!(head.starts_with("HTTP/1.1 201 "), "{head}");
    assert_eq!(&raw[separator + 4..], b"created");
    stop.send(()).map_err(|_| nagi_runtime::Error::internal("factory probe shutdown failed"))?;
    tokio::time::timeout(std::time::Duration::from_secs(3), task).await.map_err(|_| nagi_runtime::Error::internal("factory probe server shutdown timed out"))?.map_err(|error| nagi_runtime::Error::internal(error.to_string()))??;
    Ok(())
}
"#;

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
            "Failure",
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
        if changed.contains("selected, default_error") {
            assert!(
                error.contains("expected fn[view[stdlib:std.http.server::ResponseContext], str, Result[stdlib:std.http.server::Response, Error]]"),
                "expected handler error type `str` in the RouteMapped mapper contract: {error}"
            );
            assert!(
                error.contains("got fn[view[stdlib:std.http.server::ResponseContext], ")
                    && error
                        .contains("::Failure, Result[stdlib:std.http.server::Response, Error]]"),
                "expected the actual `Failure` mapper type as the mismatch cause: {error}"
            );
            let line = changed
                .lines()
                .position(|line| line.contains("selected, default_error"))
                .unwrap()
                + 1;
            let diagnostic = loaded.diagnostic(&error);
            assert!(
                diagnostic.contains(&format!("main.nagi:{line}")),
                "expected mapper mismatch at RouteMapped call line {line}: {diagnostic}"
            );
        }
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
        "import std.http.server as http\ndef mapped(context: view[http.ResponseContext], problem: Error) -> Result[http.Response, Error]:\n    return http.empty(context, http.Status.BAD_REQUEST)\ndef main():\n    state = \"borrowed\"\n    app = http.app[view[str], Error](view(state), mapped)\n",
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
        ("import std.http.server as http\ndef invalid(context: view[http.ResponseContext]) -> Result[unit, Error]:\n    encoded = try json_encode(try http.empty(context, http.Status.OK))\n    return ok(print(encoded))\n", 3),
        ("import std.http.server as http\nclass Wrapped:\n    output: http.Response\ndef invalid(context: view[http.ResponseContext]) -> Result[unit, Error]:\n    response = try http.empty(context, http.Status.OK)\n    encoded = try json_encode(Wrapped(output=response))\n    return ok(print(encoded))\n", 6),
        ("import std.http.server as http\nclass Wrapped:\n    output: http.Response\ndef invalid(context: view[http.ResponseContext]) -> Result[unit, Error]:\n    response = try http.empty(context, http.Status.OK)\n    encoded = try http.json[Wrapped](context, http.Status.OK, Wrapped(output=response))\n    return ok(print(encoded.status.value))\n", 6),
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
