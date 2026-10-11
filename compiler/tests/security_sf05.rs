use nagic::{check, emit, source};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        loop {
            // Never reuse a retired name: Windows may still be completing deletion.
            let n = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("nagi-sf05-{}-{n}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => panic!("{e}"),
            }
        }
    }
    fn reject(&self, high: &str, low: &str, line: usize, reason: &str, anchor: &str) {
        let high_path = self.0.join("source.nagi");
        fs::write(&high_path, high).unwrap();
        let loaded = source::load(&high_path, true).expect("High parse/resolve must succeed");
        let saved = emit::low(&loaded.program);
        let saved_line = saved
            .lines()
            .position(|s| s.contains(anchor))
            .expect("Low source anchor")
            + 1;
        fs::remove_file(high_path).unwrap();
        let mut accepted = Vec::new();
        for (name, text, is_high, expected_line) in [
            ("source.nagi", high, true, line),
            ("saved.low", saved.as_str(), false, saved_line),
            ("manual.low", low, false, line),
        ] {
            let path = self.0.join(name);
            fs::write(&path, text).unwrap();
            let mut loaded = source::load(&path, is_high).expect("parse/resolve before rejection");
            let error = match check::check(&mut loaded.program) {
                Ok(()) => {
                    eprintln!("{name}: legacy checker acceptance (expected {reason})");
                    accepted.push(name);
                    fs::remove_file(path).unwrap();
                    continue;
                }
                Err(error) => error,
            };
            assert!(error.contains(reason), "{name}: {error}");
            let d = loaded.diagnostic(&error);
            assert!(d.contains(&format!("{name}:{expected_line}\n")), "{d}");
            assert!(!d.contains("internal compiler error"), "{d}");
            fs::remove_file(path).unwrap();
        }
        assert!(accepted.is_empty(), "legacy SQL accepted in {accepted:?}");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn retired_db_execution_has_checker_migration_and_original_line() {
    Fixture::new().reject("async def main() -> Result[unit, Error]:\n    db = try await db_open(\":memory:\")\n    return ok(print(0))\n", "async fn main() -> Result[unit, Error] {\n let db = try await db_open(\":memory:\");\n return ok(print(0));\n}\n",2,"SF05 migration","await db_open");
}
#[test]
fn retired_db_type_has_checker_migration_and_original_line() {
    Fixture::new().reject(
        "@rust(\"native::old\")\nextern def old() -> Db\n",
        "@rust(\"native::old\")\nextern fn old() -> Db;\n",
        2,
        "SF05 migration",
        "extern fn",
    );
}
#[test]
fn sqlite_string_execution_cannot_downgrade_to_dynamic_sql() {
    Fixture::new().reject("import std.db.sqlite as sqlite\nasync def old(tx: sqlite.Tx) -> Result[i64, sqlite.Failure]:\n    return await sqlite.exec(tx, \"DELETE FROM data\", sqlite.parameters())\n", "import std.db.sqlite as sqlite;\nasync fn old(tx: sqlite.Tx) -> Result[i64, sqlite.Failure] {\n return await sqlite.exec(tx, \"DELETE FROM data\", sqlite.parameters());\n}\n",3,"SF05 migration","_f_65786563(");
}

fn reject_retired_entry(name: &str, args: &str, generic: &str) {
    let args = args.replacen("0,", "try await db_open(\":memory:\"),", 1);
    let high=format!("class Row:\n    value: i64\nasync def old() -> Result[unit, Error]:\n    result = try await {name}{generic}({args})\n    return ok(print(0))\n");
    let low=format!("record Row {{\n value: i64; }}\nasync fn old() -> Result[unit, Error] {{\n let result = try await {name}{generic}({args});\n return ok(print(0));\n}}\n");
    Fixture::new().reject(&high, &low, 4, &format!("SF05 migration: 旧{name}"), name);
}
macro_rules! retired_entry {
    ($test:ident, $name:literal, $args:literal, $generic:literal) => {
        #[test]
        fn $test() {
            reject_retired_entry($name, $args, $generic);
        }
    };
}
retired_entry!(retired_db_exec, "db_exec", "0, \"DELETE FROM data\"", "");
retired_entry!(
    retired_db_all,
    "db_all",
    "0, \"SELECT value FROM data\"",
    "[Row]"
);
retired_entry!(
    retired_db_query,
    "db_query",
    "0, \"SELECT value FROM data WHERE id=?1\", 1",
    "[Row]"
);
retired_entry!(
    retired_db_insert,
    "db_insert",
    "0, \"INSERT INTO data VALUES (?1,?2)\", \"v\", 1",
    "[Row]"
);
retired_entry!(
    retired_db_update,
    "db_update",
    "0, \"UPDATE data SET value=?2 WHERE id=?1\", 1, \"v\", 1",
    "[Row]"
);
retired_entry!(
    retired_db_write,
    "db_write",
    "0, \"DELETE FROM data WHERE id=?1\", 1",
    ""
);

#[test]
fn literal_constructor_rejects_dynamic_inputs_at_the_argument_source_line() {
    for expression in [
        "sql",
        "view(sql)",
        "\"SELECT \" + sql",
        "\"SELECT \" + \"1\"",
        "format(sql)",
        "format(\"SELECT 1\")",
    ] {
        let high=format!("import std.db.sqlite as sqlite\ndef format(value: str) -> str:\n    return value\ndef invalid(sql: str) -> sqlite.Query:\n    return sqlite.literal({expression})\n");
        let low=format!("import std.db.sqlite as sqlite;\nfn format(value: str) -> str {{\n return value; }}\nfn invalid(sql: str) -> sqlite.Query {{\n return sqlite.literal({expression}); }}\n");
        Fixture::new().reject(&high, &low, 5, "SF05 literal Query", "_f_6c69746572616c(");
    }
}
#[test]
fn aliased_literal_constructor_and_opaque_query_cannot_be_forged() {
    for (high,low,line,anchor) in [
        ("from std.db.sqlite import literal as fixed\ndef invalid(sql: str):\n    query = fixed(sql)\n", "from std.db.sqlite import literal as fixed;\nfn invalid(sql: str) {\n let query = fixed(sql); }\n",3,"_f_6c69746572616c("),
        ("import std.db.sqlite as sqlite\ndef invalid():\n    factory = sqlite.literal\n", "import std.db.sqlite as sqlite;\nfn invalid() {\n let factory = sqlite.literal; }\n",3,"factory ="),
        ("import std.db.sqlite as sqlite\ndef invalid():\n    query = sqlite.Query()\n", "import std.db.sqlite as sqlite;\nfn invalid() {\n let query = sqlite.Query(); }\n",3,"_r_5175657279("),
    ] { Fixture::new().reject(high,low,line,"SF05 literal Query",anchor); }
}
#[test]
fn unrelated_user_literal_and_legacy_names_remain_available() {
    let f = Fixture::new();
    for (name,text,high) in [
        ("user.nagi","def literal(value: str) -> str:\n    return value\ndef db_open(value: i64) -> i64:\n    return value\ndef main():\n    sql = \"SELECT 1\"\n    print(literal(sql))\n    print(db_open(7))\n",true),
        ("user.low","fn literal(value: str) -> str { return value; }\nfn db_open(value: i64) -> i64 { return value; }\nfn main() { let sql = \"SELECT 1\"; print(literal(sql)); print(db_open(7)); }\n",false),
    ] { let path=f.0.join(name);fs::write(&path,text).unwrap();let mut loaded=source::load(&path,high).unwrap();check::check(&mut loaded.program).unwrap(); }
}

#[test]
fn fixture_names_are_not_reused_after_owner_drop() {
    let first = Fixture::new();
    let old = first.0.clone();
    drop(first);
    let second = Fixture::new();
    assert_ne!(
        old, second.0,
        "retired fixture names must not be reclaimed by a different test"
    );
}
