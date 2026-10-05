#[path = "support/checked_emission.rs"]
mod checked_emission;
use nagic::{check, emit, parser};

fn checked(source: &str) -> nagic::ast::Program {
    let mut program = parser::parse(source, true).unwrap();
    check::check(&mut program).unwrap_or_else(|error| panic!("{error}\n{source}"));
    program
}

fn rejects_high_and_low(source: &str, builtin: &str, expected_line: usize) {
    let high = parser::parse(source, true).unwrap();
    for (source, mode) in [(source.to_owned(), true), (emit::low(&high), false)] {
        let mut program = parser::parse(&source, mode).unwrap();
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
        "Db",
        "Html",
        "List[fn[i64]]",
        "shared[Html]",
        "Result[i64, Error]",
        "owned[Db]",
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

fn database_source(builtin: &str, ty: &str) -> String {
    let args = match builtin {
        "db_all" => "db, \"SELECT id FROM rows\"",
        "db_query" => "db, \"SELECT id FROM rows WHERE id=?1\", 1",
        "db_insert" => "db, \"INSERT\", \"name\", 1",
        "db_update" => "db, \"UPDATE\", 1, \"name\", 1",
        _ => unreachable!(),
    };
    format!("class Row:\n    id: i64\nclass Manual:\n    values: List[i64]\nclass fn:\n    id: i64\nasync def read(db: Db) -> Result[unit, Error]:\n    value = try await {builtin}[{ty}]({args})\n    return ok(print(1))\n")
}

#[test]
fn all_database_row_builtins_reject_scalar_or_foreign_container_targets() {
    for builtin in ["db_all", "db_query", "db_insert", "db_update"] {
        for ty in [
            "i64",
            "str",
            "bytes",
            "bool",
            "unit",
            "UUID",
            "timestamp",
            "Error",
            "Db",
            "Html",
            "List[Row]",
            "Option[Row]",
            "shared[Row]",
            "Result[Row, str]",
            "fn[i64]",
            "owned[i64]",
        ] {
            rejects_high_and_low(&database_source(builtin, ty), builtin, 8);
        }
    }
}

#[test]
fn database_rows_keep_generated_and_manual_bridge_implementations() {
    for builtin in ["db_all", "db_query", "db_insert", "db_update"] {
        for ty in ["Row", "Manual", "fn", "owned[Row]", "owned[owned[Manual]]"] {
            let high = checked(&database_source(builtin, ty));
            let mut low = parser::parse(&emit::low(&high), false).unwrap();
            check::check(&mut low).unwrap();
            let rust = emit::rust(&checked_emission::seal(&low)).unwrap();
            assert!(rust.contains("impl ::nagi_runtime::FromRow for Row"));
            assert!(!rust.contains("impl ::nagi_runtime::FromRow for Manual"));
        }
    }
}

#[test]
fn user_functions_with_json_builtin_names_keep_their_own_contracts() {
    checked("def value() -> i64:\n    return 42\ndef json_encode(f: fn[i64]) -> i64:\n    return f()\ndef main() -> i64:\n    return json_encode(value)\n");
}

#[test]
fn same_named_classes_keep_the_emitted_json_and_database_contracts() {
    for name in ["Error", "Db", "Html", "UUID", "i64", "f64"] {
        let source = format!("class {name}:\n    value: bool\ndef encode() -> Result[str, Error]:\n    return json_encode({name}(value=True))\n");
        let high = checked(&source);
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
        let rust = emit::rust(&checked_emission::seal(&low)).unwrap();
        assert!(rust.contains(&format!("::nagi_runtime::encode(&{name} {{")));
    }
    // These names are fully qualified/aliased by rust_type; their local
    // classes cannot supply FromRow for the actual database type argument.
    for name in [
        "str",
        "bytes",
        "unit",
        "Error",
        "Db",
        "Html",
        "UUID",
        "timestamp",
    ] {
        rejects_high_and_low(
            &format!("class {name}:\n    value: bool\nasync def read(db: Db) -> Result[List[{name}], Error]:\n    return await db_all[{name}](db, \"rows\")\n"),
            "db_all", 4,
        );
    }
    for name in ["Error", "Db", "Html"] {
        rejects_high_and_low(
            &format!("class {name}:\n    value: bool\ndef decode(text: view[str]) -> Result[{name}, Error]:\n    return json_decode[{name}](text)\n"),
            "json_decode", 4,
        );
    }
    // Rust resolves an unqualified primitive name to a local struct. Do not
    // reject such row classes merely because Type::is_copy knows the name.
    for name in ["i64", "f64"] {
        let source = format!("class {name}:\n    value: bool\nasync def read(db: Db) -> Result[List[{name}], Error]:\n    return await db_all[{name}](db, \"rows\")\n");
        let high = checked(&source);
        let rust = emit::rust(&checked_emission::seal(&high)).unwrap();
        assert!(rust.contains(&format!("impl ::nagi_runtime::FromRow for {name}")));
        assert!(rust.contains(&format!("all::<{name}>")));
    }
    // A user bridge can implement Eq/Hash for a local f64 record, while the
    // runtime UUID/Timestamp types still cannot acquire those foreign traits.
    checked("class f64:\n    value: bool\ndef decode(text: view[str]) -> Result[Map[f64, str], Error]:\n    return json_decode[Map[f64, str]](text)\n");
}

#[test]
fn encoded_runtime_values_are_rejected_even_when_a_same_named_class_exists() {
    for name in ["Error", "Db", "Html"] {
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
