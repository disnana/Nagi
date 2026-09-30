use nagic::{check, source};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
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
fn diamond_import_is_loaded_once_and_assets_use_their_source_directory() {
    let f = Fixture::new();
    f.write(
        "lib/shared.nagi",
        "def shared() -> str:\n    return include_text(\"text.txt\")\n",
    );
    f.write("lib/text.txt", "embedded text");
    f.write(
        "left.nagi",
        "import \"lib/shared.nagi\"\ndef left() -> str:\n    return shared()\n",
    );
    f.write(
        "right.nagi",
        "import \"lib/shared.nagi\"\ndef right() -> str:\n    return shared()\n",
    );
    f.write("main.nagi", "import \"left.nagi\"\nimport \"right.nagi\"\ndef main():\n    print(left())\n    print(right())\n");
    let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    assert_eq!(loaded.program.functions.len(), 4);
    assert!(nagic::emit::low(&loaded.program).contains("text.txt"));
}

#[test]
fn cycles_missing_files_and_mixed_language_imports_are_errors() {
    let f = Fixture::new();
    f.write("a.nagi", "import \"b.nagi\"\n");
    f.write("b.nagi", "import \"a.nagi\"\n");
    assert!(source::load(&f.0.join("a.nagi"), true)
        .err()
        .unwrap()
        .contains("循環import"));
    f.write("a.nagi", "import \"missing.nagi\"\n");
    assert!(source::load(&f.0.join("a.nagi"), true).is_err());
    f.write("a.nagi", "import \"b.low\"\n");
    assert!(source::load(&f.0.join("a.nagi"), true).is_err());
}

#[test]
fn type_errors_point_to_the_original_imported_file_and_line() {
    let f = Fixture::new();
    f.write("good.nagi", "def good() -> i64:\n    return 1\n");
    f.write("bad.nagi", "def bad() -> i32:\n    return \"wrong\"\n");
    f.write(
        "main.nagi",
        "import \"good.nagi\"\nimport \"bad.nagi\"\ndef main():\n    print(good())\n",
    );
    let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
    let error = check::check(&mut loaded.program).unwrap_err();
    assert!(loaded.diagnostic(&error).contains("bad.nagi:2"));
}

#[test]
fn low_imports_use_the_low_parser() {
    let f = Fixture::new();
    f.write("math.low", "fn twice(x: i64) -> i64 { return x * 2; }\n");
    f.write(
        "main.low",
        "import \"math.low\";\nfn main() -> unit { print(twice(3)); }\n",
    );
    let mut loaded = source::load(&f.0.join("main.low"), false).unwrap();
    check::check(&mut loaded.program).unwrap();
}
