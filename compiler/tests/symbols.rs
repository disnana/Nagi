use nagic::{source, symbols};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi-symbols-{}-{}-{}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, file: &str, text: &str) {
        let path = self.0.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn symbols(&self, files: serde_json::Value) -> serde_json::Value {
        let mut command = Command::new(env!("CARGO_BIN_EXE_nagic"));
        command.args(["symbols", "--editor-input"]);
        if !self.0.join("nagi.toml").exists() {
            command.arg("main.nagi");
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
            .write_all(serde_json::json!({ "files": files }).to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn actual_symbols_command_resolves_transitive_imports_and_utf16_columns() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry = 'main.nagi'\n");
    f.write("models.nagi", "class User:\n    name: str\n");
    f.write(
        "lib/helpers.nagi",
        "import \"../models.nagi\"\ndef make() -> User:\n    return User(name=\"Nagi\")\n",
    );
    f.write("main.nagi", "import \"lib/helpers.nagi\" # make\ndef main():\n    print(\"😀 make()\"); user: User = make()\n    # make()\n    text = \"make()\"\n");
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args(["symbols", "--project"])
        .arg(&f.0)
        .current_dir(&f.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let index: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(index["format"], "nagi-symbols-v1");
    let refs = index["references"].as_array().unwrap();
    let main = fs::canonicalize(f.0.join("main.nagi"))
        .unwrap()
        .display()
        .to_string();
    let helper = fs::canonicalize(f.0.join("lib/helpers.nagi"))
        .unwrap()
        .display()
        .to_string();
    let make = refs
        .iter()
        .find(|r| {
            r["location"]["file"] == main
                && r["location"]["line"] == 3
                && r["target"]["file"] == helper
        })
        .unwrap();
    assert_eq!(make["location"]["column"], 38);
    assert_eq!(make["target"]["line"], 2);
    assert_eq!(make["target"]["column"], 5);
    assert!(!refs
        .iter()
        .any(|r| r["location"]["file"] == main
            && matches!(r["location"]["line"].as_u64(), Some(4 | 5))));
    let import = refs
        .iter()
        .find(|r| r["location"]["file"] == main && r["location"]["line"] == 1)
        .unwrap();
    assert_eq!(import["location"]["length"], 18);
    assert!(
        !f.0.join("build").exists(),
        "symbols must not generate build files"
    );
}

#[test]
fn native_low_symbols_and_calls_inside_match_keep_original_files() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry = 'main.nagi'\nnative = ['math.low']\n");
    f.write("math.low", "fn twice(x: i64) -> i64 { return x * 2; }\n");
    f.write("main.nagi", "def main():\n    match parse_i64(\"21\"):\n        case Ok(number):\n            print(twice(number))\n        case Err(_):\n            print(0)\n");
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .arg("symbols")
        .current_dir(&f.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let index: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let reference = index["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["location"]["line"] == 4)
        .unwrap();
    assert!(reference["target"]["file"]
        .as_str()
        .unwrap()
        .ends_with("math.low"));
    assert_eq!(reference["target"]["line"], 1);
    assert_eq!(reference["target"]["column"], 4);
}

#[test]
fn symbols_work_despite_type_errors_and_skip_fields_and_local_names() {
    let f = Fixture::new();
    f.write("main.nagi", "class User:\n    name: str\ndef helper() -> i64:\n    return \"wrong type\"\ndef main():\n    helper = 7\n    print(helper)\n    user = User(name=\"helper()\")\n    print(user.name)\n    helper = [1]\n    print(helper[0]); print(helper())\n");
    let loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
    let index = symbols::index(&loaded, &[&loaded.program]).unwrap();
    let refs = index["references"].as_array().unwrap();
    assert!(!refs
        .iter()
        .any(|r| r["location"]["line"] == 7 || r["location"]["line"] == 9));
    assert!(refs
        .iter()
        .any(|r| r["location"]["line"] == 8 && r["target"]["line"] == 1));
    let same_line = refs
        .iter()
        .filter(|r| r["location"]["line"] == 11)
        .collect::<Vec<_>>();
    assert_eq!(
        same_line.len(),
        1,
        "indexing a local is not a function call"
    );
    assert_eq!(same_line[0]["location"]["column"], 29);
}

#[test]
fn editor_buffers_update_imported_and_low_signatures_without_writing_files() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry = 'main.nagi'\nnative = ['math.low']\n");
    f.write(
        "main.nagi",
        "import \"models.nagi\"\ndef main():\n    print(1)\n",
    );
    let saved = "class User:\n    name: str\n";
    f.write("models.nagi", saved);
    f.write("math.low", "fn twice(x: i64) -> i64 { return x * 2; }\n");
    let input = serde_json::json!({"files": [
        {"file": "models.nagi", "text": "class User:\n    name: view[str]\n    count: i64\nasync def find(id: i64) -> Result[User?, Error]:\n    return error(\"demo\")\n@rust(\"native::fetch\")\nextern async def fetch() -> Result[i64, Error]\n"},
        {"file": "math.low", "text": "fn twice(x: i32) -> i32 { return x * 2; }\n"}
    ]});
    let mut child = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .args(["symbols", "--editor-input"])
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let index: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let defs = index["definitions"].as_array().unwrap();
    let find = defs.iter().find(|d| d["name"] == "find").unwrap();
    assert_eq!(
        find["signature"],
        "async def find(id: i64) -> Result[User?, Error]"
    );
    assert_eq!(find["return_type"], "Result[User?, Error]");
    assert_eq!(find["asynchronous"], true);
    let external = defs.iter().find(|d| d["name"] == "fetch").unwrap();
    assert_eq!(
        external["signature"],
        "extern async def fetch() -> Result[i64, Error]"
    );
    let user = defs.iter().find(|d| d["name"] == "User").unwrap();
    assert_eq!(user["fields"][0]["type"], "view[str]");
    assert_eq!(user["fields"][1]["name"], "count");
    let low = defs.iter().find(|d| d["name"] == "twice").unwrap();
    assert_eq!(low["parameters"][0]["type"], "i32");
    assert_eq!(low["signature"], "fn twice(x: i32) -> i32");
    assert_eq!(index["files"].as_array().unwrap().len(), 3);
    assert_eq!(fs::read_to_string(f.0.join("models.nagi")).unwrap(), saved);
    assert!(!f.0.join("build").exists());
}

#[test]
fn editor_input_rejects_invalid_schema_alias_duplicates_and_other_commands() {
    let f = Fixture::new();
    f.write("a.nagi", "def main():\n    print(1)\n");
    let duplicate = serde_json::json!({"files": [{"file": "a.nagi", "text": ""}, {"file": "./a.nagi", "text": ""}]}).to_string();
    for data in [
        "{}",
        "{\"files\":[],\"unknown\":true}",
        &duplicate,
        "{\"files\":[{\"file\":\"a.rs\",\"text\":\"\"}]}",
    ] {
        assert!(
            symbols::read_overlays(data.as_bytes(), &f.0).is_err(),
            "{data}"
        );
    }
    for cmd in ["check", "build", "run", "lower"] {
        let args = [cmd, "a.nagi", "--editor-input"].map(str::to_owned);
        assert!(nagic::project::resolve(&args, &f.0).is_err());
    }
}

fn local_types(index: &serde_json::Value, line: u64, name: &str) -> Vec<String> {
    index["locals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["location"]["line"] == line && l["name"] == name)
        .map(|l| l["type"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn inferred_locals_parameters_and_loop_bindings_obey_exact_scopes() {
    let f = Fixture::new();
    let source = "def first(value: i32) -> i32:\n    return value\ndef second(value: bool) -> bool:\n    return value\ndef main():\n    count = first(7); flag = True; print(count); print(flag)\n    count += 1\n    if flag:\n        inside = 3\n        print(inside)\n    else:\n        print(count)\n    print(inside)\n    for count in [True]:\n        print(count)\n    print(count)\n    unknown = missing()\n    print(unknown)\n    print(count)\n";
    f.write("main.nagi", source);
    let index = f.symbols(serde_json::json!([]));
    assert_eq!(local_types(&index, 1, "value"), ["i32"]);
    assert_eq!(local_types(&index, 2, "value"), ["i32"]);
    assert_eq!(local_types(&index, 3, "value"), ["bool"]);
    assert_eq!(local_types(&index, 4, "value"), ["bool"]);
    assert_eq!(local_types(&index, 6, "count"), ["i32", "i32"]);
    assert_eq!(local_types(&index, 6, "flag"), ["bool", "bool"]);
    assert_eq!(local_types(&index, 7, "count"), ["i32", "i32"]);
    assert_eq!(local_types(&index, 10, "inside"), ["i64"]);
    assert!(local_types(&index, 13, "inside").is_empty());
    assert_eq!(local_types(&index, 14, "count"), ["bool"]);
    assert_eq!(local_types(&index, 15, "count"), ["bool"]);
    assert_eq!(local_types(&index, 16, "count"), ["i32"]);
    assert!(local_types(&index, 17, "unknown").is_empty());
    assert!(local_types(&index, 18, "unknown").is_empty());
    assert_eq!(local_types(&index, 19, "count"), ["i32"]);
    // Editor recovery must not turn the same source into a compilable program.
    let mut parsed = nagic::parser::parse(source, true).unwrap();
    assert!(nagic::check::check(&mut parsed).is_err());
}

#[test]
fn match_payloads_and_child_locals_do_not_escape_their_arm() {
    let f = Fixture::new();
    f.write("main.nagi", "class Point:\n    x: i32\ndef inspect(result: Result[Point, Error]):\n    match result:\n        case Ok(point):\n            local = point\n            print(local.x)\n        case Err(problem):\n            print(error_kind(problem))\n            print(point.x)\n            print(local.x)\n    print(point.x)\n    print(problem)\ndef main():\n    print(0)\n");
    let index = f.symbols(serde_json::json!([]));
    assert_eq!(local_types(&index, 3, "result"), ["Result[Point, Error]"]);
    assert_eq!(local_types(&index, 5, "point"), ["Point"]);
    assert_eq!(local_types(&index, 6, "point"), ["Point"]);
    assert_eq!(local_types(&index, 6, "local"), ["Point"]);
    assert_eq!(local_types(&index, 7, "local"), ["Point"]);
    assert_eq!(local_types(&index, 8, "problem"), ["Error"]);
    assert_eq!(local_types(&index, 9, "problem"), ["Error"]);
    for (line, name) in [(10, "point"), (11, "local"), (12, "point"), (13, "problem")] {
        assert!(local_types(&index, line, name).is_empty());
    }
    let local = index["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["location"]["line"] == 7 && e["type"] == "Point")
        .unwrap();
    assert_eq!(
        local["fields"],
        serde_json::json!([{ "name": "x", "type": "i32" }])
    );
}

#[test]
fn imported_and_native_types_use_unsaved_buffers_and_utf16_ranges() {
    let f = Fixture::new();
    f.write("nagi.toml", "entry = 'main.nagi'\nnative = ['make.low']\n");
    let models = "class Point:\n    x: i64\n";
    let entry =
        "import \"models.nagi\"\ndef main():\n    print(\"😀\"); point = make(); print(point.x)\n";
    f.write("models.nagi", models);
    f.write("main.nagi", entry);
    f.write("make.low", "fn make() -> Point { return Point(x=1); }\n");
    let index = f.symbols(serde_json::json!([
        { "file": "models.nagi", "text": "class Point:\n    x: i32\n    y: bool\n" },
        { "file": "make.low", "text": "fn make() -> Point { let point = Point(x=7, y=True); return point; }\n" }
    ]));
    let point = index["locals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "point" && l["location"]["line"] == 3)
        .unwrap();
    let line = entry.lines().nth(2).unwrap();
    assert_eq!(
        point["location"]["column"],
        line[..line.find("point").unwrap()].encode_utf16().count() + 1
    );
    assert_eq!(point["type"], "Point");
    let expression = index["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["location"]["line"] == 3 && e["type"] == "Point")
        .unwrap();
    assert_eq!(
        expression["fields"],
        serde_json::json!([{ "name": "x", "type": "i32" }, { "name": "y", "type": "bool" }])
    );
    assert!(index["locals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["location"]["file"]
            .as_str()
            .unwrap()
            .ends_with("make.low")
            && l["name"] == "point"
            && l["type"] == "Point"));
    assert_eq!(fs::read_to_string(f.0.join("models.nagi")).unwrap(), models);
    assert_eq!(fs::read_to_string(f.0.join("main.nagi")).unwrap(), entry);
    assert!(!f.0.join("build").exists());
}

#[test]
fn low_records_replacements_and_same_line_bindings_keep_their_source_positions() {
    let f = Fixture::new();
    f.write(
        "nagi.toml",
        "entry = 'main.nagi'\nnative = ['counter.low']\n",
    );
    f.write("main.nagi", "def twice(input: i32) -> i32:\n    return input * 2\ndef main():\n    counter = create(); print(twice(counter.value))\n");
    f.write("counter.low", "record Counter { value: i32; }\nfn create() -> Counter { let item = Counter(value=7); return item; }\n@replace generated::twice\nfn replacement(payload: i32) -> i32 { let result = payload * 2; return result; }\n");
    let index = f.symbols(serde_json::json!([]));
    assert_eq!(local_types(&index, 1, "input"), ["i32"]);
    assert_eq!(local_types(&index, 2, "item"), ["Counter", "Counter"]);
    assert_eq!(local_types(&index, 4, "payload"), ["i32", "i32"]);
    assert_eq!(local_types(&index, 4, "result"), ["i32", "i32"]);
    let counter = index["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "Counter")
        .unwrap();
    assert_eq!(
        counter["fields"],
        serde_json::json!([{ "name": "value", "type": "i32" }])
    );
    let jump = index["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["location"]["line"] == 4 && r["target"]["line"] == 1)
        .unwrap();
    assert!(jump["target"]["file"]
        .as_str()
        .unwrap()
        .ends_with("main.nagi"));
}

#[test]
fn invalid_signatures_do_not_generate_guessed_local_types() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    point = unknown()\n    print(point.x)\ndef unknown() -> Missing:\n    return 1\n");
    let index = f.symbols(serde_json::json!([]));
    assert_eq!(index["locals"], serde_json::json!([]));
    assert_eq!(index["expressions"], serde_json::json!([]));
    assert_eq!(index["definitions"].as_array().unwrap().len(), 2);
    assert!(!index["references"].as_array().unwrap().is_empty());
}

#[test]
fn moved_values_and_scope_locals_have_no_later_inferred_type() {
    let f = Fixture::new();
    f.write("main.nagi", "class User:\n    name: str\ndef consume(user: User):\n    print(user.name)\nasync def work() -> Result[unit, Error]:\n    user = User(name=\"example\")\n    consume(user)\n    print(user.name)\n    async with scope:\n        hidden = 7\n        print(hidden)\n    print(hidden)\n    return ok(print(0))\ndef main():\n    print(0)\n");
    let index = f.symbols(serde_json::json!([]));
    assert_eq!(local_types(&index, 6, "user"), ["User"]);
    assert_eq!(local_types(&index, 7, "user"), ["User"]);
    assert!(local_types(&index, 8, "user").is_empty());
    assert_eq!(local_types(&index, 11, "hidden"), ["i64"]);
    assert!(local_types(&index, 12, "hidden").is_empty());
}

#[test]
fn expression_ranges_cover_multiline_receivers_and_do_not_unwrap_results() {
    let f = Fixture::new();
    f.write("main.nagi", "class Point:\n    x: i64\ndef fetch() -> Result[Point, Error]:\n    return ok(Point(x=7))\ndef inspect() -> Result[unit, Error]:\n    point = (\n        try fetch()\n    )\n    print(point.x)\n    return ok(print(0))\ndef main():\n    result = fetch()\n");
    let index = f.symbols(serde_json::json!([]));
    let expressions = index["expressions"].as_array().unwrap();
    let multiline = expressions
        .iter()
        .find(|e| e["location"]["line"] == 6 && e["type"] == "Point")
        .unwrap();
    assert_eq!(multiline["end_line"], 8);
    assert_eq!(multiline["end_column"], 6);
    assert_eq!(multiline["fields"][0]["name"], "x");
    let result = expressions
        .iter()
        .find(|e| e["location"]["line"] == 12 && e["type"] == "Result[Point, Error]")
        .unwrap();
    assert_eq!(result["fields"], serde_json::json!([]));
}
