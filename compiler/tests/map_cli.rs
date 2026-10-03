use nagic::project::{self, MapFormat, MapLayout, MapView};
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
        let path = std::env::temp_dir().join(format!(
            "nagi map {} {}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        let file = self.0.join(name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }
    fn run_in(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args(args)
            .current_dir(cwd)
            .env("PATH", self.0.join("no-build-tools"))
            .env("NAGI_ROOT", self.0.join("no-runtime"))
            .output()
            .unwrap()
    }
    fn run(&self, args: &[&str]) -> Output {
        self.run_in(&self.0, args)
    }
    fn no_generated_files(&self) {
        assert!(!self.0.join("build").exists());
        assert!(!self.0.join("native-target").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}
fn graph(output: Output) -> Value {
    let value: Value = serde_json::from_str(&success(output)).unwrap();
    assert_eq!(value["schema_version"], 1);
    value
}
fn failure(output: Output) -> String {
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    String::from_utf8(output.stderr).unwrap()
}
fn named<'a>(value: &'a Value, name: &str) -> &'a Value {
    value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| {
            node["qualified_name"]
                .as_str()
                .is_some_and(|qualified| qualified.ends_with(&format!("::{name}")))
        })
        .unwrap_or_else(|| panic!("missing {name}: {value}"))
}

const HIGH: &str = "class User:\n    name: str\nclass Store:\n    user: User\ndef helper() -> i64:\n    return 1\ndef main():\n    print(helper())\n";
const LOW: &str = "record User { name: str; }\nrecord Store { user: User; }\nfn helper() -> i64 { return 1; }\nfn main() { print(helper()); }\n";

#[test]
fn map_views_check_high_and_low_without_runtime_or_generated_output() {
    let f = Fixture::new();
    f.write("nagi.toml", "[broken manifest");
    f.write("main.nagi", HIGH);
    f.write("main.low", LOW);
    for source in ["main.nagi", "main.low"] {
        let types = graph(f.run(&["map", source, "--format", "json"]));
        assert_eq!(named(&types, "User")["kind"], "class");
        assert_eq!(named(&types, "Store")["kind"], "class");
        let modules = graph(f.run(&["map", "modules", source, "--format", "json"]));
        assert!(modules["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["kind"] == "module"));
        let calls = graph(f.run(&["map", "calls", source, "--format", "json"]));
        let main = &named(&calls, "main")["id"];
        let helper = &named(&calls, "helper")["id"];
        assert!(calls["edges"].as_array().unwrap().iter().any(|edge| {
            edge["from"] == *main && edge["to"] == *helper && edge["kind"] == "calls"
        }));
    }
    let default = success(f.run(&["map", "main.nagi"]));
    assert!(default.contains("User"), "{default}");
    assert!(!default.contains("checked "), "{default}");
    f.no_generated_files();
}

#[test]
fn project_discovery_reuses_native_replacements_and_extern_resolution() {
    let f = Fixture::new();
    f.write(
        "nagi.toml",
        "entry='src/main.nagi'\nnative=['native/replacement.low']\n[rust]\nfile='bridge.rs'\n",
    );
    f.write(
        "src/main.nagi",
        "def value() -> i64:\n    return 1\ndef main():\n    print(value())\n",
    );
    f.write("native/replacement.low", "@replace generated::value\nfn implementation() -> i64 { return bridge(); }\n@rust(\"native::bridge\")\nextern fn bridge() -> i64;\n");
    // map validates Nagi resolution without invoking or parsing the Rust backend.
    f.write("bridge.rs", "not valid Rust; map must not compile it\n");
    let value = graph(f.run_in(&f.0.join("src"), &["map", "calls", "--format", "json"]));
    let caller = &named(&value, "value")["id"];
    let bridge = &named(&value, "bridge")["id"];
    assert!(value["edges"].as_array().unwrap().iter().any(|edge| {
        edge["from"] == *caller && edge["to"] == *bridge && edge["kind"] == "calls"
    }));
    assert!(named(&value, "bridge")["source"]["file"]
        .as_str()
        .unwrap()
        .ends_with("replacement.low"));
    f.no_generated_files();
}

#[test]
fn namespace_module_filter_and_focus_select_original_definitions() {
    let f = Fixture::new();
    f.write(
        "models.nagi",
        "class User:\n    name: str\nclass Unrelated:\n    count: i64\n",
    );
    f.write("main.nagi", "import \"models.nagi\" as models\nclass Store:\n    user: models.User\ndef main():\n    print(1)\n");
    let value = graph(f.run(&[
        "map",
        "types",
        "main.nagi",
        "--format",
        "json",
        "--module",
        "models",
        "--focus",
        "User",
        "--depth",
        "0",
    ]));
    assert_eq!(value["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(named(&value, "User")["kind"], "class");
    assert!(value["edges"].as_array().unwrap().is_empty());
    let error = failure(f.run(&["map", "main.nagi", "--focus", "Missing"]));
    assert!(error.contains("Missing"), "{error}");
    f.no_generated_files();
}

#[test]
fn text_formats_write_only_requested_output_and_keep_stdout_clean() {
    let f = Fixture::new();
    f.write("main.nagi", HIGH);
    for format in ["mermaid", "d2", "json", "html"] {
        let filename = format!("map.{format}");
        let output = f.run(&[
            "map",
            "types",
            "main.nagi",
            "--format",
            format,
            "--output",
            &filename,
        ]);
        assert_eq!(success(output), "");
        let text = fs::read_to_string(f.0.join(&filename)).unwrap();
        assert!(text.contains("User"), "{format}: {text}");
        if format == "json" {
            assert_eq!(
                serde_json::from_str::<Value>(&text).unwrap()["schema_version"],
                1
            );
        } else if format == "html" {
            assert!(text.to_ascii_lowercase().contains("<!doctype html>"));
        }
    }
    let error = failure(f.run(&["map", "main.nagi", "--output", "missing/map.json"]));
    assert!(
        error.contains("Cannot write map") && error.contains("map.json"),
        "{error}"
    );
    assert!(!f.0.join("missing").exists());
    f.no_generated_files();
}

#[test]
fn failed_check_points_to_original_source_and_writes_no_map() {
    let f = Fixture::new();
    f.write(
        "bad.nagi",
        "def wrong() -> i64:\n    return \"wrong\"\ndef main():\n    print(wrong())\n",
    );
    let error = failure(f.run(&[
        "map",
        "bad.nagi",
        "--format",
        "json",
        "--output",
        "failed.json",
    ]));
    assert!(
        error.contains("bad.nagi:2") && error.contains("return \"wrong\""),
        "{error}"
    );
    assert!(!error.contains("generated.low"), "{error}");
    assert!(!f.0.join("failed.json").exists());
    f.no_generated_files();
}

#[test]
fn map_output_cannot_overwrite_loaded_input_or_project_configuration() {
    let f = Fixture::new();
    let imported = "class Item:\n    count: i64\n";
    let entry = "import \"models.nagi\" as models\ndef main():\n    print(1)\n";
    let manifest = "entry='main.nagi'\n[rust]\nfile='bridge.rs'\n";
    f.write("main.nagi", entry);
    f.write("models.nagi", imported);
    f.write("nagi.toml", manifest);
    f.write("bridge.rs", "adapter source\n");
    for (file, original) in [
        ("main.nagi", entry),
        ("models.nagi", imported),
        ("nagi.toml", manifest),
        ("bridge.rs", "adapter source\n"),
    ] {
        let error = failure(f.run(&["map", "--project", ".", "--output", file]));
        assert!(error.contains("cannot overwrite input"), "{error}");
        assert_eq!(fs::read_to_string(f.0.join(file)).unwrap(), original);
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(f.0.join("models.nagi"), f.0.join("alias.json")).unwrap();
        let error = failure(f.run(&["map", "--project", ".", "--output", "alias.json"]));
        assert!(error.contains("cannot overwrite input"), "{error}");
        assert_eq!(
            fs::read_to_string(f.0.join("models.nagi")).unwrap(),
            imported
        );
    }
    f.no_generated_files();
}

#[test]
fn image_formats_require_output_and_fail_helpfully_without_local_d2() {
    let f = Fixture::new();
    f.write("main.nagi", HIGH);
    for format in ["svg", "png"] {
        let error = failure(f.run(&["map", "main.nagi", "--format", format]));
        assert!(error.contains("requires --output"), "{error}");
        let filename = format!("map.{format}");
        let error = failure(f.run(&[
            "map",
            "main.nagi",
            "--format",
            format,
            "--output",
            &filename,
        ]));
        assert!(
            error.contains("D2") && error.contains("--format d2"),
            "{error}"
        );
        assert!(!f.0.join(filename).exists());
    }
    f.no_generated_files();
}

#[test]
fn help_and_argument_errors_do_not_load_a_project_or_mutate_files() {
    let f = Fixture::new();
    f.write("nagi.toml", "[broken project");
    let help = success(f.run(&["map", "--help"]));
    for word in [
        "map", "types", "modules", "calls", "--format", "--focus", "--output", "--layout",
    ] {
        assert!(help.contains(word), "{help}");
    }
    for (args, message) in [
        (vec!["map", "architecture"], "not implemented"),
        (vec!["map", "dataflow"], "not implemented"),
        (vec!["map", "trace"], "not implemented"),
        (vec!["map", "--serve"], "not implemented"),
        (vec!["map", "--editor-input"], "check/symbols"),
        (vec!["map", "--out", "generated"], "--output"),
        (vec!["map", "--depth", "-1"], "nonnegative integer"),
        (vec!["map", "--format", "yaml"], "unsupported map format"),
        (vec!["map", "--format"], "requires a value"),
        (
            vec!["map", "--format", "json", "--format", "json"],
            "only once",
        ),
        (
            vec!["map", "--layout", "tala"],
            "only with --format svg/png",
        ),
    ] {
        let error = failure(f.run(&args));
        assert!(error.contains(message), "{args:?}: {error}");
    }
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}

#[test]
fn map_preparse_keeps_existing_project_overrides_and_output_path_precedence() {
    let f = Fixture::new();
    f.write(
        "project/nagi.toml",
        "entry='entry.nagi'\nnative=['base.low']\n[rust]\nfile='adapter.rs'\n",
    );
    f.write("project/adapter.rs", "");
    f.write("override.rs", "");
    let args = [
        "map",
        "calls",
        "standalone.low",
        "--project",
        "project",
        "--native",
        "extra.low",
        "--rust",
        "override.rs",
        "--format",
        "svg",
        "--layout",
        "tala",
        "--output",
        "map.svg",
    ];
    let (input, map) = project::resolve_map(
        &args
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        &f.0,
    )
    .unwrap();
    assert_eq!(input.command, "map");
    assert_eq!(input.source, f.0.join("standalone.low"));
    assert_eq!(
        input.native,
        [f.0.join("project/base.low"), f.0.join("extra.low")]
    );
    assert_eq!(
        input.rust_file,
        Some(fs::canonicalize(f.0.join("override.rs")).unwrap())
    );
    assert_eq!(map.view, MapView::Calls);
    assert_eq!(map.format, MapFormat::Svg);
    assert_eq!(map.layout, MapLayout::Tala);
    assert_eq!(map.output, Some(f.0.join("map.svg")));
    f.no_generated_files();
}
