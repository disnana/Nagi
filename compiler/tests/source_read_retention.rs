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
