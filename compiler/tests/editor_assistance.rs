use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
static ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        for _ in 0..100 {
            let p = std::env::temp_dir().join(format!(
                "nagi-assist-{}-{}",
                std::process::id(),
                ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match fs::create_dir(&p) {
                Ok(()) => return Self(p),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("{e}"),
            }
        }
        panic!("fixture directory collision")
    }
    fn write(&self, file: &str, text: &str) {
        fs::write(self.0.join(file), text).unwrap();
    }
    fn analyze(&self, text: &str, needle: &str) -> serde_json::Value {
        let offset = text.rfind(needle).unwrap() + needle.len();
        let prefix = &text[..offset];
        let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap().encode_utf16().count() + 1;
        self.request(serde_json::json!({"files":[{"file":"main.nagi","text":text}],"query":{"file":"main.nagi","line":line,"column":column}}))
    }
    fn request(&self, input: serde_json::Value) -> serde_json::Value {
        self.request_entry("main.nagi", input)
    }
    fn request_entry(&self, entry: &str, input: serde_json::Value) -> serde_json::Value {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nagic"));
        command.args(["assist", entry, "--editor-input"]);
        if self.0.join("nagi.toml").exists() {
            command.args(["--project", "nagi.toml"]);
        }
        let mut child = command
            .current_dir(&self.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["format"], "nagi-assist-v1");
        assert_eq!(value["full_compile_checked"], false);
        value
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn names(v: &serde_json::Value) -> Vec<&str> {
    v["completion"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect()
}

#[test]
fn elif_conditions_and_bodies_keep_compiler_completion_metadata_and_module_origins() {
    let f = Fixture::new();
    let text = "def inspect(value: i64):\n    if value == 0:\n        print(0)\n    elif value > 0:\n        print(value)\n    else:\n        print(-1)\n";
    f.write("main.nagi", text);
    for needle in ["elif value", "print(value"] {
        let v = f.analyze(text, needle);
        let item = v["completion"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == "value")
            .unwrap_or_else(|| panic!("{v}"));
        assert_eq!(item["type"], "i64");
        assert_eq!(item["target"]["line"], 1);
    }
    f.write(
        "main.nagi",
        "import \"branch.nagi\"\ndef main():\n    print(0)\n",
    );
    f.write(
        "branch.nagi",
        "def inspect():\n    if True:\n        print(0)\n    elif missing:\n        print(1)\n",
    );
    let v = f.request(serde_json::json!({"files":[]}));
    assert!(v["diagnostics"][0]["file"]
        .as_str()
        .unwrap()
        .ends_with("branch.nagi"));
    assert_eq!(v["diagnostics"][0]["line"], 4);
}

#[test]
fn import_queries_reject_parenthesis_and_bracket_continuations() {
    let f = Fixture::new();
    f.write("main.nagi", "");
    f.write("main.low", "");
    f.write("foo.nagi", "def run() -> i64:\n    return 1\n");
    f.write("foo.low", "fn run() -> i64 { return 1; }\n");
    assert_eq!(names(&f.analyze("import fo", "fo")), ["foo"]);
    for (entry, text) in [
        ("main.nagi", "def call(\n    import fo\n)\n"),
        ("main.nagi", "values = [\n    import fo\n]\n"),
        ("main.low", "fn call(\n    import fo\n) {}\n"),
        ("main.nagi", "(]\nimport fo\n"),
        ("main.nagi", ")\nimport fo\n"),
    ] {
        let v = f.request_entry(entry, serde_json::json!({
            "files":[{"file":entry,"text":text}],
            "query":{"file":entry,"line":2,"column":text.lines().nth(1).unwrap().encode_utf16().count() + 1}
        }));
        assert!(v["completion"].get("context").is_none(), "{text}: {v}");
        assert!(names(&v).is_empty(), "{text}: {v}");
        assert!(
            !v["diagnostics"].as_array().unwrap().is_empty(),
            "original errors must remain visible: {v}"
        );
    }
    for text in ["# import fo", "\"import fo\""] {
        let v = f.analyze(text, "fo");
        assert!(v["completion"].get("context").is_none(), "{text}: {v}");
        assert!(names(&v).is_empty(), "{text}: {v}");
    }
    for text in [
        "# (]\nimport fo",
        "def main():\n    value = \"(]\"\nimport fo",
        "class Box:\n    values: List[i64]\nimport fo",
    ] {
        assert_eq!(names(&f.analyze(text, "fo")), ["foo"], "{text}");
    }
}

#[test]
fn import_source_catalog_and_from_exports_use_original_ranges_and_overlays() {
    let f = Fixture::new();
    f.write("main.nagi", "");
    f.write("foo.nagi", "from \"hidden.nagi\" import hidden\nclass Thing:\n    value: i64\ndef run() -> i64:\n    return 1\n");
    f.write("hidden.nagi", "def hidden() -> i64:\n    return 1\n");
    f.write("wrong.low", "");
    f.write("std.nagi", "");
    let v = f.analyze("import fo\r\n", "import fo");
    assert_eq!(v["completion"]["context"], "import-source");
    assert_eq!(names(&v), ["foo"]);
    assert_eq!(v["completion"]["replacement"]["column"], 8);
    assert_eq!(v["completion"]["replacement"]["length"], 2);
    assert_eq!(
        v["completion"]["directories"],
        serde_json::json!([fs::canonicalize(&f.0).unwrap()])
    );
    assert_eq!(v["frontend_accepted"], false);
    assert!(!v["diagnostics"].as_array().unwrap().is_empty());
    let v = f.analyze("from foo import Thing, ru", "ru");
    assert_eq!(v["completion"]["context"], "import-names");
    assert_eq!(names(&v), ["run"]);
    assert_eq!(v["completion"]["replacement"]["column"], 24);
    assert!(v["completion"]["items"][0].get("signature").is_none());
    let v = f.request(serde_json::json!({"files":[{"file":"main.nagi","text":"from foo import "},{"file":"foo.nagi","text":"enum Changed:\n    Empty\n"}],"query":{"file":"main.nagi","line":1,"column":17}}));
    assert_eq!(names(&v), ["Changed"]);
    assert_eq!(v["completion"]["items"][0]["kind"], "enum");
    for text in [
        "import foo as ",
        "from foo import Thing as ",
        "def main():\n    import ",
        "# import ",
        "def main():\n    value = \"import \"",
    ] {
        let v = f.analyze(text, text);
        assert!(names(&v).is_empty(), "{text}: {v}");
    }
}

#[test]
fn import_queries_handle_empty_std_prefix_utf16_and_full_token_replacement() {
    let f = Fixture::new();
    f.write("main.nagi", "");
    f.write("foo.nagi", "def run() -> i64:\n    return 1\n");
    f.write("🦀.nagi", "def run() -> i64:\n    return 1\n");
    let empty = f.analyze("from ", "from ");
    assert!(names(&empty).contains(&"foo"));
    assert!(!names(&empty).contains(&"main"));
    assert_eq!(empty["completion"]["replacement"]["length"], 0);
    let std = f.analyze("import std.ht", "std.ht");
    assert!(names(&std).contains(&"std.http.server"));
    assert!(names(&std).iter().all(|name| name.starts_with("std.ht")));
    assert_eq!(std["completion"]["replacement"]["column"], 8);
    assert_eq!(std["completion"]["replacement"]["length"], 6);
    let unicode = "from \"🦀.nagi\" import ru\r\n";
    let v = f.analyze(unicode, "ru");
    assert_eq!(names(&v), ["run"]);
    assert_eq!(
        v["completion"]["replacement"]["column"],
        unicode.split("ru").next().unwrap().encode_utf16().count() + 1
    );
    let middle = f.request(serde_json::json!({"files":[{"file":"main.nagi","text":"import foobar"}],"query":{"file":"main.nagi","line":1,"column":10}}));
    assert_eq!(names(&middle), ["foo"]);
    assert_eq!(middle["completion"]["replacement"]["length"], 6);
    let exports = f.analyze("from std.http.server import Sta", "Sta");
    assert_eq!(names(&exports), ["Status"]);
    assert!(exports["completion"].get("directories").is_none());
    assert!(exports["completion"]["items"][0].get("signature").is_none());
}

#[test]
fn import_source_enumeration_counts_irrelevant_entries_and_fails_closed_at_budget() {
    let f = Fixture::new();
    f.write("main.nagi", "");
    f.write("foo.nagi", "def run() -> i64:\n    return 1\n");
    for i in 0..8192 {
        f.write(&format!("irrelevant-{i}.txt"), "");
    }
    let v = f.analyze("import fo", "fo");
    assert_eq!(v["completion"]["incomplete"], true);
    assert!(names(&v).is_empty());
    f.write(
        "main.nagi",
        "import foo\ndef main():\n    print(foo.run())\n",
    );
    let loaded = nagic::source::load(&f.0.join("main.nagi"), true).unwrap();
    let mut program = loaded.program;
    nagic::check::check(&mut program).unwrap();
}

#[test]
fn low_import_queries_and_unchecked_export_names_do_not_claim_verified_types() {
    let f = Fixture::new();
    f.write("main.low", "");
    f.write("foo.low", "record Thing { value: i64; }\n");
    f.write("foo.nagi", "class Wrong:\n    value: i64\n");
    let v = f.request_entry("main.low", serde_json::json!({"files":[{"file":"main.low","text":"from foo import Thi"}],"query":{"file":"main.low","line":1,"column":20}}));
    assert_eq!(names(&v), ["Thing"]);
    assert_eq!(v["completion"]["context"], "import-names");
    f.write("main.nagi", "");
    f.write(
        "foo.nagi",
        "def run(value: Unknown) -> Unknown:\n    return value\n",
    );
    let v = f.analyze("from foo import run", "run");
    assert_eq!(names(&v), ["run"]);
    assert_eq!(v["frontend_checked"], true);
    assert_eq!(v["frontend_accepted"], false);
    assert!(v["diagnostics"][0]["file"]
        .as_str()
        .unwrap()
        .ends_with("foo.nagi"));
    assert!(v["completion"]["items"][0].get("signature").is_none());
    assert!(v["completion"]["items"][0].get("type").is_none());
    assert!(
        v["symbols"].get("definitions").is_none(),
        "unchecked import navigation must not publish signatures: {v}"
    );
    for (text, column) in [
        ("import foo()", 11),
        ("import foo, bar", 11),
        ("from foo import run()", 20),
        ("from foo import run.more", 20),
    ] {
        let v = f.request(serde_json::json!({"files":[{"file":"main.nagi","text":text}],"query":{"file":"main.nagi","line":1,"column":column}}));
        assert!(names(&v).is_empty(), "{text}: {v}");
        assert!(v["completion"].get("context").is_none());
    }
}
#[test]
fn checker_completion_uses_unsaved_types_and_excludes_moved_and_out_of_scope_bindings() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    saved = 1\n");
    let text = "from std.ownership import move\ndef main():\n    live = True\n    moved = \"value\"\n    sink = move(moved)\n    if True:\n        hidden = 1\n    li\n";
    let v = f.analyze(text, "    li\n".trim_end());
    assert!(names(&v).contains(&"live"), "{}", v["completion"]);
    for excluded in ["saved", "moved", "hidden"] {
        assert!(!names(&v).contains(&excluded), "{}", v["completion"]);
    }
    let live = v["completion"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "live")
        .unwrap();
    assert_eq!(live["type"], "bool");
}
#[test]
fn member_prefix_recovery_preserves_original_diagnostic_and_remaining_fields() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "from std.ownership import move\nclass Pair:\n    first: str\n    second: i64\ndef inspect(pair: Pair):\n    taken = move(pair.first)\n    pair.\n";
    let v = f.analyze(text, "pair.");
    assert_eq!(v["recovered"], true);
    assert_eq!(names(&v), ["second"]);
    assert_eq!(v["diagnostics"][0]["line"], 7);
    assert_eq!(v["frontend_accepted"], false);
    assert!(v["symbols"]["references"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["location"]["line"] == 7
            && r["location"]["column"] == 5
            && r["target"]["line"] == 5));
}
#[test]
fn borrowed_members_are_read_probes_and_do_not_promise_consumption() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "class Item:\n    text: str\n    count: i64\ndef inspect(item: shared[Item]):\n    item.te\n";
    let v = f.analyze(text, "item.te");
    assert_eq!(names(&v), ["count", "text"]);
    assert!(v["completion"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["access"] == "read" && i["borrowed"] == true));
    assert!(!v["diagnostics"].as_array().unwrap().is_empty());
}
#[test]
fn iterator_member_candidates_preserve_borrowed_receiver_metadata() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "class Item:\n    text: str\n    count: i64\ndef inspect(items: List[Item]):\n    for item in items:\n        item.\n";
    let v = f.analyze(text, "item.");
    assert_eq!(names(&v), ["count", "text"]);
    assert!(
        v["completion"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["borrowed"] == true && item["access"] == "read"),
        "{}",
        v["completion"]
    );
}
#[test]
fn nested_member_candidates_preserve_shared_ancestors_and_owned_counterexample() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    for (receiver, borrowed) in [("shared[Outer]", true), ("Outer", false)] {
        let text = format!("class Inner:\n    text: str\nclass Outer:\n    inner: Inner\ndef inspect(item: {receiver}):\n    item.inner.\n");
        let v = f.analyze(&text, "item.inner.");
        assert_eq!(names(&v), ["text"], "{receiver}");
        assert_eq!(
            v["completion"]["items"][0]["borrowed"], borrowed,
            "{receiver}: {}",
            v["completion"]
        );
        assert_eq!(v["completion"]["items"][0]["access"], "read");
    }
}
#[test]
fn unavailable_shadowed_function_does_not_reappear_as_a_global() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "from std.ownership import move\ndef value() -> i64:\n    return 1\ndef inspect():\n    value = \"text\"\n    gone = move(value)\n    va\n";
    let v = f.analyze(text, "    va");
    assert!(!names(&v).contains(&"value"));
    assert!(names(&v).contains(&"gone"));
}
#[test]
fn imported_unsaved_types_and_namespace_exports_are_resolver_authoritative() {
    let f = Fixture::new();
    f.write("hidden.nagi", "class Hidden:\n    other: bool\n");
    f.write(
        "orders.nagi",
        "import \"hidden.nagi\"\nclass Order:\n    saved: bool\ndef make() -> i64:\n    return 1\n",
    );
    let main = "import \"orders.nagi\" as orders\ndef inspect(item: orders.Order):\n    item.\n";
    f.write("main.nagi", main);
    let v = f.request(serde_json::json!({"files":[{"file":"orders.nagi","text":"import \"hidden.nagi\"\nclass Order:\n    fresh: i64\ndef make() -> i64:\n    return 1\n"}],"query":{"file":"main.nagi","line":3,"column":10}}));
    assert_eq!(names(&v), ["fresh"]);
    let v = f.analyze(
        "import \"orders.nagi\" as orders\ndef main():\n    orders.\n",
        "orders.",
    );
    assert_eq!(names(&v), ["Order", "make"]);
    assert!(!names(&v).contains(&"Hidden"));
}
#[test]
fn earlier_invalid_statements_and_unsupported_syntax_cannot_authorize_candidates() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    saved = 1\n");
    for (text, needle) in [
        (
            "def main():\n    live = 1\n    unknown(live)\n    li\n",
            "    li",
        ),
        ("def main():\n    live = 1\n    print(\n", "print("),
    ] {
        let v = f.analyze(text, needle);
        assert!(names(&v).is_empty());
        assert_eq!(v["semantic_status"], "none");
        assert!(!v["diagnostics"].as_array().unwrap().is_empty());
    }
}
#[test]
fn utf16_navigation_and_parser_points_use_original_positions() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "def main():\n    value = 1\n    print(\"😀\"); value\n";
    let v = f.analyze(text, "value\n".trim_end());
    let reference = v["symbols"]["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["location"]["line"] == 3 && r["location"]["column"] == 18)
        .unwrap();
    assert_eq!(reference["target"]["line"], 2);
    assert_eq!(reference["target"]["column"], 5);
    let v = f.analyze("def main():\n    print(\"😀\"); €\n", "€");
    assert_eq!(v["diagnostics"][0]["range"]["column"], 18);
    assert_eq!(v["diagnostics"][0]["range"]["length"], 0);
    let v = f.request(serde_json::json!({"files":[{"file":"main.nagi","text":text}],"query":{"file":"main.nagi","line":3,"column":13}}));
    assert!(names(&v).is_empty()); // Inside the emoji surrogate pair.
}
#[test]
fn original_module_diagnostics_are_line_only_for_checker_errors() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        "import \"dependency.nagi\"\ndef main():\n    print(0)\n",
    );
    f.write("dependency.nagi", "def broken() -> bool:\n    return 1\n");
    let v = f.request(serde_json::json!({"files":[]}));
    assert!(v["diagnostics"][0]["file"]
        .as_str()
        .unwrap()
        .ends_with("dependency.nagi"));
    assert_eq!(v["diagnostics"][0]["line"], 2);
    assert!(v["diagnostics"][0].get("range").is_none());
    assert_eq!(v["frontend_accepted"], false);
}
#[test]
fn cyclic_import_and_unfinished_input_return_structured_failure_without_disk_fallback() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    saved = 1\n");
    f.write("cycle.nagi", "import \"main.nagi\"\n");
    let v = f.analyze("import \"cycle.nagi\"\ndef main():\n    li\n", "    li");
    assert!(names(&v).is_empty());
    assert!(v.get("symbols").is_none());
    assert!(v["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("循環import"));
}
#[test]
fn control_flow_moves_and_empty_cursor_lines_use_checker_state() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    saved = 1\n");
    let branch = "from std.ownership import move\ndef inspect(condition: bool):\n    value = \"text\"\n    if condition:\n        sink = move(value)\n    \n";
    let v = f.analyze(branch, "    \n".trim_end_matches('\n'));
    assert_eq!(v["recovered"], true);
    assert!(names(&v).contains(&"condition"));
    assert!(!names(&v).contains(&"value"));
    assert!(!names(&v).contains(&"sink"));
    let loop_text = "from std.ownership import move\ndef inspect():\n    value = \"text\"\n    for index in range(2):\n        sink = move(value)\n    va\n";
    let v = f.analyze(loop_text, "    va");
    assert!(!names(&v).contains(&"value"));
    assert_eq!(v["frontend_accepted"], false);
}

#[test]
fn failed_blank_line_recovery_preserves_current_source_symbols() {
    let f = Fixture::new();
    for (entry, text, accepted) in [
        ("main.nagi", "# 日本語 😀\nclass Point:\n    x: f64\n    y: f64\n\ndef main():\n    p = Point(x=1.5, y=2.5)\n    print(p.x + p.y)\n", true),
        ("main.low", "# 日本語 😀\nfn main() { let value = 1; print(value); }\n", true),
        ("main.nagi", "def main():\n    value = missing\n", false),
        ("main.low", "fn main() { let value = missing; }\n", false),
    ] {
        f.write(entry, text);
        let baseline = f.request_entry(entry, serde_json::json!({"files":[]}));
        assert_eq!(baseline["frontend_checked"], true, "{entry}: {baseline}");
        assert_eq!(baseline["frontend_accepted"], accepted);
        assert!(baseline.get("symbols").is_some(), "{entry}: {baseline}");
        for overlays in [serde_json::json!([]), serde_json::json!([{"file":entry,"text":text}])] {
            let v = f.request_entry(entry, serde_json::json!({
                "files":overlays,
                "query":{"file":entry,"line":text.lines().count() + 1,"column":1}
            }));
            assert!(v["symbols"] == baseline["symbols"], "{entry}: query must preserve the original index");
            assert_eq!(v["frontend_accepted"], baseline["frontend_accepted"]);
            assert_eq!(v["diagnostics"], baseline["diagnostics"]);
            assert_eq!(v["recovered"], false);
            assert!(names(&v).is_empty());
        }
        let no_newline = text.trim_end_matches('\n');
        let v = f.request_entry(entry, serde_json::json!({
            "files":[{"file":entry,"text":no_newline}],
            "query":{"file":entry,"line":no_newline.lines().count(),
                "column":no_newline.lines().last().unwrap().encode_utf16().count()+1}
        }));
        assert!(v.get("symbols").is_some(), "EOF on the final token retains facts");
        assert_eq!(v["frontend_accepted"], accepted);
    }
}

#[test]
fn failed_recovery_in_imported_file_retains_the_project_graph() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry = 'main.nagi'\n");
    f.write(
        "main.nagi",
        "from helper import value\ndef main():\n    print(value())\n",
    );
    f.write("helper.nagi", "def value() -> i64:\n    return 1\n");
    let baseline = f.request(serde_json::json!({"files":[]}));
    let v = f.request(serde_json::json!({"files":[],
        "query":{"file":"helper.nagi","line":3,"column":1}}));
    assert_eq!(v["frontend_accepted"], true);
    assert!(
        v["symbols"] == baseline["symbols"],
        "the complete current graph survives"
    );
    assert!(names(&v).is_empty());
}

#[test]
fn blank_line_fallback_does_not_hide_original_parse_errors() {
    let f = Fixture::new();
    for (entry, text) in [("main.nagi", "def main(:\n"), ("main.low", "fn main( {\n")] {
        f.write(entry, text);
        let baseline = f.request_entry(entry, serde_json::json!({"files":[]}));
        let v = f.request_entry(
            entry,
            serde_json::json!({
                "files":[],"query":{"file":entry,"line":2,"column":1}
            }),
        );
        assert_eq!(v["frontend_checked"], false);
        assert_eq!(v["frontend_accepted"], false);
        assert_eq!(v["diagnostics"], baseline["diagnostics"]);
        assert!(!v["diagnostics"].as_array().unwrap().is_empty());
        assert!(v.get("symbols").is_none());
        assert!(names(&v).is_empty());
    }
}
#[test]
fn low_and_native_replacement_queries_retain_types_and_original_positions() {
    let f = Fixture::new();
    f.write(
        "nagi.toml",
        "entry = 'main.nagi'\nnative = ['native.low']\n",
    );
    f.write(
        "main.nagi",
        "def twice(input: i64) -> i64:\n    return input * 2\ndef main():\n    print(twice(2))\n",
    );
    f.write(
        "native.low",
        "@replace(\"generated::twice\")\nfn twice(input: i64) -> i64 { return input * 2; }\n",
    );
    let low = "@replace(\"generated::twice\")\r\nfn twice(input: i64) -> i64 { let fresh = input + 1; return fresh; }\r\n";
    let line = low.lines().nth(1).unwrap();
    let column = line[..line.rfind("fresh").unwrap() + 5]
        .encode_utf16()
        .count()
        + 1;
    let v = f.request(serde_json::json!({"files":[{"file":"native.low","text":low}],"query":{"file":"native.low","line":2,"column":column}}));
    assert!(names(&v).contains(&"fresh"));
    assert!(names(&v).contains(&"input"));
    assert_eq!(v["frontend_accepted"], true);
    let fresh = v["completion"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["name"] == "fresh")
        .unwrap();
    assert_eq!(fresh["type"], "i64");
    assert_eq!(fresh["target"]["line"], 2);
    assert!(fresh["target"]["file"]
        .as_str()
        .unwrap()
        .ends_with("native.low"));
    assert!(!f.0.join("build").exists());
}
#[test]
fn inherited_limits_and_invalid_surrogate_positions_do_not_fall_back_to_saved_completion() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    saved = 1\n");
    let v = f.analyze("def main():\n\tchanged = 1\n", "changed");
    assert!(names(&v).is_empty());
    assert!(v["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("タブ"));
    assert!(v["diagnostics"][0].get("range").is_none());
    let mut files = vec![];
    for _ in 0..129 {
        files.push(serde_json::json!({"file":"main.nagi","text":""}));
    }
    let input = serde_json::json!({"files":files});
    let mut child = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args(["assist", "main.nagi", "--editor-input"])
        .current_dir(&f.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("128"));
    assert!(output.stdout.is_empty());
}

#[test]
fn persistent_assistance_reuses_process_without_reusing_old_overlay_facts() {
    use std::io::{BufRead, BufReader};
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    saved = 1\n");
    let mut child = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args(["assist", "main.nagi", "--editor-input", "--serve"])
        .current_dir(&f.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    for name in ["fresh", "latest"] {
        let request = serde_json::json!({"files":[{"file":"main.nagi","text":format!("def main():\n    {name} = True\n    la\n")}],"query":{"file":"main.nagi","line":3,"column":7}});
        writeln!(input, "{request}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        assert_ne!(
            output.read_line(&mut line).unwrap(),
            0,
            "server exited before response"
        );
        let response: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(names(&response).contains(&name));
        assert!(!names(&response).contains(&"saved"));
        if name == "latest" {
            assert!(!names(&response).contains(&"fresh"));
        }
        assert_eq!(response["full_compile_checked"], false);
    }
    drop(input);
    assert!(child.wait().unwrap().success());
}

#[test]
fn frontend_acceptance_matches_normal_check_for_high_saved_and_handwritten_low() {
    let f = Fixture::new();
    let high = "def main():\n    number = 7\n    print(number)\n";
    f.write("main.nagi", high);
    let response = f.request(serde_json::json!({"files":[]}));
    assert_eq!(response["frontend_accepted"], true);
    assert!(response["diagnostics"].as_array().unwrap().is_empty());
    let lower = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(&f.0)
        .args(["lower", "main.nagi", "--out", "generated"])
        .output()
        .unwrap();
    assert!(
        lower.status.success(),
        "{}",
        String::from_utf8_lossy(&lower.stderr)
    );
    f.write(
        "manual.low",
        "fn main() { let number = 7; print(number); }\n",
    );
    for path in ["main.nagi", "generated/generated.low", "manual.low"] {
        let checked = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&f.0)
            .args(["check", path])
            .output()
            .unwrap();
        assert!(
            checked.status.success(),
            "{path}: {}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let mut child = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&f.0)
            .args(["assist", path, "--editor-input"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"files\":[]}")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(response["frontend_accepted"], true, "{path}: {response}");
        assert_eq!(response["full_compile_checked"], false);
    }
}

#[test]
fn persistent_protocol_refuses_malformed_frame_without_fallback() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(1)\n");
    let mut child = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(&f.0)
        .args(["assist", "main.nagi", "--editor-input", "--serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"files\":[],\"unknown\":true}\n{\"files\":[]}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown field"));
}

fn call_member_response(
    fixture: &Fixture,
    entry: &str,
    text: &str,
    needle: &str,
) -> serde_json::Value {
    let offset = text.rfind(needle).unwrap() + needle.len();
    let before = &text[..offset];
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().encode_utf16().count() + 1;
    let files = serde_json::json!([{"file":entry,"text":text}]);
    let original = fixture.request_entry(entry, serde_json::json!({"files":files}));
    let value = fixture.request_entry(
        entry,
        serde_json::json!({"files":files,"query":{"file":entry,"line":line,"column":column}}),
    );
    assert_eq!(value["diagnostics"], original["diagnostics"]);
    assert_eq!(value["frontend_checked"], original["frontend_checked"]);
    assert_eq!(value["frontend_accepted"], original["frontend_accepted"]);
    value
}

fn call_name_response(
    fixture: &Fixture,
    entry: &str,
    text: &str,
    needle: &str,
) -> serde_json::Value {
    let offset = text.rfind(needle).unwrap() + needle.len();
    let before = &text[..offset];
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().encode_utf16().count() + 1;
    let files = serde_json::json!([{"file":entry,"text":text}]);
    let original = fixture.request_entry(entry, serde_json::json!({"files":files}));
    let value = fixture.request_entry(
        entry,
        serde_json::json!({"files":files,"query":{"file":entry,"line":line,"column":column}}),
    );
    assert_eq!(value["diagnostics"], original["diagnostics"]);
    assert_eq!(value["frontend_checked"], original["frontend_checked"]);
    assert_eq!(value["frontend_accepted"], original["frontend_accepted"]);
    value
}

fn assert_print_candidate(value: &serde_json::Value) {
    assert_eq!(value["completion"]["kind"], "names", "{value}");
    assert_eq!(value["semantic_status"], "editor-partial", "{value}");
    assert_eq!(value["frontend_checked"], true, "{value}");
    assert_eq!(value["frontend_accepted"], false, "{value}");
    let matches = value["completion"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["name"] == "print")
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "{value}");
    let candidate = matches[0];
    assert_eq!(candidate["kind"], "function");
    assert_eq!(
        candidate["signature"],
        "print(value) -> unit; accepts i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, bool, UUID, str, view[str]"
    );
    assert_eq!(candidate["access"], "read");
    assert!(candidate.get("type").is_none());
    assert!(candidate.get("target").is_none());
}

#[test]
fn call_member_recovery_high_and_handwritten_low_preserves_original_diagnostics() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    f.write("manual.low", "fn main() { print(0); }\n");
    for prefix in ["user.", "user.na"] {
        for (entry, text) in [
            ("main.nagi", format!("class User:\n    name: str\n    age: i64\ndef inspect(user: User):\n    print({prefix})\n")),
            ("manual.low", format!("record User {{ name: str; age: i64; }}\nfn inspect(user: User) -> unit {{ print({prefix}); }}\n")),
        ] {
            let v = call_member_response(&f, entry, &text, prefix);
            assert_eq!(names(&v), ["age", "name"], "{entry}: {v}");
            assert_eq!(v["completion"]["kind"], "members");
            assert_eq!(v["recovered"], true);
            assert_eq!(v["semantic_status"], "editor-partial");
            assert_eq!(v["frontend_accepted"], false);
            assert!(!v["diagnostics"].as_array().unwrap().is_empty());
            assert!(v["diagnostics"][0]["file"].as_str().unwrap().ends_with(entry));
        }
    }
}

#[test]
fn call_member_nested_closes_unicode_crlf_keep_read_metadata_and_original_positions() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    for text in [
        "class User:\n    name: str\n    age: i64\ndef inspect(user: shared[User]):\n    print(print(user.na))\n",
        "class User:\r\n    name: str\r\n    age: i64\r\ndef inspect(user: shared[User]):\r\n    print(\"😀\"); print(user.na)\r\n",
    ] {
        let v = call_member_response(&f, "main.nagi", text, "user.na");
        assert_eq!(names(&v), ["age", "name"], "{v}");
        assert!(v["completion"]["items"].as_array().unwrap().iter().all(|item| item["access"] == "read" && item["borrowed"] == true));
        let line = text.lines().nth(4).unwrap();
        let column = line[..line.rfind("user.na").unwrap()].encode_utf16().count() + 1;
        assert!(v["symbols"]["references"].as_array().unwrap().iter().any(|r| r["location"]["line"] == 5 && r["location"]["column"] == column && r["target"]["line"] == 4 && r["target"]["file"].as_str().is_some_and(|file| file.ends_with("main.nagi"))), "{v}");
    }
    let text = "class User:\n    name: str\n    age: i64\ndef inspect(user: User):\n    print(\"😀\"); print(user.na)\n";
    for column in [13, 1000] {
        let v=f.request(serde_json::json!({"files":[{"file":"main.nagi","text":text}],"query":{"file":"main.nagi","line":5,"column":column}}));
        assert!(names(&v).is_empty());
        assert_eq!(v["recovered"], false);
    }
}

#[test]
fn call_member_ownership_and_tainted_context_stay_checker_authoritative() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let partial = "from std.ownership import move\nclass Pair:\n    first: str\n    second: i64\ndef inspect(pair: Pair):\n    taken = move(pair.first)\n    print(pair.)\n";
    let v = call_member_response(&f, "main.nagi", partial, "pair.");
    assert_eq!(names(&v), ["second"]);
    for before in ["    gone = move(pair)\n", "    unknown(pair)\n"] {
        let text = format!("from std.ownership import move\nclass Pair:\n    first: str\n    second: i64\ndef inspect(pair: Pair):\n{before}    print(pair.)\n");
        let v = call_member_response(&f, "main.nagi", &text, "pair.");
        assert!(names(&v).is_empty(), "{v}");
        assert_eq!(v["frontend_accepted"], false);
    }
}

#[test]
fn call_member_unsafe_suffix_and_noncode_do_not_fabricate_members() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    for body in [
        "print(user.na + 1)",
        "print(user.na, 1)",
        "print(user.na])",
        "print(user.na))",
        "print(\"user.na\")",
        "# user.na",
        "print(\"user.na",
        "print(user.na) # comment",
    ] {
        let text = format!(
            "class User:\n    name: str\n    age: i64\ndef inspect(user: User):\n    {body}\n"
        );
        let v = call_member_response(&f, "main.nagi", &text, "user.na");
        assert!(names(&v).is_empty(), "{body}: {v}");
        assert_ne!(v["completion"]["kind"], "members");
    }
}

#[test]
fn call_member_saved_low_queries_use_low_file_positions_without_high_mapping() {
    let f = Fixture::new();
    f.write("main.nagi", "class User:\n    name: str\n    age: i64\ndef inspect(user: shared[User]):\n    print(user.age)\ndef main():\n    print(0)\n");
    let lower = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(&f.0)
        .args(["lower", "main.nagi", "--out", "generated"])
        .output()
        .unwrap();
    assert!(
        lower.status.success(),
        "{}",
        String::from_utf8_lossy(&lower.stderr)
    );
    let entry = "generated/generated.low";
    let saved = fs::read_to_string(f.0.join(entry)).unwrap();
    assert!(saved.contains("user.age"));
    for prefix in ["user.", "user.na"] {
        let text = saved.replace("user.age", prefix);
        let v = call_member_response(&f, entry, &text, prefix);
        assert_eq!(names(&v), ["age", "name"], "{v}");
        assert_eq!(v["completion"]["kind"], "members");
        assert_eq!(v["semantic_status"], "editor-partial");
        let line = text[..text.rfind(prefix).unwrap()]
            .bytes()
            .filter(|b| *b == b'\n')
            .count()
            + 1;
        assert!(
            v["symbols"]["references"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["location"]["line"] == line
                    && r["location"]["file"]
                        .as_str()
                        .is_some_and(|file| Path::new(file).ends_with(Path::new(entry)))
                    && r["target"]["file"]
                        .as_str()
                        .is_some_and(|file| Path::new(file).ends_with(Path::new(entry)))),
            "{v}"
        );
        assert!(
            Path::new(v["diagnostics"][0]["file"].as_str().unwrap()).ends_with(Path::new(entry))
        );
    }
}

#[test]
fn compiler_offers_print_for_intrinsic_name_prefixes_across_source_forms() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    for prefix in ["pr", "pri"] {
        let text = format!("def inspect():\n    {prefix}\n");
        let v = call_name_response(&f, "main.nagi", &text, prefix);
        assert_print_candidate(&v);
    }

    f.write("manual.low", "fn inspect() { print(0); }\n");
    let handwritten_low = "fn inspect() { pr; }\n";
    let v = call_name_response(&f, "manual.low", handwritten_low, "pr");
    assert_print_candidate(&v);

    f.write("main.nagi", "def main():\n    print(0)\n");
    let lower = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(&f.0)
        .args(["lower", "main.nagi", "--out", "generated"])
        .output()
        .unwrap();
    assert!(
        lower.status.success(),
        "{}",
        String::from_utf8_lossy(&lower.stderr)
    );
    let entry = "generated/generated.low";
    let saved = fs::read_to_string(f.0.join(entry)).unwrap();
    let saved_low = saved.replace("print(0)", "pr");
    assert_ne!(saved_low, saved, "{saved}");
    let v = call_name_response(&f, entry, &saved_low, "pr");
    assert_print_candidate(&v);
}

#[test]
fn all_supported_intrinsics_are_completion_candidates_in_high_and_low() {
    let f = Fixture::new();
    // Independent supported-language inventory, including diagnostic/benchmark intrinsics.
    let expected = [
        "print",
        "write",
        "read_line",
        "include_text",
        "view",
        "copy",
        "share",
        "clone_shared",
        "len",
        "range",
        "sleep",
        "json_decode",
        "json_encode",
        "ok",
        "some",
        "error",
        "not_found",
        "internal_error",
        "fail",
        "error_kind",
        "error_message",
        "assert_true",
        "parse_i64",
        "parse_f64",
        "make_ints",
        "actor_demo",
        "actor_pair_demo",
        "queue_demo",
        "task_demo",
        "cpu_sum",
        "i64",
        "i32",
        "uuid_parse",
        "uuid_format",
        "append",
        "env",
        "slice",
        "bench_i64",
        "bench_f64",
        "bench_scalar",
        "clock_ns",
        "size_of",
        "supervisor_demo",
    ];
    for (entry, text) in [
        ("main.nagi", "def inspect():\n    candidate\n"),
        ("manual.low", "fn inspect() { candidate; }\n"),
    ] {
        f.write(entry, text);
        let v = call_name_response(&f, entry, text, "candidate");
        let actual = v["completion"]["items"].as_array().unwrap();
        for name in expected {
            let candidates = actual
                .iter()
                .filter(|item| item["name"] == name)
                .collect::<Vec<_>>();
            assert_eq!(candidates.len(), 1, "{entry} {name}: {v}");
            assert_eq!(candidates[0]["kind"], "function");
            assert_eq!(candidates[0]["access"], "read");
            assert!(
                candidates[0]["signature"].as_str().unwrap().contains("->"),
                "{name}"
            );
            assert!(candidates[0].get("target").is_none(), "{name}");
        }
        for retired in [
            "html",
            "serve",
            "db_open",
            "db_exec",
            "db_all",
            "db_query",
            "db_insert",
            "db_update",
            "db_write",
        ] {
            assert!(!names(&v).contains(&retired), "{entry} {retired}: {v}");
        }
    }
    // Every intrinsic passes through the same scope/taint gates, including
    // moved locals, imports and namespace/member contexts tested below.
    for name in expected {
        let text = format!("def inspect():\n    {name} = 1\n    candidate\n");
        let v = call_name_response(&f, "main.nagi", &text, "candidate");
        assert!(
            !v["completion"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["name"] == name && item["kind"] == "function"),
            "{name}: {v}"
        );
    }
}

#[test]
fn compiler_offers_both_read_line_call_forms_with_explicit_result_signature() {
    let f = Fixture::new();
    for (entry, text, needle) in [
        ("main.nagi", "def inspect():\n    read_li\n", "read_li"),
        (
            "main.nagi",
            "def inspect() -> Result[str, Error]:\n    text = try read_li\n    return ok(text)\n",
            "read_li",
        ),
        ("manual.low", "fn inspect() { read_li; }\n", "read_li"),
    ] {
        f.write(entry, text);
        let v = call_name_response(&f, entry, text, needle);
        assert_eq!(v["semantic_status"], "editor-partial", "{v}");
        let candidates = v["completion"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["name"] == "read_line")
            .collect::<Vec<_>>();
        assert_eq!(candidates.len(), 1, "{v}");
        assert_eq!(candidates[0]["kind"], "function");
        assert_eq!(
            candidates[0]["signature"],
            "read_line() -> Result[str, Error]; read_line(prompt: str | view[str]) -> Result[str, Error]"
        );
        assert_eq!(candidates[0]["access"], "read");
        assert!(candidates[0].get("target").is_none(), "{v}");
        assert!(!v["symbols"]["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|definition| definition["name"] == "read_line"));
    }
}

#[test]
fn read_line_completion_respects_local_global_and_import_shadowing() {
    let f = Fixture::new();
    f.write("main.nagi", "");
    f.write("helper.nagi", "def replacement() -> i64:\n    return 1\n");
    for (text, kind) in [
        ("def inspect():\n    read_line = 1\n    read_li\n", "local"),
        (
            "def read_line() -> i64:\n    return 1\ndef inspect():\n    read_li\n",
            "function",
        ),
        (
            "class read_line:\n    value: i64\ndef inspect():\n    read_li\n",
            "class",
        ),
        (
            "enum read_line:\n    Empty\ndef inspect():\n    read_li\n",
            "enum",
        ),
        (
            "from \"helper.nagi\" import replacement as read_line\ndef inspect():\n    read_li\n",
            "function",
        ),
        (
            "from std.result import map_error as read_line\ndef inspect():\n    read_li\n",
            "function",
        ),
        (
            "import \"helper.nagi\" as read_line\ndef inspect():\n    read_li\n",
            "module",
        ),
    ] {
        let v = call_name_response(&f, "main.nagi", text, "read_li");
        let candidates = v["completion"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["name"] == "read_line")
            .collect::<Vec<_>>();
        assert_eq!(candidates.len(), 1, "{text}: {v}");
        assert_eq!(candidates[0]["kind"], kind, "{text}: {v}");
        assert!(candidates[0]["target"].is_object(), "{text}: {v}");
        assert_ne!(
            candidates[0]["signature"], "read_line() -> Result[str, Error]",
            "{text}: {v}"
        );
    }
}

#[test]
fn read_line_intrinsic_does_not_leak_into_members_imports_or_tainted_contexts() {
    let f = Fixture::new();
    f.write("main.nagi", "");
    f.write("helper.nagi", "def replacement() -> i64:\n    return 1\n");
    for text in [
        "class User:\n    name: str\ndef inspect(user: User):\n    user.read_li\n",
        "import \"helper.nagi\" as helper\ndef inspect():\n    helper.read_li\n",
        "from \"helper.nagi\" import read_li",
        "def inspect():\n    unknown_call()\n    read_li\n",
        "from std.ownership import move\ndef inspect():\n    read_line = \"owned\"\n    saved = move(read_line)\n    read_li\n",
        "def inspect():\n    read_line(\n",
    ] {
        let needle = if text.ends_with("(\n") { "read_line(" } else { "read_li" };
        let v = call_name_response(&f, "main.nagi", text, needle);
        assert!(!names(&v).contains(&"read_line"), "{text}: {v}");
    }
}

#[test]
fn read_line_preserves_zero_arguments_and_requires_explicit_result_handling() {
    let f = Fixture::new();
    for (entry, accepted, discarded, prompt, implicit) in [
        ("main.nagi",
         "def inspect() -> Result[str, Error]:\n    text = try read_line()\n    return ok(text)\n",
         "def inspect():\n    read_line()\n",
         "def inspect() -> Result[str, Error]:\n    return read_line(\"prompt\", \"extra\")\n",
         "def inspect() -> str:\n    return read_line()\n"),
        ("manual.low",
         "fn inspect() -> Result[str, Error] { let text = try read_line(); return ok(text); }\n",
         "fn inspect() { read_line(); }\n",
         "fn inspect() -> Result[str, Error] { return read_line(\"prompt\", \"extra\"); }\n",
         "fn inspect() -> str { return read_line(); }\n"),
    ] {
        f.write(entry, accepted);
        let v = f.request_entry(entry, serde_json::json!({"files":[{"file":entry,"text":accepted}]}));
        assert_eq!(v["frontend_accepted"], true, "{entry}: {v}");
        for (text, diagnostic) in [(discarded, "Resultを無視できません"), (prompt, "引数"), (implicit, "型が一致しません")] {
            let v = f.request_entry(entry, serde_json::json!({"files":[{"file":entry,"text":text}]}));
            assert_eq!(v["frontend_checked"], true, "{entry}: {v}");
            assert_eq!(v["frontend_accepted"], false, "{entry}: {v}");
            assert!(v["diagnostics"][0]["message"].as_str().unwrap().contains(diagnostic), "{text}: {v}");
        }
    }
}

#[test]
fn local_print_is_the_only_print_candidate_when_it_shadows_the_intrinsic() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "def inspect():\n    print = 1\n    pri\n";
    let v = call_name_response(&f, "main.nagi", text, "pri");
    let matches = v["completion"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["name"] == "print")
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "{}", v["completion"]);
    assert_eq!(matches[0]["kind"], "local");
    assert_eq!(matches[0]["type"], "i64");
}

#[test]
fn user_print_function_shadows_the_intrinsic_completion_candidate() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "def print(value: i64) -> i64:\n    return value\ndef inspect():\n    pr\n";
    let v = call_name_response(&f, "main.nagi", text, "pr");
    let matches = v["completion"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["name"] == "print")
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "{}", v["completion"]);
    assert_eq!(matches[0]["kind"], "function");
    assert_eq!(matches[0]["signature"], "def print(value: i64) -> i64");
    assert_eq!(matches[0]["target"]["line"], 1);
}

#[test]
fn invalid_prior_statement_does_not_authorize_intrinsic_completion() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "def inspect():\n    unknown_call()\n    pr\n";
    let v = call_name_response(&f, "main.nagi", text, "pr");
    assert_eq!(v["semantic_status"], "none", "{v}");
    assert!(names(&v).is_empty(), "{v}");
}

#[test]
fn local_point_parameter_shadowing_module_alias_keeps_field_candidates() {
    let f = Fixture::new();
    f.write(
        "geometry.nagi",
        "class Marker:\n    id: i64\ndef make() -> i64:\n    return 1\n",
    );
    f.write(
        "main.nagi",
        "import \"geometry.nagi\" as geo\ndef main():\n    print(0)\n",
    );
    let text = "import \"geometry.nagi\" as geo\nclass Point:\n    x: i64\n    y: i64\ndef inspect(geo: Point):\n    geo.\n";
    let v = f.analyze(text, "    geo.");
    assert_eq!(v["completion"]["kind"], "members", "{v}");
    assert_eq!(v["recovered"], true, "{v}");
    assert_eq!(names(&v), ["x", "y"], "{v}");
    assert!(v["completion"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["kind"] == "field" && item["access"] == "read"));
}

fn assert_point_member_candidates(value: &serde_json::Value, entry: &str) {
    assert_eq!(value["recovered"], true, "{value}");
    assert_eq!(value["completion"]["kind"], "members", "{value}");
    assert_eq!(value["semantic_status"], "editor-partial", "{value}");
    assert_eq!(value["frontend_checked"], false, "{value}");
    assert_eq!(value["frontend_accepted"], false, "{value}");
    assert_eq!(names(value), ["x", "y"], "{value}");

    let diagnostics = value["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty(), "{value}");
    assert_eq!(diagnostics[0]["stage"], "load", "{value}");
    assert!(
        diagnostics[0]["file"].as_str().unwrap().ends_with(entry),
        "{value}"
    );

    let files = value["symbols"]["files"].as_array().unwrap();
    assert!(
        files
            .iter()
            .any(|file| file.as_str().is_some_and(|path| path.ends_with(entry))),
        "{value}"
    );
    let items = value["completion"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{value}");
    for item in items {
        assert_eq!(item["kind"], "field", "{value}");
        assert_eq!(item["type"], "f64", "{value}");
        assert_eq!(item["borrowed"], false, "{value}");
        assert_eq!(item["access"], "read", "{value}");
    }
}

fn assert_no_point_member_candidates(value: &serde_json::Value, entry: &str) {
    assert!(names(value).is_empty(), "{value}");
    assert_eq!(value["recovered"], false, "{value}");
    assert_eq!(value["semantic_status"], "none", "{value}");
    assert_eq!(value["frontend_accepted"], false, "{value}");
    assert!(
        value["diagnostics"][0]["file"]
            .as_str()
            .unwrap()
            .ends_with(entry),
        "{value}"
    );
}

#[test]
fn point_member_completion_handles_right_closer_in_high_and_handwritten_low() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    f.write("manual.low", "fn main() { print(0); }\n");

    let high = |call: &str| {
        format!(
            "class Point:\n    x: f64\n    y: f64\n\ndef consume(value: Point):\n    print(value.x)\ndef main():\n    p = Point(x = 1.5, y = 2.5)\n    {call}\n"
        )
    };
    let low = |call: &str| {
        format!(
            "record Point {{ x: f64; y: f64; }}\nfn consume(value: Point) {{ print(value.x); }}\nfn main() {{ let p = Point(x = 1.5, y = 2.5); {call}; }}\n"
        )
    };

    for (entry, source) in [
        ("main.nagi", high("print(p.)")),
        ("main.nagi", high("consume(p.)")),
        ("manual.low", low("print(p.)")),
        ("manual.low", low("consume(p.)")),
    ] {
        for (dot_cursor, after_closer) in [("p.", false), ("p.)", true)] {
            let value = call_member_response(&f, entry, &source, dot_cursor);
            if after_closer {
                assert_no_point_member_candidates(&value, entry);
            } else {
                assert_point_member_candidates(&value, entry);
            }
        }
    }
}

#[test]
fn point_member_completion_does_not_invent_fields_for_unknown_receiver() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    let text = "class Point:\n    x: f64\n    y: f64\ndef main():\n    p = Point(x = 1.5, y = 2.5)\n    print(missing.)\n";
    let value = call_member_response(&f, "main.nagi", text, "missing.");
    assert_eq!(value["recovered"], true, "{value}");
    assert_eq!(value["completion"]["kind"], "members", "{value}");
    assert!(names(&value).is_empty(), "{value}");
    assert_eq!(value["frontend_accepted"], false, "{value}");
    assert!(
        value["diagnostics"][0]["file"]
            .as_str()
            .unwrap()
            .ends_with("main.nagi"),
        "{value}"
    );
}
