use nagic::{check, emit, source};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        for n in 0u64.. {
            let path = std::env::temp_dir().join(format!("nagi-sf05-{}-{n}",std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists => (),
                Err(e) => panic!("{e}"),
            }
        }
        unreachable!()
    }
    fn reject(&self, high: &str, low: &str, line: usize, reason: &str, anchor: &str) {
        let high_path=self.0.join("source.nagi");
        fs::write(&high_path,high).unwrap();
        let loaded=source::load(&high_path,true).expect("High parse/resolve must succeed");
        let saved=emit::low(&loaded.program);
        let saved_line=saved.lines().position(|s|s.contains(anchor)).expect("Low source anchor")+1;
        fs::remove_file(high_path).unwrap();
        let mut accepted=Vec::new();
        for (name,text,is_high,expected_line) in [("source.nagi",high,true,line),("saved.low",saved.as_str(),false,saved_line),("manual.low",low,false,line)] {
            let path=self.0.join(name);
            fs::write(&path,text).unwrap();
            let mut loaded=source::load(&path,is_high).expect("parse/resolve before rejection");
            let error=match check::check(&mut loaded.program) {
                Ok(()) => { eprintln!("{name}: legacy checker acceptance (expected {reason})"); accepted.push(name); fs::remove_file(path).unwrap(); continue; }
                Err(error) => error,
            };
            assert!(error.contains(reason),"{name}: {error}");
            let d=loaded.diagnostic(&error);
            assert!(d.contains(&format!("{name}:{expected_line}\n")),"{d}");
            assert!(!d.contains("internal compiler error"),"{d}");
            fs::remove_file(path).unwrap();
        }
        assert!(accepted.is_empty(),"legacy SQL accepted in {accepted:?}");
    }
}
impl Drop for Fixture { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }
#[test]
fn retired_db_execution_has_checker_migration_and_original_line() {
    Fixture::new().reject("async def main() -> Result[unit, Error]:\n    db = try await db_open(\":memory:\")\n    return ok(print(0))\n", "async fn main() -> Result[unit, Error] {\n let db = try await db_open(\":memory:\");\n return ok(print(0));\n}\n",2,"SF05 migration","await db_open");
}
#[test]
fn retired_db_type_has_checker_migration_and_original_line() {
    Fixture::new().reject("@rust(\"native::old\")\nextern def old() -> Db\n", "@rust(\"native::old\")\nextern fn old() -> Db;\n",2,"SF05 migration","extern fn");
}
#[test]
fn sqlite_string_execution_cannot_downgrade_to_dynamic_sql() {
    Fixture::new().reject("import std.db.sqlite as sqlite\nasync def old(tx: sqlite.Tx) -> Result[i64, sqlite.Failure]:\n    return await sqlite.exec(tx, \"DELETE FROM data\", sqlite.parameters())\n", "import std.db.sqlite as sqlite;\nasync fn old(tx: sqlite.Tx) -> Result[i64, sqlite.Failure] {\n return await sqlite.exec(tx, \"DELETE FROM data\", sqlite.parameters());\n}\n",3,"SF05 migration","_f_65786563(");
}

#[test]
fn every_retired_builtin_execution_entry_reports_migration() {
    for (name,args,generic) in [
        ("db_exec","0, \"DELETE FROM data\"",""),
        ("db_all","0, \"SELECT value FROM data\"","[Row]"),
        ("db_query","0, \"SELECT value FROM data WHERE id=?1\", 1","[Row]"),
        ("db_insert","0, \"INSERT INTO data VALUES (?1,?2)\", \"v\", 1","[Row]"),
        ("db_update","0, \"UPDATE data SET value=?2 WHERE id=?1\", 1, \"v\", 1","[Row]"),
        ("db_write","0, \"DELETE FROM data WHERE id=?1\", 1",""),
    ] {
        let args=args.replacen("0,", "try await db_open(\":memory:\"),", 1);
        let high=format!("class Row:\n    value: i64\nasync def old() -> Result[unit, Error]:\n    result = try await {name}{generic}({args})\n    return ok(print(0))\n");
        let low=format!("record Row {{\n value: i64;\n}}\nasync fn old() -> Result[unit, Error] {{\n let result = try await {name}{generic}({args});\n return ok(print(0));\n}}\n");
        // Handwritten Low has one extra record-brace line; keep shared origin
        // line 4 by formatting the record on the first two lines.
        let low=low.replace(" value: i64;\n}\n", " value: i64; }\n");
        Fixture::new().reject(&high,&low,4,"SF05 migration",name);
    }
}
