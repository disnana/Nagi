use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-go-backend-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create {}: {error}", path.display()),
            }
        }
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args(args)
            .current_dir(&self.0)
            .env_remove("NAGI_RUN_RETENTION")
            .output()
            .unwrap()
    }

    fn go(&self, command: &str, source: &str, out: &str) -> Output {
        self.command(&[
            command,
            source,
            "--no-project",
            "--backend",
            "go",
            "--profile",
            "dev",
            "--out",
            out,
        ])
    }

    fn latest(&self, out: &str) -> (PathBuf, Value) {
        let apps = self.0.join(out).join(".nagi/apps");
        let app = fs::read_dir(apps)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("nagi-go-")
            })
            .unwrap();
        let value: Value =
            serde_json::from_slice(&fs::read(app.join("latest.json")).unwrap()).unwrap();
        (app, value)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PROGRAM: &str = r#"class Point:
    x: f64
    y: f64

def parse_plus(text: str) -> Result[i64, Error]:
    value = try parse_i64(text)
    return ok(value + 1)

def main() -> Result[unit, Error]:
    point = Point(x=1.5, y=2.5)
    print(point.x + point.y)
    answer = try parse_plus("41")
    print(answer)
    return ok(print("go-ok"))
"#;

#[test]
fn go_backend_runs_checked_high_and_independent_saved_low() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", PROGRAM);
    let high = fixture.go("run", "main.nagi", "out-high");
    assert!(
        high.status.success(),
        "{}",
        String::from_utf8_lossy(&high.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&high.stdout), "4\n42\ngo-ok\n");
    let generated = fs::read_to_string(fixture.0.join("out-high/generated.low")).unwrap();
    fixture.write("saved.low", &generated);
    fs::remove_file(fixture.0.join("main.nagi")).unwrap();
    let low = fixture.go("run", "saved.low", "out-low");
    assert!(
        low.status.success(),
        "{}",
        String::from_utf8_lossy(&low.stderr)
    );
    assert_eq!(low.stdout, high.stdout);

    let (app, latest) = fixture.latest("out-low");
    assert_eq!(latest["backend"], "go");
    let generation = app.join(latest["executable"].as_str().unwrap());
    assert!(generation.is_file());
    let receipt: Value =
        serde_json::from_slice(&fs::read(app.join(latest["receipt"].as_str().unwrap())).unwrap())
            .unwrap();
    assert_eq!(receipt["environment"]["GOTOOLCHAIN"], "local");
    assert_eq!(receipt["toolchain_auto_download"], false);
    assert!(fixture.0.join("out-low/go/main.go").is_file());
    assert!(!fixture.0.join("out-low/Cargo.toml").exists());
}

#[test]
fn cli_overrides_project_backend_and_rust_remains_default() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", "def main():\n    print(1)\n");
    fixture.write("nagi.toml", "entry = \"main.nagi\"\nbackend = \"go\"\n");

    let project_go = fixture.command(&["check", "--project", "."]);
    assert!(
        project_go.status.success(),
        "{}",
        String::from_utf8_lossy(&project_go.stderr)
    );
    let override_rust = fixture.command(&["check", "--project", ".", "--backend", "rust"]);
    assert!(override_rust.status.success());

    fixture.write("nagi.toml", "entry = \"main.nagi\"\n");
    let default_rust = fixture.command(&["check", "--project", "."]);
    assert!(default_rust.status.success());
    let unknown = fixture.command(&["check", "main.nagi", "--no-project", "--backend", "java"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("--backend requires rust or go"));
}

#[test]
fn unsupported_go_feature_fails_before_toolchain_and_keeps_last_good() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", "def main():\n    print(7)\n");
    let good = fixture.go("build", "main.nagi", "out");
    assert!(
        good.status.success(),
        "{}",
        String::from_utf8_lossy(&good.stderr)
    );
    let (app, before) = fixture.latest("out");
    let old_binary = fs::read(app.join(before["executable"].as_str().unwrap())).unwrap();

    fixture.write(
        "main.nagi",
        "def main():\n    values = make_ints(2)\n    print(len(values))\n",
    );
    let failed = fixture.go("build", "main.nagi", "out");
    assert!(!failed.status.success());
    let diagnostic = String::from_utf8_lossy(&failed.stderr);
    assert!(diagnostic.contains("Go backend"), "{diagnostic}");
    assert!(diagnostic.contains("make_ints"), "{diagnostic}");
    assert!(diagnostic.contains("main.nagi:2"), "{diagnostic}");
    let (_, after) = fixture.latest("out");
    assert_eq!(after, before);
    assert_eq!(
        fs::read(app.join(after["executable"].as_str().unwrap())).unwrap(),
        old_binary
    );
}

#[test]
fn reachable_rust_native_dependency_reports_the_chain() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        "@rust(\"native::digest\")\nextern def digest(value: i64) -> i64\n\ndef create(value: i64) -> i64:\n    return digest(value)\n\ndef main():\n    print(create(1))\n",
    );
    let output = fixture.command(&[
        "check",
        "main.nagi",
        "--no-project",
        "--backend",
        "go",
        "--out",
        "out",
    ]);
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("selected backend go cannot provide native function"));
    assert!(diagnostic.contains("main.nagi::digest"), "{diagnostic}");
    assert!(diagnostic.contains("required by main -> "), "{diagnostic}");
    assert!(diagnostic.contains("main.nagi::create -> "), "{diagnostic}");
    assert!(
        diagnostic.contains("available binding: rust (native::digest)"),
        "{diagnostic}"
    );
}

#[test]
fn go_specific_options_are_not_silently_ignored() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", "def main():\n    print(1)\n");
    for option in ["--runtime-profile", "--seed-lock", "--incremental"] {
        let mut args = vec!["build", "main.nagi", "--no-project", "--backend", "go"];
        if option == "--runtime-profile" {
            args.extend([option, "auto"]);
        } else if option == "--incremental" {
            args.extend(["--profile", "dev", option]);
        } else {
            args.push(option);
        }
        let output = fixture.command(&args);
        assert!(!output.status.success(), "{option}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("supported only by the rust backend"),
            "{option}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn declared_go_adapter_is_rejected_until_the_audited_adapter_abi_exists() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", "def main():\n    print(1)\n");
    fixture.write("native/bridge.go", "package native\n");
    fixture.write(
        "nagi.toml",
        "entry = \"main.nagi\"\nbackend = \"go\"\n[go]\nfiles = [\"native/bridge.go\"]\n",
    );
    let output = fixture.command(&["check", "--project", "."]);
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("Go native adapter files are not supported"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains(
            &Path::new("native")
                .join("bridge.go")
                .to_string_lossy()
                .to_string()
        ),
        "{diagnostic}"
    );
}

#[test]
fn preserves_evaluation_order_short_circuiting_and_distinct_identifiers() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        r#"def left() -> i64:
    print(1)
    return 1

def right() -> Result[i64, Error]:
    print(2)
    return ok(2)

def should_not_run() -> Result[bool, Error]:
    print(99)
    return ok(true)

def add(left_value: i64, right_value: i64) -> i64:
    return left_value + right_value

def main() -> Result[unit, Error]:
    go = 4
    nagi_go = 5
    print(add(left(), try right()))
    if false and try should_not_run():
        print(100)
    print(go + nagi_go)
    return ok(print("done"))
"#,
    );
    let output = fixture.go("run", "main.nagi", "out");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "1\n2\n3\n9\ndone\n"
    );
}

#[test]
fn opposite_backend_assets_are_rejected_instead_of_ignored() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", "def main():\n    print(1)\n");
    fixture.write("native/bridge.go", "package native\n");
    fixture.write("native/bridge.rs", "pub fn bridge() {}\n");

    fixture.write(
        "nagi.toml",
        "entry = \"main.nagi\"\nbackend = \"rust\"\n[go]\nfiles = [\"native/bridge.go\"]\n",
    );
    let rust = fixture.command(&["check", "--project", "."]);
    assert!(!rust.status.success());
    assert!(
        String::from_utf8_lossy(&rust.stderr)
            .contains("selected backend rust cannot use Go native assets"),
        "{}",
        String::from_utf8_lossy(&rust.stderr)
    );

    fixture.write(
        "nagi.toml",
        "entry = \"main.nagi\"\nbackend = \"go\"\n[rust]\nfile = \"native/bridge.rs\"\n",
    );
    let go = fixture.command(&["check", "--project", "."]);
    assert!(!go.status.success());
    assert!(
        String::from_utf8_lossy(&go.stderr)
            .contains("selected backend go cannot use Rust native assets"),
        "{}",
        String::from_utf8_lossy(&go.stderr)
    );
}

#[test]
fn missing_go_toolchain_is_actionable_and_does_not_publish_latest() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", "def main():\n    print(1)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args([
            "build",
            "main.nagi",
            "--no-project",
            "--backend",
            "go",
            "--profile",
            "dev",
            "--out",
            "out",
        ])
        .current_dir(&fixture.0)
        .env("NAGI_GO", fixture.0.join("missing-go"))
        .env_remove("NAGI_RUN_RETENTION")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("Go toolchain was not found"),
        "{diagnostic}"
    );
    let apps = fixture.0.join("out/.nagi/apps");
    if apps.exists() {
        for app in fs::read_dir(apps).unwrap() {
            let app = app.unwrap().path();
            assert!(!app.join("latest.json").exists());
            let generations = app.join("generations");
            if generations.exists() {
                assert_eq!(fs::read_dir(generations).unwrap().count(), 0);
            }
        }
    }
}

#[test]
fn imported_functions_share_frontend_resolution_with_the_go_backend() {
    let fixture = Fixture::new();
    fixture.write(
        "library.nagi",
        "def answer(value: i64) -> i64:\n    return value + 1\n",
    );
    fixture.write(
        "main.nagi",
        "import \"library.nagi\" as library\ndef main():\n    print(library.answer(41))\n",
    );
    let output = fixture.go("run", "main.nagi", "out");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");
}

#[test]
fn exhaustive_matches_and_float_console_contract_compile_and_run() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        r#"def option_value(value: i64?) -> i64:
    match value:
        case Some(number):
            return number
        case None:
            return 0

def result_value(value: Result[i64, Error]) -> i64:
    match value:
        case Ok(number):
            return number
        case Err(_):
            return 0

def inverse_f32(value: f32) -> f32:
    one: f32 = 1.0
    return one / value

def main():
    print("os.")
    print(option_value(some(4)))
    print(result_value(ok(5)))
    print(5.5 % 2.0)
    negative_zero: f64 = -0.0
    print(1.0 / negative_zero)
    print(inverse_f32(-0.0))
    print(1000000.0)
    rounded: f32 = 1952495.25
    print(rounded)
"#,
    );
    let output = fixture.go("run", "main.nagi", "out");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "os.\n4\n5\n1.5\n-inf\n-inf\n1000000\n1952495.3\n"
    );
}

#[test]
fn list_index_append_and_copy_iteration_compile_and_run() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        r#"def make_rows() -> List[List[i64]]:
    rows: List[List[i64]] = []
    append(rows, [1, 2])
    append(rows, [3])
    return rows

def main():
    nested = make_rows()
    print(len(nested))
    values: List[i64] = [3, 4]
    empty: List[i64] = []
    append(empty, 9)
    print(empty[0])
    append(values, 5)
    print(len(values))
    print(values[0] + values[2])
    total: i64 = 0
    for value in values:
        total = total + value
    print(total)
    skipped: i64 = 0
    for index in range(-1):
        skipped = skipped + index
    for index in range(0):
        skipped = skipped + index
    print(skipped)
    for index in range(3):
        total = total + index
    print(total)
"#,
    );
    let output = fixture.go("run", "main.nagi", "out-high");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "2\n9\n3\n8\n12\n0\n15\n"
    );

    let generated = fs::read_to_string(fixture.0.join("out-high/generated.low")).unwrap();
    fixture.write("saved.low", &generated);
    fs::remove_file(fixture.0.join("main.nagi")).unwrap();
    let low = fixture.go("run", "saved.low", "out-low");
    assert!(
        low.status.success(),
        "{}",
        String::from_utf8_lossy(&low.stderr)
    );
    assert_eq!(low.stdout, output.stdout);
}

#[test]
fn list_expressions_preserve_left_to_right_single_evaluation() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        r#"def marked(value: i64) -> i64:
    print(value)
    return value

def values() -> List[i64]:
    print("values")
    return [7]

def index() -> i64:
    print("index")
    return 0

def main():
    items: List[i64] = [marked(1), marked(2)]
    append(items, marked(3))
    print(values()[index()])
    print(items[0] + items[1] + items[2])
"#,
    );
    let output = fixture.go("run", "main.nagi", "out");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "1\n2\n3\nvalues\nindex\n7\n6\n"
    );
}

#[test]
fn sealed_identity_moves_run_in_high_saved_and_handwritten_low() {
    let fixture = Fixture::new();
    fixture.write(
        "library.nagi",
        "from std.ownership import move\ndef transfer_text(value: str) -> str:\n    return move(value)\n",
    );
    fixture.write(
        "main.nagi",
        r#"from std.ownership import move as transfer
import std.ownership as ownership
import "library.nagi" as library
class Packet:
    text: str

def marked(value: str) -> str:
    print(value)
    return value

def move(value: i64) -> i64:
    return value + 10

def parsed() -> Result[str, Error]:
    return ok(marked("try"))

def main() -> Result[unit, Error]:
    copy_source: i8 = 7
    copy_value: i8 = transfer(copy_source)
    print(copy_source)
    print(copy_value)
    print(move(2))
    text = marked("text")
    moved_text = ownership.move(text)
    print(moved_text)
    imported = library.transfer_text(marked("library"))
    print(imported)
    values: List[i64] = [1, 2]
    moved_values = transfer(values)
    print(len(moved_values))
    packet = Packet(text=marked("packet"))
    moved_packet = transfer(packet)
    print(moved_packet.text)
    pending: Result[str, Error] = ok(marked("result"))
    moved_result = transfer(pending)
    match moved_result:
        case Ok(value):
            print(value)
        case Err(_):
            print("unexpected")
    absent: i64? = transfer(None)
    match absent:
        case Some(value):
            print(value)
        case None:
            print("none")
    tried = transfer(try parsed())
    print(tried)
    return ok(print("done"))
"#,
    );
    let expected = "7\n7\n12\ntext\ntext\nlibrary\nlibrary\n2\npacket\npacket\nresult\nresult\nnone\ntry\ntry\ndone\n";
    let high = fixture.go("run", "main.nagi", "out-high");
    assert!(
        high.status.success(),
        "{}",
        String::from_utf8_lossy(&high.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&high.stdout), expected);
    let generated = fs::read_to_string(fixture.0.join("out-high/generated.low")).unwrap();
    fixture.write("saved.low", &generated);
    fs::remove_file(fixture.0.join("main.nagi")).unwrap();
    fs::remove_file(fixture.0.join("library.nagi")).unwrap();
    let saved = fixture.go("run", "saved.low", "out-saved");
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    assert_eq!(saved.stdout, high.stdout);

    fixture.write(
        "manual.low",
        "from std.ownership import move;\nfn main() -> unit { let source: str = \"manual\"; let destination: str = move(source); print(destination); }\n",
    );
    let manual = fixture.go("run", "manual.low", "out-manual");
    assert!(
        manual.status.success(),
        "{}",
        String::from_utf8_lossy(&manual.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&manual.stdout), "manual\n");

    let go = fs::read_to_string(fixture.0.join("out-high/go/main.go")).unwrap();
    assert!(go.contains("var __nagi_value_"), "{go}");
    assert!(!go.contains("ownership.move"), "{go}");
}

#[test]
fn move_checker_rejections_happen_before_the_go_toolchain() {
    let fixture = Fixture::new();
    for source in [
        "from std.ownership import move\ndef main():\n    source = \"value\"\n    destination = move(source)\n    print(source)\n",
        "from std.ownership import move\ndef main():\n    source = \"value\"\n    first = move(source)\n    second = move(source)\n",
    ] {
        fixture.write("main.nagi", source);
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args([
                "build",
                "main.nagi",
                "--no-project",
                "--backend",
                "go",
                "--profile",
                "dev",
                "--out",
                "out",
            ])
            .current_dir(&fixture.0)
            .env("NAGI_GO", fixture.0.join("missing-go"))
            .env_remove("NAGI_RUN_RETENTION")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("move後"), "{diagnostic}");
        assert!(diagnostic.contains("main.nagi:5"), "{diagnostic}");
        assert!(!diagnostic.contains("Go toolchain was not found"), "{diagnostic}");
        assert!(!fixture.0.join("out/.nagi/apps").exists());
    }
}

#[test]
fn other_standard_operations_stay_unsupported_before_the_go_toolchain() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        "import std.http.server as http\ndef main():\n    status = http.status(200)\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args([
            "build",
            "main.nagi",
            "--no-project",
            "--backend",
            "go",
            "--profile",
            "dev",
            "--out",
            "out",
        ])
        .current_dir(&fixture.0)
        .env("NAGI_GO", fixture.0.join("missing-go"))
        .env_remove("NAGI_RUN_RETENTION")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("operation stdlib:std.http.server::status is not supported"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("main.nagi:3"), "{diagnostic}");
    assert!(
        !diagnostic.contains("Go toolchain was not found"),
        "{diagnostic}"
    );
    assert!(!fixture.0.join("out/.nagi/apps").exists());
}

#[test]
fn list_index_bounds_fail_at_runtime_without_re_evaluation() {
    let fixture = Fixture::new();
    for (name, index) in [("negative", -1), ("upper", 2)] {
        fixture.write(
            &format!("{name}.nagi"),
            &format!(
                "def index() -> i64:\n    print(\"index\")\n    return {index}\ndef main():\n    values = [1]\n    print(values[index()])\n"
            ),
        );
        let output = fixture.go("run", &format!("{name}.nagi"), &format!("out-{name}"));
        assert!(
            !output.status.success(),
            "{name} index unexpectedly succeeded"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "index\n");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("index out of bounds"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn non_copy_list_iteration_is_rejected_before_the_go_toolchain() {
    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        "class Item:\n    name: str\ndef main():\n    values = [Item(name=\"one\")]\n    for value in values:\n        print(value.name)\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args([
            "build",
            "main.nagi",
            "--no-project",
            "--backend",
            "go",
            "--out",
            "out",
        ])
        .current_dir(&fixture.0)
        .env("NAGI_GO", fixture.0.join("missing-go"))
        .env_remove("NAGI_RUN_RETENTION")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains("non-Copy List elements"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("main.nagi:5"), "{diagnostic}");
    assert!(
        !diagnostic.contains("Go toolchain was not found"),
        "{diagnostic}"
    );
    assert!(!fixture.0.join("out/.nagi/apps").exists());
}

#[test]
fn payload_free_enums_run_from_high_saved_and_handwritten_low() {
    let fixture = Fixture::new();
    fixture.write(
        "states.nagi",
        r#"enum State:
    Ready
    Busy
    Done

def choose() -> State:
    print(10)
    return State.Busy
"#,
    );
    fixture.write(
        "other.nagi",
        r#"enum State:
    Ready
    Busy
    Done

def score(value: State) -> i64:
    match value:
        case State.Ready:
            return 10
        case State.Busy:
            return 20
        case State.Done:
            return 30
"#,
    );
    fixture.write(
        "main.nagi",
        r#"from std.ownership import move
import "states.nagi" as states
import "other.nagi" as other
from "states.nagi" import State as Choice

class Envelope:
    state: Choice

def score(value: Choice) -> i64:
    match value:
        case states.State.Done:
            return 3
        case Choice.Ready:
            return 1
        case states.State.Busy:
            return 2

def once() -> Choice:
    print(11)
    return Choice.Ready

def direct_score() -> i64:
    match once():
        case Choice.Ready:
            return 1
        case Choice.Busy:
            return 2
        case Choice.Done:
            return 3

def internal_error() -> Result[i64, Choice]:
    pending: Result[Choice, Choice] = fail(Choice.Done)
    value = try pending
    return ok(score(value))

def main():
    selected = states.choose()
    transferred = move(selected)
    print(score(selected))
    print(score(transferred))
    values = [Choice.Ready(), states.State.Done]
    for value in values:
        print(score(value))
    print(other.score(other.State.Done))
    print(direct_score())
    envelope = Envelope(state=Choice.Busy)
    print(score(envelope.state))
    optional: Choice? = some(Choice.Ready)
    match optional:
        case Some(value):
            print(score(value))
        case None:
            print(0)
    match internal_error():
        case Ok(value):
            print(value)
        case Err(problem):
            print(score(problem))
"#,
    );
    let high = fixture.go("run", "main.nagi", "out-high-enum");
    assert!(
        high.status.success(),
        "{}",
        String::from_utf8_lossy(&high.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&high.stdout),
        "10\n2\n2\n1\n3\n30\n11\n1\n2\n1\n3\n"
    );
    let go = fs::read_to_string(fixture.0.join("out-high-enum/go/main.go")).unwrap();
    assert_eq!(go.matches("type nagiEnum_").count(), 2, "{go}");
    assert!(go.contains("switch __nagi_match_"), "{go}");
    assert!(!go.contains("interface{}"), "{go}");

    let generated = fs::read_to_string(fixture.0.join("out-high-enum/generated.low")).unwrap();
    fixture.write("saved.low", &generated);
    fs::remove_file(fixture.0.join("main.nagi")).unwrap();
    fs::remove_file(fixture.0.join("states.nagi")).unwrap();
    fs::remove_file(fixture.0.join("other.nagi")).unwrap();
    let saved = fixture.go("run", "saved.low", "out-saved-enum");
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    assert_eq!(saved.stdout, high.stdout);

    fixture.write(
        "hand.low",
        r#"from std.ownership import move;
enum State { Ready; Busy; Done; }
fn score(value: State) -> i64 {
    match value {
        case State.Ready { return 1; }
        case State.Busy { return 2; }
        case State.Done { return 3; }
    }
}
fn main() -> unit {
    selected = State.Busy;
    transferred = move(selected);
    print(score(selected));
    print(score(transferred));
}
"#,
    );
    let handwritten = fixture.go("run", "hand.low", "out-hand-enum");
    assert!(
        handwritten.status.success(),
        "{}",
        String::from_utf8_lossy(&handwritten.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&handwritten.stdout), "2\n2\n");
}

#[test]
fn enum_payload_and_observable_enum_output_fail_before_the_go_toolchain() {
    for (name, source, expected_line, message) in [
        (
            "payload",
            "enum State:\n    Ready\n    Busy(reason: str)\ndef main():\n    print(1)\n",
            3,
            "payload variant Busy",
        ),
        (
            "print",
            "enum State:\n    Ready\ndef main():\n    print(State.Ready)\n",
            4,
            "printに",
        ),
        (
            "main-error",
            "enum Problem:\n    Failed\ndef main() -> Result[unit, Problem]:\n    return fail(Problem.Failed)\n",
            3,
            "main Result error types containing enum",
        ),
        (
            "nested-main-error",
            "enum Code:\n    Failed\nclass Inner:\n    codes: List[Code?]\nclass Problem:\n    inner: Inner\ndef main() -> Result[unit, Problem]:\n    return fail(Problem(inner=Inner(codes=[some(Code.Failed)])))\n",
            7,
            "main Result error types containing enum",
        ),
        (
            "comparison",
            "enum State:\n    Ready\n    Busy\ndef main():\n    print(State.Ready == State.Busy)\n",
            5,
            "==による比較に対応していません",
        ),
    ] {
        let fixture = Fixture::new();
        fixture.write("main.nagi", source);
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args([
                "build",
                "main.nagi",
                "--no-project",
                "--backend",
                "go",
                "--out",
                "out",
            ])
            .current_dir(&fixture.0)
            .env("NAGI_GO", fixture.0.join("missing-go"))
            .env_remove("NAGI_RUN_RETENTION")
            .output()
            .unwrap();
        assert!(!output.status.success(), "{name} unexpectedly succeeded");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains(message), "{name}: {diagnostic}");
        assert!(
            diagnostic.contains(&format!("main.nagi:{expected_line}")),
            "{name}: {diagnostic}"
        );
        assert!(
            !diagnostic.contains("Go toolchain was not found"),
            "{name}: {diagnostic}"
        );
        assert!(!fixture.0.join("out/.nagi/apps").exists(), "{name}");
    }

    let fixture = Fixture::new();
    fixture.write(
        "payload.nagi",
        "enum Imported:\n    Empty\n    Data(value: i64)\n",
    );
    fixture.write(
        "main.nagi",
        "import \"payload.nagi\" as unused\ndef main():\n    print(1)\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args([
            "build",
            "main.nagi",
            "--no-project",
            "--backend",
            "go",
            "--out",
            "out",
        ])
        .current_dir(&fixture.0)
        .env("NAGI_GO", fixture.0.join("missing-go"))
        .env_remove("NAGI_RUN_RETENTION")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("payload variant Data"), "{diagnostic}");
    assert!(diagnostic.contains("payload.nagi:3"), "{diagnostic}");
    assert!(!diagnostic.contains("Go toolchain was not found"));

    let fixture = Fixture::new();
    fixture.write(
        "main.nagi",
        "enum Code:\n    Failed\nclass Problem:\n    code: Code\ndef main() -> Result[unit, Problem]:\n    return fail(Problem(code=Code.Failed))\n",
    );
    let lower = fixture.command(&["lower", "main.nagi", "--no-project", "--out", "lowered"]);
    assert!(
        lower.status.success(),
        "{}",
        String::from_utf8_lossy(&lower.stderr)
    );
    fs::remove_file(fixture.0.join("main.nagi")).unwrap();
    let saved = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args([
            "build",
            "lowered/generated.low",
            "--no-project",
            "--backend",
            "go",
            "--out",
            "out-saved",
        ])
        .current_dir(&fixture.0)
        .env("NAGI_GO", fixture.0.join("missing-go"))
        .env_remove("NAGI_RUN_RETENTION")
        .output()
        .unwrap();
    assert!(!saved.status.success());
    let diagnostic = String::from_utf8_lossy(&saved.stderr);
    assert!(
        diagnostic.contains("main Result error types containing enum"),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("Go toolchain was not found"));

    fixture.write(
        "manual.low",
        "enum Code { Failed; }\nrecord Problem { code: Code; }\nfn main() -> Result[unit, Problem] { return fail(Problem(code=Code.Failed)); }\n",
    );
    let manual = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args([
            "build",
            "manual.low",
            "--no-project",
            "--backend",
            "go",
            "--out",
            "out-manual",
        ])
        .current_dir(&fixture.0)
        .env("NAGI_GO", fixture.0.join("missing-go"))
        .env_remove("NAGI_RUN_RETENTION")
        .output()
        .unwrap();
    assert!(!manual.status.success());
    let diagnostic = String::from_utf8_lossy(&manual.stderr);
    assert!(
        diagnostic.contains("main Result error types containing enum"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("manual.low:3"), "{diagnostic}");
    assert!(!diagnostic.contains("Go toolchain was not found"));
}

#[test]
fn integer_profile_and_signed_division_overflow_match_the_rust_contract() {
    let fixture = Fixture::new();
    fixture.write(
        "decimal.nagi",
        r#"def main():
    ten: i64 = 010
    eight: i64 = 08
    minimum: i8 = -0128
    print(ten)
    print(eight)
    print(minimum)
"#,
    );
    let decimal = fixture.go("run", "decimal.nagi", "decimal");
    assert!(
        decimal.status.success(),
        "{}",
        String::from_utf8_lossy(&decimal.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&decimal.stdout), "10\n8\n-128\n");

    fixture.write(
        "wrap.nagi",
        r#"def main():
    added: u8 = 255 + 1
    subtracted: u8 = 0 - 1
    multiplied: u8 = 200 * 2
    print(added)
    print(subtracted)
    print(multiplied)
"#,
    );
    let release = fixture.command(&[
        "run",
        "wrap.nagi",
        "--no-project",
        "--backend",
        "go",
        "--profile",
        "release",
        "--out",
        "release",
    ]);
    assert!(
        release.status.success(),
        "{}",
        String::from_utf8_lossy(&release.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&release.stdout), "0\n255\n144\n");
    let development = fixture.go("run", "wrap.nagi", "development");
    assert!(!development.status.success());
    assert!(
        String::from_utf8_lossy(&development.stderr).contains("integer overflow"),
        "{}",
        String::from_utf8_lossy(&development.stderr)
    );

    for (name, ty, value, expression) in [
        ("unsigned-add", "u8", "255", "value + 1"),
        ("unsigned-subtract", "u8", "0", "value - 1"),
        ("unsigned-multiply", "u8", "200", "value * 2"),
        ("signed-add", "i8", "127", "value + 1"),
        ("signed-subtract", "i8", "-128", "value - 1"),
        ("signed-multiply", "i8", "100", "value * 2"),
    ] {
        fixture.write(
            "checked.nagi",
            &format!(
                "def operation(value: {ty}) -> {ty}:\n    return {expression}\n\ndef main():\n    print(operation({value}))\n"
            ),
        );
        let out = format!("checked-{name}");
        let output = fixture.go("run", "checked.nagi", &out);
        assert!(!output.status.success(), "{name} unexpectedly succeeded");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("integer overflow"),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for (name, expression) in [
        ("negate", "-value"),
        ("divide", "value / -1"),
        ("remainder", "value % -1"),
    ] {
        fixture.write(
            "overflow.nagi",
            &format!(
                "def operation(value: i64) -> i64:\n    return {expression}\n\ndef main():\n    print(operation(-9223372036854775808))\n"
            ),
        );
        for profile in ["dev", "release"] {
            let out = format!("{name}-{profile}");
            let output = fixture.command(&[
                "run",
                "overflow.nagi",
                "--no-project",
                "--backend",
                "go",
                "--profile",
                profile,
                "--out",
                &out,
            ]);
            let should_panic = profile == "dev" || name != "negate";
            assert_eq!(
                !output.status.success(),
                should_panic,
                "{name}/{profile}: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if should_panic {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("integer overflow"),
                    "{name}/{profile}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            } else {
                assert_eq!(
                    String::from_utf8_lossy(&output.stdout),
                    "-9223372036854775808\n"
                );
            }
        }
    }
}

#[test]
fn compatibility_projection_cannot_mutate_retained_generations() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", "def main():\n    print(1)\n");
    let first = fixture.go("build", "main.nagi", "out");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let (app, initial) = fixture.latest("out");
    let old = app.join(initial["executable"].as_str().unwrap());
    let old_bytes = fs::read(&old).unwrap();
    let projection = fixture.0.join("out/go/main.go");
    fs::remove_file(&projection).unwrap();
    fs::hard_link(&old, &projection).unwrap();

    fixture.write("main.nagi", "def main():\n    print(2)\n");
    let second = fixture.go("build", "main.nagi", "out");
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(fs::read(&old).unwrap(), old_bytes);
    let old_output = Command::new(&old).output().unwrap();
    assert!(old_output.status.success());
    assert_eq!(String::from_utf8_lossy(&old_output.stdout), "1\n");
    assert!(fs::read_to_string(&projection)
        .unwrap()
        .contains("Code generated by nagic"));
}

#[cfg(unix)]
#[test]
fn compatibility_projection_symlink_fails_without_mutating_or_advancing_latest() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write("main.nagi", "def main():\n    print(1)\n");
    let first = fixture.go("build", "main.nagi", "out");
    assert!(first.status.success());
    let (app, initial) = fixture.latest("out");
    let retained = app.join(initial["go"].as_str().unwrap());
    let retained_bytes = fs::read(&retained).unwrap();
    let projection = fixture.0.join("out/go/main.go");
    fs::remove_file(&projection).unwrap();
    symlink(&retained, &projection).unwrap();

    fixture.write("main.nagi", "def main():\n    print(2)\n");
    let failed = fixture.go("build", "main.nagi", "out");
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("cannot replace a symlink"),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
    assert_eq!(fixture.latest("out").1, initial);
    assert_eq!(fs::read(&retained).unwrap(), retained_bytes);

    fs::remove_file(&projection).unwrap();
    fs::remove_file(fixture.0.join("out/go/go.mod")).unwrap();
    fs::remove_dir(fixture.0.join("out/go")).unwrap();
    symlink(retained.parent().unwrap(), fixture.0.join("out/go")).unwrap();
    let directory_alias = fixture.go("build", "main.nagi", "out");
    assert!(!directory_alias.status.success());
    assert!(
        String::from_utf8_lossy(&directory_alias.stderr).contains("plain directory"),
        "{}",
        String::from_utf8_lossy(&directory_alias.stderr)
    );
    assert_eq!(fixture.latest("out").1, initial);
    assert_eq!(fs::read(retained).unwrap(), retained_bytes);
}
