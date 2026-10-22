use nagic::{check, source};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        for _ in 0..256 {
            let path = std::env::temp_dir().join(format!(
                "nagi-source-read-{}-{}",
                std::process::id(),
                FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("owned fixture creation failed: {error}"),
            }
        }
        panic!("owned fixture namespace exhausted");
    }
    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) {
        fs::write(self.0.join(name), bytes).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn small_high_disk_import_and_overlay_keep_text_and_check_behavior() {
    let f = Fixture::new();
    let part = "def value() -> i64:\n    return 7\n";
    f.write("part.nagi", part);
    f.write(
        "main.nagi",
        "import \"part.nagi\"\ndef main():\n    print(value())\n",
    );
    let mut disk = source::load(&f.0.join("main.nagi"), true).unwrap();
    check::check(&mut disk.program).unwrap();
    assert!(disk.text.contains(part));
    let overlay = "def value() -> i64:\n    return 8\n";
    let overlays = HashMap::from([(
        fs::canonicalize(f.0.join("part.nagi")).unwrap(),
        overlay.into(),
    )]);
    let mut loaded = source::load_with_overlays(&f.0.join("main.nagi"), true, &overlays).unwrap();
    check::check(&mut loaded.program).unwrap();
    assert!(loaded.text.contains(overlay));
    assert_eq!(fs::read_to_string(f.0.join("part.nagi")).unwrap(), part);
}

#[test]
fn small_saved_low_disk_import_keeps_original_loader_path() {
    let f = Fixture::new();
    f.write("part.low", "fn value() -> i64 { return 7; }\n");
    f.write(
        "main.low",
        "import \"part.low\";\nfn main() -> unit { print(value()); }\n",
    );
    let mut loaded = source::load(&f.0.join("main.low"), false).unwrap();
    check::check(&mut loaded.program).unwrap();
    assert!(loaded.text.contains("fn value() -> i64 { return 7; }"));
}

#[test]
fn small_invalid_utf8_import_keeps_standard_read_error_at_parent_line() {
    let f = Fixture::new();
    f.write("bad.nagi", b"a\xffbc");
    f.write(
        "main.nagi",
        "# owned fixture\nimport \"bad.nagi\"\ndef main():\n    pass\n",
    );
    let standard = fs::read_to_string(f.0.join("bad.nagi")).unwrap_err();
    let expected = format!(
        "error: line 2: {}: {standard}\n --> {}:2\n 2 | import \"bad.nagi\"",
        fs::canonicalize(f.0.join("bad.nagi")).unwrap().display(),
        fs::canonicalize(f.0.join("main.nagi")).unwrap().display()
    );
    let error = source::load(&f.0.join("main.nagi"), true).err().unwrap();
    assert_eq!(error, expected);
    let root_error = source::load(&f.0.join("bad.nagi"), true).err().unwrap();
    assert_eq!(
        root_error,
        format!(
            "{}: {standard}",
            fs::canonicalize(f.0.join("bad.nagi")).unwrap().display()
        )
    );
}

#[test]
fn retained_module_tokens_preserve_crlf_unicode_and_eof_locations() {
    for high in [true, false] {
        let f = Fixture::new();
        let extension = if high { "nagi" } else { "low" };
        let (part, main) = if high {
            (
                "# 日本語\r\ndef value(number: i64) -> i64:\r\n    return number",
                "import \"part.nagi\" as part\r\ndef main():\r\n    print(part.value(7))",
            )
        } else {
            (
                "# 日本語\r\nfn value(number: i64) -> i64 { return number; }",
                "import \"part.low\" as part;\r\nfn main() -> unit { print(part.value(7)); }",
            )
        };
        f.write(&format!("part.{extension}"), part);
        f.write(&format!("main.{extension}"), main);
        let mut loaded = source::load(&f.0.join(format!("main.{extension}")), high).unwrap();
        check::check(&mut loaded.program).unwrap();
        let index = nagic::symbols::index(&loaded, &[&loaded.program]).unwrap();
        let reference = index["references"]
            .as_array()
            .unwrap()
            .iter()
            .find(|reference| {
                reference["location"]["file"]
                    .as_str()
                    .unwrap()
                    .ends_with(&format!("main.{extension}"))
                    && reference["location"]["line"] == if high { 3 } else { 2 }
                    && reference["location"]["column"]
                        == main.lines().last().unwrap().find("value").unwrap() + 1
            });
        let reference = reference.unwrap_or_else(|| panic!("qualified reference lost: {index}"));
        assert_eq!(reference["location"]["length"], 5);
        assert_eq!(reference["target"]["line"], 2);
        assert!(reference["target"]["file"]
            .as_str()
            .unwrap()
            .ends_with(&format!("part.{extension}")));
    }
}

#[test]
fn imported_parse_and_lex_errors_keep_physical_source_locations() {
    let f = Fixture::new();
    for high in [true, false] {
        let extension = if high { "nagi" } else { "low" };
        let name = format!("bad.{extension}");
        let main = format!("import \"{name}\"{}\n", if high { "" } else { ";" });
        f.write(&format!("main.{extension}"), main);
        for bad in ["# 日本語\r\n\tbad", "# 日本語\r\n)"] {
            f.write(&name, bad);
            let parser_error = nagic::parser::parse(bad, high).err().unwrap();
            let error = source::load(&f.0.join(format!("main.{extension}")), high)
                .err()
                .unwrap();
            assert!(
                error.starts_with(&format!("error: {parser_error}\n")),
                "{error}"
            );
            assert!(error.contains(&format!("{name}:2")), "{error}");
            assert!(
                error.ends_with(if bad.contains('\t') {
                    "2 | \tbad"
                } else {
                    "2 | )"
                }),
                "{error}"
            );
        }
    }
}
