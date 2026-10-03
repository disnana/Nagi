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
            "nagi nullable database {} {}",
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

const PROGRAM: &str = r#"class NullableRow:
    id: i64
    signed8: i8?
    signed16: i16?
    signed32: i32?
    signed64: i64?
    unsigned8: u8?
    unsigned16: u16?
    unsigned32: u32?
    real32: f32?
    real64: f64?
    enabled: bool?
    type: str?
    payload: bytes?

@rust("native::verify")
extern def verify(rows: List[NullableRow])

async def main() -> Result[i64, Error]:
    db = try await db_open(":memory:")
    try await db_exec(db, "CREATE TABLE readings(id INTEGER PRIMARY KEY, signed8 INTEGER, signed16 INTEGER, signed32 INTEGER, signed64 INTEGER, unsigned8 INTEGER, unsigned16 INTEGER, unsigned32 INTEGER, real32 REAL, real64 REAL, enabled INTEGER, type TEXT, payload BLOB); INSERT INTO readings VALUES (1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL), (2, -128, -32768, -2147483648, -9223372036854775808, 255, 65535, 4294967295, 1.25, -2.5, 1, '凪', X'007FFF'), (3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 0, 0, 0, '', X'');")
    rows = try await db_all[NullableRow](db, "SELECT payload, type, enabled, real64, real32, unsigned32, unsigned16, unsigned8, signed64, signed32, signed16, signed8, id FROM readings ORDER BY id")
    verify(rows)
    return ok(0)
"#;

const BRIDGE: &str = r#"pub fn verify(rows: Vec<super::NullableRow>) {
    assert_eq!(<super::NullableRow as nagi_runtime::FromRow>::columns(), &[
        "id", "signed8", "signed16", "signed32", "signed64", "unsigned8",
        "unsigned16", "unsigned32", "real32", "real64", "enabled", "type", "payload"
    ]);
    let actual = nagi_runtime::serde_json::to_value(rows).unwrap();
    let expected = nagi_runtime::serde_json::json!([
        {"id":1, "signed8":null, "signed16":null, "signed32":null, "signed64":null,
         "unsigned8":null, "unsigned16":null, "unsigned32":null,
         "real32":null, "real64":null, "enabled":null, "type":null, "payload":null},
        {"id":2, "signed8":-128, "signed16":-32768, "signed32":-2147483648i64,
         "signed64":-9223372036854775808i64, "unsigned8":255, "unsigned16":65535,
         "unsigned32":4294967295u64, "real32":1.25, "real64":-2.5,
         "enabled":true, "type":"凪", "payload":[0,127,255]},
        {"id":3, "signed8":null, "signed16":null, "signed32":null, "signed64":null,
         "unsigned8":null, "unsigned16":null, "unsigned32":null,
         "real32":0.0, "real64":0.0, "enabled":false, "type":"", "payload":[]}
    ]);
    assert_eq!(actual, expected);
    println!("nullable SQLite rows verified");
}
"#;

#[test]
fn nullable_scalars_read_sqlite_null_and_non_null_rows_in_high_and_saved_low() {
    let f = Fixture::new();
    f.write("main.nagi", PROGRAM);
    f.write("native.rs", BRIDGE);
    let mut loaded = source::load(&f.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let low = emit::low(&loaded.program);
    let mut independent = parser::parse(&low, false).unwrap();
    check::check(&mut independent).unwrap();
    assert_eq!(
        emit::rust(&loaded.program).unwrap(),
        emit::rust(&independent).unwrap()
    );
    f.write("saved.low", &low);

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("native-target"));
    for file in ["main.nagi", "saved.low"] {
        let checked = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&f.0)
            .args(["check", file])
            .output()
            .unwrap();
        assert!(
            checked.status.success(),
            "{file}: {}",
            String::from_utf8_lossy(&checked.stderr)
        );
        let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(&f.0)
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
            String::from_utf8_lossy(&output.stdout).lines().last(),
            Some("nullable SQLite rows verified"),
            "{file}"
        );
        if file == "main.nagi" {
            // Saved Low must carry the row identity without the original source.
            fs::remove_file(f.0.join("main.nagi")).unwrap();
        }
    }
}

#[test]
fn unsupported_nullable_payloads_keep_manual_from_row_eligibility() {
    let f = Fixture::new();
    f.write(
        "manual.nagi",
        "class Nested:\n    value: Option[Option[i64]]\nclass Sequence:\n    value: Option[List[i64]]\nclass Wide:\n    value: u64?\nclass bool:\n    value: i64\nclass Nominal:\n    value: bool?\n",
    );
    let mut loaded = source::load(&f.0.join("manual.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let low = emit::low(&loaded.program);
    let mut independent = parser::parse(&low, false).unwrap();
    check::check(&mut independent).unwrap();
    for program in [&loaded.program, &independent] {
        let rust = emit::rust(program).unwrap();
        for name in ["Nested", "Sequence", "Wide", "Nominal"] {
            let definition = program
                .modules
                .definitions
                .iter()
                .find(|definition| definition.id.name == name)
                .unwrap();
            assert!(
                !rust.contains(&format!(
                    "impl ::nagi_runtime::FromRow for {}",
                    definition.symbol
                )),
                "{name} must keep its existing manual FromRow boundary"
            );
        }
    }
}

#[test]
fn raw_numeric_and_bool_field_shadows_keep_manual_row_implementations() {
    for name in [
        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "f32", "f64", "bool",
    ] {
        let source = format!(
            "class {name}:\n    label: str\nclass Required:\n    value: {name}\nclass Nullable:\n    value: {name}?\nclass Text:\n    value: str?\nclass Binary:\n    value: bytes?\n"
        );
        let mut high = parser::parse(&source, true).unwrap();
        check::check(&mut high).unwrap();
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
        for program in [&high, &low] {
            let rust = emit::rust(program).unwrap();
            for row in ["Required", "Nullable"] {
                assert!(
                    !rust.contains(&format!("impl ::nagi_runtime::FromRow for {row}")),
                    "{name} field is a local class in the raw API, not a SQLite scalar"
                );
            }
            for row in ["Text", "Binary"] {
                assert!(rust.contains(&format!("impl ::nagi_runtime::FromRow for {row}")));
            }
        }
    }
    // These heads remain fully qualified String/Vec representations even if
    // the raw program contains classes with the same names.
    let mut program = parser::parse(
        "class str:\n    value: i64\nclass bytes:\n    value: i64\nclass Text:\n    value: str?\nclass Binary:\n    value: bytes?\n",
        true,
    )
    .unwrap();
    check::check(&mut program).unwrap();
    let rust = emit::rust(&program).unwrap();
    for row in ["Text", "Binary"] {
        assert!(rust.contains(&format!("impl ::nagi_runtime::FromRow for {row}")));
    }
}

#[test]
fn raw_nullable_classes_can_supply_a_native_sqlite_row_bridge() {
    let f = Fixture::new();
    let mut high = parser::parse(
        "class bool:\n    label: str\nclass f64:\n    label: str\nclass Row:\n    enabled: bool?\n    ratio: f64?\n    text: str?\n    payload: bytes?\n",
        true,
    )
    .unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    let rust = emit::rust(&high).unwrap();
    assert_eq!(rust, emit::rust(&low).unwrap());
    let rust = rust.strip_suffix("fn main() {}\n").unwrap();
    let bridge = r#"
impl nagi_runtime::FromRow for Row {
    fn columns() -> &'static [&'static ::std::primitive::str] {
        &["enabled", "ratio", "text", "payload"]
    }
    fn read(row: &nagi_runtime::rusqlite::Row<'_>, indices: &[usize]) -> nagi_runtime::rusqlite::Result<Self> {
        let enabled: Option<String> = row.get(indices[0])?;
        let ratio: Option<String> = row.get(indices[1])?;
        Ok(Self {
            enabled: enabled.map(|label| crate::bool { label }),
            ratio: ratio.map(|label| crate::f64 { label }),
            text: row.get(indices[2])?,
            payload: row.get(indices[3])?,
        })
    }
}
fn main() {
    let connection = nagi_runtime::rusqlite::Connection::open_in_memory().unwrap();
    let absent = connection.query_row("SELECT NULL, NULL, NULL, NULL", [],
        |row| <Row as nagi_runtime::FromRow>::read(row, &[0, 1, 2, 3])).unwrap();
    assert!(absent.enabled.is_none() && absent.ratio.is_none());
    assert!(absent.text.is_none() && absent.payload.is_none());
    let present = connection.query_row("SELECT 'nominal bool', 'nominal f64', '', X'007FFF'", [],
        |row| <Row as nagi_runtime::FromRow>::read(row, &[0, 1, 2, 3])).unwrap();
    assert_eq!(present.enabled.unwrap().label, "nominal bool");
    assert_eq!(present.ratio.unwrap().label, "nominal f64");
    assert_eq!(present.text.unwrap(), "");
    assert_eq!(present.payload.unwrap(), vec![0, 127, 255]);
    println!("raw nullable row bridge verified");
}
"#;
    fs::create_dir_all(f.0.join("src")).unwrap();
    f.write("src/main.rs", &format!("{rust}{bridge}"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let runtime = serde_json::to_string(&root.join("runtime").to_string_lossy()).unwrap();
    f.write(
        "Cargo.toml",
        &format!(
            "[package]\nname='nagi-raw-nullable'\nversion='0.0.0'\nedition='2021'\n[workspace]\n[dependencies]\nnagi-runtime={{path={runtime}}}\n[profile.release]\nopt-level=3\nlto=false\ncodegen-units=1\npanic='abort'\n"
        ),
    );
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("native-target"));
    let output = Command::new("cargo")
        .current_dir(&f.0)
        .args([
            "run",
            "--release",
            "--offline",
            "--manifest-path",
            "Cargo.toml",
        ])
        .env("CARGO_TARGET_DIR", target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "raw nullable row bridge verified"
    );
}
