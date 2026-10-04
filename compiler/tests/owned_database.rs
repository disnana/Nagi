use nagic::{check, emit, parser, source};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi owned database {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PROGRAM: &str = r#"class OwnedRow:
    id: owned[i64]
    repeated: owned[owned[i64]]
    optional_owned: Option[owned[i64]]
    owned_optional: owned[Option[i64]]
    both: owned[Option[owned[i64]]]
    label: owned[str]
    text: owned[Option[owned[str]]]
    enabled: owned[bool?]
    ratio: Option[owned[f64]]
    payload: owned[bytes?]
    narrow: owned[i8?]
    unsigned: owned[u32?]

class Inner:
    id: i64

class ManualRow:
    value: owned[Inner]

@rust("native::verify")
extern def verify(rows: List[OwnedRow], manual: List[ManualRow])

async def main() -> Result[unit, Error]:
    db = try await db_open(":memory:")
    try await db_exec(db, "CREATE TABLE readings(id INTEGER, repeated INTEGER, optional_owned INTEGER, owned_optional INTEGER, both INTEGER, label TEXT, text TEXT, enabled INTEGER, ratio REAL, payload BLOB, narrow INTEGER, unsigned INTEGER); INSERT INTO readings VALUES (1, 11, NULL, NULL, NULL, 'one', NULL, NULL, NULL, NULL, NULL, NULL), (2, 22, -7, 7, 9, 'two', '凪', 1, 1.25, X'007FFF', -128, 4294967295), (3, 33, 0, 0, 0, '', '', 0, 0, X'', 0, 0);")
    rows = try await db_all[OwnedRow](db, "SELECT unsigned, narrow, payload, ratio, enabled, text, label, both, owned_optional, optional_owned, repeated, id FROM readings ORDER BY id")
    manual = try await db_all[ManualRow](db, "SELECT 7 AS id UNION ALL SELECT 9 AS id")
    return ok(verify(rows, manual))
"#;

const BRIDGE: &str = r#"impl nagi_runtime::FromRow for super::ManualRow {
    fn columns() -> &'static [&'static str] { &["id"] }
    fn read(row: &nagi_runtime::rusqlite::Row<'_>, indices: &[usize]) -> nagi_runtime::rusqlite::Result<Self> {
        Ok(Self { value: super::Inner { id: row.get(indices[0])? } })
    }
}
pub fn verify(rows: Vec<super::OwnedRow>, manual: Vec<super::ManualRow>) {
    assert_eq!(<super::OwnedRow as nagi_runtime::FromRow>::columns(), &[
        "id", "repeated", "optional_owned", "owned_optional", "both", "label",
        "text", "enabled", "ratio", "payload", "narrow", "unsigned"
    ]);
    let actual = nagi_runtime::serde_json::to_value(rows).unwrap();
    let expected = nagi_runtime::serde_json::json!([
        {"id":1, "repeated":11, "optional_owned":null, "owned_optional":null,
         "both":null, "label":"one", "text":null, "enabled":null, "ratio":null,
         "payload":null, "narrow":null, "unsigned":null},
        {"id":2, "repeated":22, "optional_owned":-7, "owned_optional":7,
         "both":9, "label":"two", "text":"凪", "enabled":true, "ratio":1.25,
         "payload":[0,127,255], "narrow":-128, "unsigned":4294967295u64},
        {"id":3, "repeated":33, "optional_owned":0, "owned_optional":0,
         "both":0, "label":"", "text":"", "enabled":false, "ratio":0.0,
         "payload":[], "narrow":0, "unsigned":0}
    ]);
    assert_eq!(actual, expected);
    assert_eq!(manual.len(), 2);
    assert_eq!(manual[0].value.id, 7);
    assert_eq!(manual[1].value.id, 9);
    println!("owned SQLite fields and manual row bridge verified");
}
"#;

#[test]
fn owned_scalars_decode_sqlite_rows_in_high_and_independent_saved_low() {
    let fixture = Fixture::new();
    fixture.write("main.nagi", PROGRAM);
    fixture.write("native.rs", BRIDGE);
    let mut high = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut high.program).unwrap();
    let low = emit::low(&high.program);
    let mut independent = parser::parse(&low, false).unwrap();
    check::check(&mut independent).unwrap();
    assert_eq!(
        emit::rust(&high.program).unwrap(),
        emit::rust(&independent).unwrap()
    );
    fixture.write("saved.low", &low);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("native-target"));
    for file in ["main.nagi", "saved.low"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&fixture.0)
            .args(["run", file, "--rust", "native.rs", "--out", "build"])
            .env("NAGI_ROOT", root)
            .env("NAGI_NATIVE_TARGET_DIR", &target)
            .env("CARGO_NET_OFFLINE", "true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{file}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "owned SQLite fields and manual row bridge verified",
            "{file}"
        );
        if file == "main.nagi" {
            fs::remove_file(fixture.0.join("main.nagi")).unwrap();
        }
    }
}

#[test]
fn owned_wrappers_preserve_unsupported_fields_and_manual_row_eligibility() {
    let mut high = parser::parse(
        "class Custom:\n    value: i64\nclass Wide:\n    value: owned[u64]\nclass Nested:\n    value: owned[Option[owned[Option[i64]]]]\nclass Sequence:\n    value: owned[Option[owned[List[i64]]]]\nclass Nominal:\n    value: owned[Custom]\nclass Resource:\n    value: owned[Db]\nclass Shared:\n    value: owned[shared[i64]]\nclass i64:\n    label: str\nclass Shadowed:\n    value: owned[Option[owned[i64]]]\nclass Text:\n    value: owned[Option[owned[str]]]\n",
        true,
    )
    .unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    for program in [&high, &low] {
        let rust = emit::rust(program).unwrap();
        for name in [
            "Wide", "Nested", "Sequence", "Nominal", "Resource", "Shared", "Shadowed",
        ] {
            assert!(
                !rust.contains(&format!("impl ::nagi_runtime::FromRow for {name}")),
                "{name} requires a manual row bridge"
            );
        }
        assert!(rust.contains("impl ::nagi_runtime::FromRow for Text"));
    }
}
