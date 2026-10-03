use nagic::{check, emit, parser, source};
use std::{fs, path::PathBuf, process::Command};

static FIXTURE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi-module-emission-{}-{}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }

    fn compile_and_run(&self, text: &str) {
        let source = self.0.join("generated.rs");
        let binary = self
            .0
            .join(format!("generated{}", std::env::consts::EXE_SUFFIX));
        fs::write(&source, text).unwrap();
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2021", "--test", "-D", "unused-imports"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn independent_low_parser_preserves_module_ids_and_native_function_aliases() {
    let fixture = Fixture::new();
    fixture.write(
        "orders.nagi",
        "def score(value: i64) -> i64:\n    return value + 1\ndef main() -> i64:\n    return score(40)\ndef type(value: i64) -> i64:\n    return value + 1\ndef len(value: i64) -> i64:\n    return value + 1000\n",
    );
    fixture.write(
        "other.nagi",
        "def score(value: i64) -> i64:\n    return value + 100\n",
    );
    fixture.write(
        "main.nagi",
        "import \"orders.nagi\" as orders\nimport \"other.nagi\" as other\nfrom \"orders.nagi\" import score as SavedScore\nfrom \"orders.nagi\" import main as helper_main\ndef answer() -> i64:\n    callback = orders.score\n    return callback(20) + other.score(1) + SavedScore(2)\ndef main():\n    assert_true(orders.main() == 41)\n    assert_true(helper_main() == 41)\n    assert_true(orders.len(1) == 1001)\n    assert_true(answer() == 125)\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let emitted = emit::low_with_lines(&loaded.program);
    assert!(emitted.text.contains("# nagi-modules-v1 "));
    let mut low = parser::parse(&emitted.text, false).unwrap();
    let identities = |program: &nagic::ast::Program| {
        program
            .modules
            .definitions
            .iter()
            .map(|definition| (definition.id.clone(), definition.symbol.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(identities(&low), identities(&loaded.program));
    emitted.restore_lines(&mut low).unwrap();
    check::check(&mut low).unwrap();
    let report = emit::cost_report(&low);
    let costs = report["functions"].as_object().unwrap();
    for name in ["answer", "main", "SavedScore", "other.score", "orders.len"] {
        assert!(costs.contains_key(name), "{report}");
    }
    assert!(costs.keys().all(|name| !name.contains("__nagi_def_")));
    let mut rust = emit::rust(&low).unwrap();
    rust.push_str(
        "\nmod native {\n    pub fn verify() {\n        assert_eq!(super::orders::score(1), 2);\n        assert_eq!(super::SavedScore(3), 4);\n        assert_eq!(super::other::score(1), 101);\n        assert_eq!(super::orders::main(), 41);\n        assert_eq!(super::helper_main(), 41);\n        assert_eq!(super::orders::len(1), 1001);\n        assert_eq!(super::orders::r#type(4), 5);\n    }\n}\n#[test] fn adapter_uses_aliases_of_the_same_function() { native::verify(); assert_eq!(answer(), 125); main(); }\n",
    );
    fixture.compile_and_run(&rust);
}

#[test]
fn qualified_nested_types_emit_internal_names_through_low() {
    let fixture = Fixture::new();
    fixture.write("orders.nagi", "class Order:\n    value: i64\n");
    fixture.write("other.nagi", "class Order:\n    value: i64\n");
    fixture.write(
        "main.nagi",
        "import \"orders.nagi\" as orders\nimport \"other.nagi\" as other\nfrom \"orders.nagi\" import Order as SavedOrder\nclass Pair:\n    first: orders.Order\n    others: List[other.Order]\ndef keep(value: List[SavedOrder]) -> List[orders.Order]:\n    return value\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let text = emit::low(&loaded.program);
    let mut low = parser::parse(&text, false).unwrap();
    check::check(&mut low).unwrap();
    assert_eq!(
        loaded.program.functions[0].params[0].1,
        low.functions[0].params[0].1
    );
    assert_eq!(loaded.program.functions[0].ret, low.functions[0].ret);
    assert_eq!(loaded.program.classes[2].fields, low.classes[2].fields);
    let rust = emit::rust(&low).unwrap();
    assert!(rust.contains("pub mod orders {"));
    assert!(rust.contains(" as SavedOrder;"));
}

#[test]
fn unresolved_high_imports_keep_as_and_from_syntax_in_low() {
    let high = parser::parse(
        "import \"orders.nagi\" as orders\nfrom \"orders.nagi\" import Order as SavedOrder\nfrom \"orders.nagi\" import score\n",
        true,
    )
    .unwrap();
    let text = emit::low(&high);
    assert!(text.contains("import \"orders.nagi\" as orders;"));
    assert!(text.contains("from \"orders.nagi\" import Order as SavedOrder;"));
    assert!(text.contains("from \"orders.nagi\" import score;"));
    let low = parser::parse(&text, false).unwrap();
    assert_eq!(low.module_imports.len(), 3);
}

#[test]
fn legacy_flat_cost_report_keeps_public_function_names() {
    let fixture = Fixture::new();
    fixture.write("library.nagi", "def score() -> i64:\n    return 42\n");
    fixture.write(
        "main.nagi",
        "import \"library.nagi\"\ndef answer() -> i64:\n    return score()\n",
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let report = emit::cost_report(&loaded.program);
    let costs = report["functions"].as_object().unwrap();
    assert_eq!(costs.len(), 2);
    assert!(costs.contains_key("score"));
    assert!(costs.contains_key("answer"));
}

#[test]
fn source_locals_that_match_generated_symbols_keep_distinct_values() {
    let fixture = Fixture::new();
    fixture.write("library.nagi", "def score() -> i64:\n    return 3\n");
    fixture.write("main.nagi", "");
    let library = nagic::ast::ModuleId(
        fs::canonicalize(fixture.0.join("library.nagi"))
            .unwrap()
            .display()
            .to_string(),
    );
    let foreign_symbol = nagic::modules::symbol(&nagic::ast::DefId {
        module: library,
        kind: nagic::ast::DefKind::Function,
        name: "score".into(),
    });
    let root_path = fs::canonicalize(fixture.0.join("main.nagi"))
        .unwrap()
        .display()
        .to_string();
    let root_hex = root_path
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let reserved_local = format!("__nagi_local_{root_hex}_0");
    fixture.write(
        "main.nagi",
        &format!(
            "import \"library.nagi\" as library\ndef compute({foreign_symbol}: i64, {reserved_local}: i64) -> i64:\n    return {foreign_symbol} * 100 + {reserved_local} * 10 + library.score()\ndef main():\n    assert_true(compute(2, 3) == 233)\n"
        ),
    );
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let mut low = parser::parse(&emit::low(&loaded.program), false).unwrap();
    check::check(&mut low).unwrap();
    let mut rust = emit::rust(&low).unwrap();
    rust.push_str("\n#[test] fn preserved_local_values() { assert_eq!(compute(2, 3), 233); assert_eq!(compute(4, 5), 453); main(); }\n");
    fixture.compile_and_run(&rust);
}

#[test]
fn source_function_names_that_match_foreign_symbols_keep_distinct_call_targets() {
    let fixture = Fixture::new();
    fixture.write("library.nagi", "def score() -> i64:\n    return 3\n");
    let foreign_symbol = nagic::modules::symbol(&nagic::ast::DefId {
        module: nagic::ast::ModuleId(
            fs::canonicalize(fixture.0.join("library.nagi"))
                .unwrap()
                .display()
                .to_string(),
        ),
        kind: nagic::ast::DefKind::Function,
        name: "score".into(),
    });
    fixture.write("main.nagi", &format!("import \"library.nagi\" as library\ndef {foreign_symbol}() -> i64:\n    return 40\ndef answer() -> i64:\n    return {foreign_symbol}() + library.score()\ndef main():\n    assert_true(answer() == 43)\n"));
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let mut low = parser::parse(&emit::low(&loaded.program), false).unwrap();
    check::check(&mut low).unwrap();
    let mut rust = emit::rust(&low).unwrap();
    rust.push_str(
        "\n#[test] fn distinct_global_call_targets() { assert_eq!(answer(), 43); main(); }\n",
    );
    fixture.compile_and_run(&rust);
}

#[test]
fn inferred_builtin_types_survive_module_aliases_low_transport_and_rust_emission() {
    for alias in ["Option", "str", "i64", "bool", "unit", "usize", "u8"] {
        let fixture = Fixture::new();
        fixture.write("library.nagi", "def score() -> i64:\n    return 3\ndef byte_length(value: bytes) -> i64:\n    part = view(value)\n    return len(part)\ndef text_length(value: view[str]) -> i64:\n    return len(value)\ndef from_bytes(value: view[bytes]) -> bytes:\n    return copy(value)\ndef first(values: view[i64]) -> i64:\n    return values[1]\ndef cast(value: u8) -> i64:\n    return i64(value)\n");
        fixture.write("main.nagi", &format!("import \"library.nagi\" as {alias}\ndef main():\n    option = some(1)\n    text = \"hello\"\n    part = view(text)\n    value = 40\n    flag = true\n    assert_true(flag)\n    assert_true(len(part) == 5)\n    assert_true(value + {alias}.score() == 43)\n    values = [2, 3, 5]\n    assert_true(values[1] == 3)\n    assert_true(len(view(values)) == 3)\n    callback = {alias}.score\n    assert_true(callback() == 3)\n"));
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{alias}: {}", loaded.diagnostic(&error)));
        let types = |program: &nagic::ast::Program| {
            let main = program
                .functions
                .iter()
                .find(|function| function.name == "main")
                .unwrap();
            assert_eq!(main.ret, nagic::ast::Type::named("unit"));
            main.body
                .iter()
                .take(5)
                .map(|statement| statement.binding_type.clone().unwrap())
                .collect::<Vec<_>>()
        };
        let expected = vec![
            nagic::ast::Type::generic("Option", vec![nagic::ast::Type::named("i64")]),
            nagic::ast::Type::named("str"),
            nagic::ast::Type::generic("view", vec![nagic::ast::Type::named("str")]),
            nagic::ast::Type::named("i64"),
            nagic::ast::Type::named("bool"),
        ];
        assert_eq!(types(&loaded.program), expected, "High alias {alias}");
        let original_ids = loaded
            .program
            .modules
            .definitions
            .iter()
            .map(|definition| definition.id.clone())
            .collect::<Vec<_>>();
        let emitted = emit::low(&loaded.program);
        let mut independent = parser::parse(&emitted, false).unwrap();
        check::check(&mut independent)
            .unwrap_or_else(|error| panic!("independent Low alias {alias}: {error}\n{emitted}"));
        assert_eq!(
            types(&independent),
            expected,
            "independent Low alias {alias}"
        );
        fixture.write("saved.low", &emitted);
        let mut saved = source::load(&fixture.0.join("saved.low"), false).unwrap();
        check::check(&mut saved.program).unwrap_or_else(|error| {
            panic!("saved Low alias {alias}: {}", saved.diagnostic(&error))
        });
        assert_eq!(types(&saved.program), expected, "saved Low alias {alias}");
        assert_eq!(
            saved
                .program
                .modules
                .definitions
                .iter()
                .map(|definition| definition.id.clone())
                .collect::<Vec<_>>(),
            original_ids
        );
        let mut rust = emit::rust(&saved.program).unwrap();
        rust.push_str(&format!("\n#[test] fn intrinsic_values_and_public_module_alias() {{ assert_eq!({alias}::score(), 3); assert_eq!({alias}::byte_length(vec![0, 128, 255]), 3); assert_eq!({alias}::text_length(\"hello\"), 5); assert_eq!({alias}::from_bytes(&[0, 128, 255]), vec![0, 128, 255]); assert_eq!({alias}::first(&[7, 8]), 8); assert_eq!({alias}::cast(255), 255); main(); }}\n"));
        fixture.compile_and_run(&rust);
    }
}

#[test]
fn inferred_primitive_values_and_a_class_alias_keep_distinct_type_identities() {
    let fixture = Fixture::new();
    fixture.write("library.nagi", "class Record:\n    value: i64\n");
    fixture.write("main.nagi", "from \"library.nagi\" import Record as str\ndef main():\n    text = \"hello\"\n    number = 42\n    flag = true\n    record = str(value=number)\n    assert_true(flag)\n    print(text)\n    print(record.value)\n");
    let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
    check::check(&mut loaded.program).unwrap();
    let record = loaded
        .program
        .modules
        .resolve_root_path("str")
        .unwrap()
        .clone();
    let verify = |program: &nagic::ast::Program| {
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        let types = main
            .body
            .iter()
            .take(4)
            .map(|statement| statement.binding_type.clone().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            types,
            vec![
                nagic::ast::Type::named("str"),
                nagic::ast::Type::named("i64"),
                nagic::ast::Type::named("bool"),
                nagic::ast::Type::named(&record.symbol)
            ]
        );
        assert_eq!(
            program.modules.resolve_root_path("str").unwrap().id,
            record.id
        );
    };
    verify(&loaded.program);
    let emitted = emit::low(&loaded.program);
    let mut independent = parser::parse(&emitted, false).unwrap();
    check::check(&mut independent).unwrap();
    verify(&independent);
    fixture.write("saved.low", &emitted);
    let mut saved = source::load(&fixture.0.join("saved.low"), false).unwrap();
    check::check(&mut saved.program).unwrap();
    verify(&saved.program);
}
