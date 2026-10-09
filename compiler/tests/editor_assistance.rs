use std::{
    fs,
    io::Write,
    path::PathBuf,
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_nagic"));
        command.args(["assist", "main.nagi", "--editor-input"]);
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
    assert!(v["completion"]["items"].as_array().unwrap().iter().all(|item| item["borrowed"] == true && item["access"] == "read"), "{}", v["completion"]);
}
#[test]
fn nested_member_candidates_preserve_shared_ancestors_and_owned_counterexample() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    print(0)\n");
    for (receiver, borrowed) in [("shared[Outer]", true), ("Outer", false)] {
        let text = format!("class Inner:\n    text: str\nclass Outer:\n    inner: Inner\ndef inspect(item: {receiver}):\n    item.inner.\n");
        let v = f.analyze(&text, "item.inner.");
        assert_eq!(names(&v), ["text"], "{receiver}");
        assert_eq!(v["completion"]["items"][0]["borrowed"], borrowed, "{receiver}: {}", v["completion"]);
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
