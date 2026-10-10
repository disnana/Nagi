#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};

fn parsed(text: &str, high: bool) -> nagic::ast::Program {
    if !text.contains("import std.") {
        return parser::parse(text, high).unwrap();
    }
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "nagi-builtin-contract-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    let file = path.join(if high { "main.nagi" } else { "main.low" });
    std::fs::write(&file, text).unwrap();
    let program = nagic::source::load(&file, high).unwrap().program;
    std::fs::remove_dir_all(path).unwrap();
    program
}

fn checked(source: &str) -> nagic::ast::Program {
    let mut program = parsed(source, true);
    check::check(&mut program).unwrap_or_else(|error| panic!("{error}\n{source}"));
    program
}

fn rejects_high_and_low(source: &str, builtin: &str, expected_line: usize) {
    let high = parsed(source, true);
    for (source, mode) in [(source.to_owned(), true), (emit::low(&high), false)] {
        let mut program = parsed(&source, mode);
        let error = check::check(&mut program).unwrap_err();
        assert!(error.contains(builtin), "{error}\n{source}");
        if mode {
            assert!(
                error.starts_with(&format!("line {expected_line}:")),
                "{error}"
            );
        }
    }
}

#[test]
fn json_rejects_definitely_non_serializable_types_at_the_call() {
    for ty in [
        "fn[i64]",
        "Error",
        "List[fn[i64]]",
        "Result[i64, Error]",
        "Map[str, List[Error]]",
    ] {
        rejects_high_and_low(
            &format!(
                "def encode(value: {ty}) -> Result[str, Error]:\n    return json_encode(value)\n"
            ),
            "json_encode",
            2,
        );
        rejects_high_and_low(
            &format!("def decode(text: view[str]) -> Result[{ty}, Error]:\n    return json_decode[{ty}](text)\n"),
            "json_decode", 2,
        );
    }
}

#[test]
fn json_decoder_checks_borrowed_slices_and_builtin_map_key_traits() {
    for ty in [
        "view[i64]",
        "view[owned[str]]",
        "List[view[bool]]",
        "Map[f64, str]",
        "Map[UUID, i64]",
        "Map[timestamp, str]",
        "Map[shared[List[f32]], i64]",
        "Map[Map[str, i64], str]",
    ] {
        rejects_high_and_low(
            &format!("def decode(text: view[str]) -> Result[{ty}, Error]:\n    return json_decode[{ty}](text)\n"),
            "json_decode", 2,
        );
    }
}

#[test]
fn json_keeps_data_classes_shared_values_and_borrowed_strings_or_bytes() {
    let models = "class Key:\n    id: i64\nclass Payload:\n    values: shared[List[Result[i64, str]]]\n    children: List[Payload]\nclass fn:\n    count: i64\nclass Future:\n    count: i64\n";
    for ty in [
        "i64",
        "bool",
        "unit",
        "str",
        "bytes",
        "UUID",
        "timestamp",
        "Payload",
        "fn",
        "Future",
        "shared[List[Result[i64, str]]]",
        "Map[str, owned[Payload]]",
        "Map[Key, str]",
        "view[str]",
        "view[bytes]",
        "view[u8]",
        "view[owned[u8]]",
        "List[view[str]]",
    ] {
        let source = format!("{models}def encode(value: {ty}) -> Result[str, Error]:\n    return json_encode(value)\ndef decode(text: view[str]) -> Result[{ty}, Error]:\n    return json_decode[{ty}](text)\n");
        let high = checked(&source);
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
    }
    // Serialization needs no Eq/Hash on a Map key. A value-specific JSON key
    // rejection remains a runtime Result error, rather than a missing trait.
    checked(
        "def encode(value: Map[f64, i64]) -> Result[str, Error]:\n    return json_encode(value)\n",
    );
    checked("def encode(value: view[i64]) -> Result[str, Error]:\n    return json_encode(value)\n");
}

fn database_source(operation: &str, ty: &str) -> String {
    format!("import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nclass Manual:\n    values: List[i64]\nclass fn:\n    id: i64\nasync def read(db: view[sqlite.Tx]) -> Result[unit, sqlite.Failure]:\n    value = try await sqlite.{operation}[{ty}](db, sqlite.literal(\"SELECT id FROM rows\"), sqlite.parameters())\n    return ok(print(1))\n")
}

#[test]
fn sqlite_row_operations_reject_scalar_or_foreign_container_targets() {
    for operation in ["all", "query"] {
        for ty in [
            "i64",
            "str",
            "bytes",
            "bool",
            "unit",
            "UUID",
            "timestamp",
            "Error",
            "sqlite.Pool",
            "List[Row]",
            "Option[Row]",
            "shared[Row]",
            "Result[Row, str]",
            "fn[i64]",
            "owned[i64]",
            "Manual",
            "owned[Row]",
            "owned[owned[Manual]]",
        ] {
            rejects_high_and_low(&database_source(operation, ty), "SQLite行型", 9);
        }
    }
}

#[test]
fn sqlite_rows_keep_generated_scalars_and_native_manual_bridge_boundary() {
    for operation in ["all", "query"] {
        for ty in ["Row", "fn"] {
            let high = checked(&database_source(operation, ty));
            let mut low = parser::parse(&emit::low(&high), false).unwrap();
            check::check(&mut low).unwrap();
            let rust = emit::rust(&checked_emission::seal(&low)).unwrap();
            assert!(rust.contains("impl ::nagi_runtime::FromRow"));
            // A non-scalar class remains valid data but receives no automatic
            // FromRow. owned_database exercises its trusted native adapter.
            let manual = high
                .modules
                .definitions
                .iter()
                .find(|d| d.id.name == "Manual")
                .unwrap();
            assert!(!rust.contains(&format!(
                "impl ::nagi_runtime::FromRow for {}",
                manual.symbol
            )));
        }
    }
}

#[test]
fn sqlite_resource_json_contracts_reject_at_the_call() {
    for ty in ["sqlite.Pool", "owned[sqlite.Pool]", "sqlite.Query"] {
        rejects_high_and_low(&format!("import std.db.sqlite as sqlite\ndef encode(value: {ty}) -> Result[str, Error]:\n    return json_encode(value)\n"), "json_encode", 3);
        rejects_high_and_low(&format!("import std.db.sqlite as sqlite\ndef decode(text: view[str]) -> Result[{ty}, Error]:\n    return json_decode[{ty}](text)\n"), "json_decode", 3);
    }
}

#[test]
fn user_functions_with_json_builtin_names_keep_their_own_contracts() {
    checked("def value() -> i64:\n    return 42\ndef json_encode(f: fn[i64]) -> i64:\n    return f()\ndef main() -> i64:\n    return json_encode(value)\n");
}

#[test]
fn same_named_classes_keep_the_emitted_json_and_database_contracts() {
    for name in ["Error", "UUID", "i64", "f64"] {
        let source = format!("class {name}:\n    value: bool\ndef encode() -> Result[str, Error]:\n    return json_encode({name}(value=True))\n");
        let high = checked(&source);
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
        let rust = emit::rust(&checked_emission::seal(&low)).unwrap();
        assert!(rust.contains(&format!("::nagi_runtime::encode(&{name} {{")));
    }
    for name in [
        "i64",
        "f64",
        "Db",
        "str",
        "bytes",
        "unit",
        "Error",
        "UUID",
        "timestamp",
    ] {
        let high = checked(&format!("import std.db.sqlite as sqlite\nclass {name}:\n    value: bool\nasync def read(db: view[sqlite.Tx]) -> Result[List[{name}], sqlite.Failure]:\n    return await sqlite.all[{name}](db, sqlite.literal(\"SELECT value FROM rows\"), sqlite.parameters())\n"));
        let rust = emit::rust(&checked_emission::seal(&high)).unwrap();
        let row = high
            .modules
            .definitions
            .iter()
            .find(|d| d.id.name == name)
            .unwrap();
        assert!(rust.contains(&format!("impl ::nagi_runtime::FromRow for {}", row.symbol)));
        assert!(rust.contains(&format!("all::<{}>", row.symbol)));
    }
    rejects_high_and_low("class Error:\n    value: bool\ndef decode(text: view[str]) -> Result[Error, Error]:\n    return json_decode[Error](text)\n", "json_decode", 4);
    // A user bridge can implement Eq/Hash for a local f64 record, while the
    // runtime UUID/Timestamp types still cannot acquire those foreign traits.
    checked("class f64:\n    value: bool\ndef decode(text: view[str]) -> Result[Map[f64, str], Error]:\n    return json_decode[Map[f64, str]](text)\n");
}

#[test]
fn encoded_runtime_values_are_rejected_even_when_a_same_named_class_exists() {
    for name in ["Error"] {
        rejects_high_and_low(
            &format!("class {name}:\n    value: bool\ndef encode(value: {name}) -> Result[str, Error]:\n    return json_encode(value)\n"),
            "json_encode", 4,
        );
        for expression in [
            format!("[{name}(value=True)]"),
            format!("some({name}(value=True))"),
            format!("share({name}(value=True))"),
            format!("share([{name}(value=True)])"),
            format!("clone_shared(share({name}(value=True)))"),
            format!("copy(view([{name}(value=True)]))"),
            format!("[{name}(value=True)][0]"),
        ] {
            let source = format!("class {name}:\n    value: bool\ndef encode() -> Result[str, Error]:\n    return json_encode({expression})\n");
            let high = checked(&source);
            let mut low = parser::parse(&emit::low(&high), false).unwrap();
            check::check(&mut low).unwrap();
        }
    }
}

#[test]
fn borrowed_byte_decoding_requires_builtin_u8_in_generated_slices() {
    for ty in [
        "view[u8]",
        "view[owned[u8]]",
        "view[bytes]",
        "List[view[u8]]",
    ] {
        rejects_high_and_low(
            &format!("class u8:\n    value: bool\ndef decode(text: view[str]) -> Result[{ty}, Error]:\n    return json_decode[{ty}](text)\n"),
            "json_decode", 4,
        );
    }
}

#[test]
fn json_encoding_rejects_unawaited_futures_and_inferred_future_containers() {
    for expression in [
        "sleep(1)",
        "[sleep(1)]",
        "some(sleep(1))",
        "share(sleep(1))",
        "share([sleep(1)])",
        "clone_shared(share(sleep(1)))",
    ] {
        rejects_high_and_low(
            &format!("def encode() -> Result[str, Error]:\n    return json_encode({expression})\n"),
            "json_encode",
            2,
        );
    }
}

// Html is now a retired builtin, so its migration rejection must not count
// as a JSON/DB serialization-contract success. Keep the stages distinct.
#[test]
fn retired_html_annotations_report_migration_before_json_or_db_contracts() {
    for text in [
        "def encode(value: Html) -> Result[str, Error]:\n    return json_encode(value)\n",
        "def decode(value: view[str]) -> Result[shared[Html], Error]:\n    return json_decode[shared[Html]](value)\n",
        "class Html:\n    value: bool\ndef encode(value: Html) -> Result[str, Error]:\n    return json_encode(value)\n",
    ] {
        let line = if text.starts_with("class") { 3 } else { 1 };
        rejects_high_and_low(text, "SF01 migration", line);
    }
    for operation in ["all", "query"] {
        rejects_high_and_low(&database_source(operation, "Html"), "SF01 migration", 9);
    }
}
