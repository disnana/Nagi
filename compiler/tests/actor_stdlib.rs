use nagic::{ast::DefKind, check, emit, modules, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

static NEXT: AtomicU64 = AtomicU64::new(0);
static NATIVE_RUN: Mutex<()> = Mutex::new(());
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi actor 凪 {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn checked(&self, name: &str) -> source::Sources {
        let mut loaded = source::load(&self.0.join(name), name.ends_with(".nagi"))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        loaded
    }
    fn roundtrip(&self) -> source::Sources {
        let high = self.checked("main.nagi");
        let low = emit::low(&high.program);
        let mut parsed = parser::parse(&low, false).unwrap();
        check::check(&mut parsed).unwrap_or_else(|error| panic!("{error}\n{low}"));
        modules::validate(&parsed).unwrap();
        let identities = |p: &nagic::ast::Program| {
            p.modules
                .definitions
                .iter()
                .map(|d| (d.id.clone(), d.symbol.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(identities(&parsed), identities(&high.program));
        self.write("saved.low", &low);
        let saved = self.checked("saved.low");
        assert_eq!(identities(&saved.program), identities(&high.program));
        saved
    }
    fn rejected(&self, text: &str) {
        self.write("main.nagi", text);
        let error = match source::load(&self.0.join("main.nagi"), true) {
            Err(error) => error,
            Ok(mut loaded) => {
                let error = check::check(&mut loaded.program)
                    .expect_err("invalid actor program passed check");
                loaded.diagnostic(&error)
            }
        };
        assert!(error.contains("main.nagi:"), "{text}\n{error}");
        for command in ["check", "build"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&self.0)
                .args([command, "main.nagi", "--no-project"])
                .env("PATH", "")
                .env("NAGI_ROOT", self.0.join("missing-runtime"))
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !output.status.success() && stderr.contains("main.nagi:"),
                "{stderr}"
            );
            assert!(
                !stderr.contains("Cargoが見つかりません") && !stderr.contains("Build failed"),
                "actor rejection reached Rust: {stderr}"
            );
            assert!(!self.0.join("build/main/src/main.rs").exists());
        }
    }
    fn run_both(&self, expected: &str) {
        let _guard = NATIVE_RUN.lock().unwrap();
        let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("native-target")
            });
        for name in ["main.nagi", "saved.low"] {
            let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
                .current_dir(&self.0)
                .args(["run", name, "--out", "build", "--no-project"])
                .env("NAGI_NATIVE_TARGET_DIR", &target)
                .env("CARGO_NET_OFFLINE", "true")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stderr = String::from_utf8(output.stderr).unwrap();
            let launcher = stderr
                .lines()
                .find(|line| line.starts_with("native: "))
                .unwrap();
            let actual = String::from_utf8(output.stdout).unwrap();
            let executable = launcher
                .trim_end_matches('\r')
                .strip_prefix("native: ")
                .unwrap();
            assert!(Path::new(executable).is_file(), "{launcher}");
            assert_eq!(actual.trim(), expected, "{name}");
            if name.ends_with(".nagi") {
                // Saved Low must retain its own module identities without
                // reading or rebuilding the original High source.
                fs::remove_file(self.0.join(name)).unwrap();
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const COUNTER: &str = r#"import std.actor as actor
class Context:
    seed: i64
    label: str
class State:
    count: i64
    label: str
enum Message:
    Add(amount: i64)
    Read
    Business
    Crash
enum Failure:
    Rejected(reason: str)
async def factory(context: shared[Context]) -> Result[State, Error]:
    await actor.yield_now()
    return ok(State(count=context.seed, label=copy(view(context.label))))
async def handler(state: State, message: Message) -> Result[actor.Turn[State, i64, Failure], Error]:
    match message:
        case Message.Add(amount):
            count = state.count + amount
            next = State(count=count, label=state.label)
            return ok(actor.turn[State, i64, Failure](next, ok(count)))
        case Message.Read:
            count = state.count
            return ok(actor.turn[State, i64, Failure](state, ok(count)))
        case Message.Business:
            return ok(actor.turn[State, i64, Failure](state, fail(Failure.Rejected("expected failure"))))
        case Message.Crash:
            return error("controlled actor reset")
async def monitor(group: actor.Supervisor[Context]) -> Result[unit, Error]:
    return await actor.run(group)
async def query(worker: view[actor.Actor[Message, i64, Failure]], message: Message) -> Result[i64, Error]:
    match await actor.call(worker, message, 5000, 5000):
        case Ok(reply):
            match reply:
                case Ok(value):
                    return ok(value)
                case Err(_):
                    return error("unexpected business failure")
        case Err(_):
            return error("actor call failed")
async def exercise(first: actor.Actor[Message, i64, Failure], second: actor.Actor[Message, i64, Failure], control: actor.Control) -> Result[unit, Error]:
    match await actor.ready(view(first), 5000):
        case Ok(_):
            assert_true(True)
        case Err(_):
            return error("first actor did not start")
    match await actor.ready(view(second), 5000):
        case Ok(_):
            assert_true(True)
        case Err(_):
            return error("second actor did not start")
    assert_true(try await query(view(first), Message.Add(5)) == 15)
    assert_true(try await query(view(second), Message.Read) == 10)
    match await actor.call(view(first), Message.Business, 5000, 5000):
        case Ok(reply):
            match reply:
                case Ok(_):
                    assert_true(False)
                case Err(problem):
                    match problem:
                        case Failure.Rejected(reason):
                            assert_true(reason == "expected failure")
        case Err(_):
            return error("business reply was lost")
    assert_true(try await query(view(first), Message.Read) == 15)
    assert_true(try await query(view(second), Message.Add(3)) == 13)
    match await actor.call(view(first), Message.Crash, 5000, 5000):
        case Ok(_):
            assert_true(False)
        case Err(problem):
            assert_true(problem.kind == actor.CallKind.REPLY_LOST)
    restarted = False
    while not restarted:
        match try await actor.next_event(view(control)):
            case Some(event):
                if event.child_name == "first" and event.kind == actor.EventKind.STARTED and event.generation > 1:
                    restarted = True
            case None:
                return error("group stopped before restart")
    assert_true(try await query(view(first), Message.Read) == 10)
    assert_true(try await query(view(second), Message.Read) == 13)
    try await actor.shutdown(view(control))
    return ok(print("independent actors and explicit failures preserved"))
async def main() -> Result[unit, Error]:
    group = actor.supervisor[Context](Context(seed=10, label="private state"), actor.default_options())
    creator = factory
    dispatch = handler
    first = try actor.register[State, Message, i64, Failure](view(group), "first", creator, dispatch, actor.default_actor_options())
    second = try actor.register[State, Message, i64, Failure](view(group), "second", factory, handler, actor.default_actor_options())
    match await actor.ready(view(first), 0):
        case Ok(_):
            assert_true(False)
        case Err(problem):
            assert_true(problem.kind == actor.CallKind.NOT_READY)
    control = actor.control(view(group))
    async with scope:
        spawn monitor(group)
        spawn exercise(first, second, control)
    return ok(assert_true(True))
"#;

#[test]
fn pending_owned_state_business_failure_and_restart_execute_from_high_and_low() {
    let f = Fixture::new();
    f.write("main.nagi", COUNTER);
    f.roundtrip();
    f.run_both("independent actors and explicit failures preserved");
}

const CLASS_ERRORS: &str = r#"import std.actor as actor
class Context:
    seed: i64
class State:
    count: i64
    label: str
class Reply:
    count: i64
    label: str
class Failure:
    code: i64
    detail: str
async def factory(context: shared[Context]) -> Result[State, Error]:
    return ok(State(count=context.seed, label="owned reply"))
async def handler(state: State, message: i64) -> Result[actor.Turn[State, Reply, Failure], Error]:
    if message < 0:
        return ok(actor.turn[State, Reply, Failure](state, fail(Failure(code=409, detail="expected rejection"))))
    count = state.count + message
    next = State(count=count, label=state.label)
    reply = Reply(count=count, label=copy(view(next.label)))
    return ok(actor.turn[State, Reply, Failure](next, ok(reply)))
async def monitor(group: actor.Supervisor[Context]) -> Result[unit, Error]:
    return await actor.run(group)
async def exercise(worker: actor.Actor[i64, Reply, Failure], control: actor.Control) -> Result[unit, Error]:
    match await actor.ready(view(worker), 5000):
        case Ok(_):
            assert_true(True)
        case Err(_):
            return error("actor did not start")
    match await actor.call(view(worker), 5, 5000, 5000):
        case Ok(reply):
            match reply:
                case Ok(value):
                    assert_true(value.count == 5 and value.label == "owned reply")
                case Err(_):
                    assert_true(False)
        case Err(_):
            return error("first reply was lost")
    match await actor.call(view(worker), -1, 5000, 5000):
        case Ok(reply):
            match reply:
                case Ok(_):
                    assert_true(False)
                case Err(problem):
                    assert_true(problem.code == 409 and problem.detail == "expected rejection")
        case Err(_):
            return error("class error was lost")
    match await actor.call(view(worker), 0, 5000, 5000):
        case Ok(reply):
            match reply:
                case Ok(value):
                    assert_true(value.count == 5 and value.label == "owned reply")
                case Err(_):
                    assert_true(False)
        case Err(_):
            return error("state was lost after class error")
    try await actor.shutdown(view(control))
    return ok(print("owned class replies and errors preserve state"))
async def main() -> Result[unit, Error]:
    group = actor.supervisor[Context](Context(seed=0), actor.default_options())
    worker = try actor.register[State, i64, Reply, Failure](view(group), "class errors", factory, handler, actor.default_actor_options())
    control = actor.control(view(group))
    async with scope:
        spawn monitor(group)
        spawn exercise(worker, control)
    return ok(assert_true(True))
"#;

#[test]
fn owned_custom_class_replies_and_errors_execute_without_clone_or_serde_bounds() {
    let f = Fixture::new();
    f.write("main.nagi", CLASS_ERRORS);
    f.roundtrip();
    f.run_both("owned class replies and errors preserve state");
}

#[test]
fn actor_and_http_aliases_keep_distinct_registry_identity_without_fake_classes() {
    let f = Fixture::new();
    f.write("main.nagi", &format!("import std.http.server as http\nfrom std.actor import Options as GroupOptions, Actor as Worker\nfrom std.http.server import Options as HttpOptions\n{COUNTER}"));
    let p = f.checked("main.nagi").program;
    for (path, module) in [
        ("actor.Supervisor", "stdlib:std.actor"),
        ("Worker", "stdlib:std.actor"),
        ("GroupOptions", "stdlib:std.actor"),
        ("HttpOptions", "stdlib:std.http.server"),
    ] {
        let definition = p.modules.resolve_root_path(path).unwrap();
        assert_eq!(definition.id.kind, DefKind::Resource);
        assert_eq!(definition.id.module.0, module);
        assert!(!p
            .classes
            .iter()
            .any(|class| class.name == definition.symbol));
    }
    assert_ne!(
        p.modules.resolve_root_path("GroupOptions").unwrap().id,
        p.modules.resolve_root_path("HttpOptions").unwrap().id
    );
    f.roundtrip();
    f.rejected("import std.actor as actor\nimport std.http.server as http\ndef wrong(value: actor.Options) -> http.Options:\n    return value\n");
}

#[test]
fn saved_low_cannot_invent_actor_types_capabilities_or_standard_implementations() {
    let f = Fixture::new();
    f.write("main.nagi", COUNTER);
    let p = f.checked("main.nagi").program;
    let low = emit::low(&p);
    let header = low
        .lines()
        .find_map(|line| line.strip_prefix("# nagi-modules-v1 "))
        .unwrap();
    let metadata: serde_json::Value = serde_json::from_str(header).unwrap();
    let index = metadata["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| d["id"]["module"] == "stdlib:std.actor" && d["id"]["name"] == "Actor")
        .unwrap();
    for (field, value) in [
        ("name", "FakeActor"),
        ("kind", "Class"),
        ("module", "stdlib:std.http.server"),
    ] {
        let mut changed = metadata.clone();
        changed["definitions"][index]["id"][field] = value.into();
        let corrupt = low
            .lines()
            .map(|line| {
                if line.starts_with("# nagi-modules-v1 ") {
                    format!("# nagi-modules-v1 {changed}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        f.write("corrupt.low", &corrupt);
        assert!(
            source::load(&f.0.join("corrupt.low"), false).is_err(),
            "forged {field} passed"
        );
    }
    let mut changed = metadata;
    changed["definitions"][index]["charge"] = true.into();
    let corrupt = low
        .lines()
        .map(|line| {
            if line.starts_with("# nagi-modules-v1 ") {
                format!("# nagi-modules-v1 {changed}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    f.write("corrupt.low", &corrupt);
    assert!(source::load(&f.0.join("corrupt.low"), false).is_err());
    let resource = p.modules.resolve_root_path("actor.Actor").unwrap();
    f.write(
        "corrupt.low",
        &format!("{low}\nclass {} {{ forged: i64; }}\n", resource.symbol),
    );
    assert!(source::load(&f.0.join("corrupt.low"), false).is_err());
}

#[test]
fn callback_shapes_and_custom_error_identity_are_checked_before_native_build() {
    let f = Fixture::new();
    // Each callback body remains valid after the substitution. Rejections
    // therefore exercise registration's callback contract rather than an
    // unrelated move or return-type error within the callback itself.
    let callbacks = "import std.actor as actor\nclass Context:\n    seed: i64\nclass State:\n    count: i64\nenum Failure:\n    Rejected\nasync def factory(context: shared[Context]) -> Result[State, Error]:\n    return ok(State(count=context.seed))\nasync def handler(state: State, message: i64) -> Result[actor.Turn[State, i64, Failure], Error]:\n    return ok(actor.turn[State, i64, Failure](State(count=state.count), ok(message)))\ndef setup(group: view[actor.Supervisor[Context]]) -> Result[actor.Actor[i64, i64, Failure], Error]:\n    return actor.register[State, i64, i64, Failure](group, \"worker\", factory, handler, actor.default_actor_options())\n";
    f.write("main.nagi", callbacks);
    f.roundtrip();
    for changed in [
        callbacks.replace("async def factory(", "def factory("),
        callbacks.replace("context: shared[Context]", "context: Context"),
        callbacks.replace(
            "handler(state: State, message: i64)",
            "handler(state: shared[State], message: i64)",
        ),
        callbacks.replace(
            "Result[actor.Turn[State, i64, Failure], Error]",
            "Result[actor.Turn[State, i64, Failure], Failure]",
        ),
        callbacks.replace(
            "actor.register[State, i64, i64, Failure]",
            "actor.register[State, i64, str, Failure]",
        ),
    ] {
        f.rejected(&changed);
    }
    f.write("left.nagi", "enum Failure:\n    Rejected(reason: str)\n");
    f.write("right.nagi", "enum Failure:\n    Rejected(reason: str)\n");
    f.rejected("import std.actor as actor\nimport \"left.nagi\" as left\nimport \"right.nagi\" as right\ndef wrong(value: actor.Actor[i64, i64, left.Failure]) -> actor.Actor[i64, i64, right.Failure]:\n    return value\n");
}

#[test]
fn named_task_factory_aliases_require_shared_context_async_unit_and_builtin_error() {
    let f = Fixture::new();
    let valid = "import std.actor as actor\nclass Context:\n    flag: bool\nasync def background(context: shared[Context]) -> Result[unit, Error]:\n    return ok(assert_true(context.flag))\ndef setup(group: view[actor.Supervisor[Context]]) -> Result[unit, Error]:\n    factory = background\n    return actor.task(group, \"background\", factory, actor.RestartPolicy.TEMPORARY)\n";
    f.write("main.nagi", valid);
    f.roundtrip();
    for changed in [
        valid.replace("async def background", "def background"),
        valid.replace("context: shared[Context]", "context: Context"),
        valid.replace(
            "Result[unit, Error]:\n    return ok(assert_true(context.flag))",
            "Result[bool, Error]:\n    return ok(context.flag)",
        ),
        valid
            .replace(
                "class Context:",
                "class Failure:\n    message: str\nclass Context:",
            )
            .replace(
                "Result[unit, Error]:\n    return ok(assert_true(context.flag))",
                "Result[unit, Failure]:\n    return fail(Failure(message=\"custom task failure\"))",
            ),
    ] {
        f.rejected(&changed);
    }
}

#[test]
fn spawning_borrowing_native_futures_requires_an_owned_named_wrapper() {
    let f = Fixture::new();
    for arg in ["control", "view(control)"] {
        f.rejected(&format!("import std.actor as actor\nasync def bad(control: actor.Control) -> Result[unit, Error]:\n    async with scope:\n        spawn actor.shutdown({arg})\n    return ok(assert_true(True))\n"));
    }
    f.write("main.nagi", "import std.actor as actor\nasync def stop(control: actor.Control) -> Result[unit, Error]:\n    return await actor.shutdown(control)\nasync def start(group: actor.Supervisor[i64], control: actor.Control) -> Result[unit, Error]:\n    async with scope:\n        spawn actor.run(group)\n        spawn stop(control)\n        spawn actor.yield_now()\n    return ok(assert_true(True))\n");
    f.roundtrip();
}

#[test]
fn messages_replies_and_business_errors_reject_unaccounted_nested_payloads() {
    let f = Fixture::new();
    for ty in [
        "Map[str, i64]",
        "shared[str]",
        "view[str]",
        "http.Status",
        "actor.CallError",
        "List[Map[str, i64]]",
        "Result[i64, shared[str]]",
    ] {
        for (m, r, e) in [
            (ty, "i64", "Error"),
            ("i64", ty, "Error"),
            ("i64", "i64", ty),
        ] {
            f.rejected(&format!("import std.actor as actor\nimport std.http.server as http\ndef bad(value: actor.Actor[{m}, {r}, {e}]):\n    print(0)\n"));
        }
    }
    for field in ["Map[str, i64]", "List[shared[str]]", "http.Request"] {
        f.rejected(&format!("import std.actor as actor\nimport std.http.server as http\nclass Nested:\n    value: {field}\nenum Packet:\n    Empty\n    Data(value: List[Nested])\ndef bad(value: actor.Actor[Packet, i64, Error]):\n    print(0)\n"));
    }
    f.rejected(
        "import std.actor as actor\ndef bad(value: actor.Supervisor[view[str]]):\n    print(0)\n",
    );
}

#[test]
fn turn_inline_layout_generics_and_left_to_right_moves_are_not_erased() {
    let f = Fixture::new();
    f.rejected("import std.actor as actor\nclass Recursive:\n    next: actor.Turn[Recursive, i64, Error]\n");
    f.rejected("import std.actor as actor\nclass Recursive:\n    next: actor.Turn[i64, Recursive, Error]\n");
    f.rejected(
        "import std.actor as actor\nclass Recursive:\n    next: actor.Turn[i64, i64, Recursive]\n",
    );
    f.rejected("import std.actor as actor\ndef bad(value: actor.Actor[i64]):\n    print(0)\n");
    f.rejected("import std.actor as actor\nclass State:\n    label: str\ndef bad(state: State) -> actor.Turn[State, str, Error]:\n    return actor.turn[State, str, Error](state, ok(state.label))\n");
    f.rejected("import std.actor as actor\ndef bad(group: actor.Supervisor[i64]) -> actor.Control:\n    alias = group\n    return actor.control(view(group))\n");
}

#[test]
fn lifecycle_text_views_follow_their_native_owner_and_cannot_serialize() {
    let f = Fixture::new();
    f.write("main.nagi", "import std.actor as actor\ndef message(problem: view[actor.CallError]) -> view[str]:\n    return problem.message\ndef name(event: view[actor.Event]) -> view[str]:\n    return event.child_name\ndef policy() -> actor.RestartPolicy:\n    return copy(view(actor.RestartPolicy.TRANSIENT))\n");
    f.roundtrip();
    f.rejected("import std.actor as actor\ndef escape(problem: actor.CallError) -> view[str]:\n    return problem.message\n");
    f.rejected("import std.actor as actor\ndef encode(event: actor.Event) -> Result[str, Error]:\n    return json_encode(event)\n");
    f.rejected("import std.actor as actor\ndef bad(value: view[actor.RestartPolicy]) -> i64:\n    return len(value)\n");
    f.rejected("import std.actor as actor\ndef bad(kind: actor.CallKind) -> bool:\n    match kind:\n        case actor.CallKind.STOPPED:\n            return True\n");
}

const READY_TASK: &str = r#"import std.actor as actor
class Context:
    active: bool
async def factory(context: shared[Context], signal: actor.TaskReady) -> Result[unit, Error]:
    await actor.yield_now()
    try actor.mark_ready(view(signal))
    match actor.mark_ready(view(signal)):
        case Ok(_):
            return error("duplicate readiness was accepted")
        case Err(_):
            assert_true(True)
    while context.active:
        await actor.yield_now()
    return ok(assert_true(True))
async def monitor(group: actor.Supervisor[Context]) -> Result[unit, Error]:
    return await actor.run(group)
async def exercise(control: actor.Control) -> Result[unit, Error]:
    reached = False
    while not reached:
        match await actor.next_event_timeout(view(control), 5000):
            case Ok(next):
                match next:
                    case Some(event):
                        if event.kind == actor.EventKind.READY:
                            assert_true(event.child_name == "connector" and event.generation == 1)
                            reached = True
                    case None:
                        return error("event stream closed before readiness")
            case Err(problem):
                assert_true(problem.kind == actor.WaitKind.TIMEOUT)
                return error("readiness deadline exceeded")
    try await actor.shutdown(view(control))
    return ok(print("task readiness and event deadlines preserved"))
async def main() -> Result[unit, Error]:
    group = actor.supervisor[Context](Context(active=True), actor.default_options())
    creator = factory
    try actor.task_with_ready(view(group), "connector", creator, actor.RestartPolicy.TEMPORARY)
    control = actor.control(view(group))
    match await actor.next_event_timeout(view(control), 0):
        case Ok(_):
            return error("invalid deadline was accepted")
        case Err(problem):
            assert_true(problem.kind == actor.WaitKind.INVALID_TIMEOUT)
            assert_true(len(problem.message) > 0)
    match await actor.next_event_timeout(view(control), 1):
        case Ok(_):
            return error("unstarted supervisor produced an event")
        case Err(problem):
            assert_true(problem.kind == actor.WaitKind.TIMEOUT)
    async with scope:
        spawn monitor(group)
        spawn exercise(control)
    return ok(assert_true(True))
"#;

#[test]
fn explicit_task_readiness_and_typed_event_deadlines_execute_from_high_and_low() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", READY_TASK);
    fixture.roundtrip();
    fixture.run_both("task readiness and event deadlines preserved");
}

#[test]
fn readiness_factory_and_wait_error_contracts_are_checked_before_native_build() {
    let fixture = Fixture::new();
    for changed in [
        READY_TASK.replace("async def factory(", "def factory("),
        READY_TASK.replace(
            "context: shared[Context], signal: actor.TaskReady",
            "context: shared[Context]",
        ),
        READY_TASK.replace("signal: actor.TaskReady", "signal: view[actor.TaskReady]"),
        READY_TASK.replace("context: shared[Context]", "context: Context"),
        READY_TASK.replace(
            "actor.mark_ready(view(signal))",
            "actor.mark_ready(view(context))",
        ),
        READY_TASK.replace(
            "actor.next_event_timeout(view(control), 1)",
            "actor.next_event_timeout(view(control), False)",
        ),
        READY_TASK.replace(
            "problem.kind == actor.WaitKind.TIMEOUT",
            "problem.kind == actor.CallKind.REPLY_TIMEOUT",
        ),
    ] {
        fixture.rejected(&changed);
    }
    fixture.rejected("import std.actor as actor\nasync def wrong(control: view[actor.Control]) -> Result[Option[actor.Event], Error]:\n    return await actor.next_event_timeout(control, 1000)\n");
}
