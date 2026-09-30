use nagic::{source, symbols};
use std::{fs, path::PathBuf, process::Command};

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
