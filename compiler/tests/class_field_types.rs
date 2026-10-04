use nagic::{check, emit, parser};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[test]
fn unsupported_resource_and_function_fields_fail_at_the_field_line() {
    for ty in [
        "fn[i64, i64]",
        "List[fn[i64]]",
        "Option[fn[i64]]",
        "Html",
        "shared[Html]",
    ] {
        for (source, high) in [
            (
                format!("class Payload:\n    value: i64\n    unsupported: {ty}\n"),
                true,
            ),
            (
                format!("record Payload {{\n    value: i64;\n    unsupported: {ty};\n}}\n"),
                false,
            ),
        ] {
            let mut program = parser::parse(&source, high).unwrap();
            let error = check::check(&mut program).unwrap_err();
            assert!(
                error.starts_with("line 3:") && error.contains("classのフィールドに保存できません"),
                "{error}"
            );
        }
    }
}

#[test]
fn owned_database_fields_are_private_state_and_keep_high_low_emission_valid() {
    let source = "class DatabaseState:\n    database: Db\n    backups: shared[List[Db]]\n    label: str\ndef borrow(state: shared[DatabaseState]) -> i64:\n    return len(view(state.label))\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
    for program in [high, low] {
        let rust = emit::rust(&program).unwrap();
        assert!(rust.contains("::nagi_runtime::Db"), "{rust}");
        assert!(
            !rust.contains("Serialize") && !rust.contains("Deserialize"),
            "database state must have no JSON representation: {rust}"
        );
    }
    for operation in ["json_encode(state)", "json_decode[DatabaseState](\"{}\")"] {
        let source = format!("class DatabaseState:\n    database: Db\ndef encode(state: DatabaseState) -> Result[str, Error]:\n    value = try {operation}\n    return ok(\"encoded\")\n");
        let mut program = parser::parse(&source, true).unwrap();
        assert!(
            check::check(&mut program).is_err(),
            "database state became serializable: {source}"
        );
    }
}

struct NativeFixture(PathBuf);

impl Drop for NativeFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_database_debug_hides_connection_details_inside_shared_state() {
    let fixture = NativeFixture(std::env::temp_dir().join(format!(
        "nagi database state {} {}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )));
    fs::create_dir_all(&fixture.0).unwrap();
    fs::write(fixture.0.join("database-state.nagi"), "class DatabaseState:\n    database: Db\n    label: str\nasync def main() -> Result[unit, shared[DatabaseState]]:\n    match await db_open(\"credential-private-db.sqlite3\"):\n        case Ok(database):\n            state = share(DatabaseState(database=database, label=\"visible\"))\n            return fail(state)\n        case Err(_):\n            return ok(assert_true(False))\n").unwrap();
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(&fixture.0)
        .args(["build", "database-state.nagi", "--no-project"])
        .env("NAGI_NATIVE_TARGET_DIR", &target)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    let binary = stderr
        .lines()
        .find_map(|line| line.strip_prefix("native: "))
        .expect("the build must report its native artifact");
    let output = Command::new(binary)
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "the deliberately returned error must produce a nonzero exit"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("database: Db { .. }") && stderr.contains("visible"),
        "{stderr}"
    );
    for private in [
        "credential-private-db",
        "Connection",
        "Sender",
        "JoinHandle",
    ] {
        assert!(
            !stderr.contains(private),
            "database internals leaked through Debug: {stderr}"
        );
    }
}

#[test]
fn map_keys_with_missing_equality_or_hashing_fail_before_derivation() {
    for ty in [
        "f32",
        "f64",
        "UUID",
        "timestamp",
        "List[f64]",
        "Option[f32]",
        "shared[f64]",
        "Result[i64, f64]",
        "Map[i64, i64]",
    ] {
        let source = format!("class Lookup:\n    entries: Map[{ty}, i64]\n");
        let mut program = parser::parse(&source, true).unwrap();
        let error = check::check(&mut program).unwrap_err();
        assert!(
            error.starts_with("line 2:") && error.contains("classのMapフィールドのキー"),
            "{error}"
        );
    }
}

#[test]
fn nested_data_fields_and_bridge_defined_key_traits_remain_available() {
    let source = "class Key:\n    value: i64\nclass Payload:\n    values: shared[List[Result[i64, str]]]\n    names: Map[str, i64]\n    custom: Map[Key, str]\n    children: List[Payload]\n";
    let mut high = parser::parse(source, true).unwrap();
    check::check(&mut high).unwrap();
    let mut low = parser::parse(&emit::low(&high), false).unwrap();
    check::check(&mut low).unwrap();
}
