use nagic::{check, emit, source};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi modules {} {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn load(&self, name: &str) -> Result<source::Sources, String> {
        source::load(&self.0.join(name), name.ends_with(".nagi"))
    }

    fn checked(&self, name: &str) -> source::Sources {
        let mut loaded = self.load(name).unwrap();
        check::check(&mut loaded.program)
            .unwrap_or_else(|error| panic!("{}", loaded.diagnostic(&error)));
        loaded
    }

    fn error(&self, name: &str) -> String {
        match self.load(name) {
            Err(error) => error,
            Ok(mut loaded) => {
                let error = check::check(&mut loaded.program)
                    .expect_err("invalid module program passed check");
                loaded.diagnostic(&error)
            }
        }
    }

    fn cli(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .current_dir(cwd)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn successful(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn aliases_from_imports_and_flat_imports_share_the_canonical_class() {
    let f = Fixture::new();
    f.write(
        "domain/orders.nagi",
        "class Order:\n    number: i64\ndef score(value: Order) -> i64:\n    return value.number\n",
    );
    f.write("main.nagi", "import \"domain/orders.nagi\" as orders\nimport \"domain/../domain/orders.nagi\" as again\nfrom \"domain/orders.nagi\" import Order as SavedOrder\nfrom \"domain/orders.nagi\" import score as saved_score\nimport \"domain/orders.nagi\"\nclass Envelope:\n    item: orders.Order\ndef identity(value: orders.Order) -> SavedOrder:\n    return value\ndef count(values: List[SavedOrder]) -> i64:\n    return len(view(values))\ndef main():\n    first: Order = identity(again.Order(number=40))\n    wrapped = Envelope(item=SavedOrder(number=2))\n    print(orders.score(first) + saved_score(wrapped.item))\n    print(count([orders.Order(number=1)]))\n");
    let loaded = f.checked("main.nagi");
    assert_eq!(
        loaded.program.classes.len(),
        2,
        "one Order definition and one Envelope definition"
    );
    assert_eq!(
        loaded.program.functions.len(),
        4,
        "same real file must be loaded once"
    );
    successful(f.cli(&f.0, &["check", "main.nagi", "--out", "lowered"]));
    let generated = fs::read_to_string(f.0.join("lowered/generated.low")).unwrap();
    // This executes the complete public High -> generated Low -> loader/check path.
    f.write("roundtrip.low", &generated);
    successful(f.cli(&f.0, &["check", "roundtrip.low", "--out", "roundtrip"]));
}

#[test]
fn same_named_classes_from_different_files_are_distinct_in_all_type_positions() {
    let f = Fixture::new();
    for name in ["left.nagi", "right.nagi"] {
        f.write(name, "class Order:\n    number: i64\n");
    }
    let imports = "import \"left.nagi\" as left\nimport \"right.nagi\" as right\n";
    for (body, line) in [
        ("def take(value: left.Order):\n    print(value.number)\ndef main():\n    take(right.Order(number=1))\n", 6),
        ("def create() -> left.Order:\n    return right.Order(number=1)\n", 4),
        ("class Envelope:\n    item: left.Order\ndef main():\n    item = Envelope(item=right.Order(number=1))\n", 6),
        ("def take(values: List[left.Order]):\n    print(len(view(values)))\ndef main():\n    take([right.Order(number=1)])\n", 6),
    ] {
        f.write("main.nagi", &format!("{imports}{body}"));
        let error = f.error("main.nagi");
        assert!(error.contains(&format!("main.nagi:{line}")), "{body}\n{error}");
        assert!(error.contains("Order"), "diagnostic should describe the incompatible record types: {error}");
    }
}

#[test]
fn imported_bindings_are_private_but_own_definitions_can_use_them() {
    let f = Fixture::new();
    f.write(
        "hidden.nagi",
        "class Hidden:\n    value: i64\ndef secret() -> i64:\n    return 42\n",
    );
    f.write("facade.nagi", "from \"hidden.nagi\" import secret as helper\nfrom \"hidden.nagi\" import Hidden as Private\ndef answer() -> i64:\n    return helper()\n");
    f.write(
        "main.nagi",
        "import \"facade.nagi\" as facade\ndef main():\n    print(facade.answer())\n",
    );
    f.checked("main.nagi");
    for (text, line, name) in [
        (
            "from \"facade.nagi\" import helper\ndef main():\n    print(helper())\n",
            1,
            "helper",
        ),
        ("from \"facade.nagi\" import Private\n", 1, "Private"),
        (
            "import \"facade.nagi\" as facade\ndef main():\n    print(facade.helper())\n",
            3,
            "helper",
        ),
        (
            "import \"facade.nagi\" as facade\ndef main():\n    print(helper())\n",
            3,
            "helper",
        ),
        ("from \"facade.nagi\" import missing\n", 1, "missing"),
    ] {
        f.write("main.nagi", text);
        let error = f.error("main.nagi");
        assert!(error.contains(&format!("main.nagi:{line}")), "{error}");
        assert!(error.contains(name), "{error}");
    }
}

#[test]
fn guessed_internal_symbols_cannot_bypass_private_imports_in_raw_source() {
    let f = Fixture::new();
    f.write(
        "hidden.nagi",
        "class Hidden:\n    value: i64\ndef secret() -> i64:\n    return 42\n",
    );
    f.write("facade.nagi", "from \"hidden.nagi\" import secret as helper\nfrom \"hidden.nagi\" import Hidden as Private\ndef answer() -> i64:\n    value = Private(value=helper())\n    return value.value\n");
    let module = nagic::ast::ModuleId(
        fs::canonicalize(f.0.join("hidden.nagi"))
            .unwrap()
            .display()
            .to_string(),
    );
    let function = nagic::modules::symbol(&nagic::ast::DefId {
        module: module.clone(),
        kind: nagic::ast::DefKind::Function,
        name: "secret".into(),
    });
    let class = nagic::modules::symbol(&nagic::ast::DefId {
        module,
        kind: nagic::ast::DefKind::Class,
        name: "Hidden".into(),
    });
    f.write(
        "main.nagi",
        "import \"facade.nagi\" as facade\ndef main():\n    print(facade.answer())\n",
    );
    let loaded = f.checked("main.nagi");
    assert!(loaded.program.modules.definition(&function).is_some());
    assert!(loaded.program.modules.definition(&class).is_some());
    f.write("saved.low", &emit::low(&loaded.program));
    f.checked("saved.low");
    for (body, line) in [
        (format!("def main():\n    print({function}())\n"), 3),
        (
            format!("def take(value: {class}):\n    print(value.value)\n"),
            2,
        ),
        (
            format!("def main():\n    value = {class}(value=42)\n    print(value.value)\n"),
            3,
        ),
        (
            format!("class {function}:\n    value: i64\ndef main():\n    print({function}())\n"),
            5,
        ),
        (
            format!("def {class}() -> i64:\n    return 1\ndef take(value: {class}):\n    print(value.value)\n"),
            4,
        ),
    ] {
        let source = format!("import \"facade.nagi\" as facade\n{body}");
        f.write("main.nagi", &source);
        let error = f.error("main.nagi");
        assert!(error.contains(&format!("main.nagi:{line}")), "{error}");
        assert!(
            error.contains(source.lines().nth(line - 1).unwrap()),
            "the diagnostic must retain the actual raw source: {error}"
        );
        let output = f.cli(&f.0, &["check", "main.nagi"]);
        assert!(
            !output.status.success(),
            "a guessed private identity must not become a raw-source binding"
        );
    }
}

#[test]
fn conflicting_import_bindings_report_the_import_statement() {
    let f = Fixture::new();
    f.write(
        "one.nagi",
        "class Order:\n    number: i64\ndef score() -> i64:\n    return 1\n",
    );
    f.write("two.nagi", "def score() -> i64:\n    return 2\n");
    for (text, line) in [
        (
            "import \"one.nagi\" as orders\nimport \"two.nagi\" as orders\n",
            2,
        ),
        (
            "from \"one.nagi\" import Order as orders\nimport \"two.nagi\" as orders\n",
            2,
        ),
        (
            "from \"one.nagi\" import score\nfrom \"two.nagi\" import score\n",
            2,
        ),
        (
            "import \"one.nagi\" as score\ndef score() -> i64:\n    return 0\n",
            1,
        ),
        (
            "from \"one.nagi\" import Order\nclass Order:\n    number: i64\n",
            1,
        ),
    ] {
        f.write("main.nagi", text);
        let error = f.error("main.nagi");
        assert!(
            error.contains(&format!("main.nagi:{line}")),
            "{text}\n{error}"
        );
        assert!(
            error.contains(text.lines().nth(line - 1).unwrap()),
            "import source must be retained: {error}"
        );
    }
}

#[test]
fn local_fields_and_function_values_shadow_imports_only_in_their_scope() {
    let f = Fixture::new();
    f.write(
        "math.nagi",
        "def score(value: i64) -> i64:\n    return value + 1\n",
    );
    f.write(
        "main.nagi",
        r#"import "math.nagi" as math
from "math.nagi" import score
class Holder:
    score: i64
def fixed(value: i64) -> i64:
    return 99
def main():
    print(math.score(1))
    math = Holder(score=7)
    print(math.score)
    score = fixed
    print(score(1))
def field(math: Holder) -> i64:
    return math.score
def outside() -> i64:
    if True:
        math = Holder(score=7)
        print(math.score)
    while False:
        math = Holder(score=7)
        print(math.score)
    for math in range(1):
        print(math)
    match parse_i64("7"):
        case Ok(math):
            print(math)
        case Err(_):
            print(0)
    return math.score(40) + score(0)
async def scoped() -> Result[i64, Error]:
    async with scope:
        math = Holder(score=7)
        print(math.score)
    return ok(math.score(40))
"#,
    );
    f.checked("main.nagi");
    // Reject calling the record field after the local namespace name has shadowed it.
    f.write("bad.nagi", "import \"math.nagi\" as math\nclass Holder:\n    score: i64\ndef main():\n    math = Holder(score=7)\n    math.score(1)\n");
    let error = f.error("bad.nagi");
    assert!(error.contains("bad.nagi:6"), "{error}");
}

#[test]
fn aliases_keep_original_source_diagnostics_and_embedded_asset_paths() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        "import \"lib/read.nagi\" as reader\ndef main():\n    print(reader.read())\n",
    );
    f.write("lib/read.nagi", "def read() -> str:\n    label = \"orders.Order SavedOrder generated::orders::score unchanged\"\n    print(label)\n    return include_text(\"text.txt\")\n");
    f.write(
        "lib/text.txt",
        "orders.Order SavedOrder generated::orders::score unchanged\n",
    );
    let loaded = f.checked("main.nagi");
    let generated = emit::low(&loaded.program);
    assert!(
        generated.contains("\"orders.Order SavedOrder generated::orders::score unchanged\""),
        "string literals must retain user text: {generated}"
    );
    assert!(
        generated.contains(
            &fs::canonicalize(f.0.join("lib/text.txt"))
                .unwrap()
                .display()
                .to_string()
        ),
        "{generated}"
    );
    f.write("roundtrip.low", &generated);
    f.checked("roundtrip.low");
    f.write("lib/read.nagi", "def read() -> str:\n    return 1\n");
    let error = f.error("main.nagi");
    assert!(error.contains("lib/read.nagi:2"), "{error}");
    assert!(error.contains("return 1"), "{error}");
}

#[test]
fn unsaved_overlays_change_imported_definitions_and_keep_disk_sources_unchanged() {
    let f = Fixture::new();
    let saved =
        "class Order:\n    number: i64\ndef score(value: Order) -> str:\n    return value.number\n";
    f.write("orders.nagi", saved);
    f.write("main.nagi", "import \"orders.nagi\" as orders\nfrom \"orders.nagi\" import Order as SavedOrder\ndef main():\n    print(orders.score(SavedOrder(number=42)))\n");
    assert!(f.error("main.nagi").contains("orders.nagi:4"));
    let overlays = HashMap::from([(
        fs::canonicalize(f.0.join("orders.nagi")).unwrap(),
        "class Order:\n    number: i64\ndef score(value: Order) -> i64:\n    return value.number\n"
            .to_owned(),
    )]);
    let mut loaded = source::load_with_overlays(&f.0.join("main.nagi"), true, &overlays).unwrap();
    check::check(&mut loaded.program).unwrap();
    assert_eq!(loaded.program.classes.len(), 1);
    assert_eq!(fs::read_to_string(f.0.join("orders.nagi")).unwrap(), saved);
    let mut overlays = overlays;
    overlays.insert(
        fs::canonicalize(f.0.join("main.nagi")).unwrap(),
        "from \"orders.nagi\" import Missing as SavedOrder\n".into(),
    );
    let error = source::load_with_overlays(&f.0.join("main.nagi"), true, &overlays)
        .err()
        .unwrap();
    assert!(
        error.contains("main.nagi:1") && error.contains("Missing"),
        "{error}"
    );
}

#[test]
fn low_alias_syntax_resolves_qualified_record_and_generic_types() {
    let f = Fixture::new();
    f.write(
        "orders.low",
        "record Order { number: i64; }\nfn score(value: Order) -> i64 { return value.number; }\n",
    );
    f.write("main.low", "import \"orders.low\" as orders;\nfrom \"orders.low\" import Order as SavedOrder;\nfrom \"orders.low\" import score as saved_score;\nrecord Envelope { item: orders.Order; }\nfn identity(value: SavedOrder) -> orders.Order { return value; }\nfn count(values: List[orders.Order]) -> i64 { return len(view(values)); }\nfn main() -> unit { let value = Envelope(item=identity(SavedOrder(number=42))); print(saved_score(value.item)); print(count([orders.Order(number=1)])); }\n");
    f.checked("main.low");
    successful(f.cli(&f.0, &["check", "main.low"]));
}

#[test]
fn old_flat_imports_keep_transitive_names_and_diamond_deduplication() {
    let f = Fixture::new();
    f.write(
        "shared.nagi",
        "class Item:\n    value: i64\ndef shared() -> i64:\n    return 40\n",
    );
    f.write(
        "left.nagi",
        "import \"shared.nagi\"\ndef left() -> i64:\n    return shared() + 1\n",
    );
    f.write(
        "right.nagi",
        "import \"shared.nagi\"\ndef right() -> i64:\n    return shared() + 2\n",
    );
    f.write("main.nagi", "import \"left.nagi\"\nimport \"right.nagi\"\ndef main():\n    item = Item(value=shared())\n    print(item.value + left() + right())\n");
    let loaded = f.checked("main.nagi");
    assert_eq!(loaded.program.classes.len(), 1);
    assert_eq!(loaded.program.functions.len(), 4);
    successful(f.cli(&f.0, &["check", "main.nagi"]));
}

#[test]
fn legacy_flat_helpers_can_call_root_definitions_without_inheriting_private_aliases() {
    let f = Fixture::new();
    f.write(
        "library.nagi",
        "def helper() -> i64:\n    return answer()\n",
    );
    f.write("main.nagi", "import \"library.nagi\"\ndef answer() -> i64:\n    return 42\ndef main():\n    print(helper())\n");
    let loaded = f.checked("main.nagi");
    let answer = loaded
        .program
        .modules
        .resolve_root_path("answer")
        .unwrap()
        .clone();
    let helper = loaded
        .program
        .modules
        .resolve_root_path("helper")
        .unwrap()
        .clone();
    let verify = |program: &nagic::ast::Program| {
        assert_eq!(
            program.modules.resolve_root_path("answer").unwrap().id,
            answer.id
        );
        assert_eq!(
            program.modules.resolve_root_path("helper").unwrap().id,
            helper.id
        );
        let function = program
            .functions
            .iter()
            .find(|function| function.name == helper.symbol)
            .unwrap();
        let nagic::ast::S::Return(Some(value)) = &function.body[0].kind else {
            panic!("legacy helper must retain its returned call")
        };
        let nagic::ast::E::Call(target, _, _) = &value.kind else {
            panic!("legacy helper must retain its root call")
        };
        assert_eq!(
            target, &answer.symbol,
            "the legacy helper must call the root definition's canonical identity"
        );
        assert_eq!(value.resolution, Some(nagic::ast::NameResolution::Function));
    };
    verify(&loaded.program);
    let generated = emit::low(&loaded.program);
    let mut parsed = nagic::parser::parse(&generated, false).unwrap();
    check::check(&mut parsed).unwrap();
    verify(&parsed);
    f.write("saved.low", &generated);
    verify(&f.checked("saved.low").program);
    successful(f.cli(&f.0, &["check", "main.nagi"]));
    successful(f.cli(&f.0, &["check", "saved.low"]));

    f.write("main.nagi", "import \"library.nagi\" as library\ndef answer() -> i64:\n    return 42\ndef main():\n    print(library.helper())\n");
    let error = f.error("main.nagi");
    assert!(
        error.contains("library.nagi:2") && error.contains("answer"),
        "a namespace-imported file must retain its own scope: {error}"
    );

    f.write("service.nagi", "def answer() -> i64:\n    return 42\n");
    for private_import in [
        "from \"service.nagi\" import answer as private_answer\n",
        "import \"service.nagi\" as private_service\n",
    ] {
        let call = if private_import.starts_with("from") {
            "private_answer()"
        } else {
            "private_service.answer()"
        };
        f.write(
            "library.nagi",
            &format!("def helper() -> i64:\n    return {call}\n"),
        );
        f.write(
            "main.nagi",
            &format!("import \"library.nagi\"\n{private_import}def main():\n    print(helper())\n"),
        );
        let error = f.error("main.nagi");
        assert!(
            error.contains("library.nagi:2") && error.contains(call),
            "private root aliases must not enter the legacy shared definition scope: {error}"
        );
    }
}

#[test]
fn alias_imports_keep_cycle_relative_path_and_language_guards() {
    let f = Fixture::new();
    f.write("a.nagi", "import \"b.nagi\" as b\n");
    f.write("b.nagi", "from \"a.nagi\" import value\n");
    let cycle = f.load("a.nagi").err().unwrap();
    assert!(
        cycle.contains("循環import") && cycle.contains("b.nagi:1"),
        "{cycle}"
    );
    for text in [
        "import \"missing.nagi\" as missing\n".to_owned(),
        "from \"wrong.low\" import value\n".to_owned(),
        format!("import \"{}\" as absolute\n", f.0.join("a.nagi").display()),
    ] {
        f.write("a.nagi", &text);
        let error = f.load("a.nagi").err().unwrap();
        assert!(
            error.contains("a.nagi:1") && error.contains(text.trim()),
            "{error}"
        );
    }
}

#[test]
fn namespace_imports_preserve_depth_file_count_and_total_byte_limits() {
    let f = Fixture::new();
    for depth in 0..64 {
        f.write(
            &format!("depth/{depth}.nagi"),
            if depth == 63 {
                ""
            } else {
                "# replaced below\n"
            },
        );
        if depth < 63 {
            f.write(
                &format!("depth/{depth}.nagi"),
                &format!("import \"{}.nagi\" as next\n", depth + 1),
            );
        }
    }
    f.load("depth/0.nagi").unwrap();
    f.write("depth/63.nagi", "import \"64.nagi\" as next\n");
    f.write("depth/64.nagi", "");
    let error = f.load("depth/0.nagi").err().unwrap();
    assert!(
        error.contains("63.nagi:1") && error.contains("上限"),
        "{error}"
    );

    let mut imports = String::new();
    for number in 0..127 {
        f.write(&format!("files/{number}.nagi"), "");
        imports.push_str(&format!("import \"files/{number}.nagi\" as m{number}\n"));
    }
    f.write("main.nagi", &imports);
    f.load("main.nagi").unwrap();
    f.write("files/127.nagi", "");
    imports.push_str("import \"files/127.nagi\" as m127\n");
    f.write("main.nagi", &imports);
    let error = f.load("main.nagi").err().unwrap();
    assert!(
        error.contains("main.nagi:128") && error.contains("上限"),
        "{error}"
    );

    f.write("large.nagi", &format!("#{}\n", "x".repeat(8_000_000)));
    f.write("main.nagi", "import \"large.nagi\" as large\n");
    let error = f.load("main.nagi").err().unwrap();
    assert!(
        error.contains("main.nagi:1") && error.contains("8 MB"),
        "{error}"
    );
}

#[test]
fn flat_imported_main_remains_an_entry_but_namespace_imported_main_does_not() {
    let f = Fixture::new();
    f.write("entry.nagi", "def main():\n    print(42)\n");
    f.write("flat.nagi", "import \"entry.nagi\"\n");
    let flat = f.checked("flat.nagi");
    let binding = flat
        .program
        .modules
        .root_bindings()
        .find(|binding| binding.name == "main")
        .unwrap();
    let nagic::ast::BindingTarget::Definition(id) = &binding.target else {
        panic!("flat main must identify a function")
    };
    assert_eq!(
        id.module.0,
        fs::canonicalize(f.0.join("entry.nagi"))
            .unwrap()
            .display()
            .to_string()
    );
    assert_eq!(
        flat.program.modules.definition_id(id).unwrap().symbol,
        "main"
    );
    assert!(flat
        .program
        .functions
        .iter()
        .any(|function| function.name == "main"));
    successful(f.cli(&f.0, &["check", "flat.nagi"]));

    f.write("namespace.nagi", "import \"entry.nagi\" as entry\n");
    let namespace = f.checked("namespace.nagi");
    assert!(!namespace
        .program
        .modules
        .root_bindings()
        .any(|binding| binding.name == "main"));
    assert!(!namespace
        .program
        .functions
        .iter()
        .any(|function| function.name == "main"));
}

#[test]
fn saved_generated_low_can_be_imported_without_reidentifying_its_main() {
    let f = Fixture::new();
    f.write(
        "original.nagi",
        "def answer() -> i64:\n    return 42\ndef main():\n    print(answer())\n",
    );
    let original = f.checked("original.nagi");
    let former_ids = original
        .program
        .modules
        .definitions
        .iter()
        .map(|definition| definition.id.clone())
        .collect::<Vec<_>>();
    f.write("saved.low", &emit::low(&original.program));
    f.write("main.low", "import \"saved.low\" as previous;\nfn main() -> unit { print(previous.answer()); previous.main(); }\n");
    let loaded = f.checked("main.low");
    for id in former_ids {
        assert!(
            loaded.program.modules.definition_id(&id).is_some(),
            "saved Low must retain the original source definition ID: {id:?}"
        );
    }
    assert_eq!(loaded.program.functions.len(), 3);
    assert_eq!(
        loaded
            .program
            .functions
            .iter()
            .filter(|function| function.name == "main")
            .count(),
        1
    );
    successful(f.cli(&f.0, &["check", "main.low"]));
    f.write("roundtrip.low", &emit::low(&loaded.program));
    f.checked("roundtrip.low");
}

#[test]
fn saved_low_flat_and_from_imports_promote_the_original_definition_to_entry() {
    let f = Fixture::new();
    f.write(
        "original.nagi",
        "def answer() -> i64:\n    return 42\ndef main():\n    print(answer())\n",
    );
    let original = f.checked("original.nagi");
    f.write("saved.low", &emit::low(&original.program));
    for (source, definition_name) in [
        ("import \"saved.low\";\n", "main"),
        ("from \"saved.low\" import main;\n", "main"),
        ("from \"saved.low\" import answer as main;\n", "answer"),
    ] {
        f.write("entry.low", source);
        let loaded = f.checked("entry.low");
        let original_id = &original
            .program
            .modules
            .definitions
            .iter()
            .find(|definition| definition.id.name == definition_name)
            .unwrap()
            .id;
        assert!(
            loaded
                .program
                .functions
                .iter()
                .any(|function| function.name == "main"),
            "the imported entry must exist in the executable AST: {source}"
        );
        assert_eq!(
            &loaded.program.modules.definition("main").unwrap().id,
            original_id
        );
        f.write("roundtrip.low", &emit::low(&loaded.program));
        let roundtrip = f.checked("roundtrip.low");
        assert_eq!(
            &roundtrip.program.modules.definition("main").unwrap().id,
            original_id
        );
        let index: serde_json::Value =
            serde_json::from_str(&successful(f.cli(&f.0, &["symbols", "entry.low"]))).unwrap();
        let binding = index["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|binding| {
                binding["name"] == "main"
                    && binding["file"].as_str().unwrap().ends_with("entry.low")
            })
            .expect("editor metadata must expose the entry binding");
        assert_eq!(
            binding["definition_id"],
            serde_json::to_value(original_id).unwrap()
        );
        assert!(
            binding["target"]["file"]
                .as_str()
                .unwrap()
                .ends_with("saved.low"),
            "{binding}"
        );
    }
}

#[test]
fn edited_saved_low_body_resolves_original_aliases_and_maps_only_its_physical_root() {
    let f = Fixture::new();
    f.write(
        "orders.nagi",
        "class Order:\n    number: i64\ndef score(value: Order) -> i64:\n    return value.number\n",
    );
    f.write("main.nagi", "import \"orders.nagi\" as orders\nfrom \"orders.nagi\" import Order as SavedOrder\ndef main():\n    print(orders.score(SavedOrder(number=1)))\n");
    let original = f.checked("main.nagi");
    let generated = emit::low(&original.program);
    let mut edited = String::new();
    let mut inserted = false;
    for line in generated.lines() {
        edited.push_str(line);
        edited.push('\n');
        if line.starts_with("fn main(") {
            edited.push_str("    let saved: SavedOrder = orders.Order(number=42);\n    print(orders.score(saved));\n");
            inserted = true;
        }
    }
    assert!(
        inserted,
        "fixture must edit the existing executable entry body"
    );
    f.write("saved.low", &edited);
    let loaded = f.checked("saved.low");
    assert_eq!(loaded.program.classes.len(), original.program.classes.len());
    assert_eq!(
        loaded.program.functions.len(),
        original.program.functions.len()
    );
    for definition in &original.program.modules.definitions {
        assert!(loaded
            .program
            .modules
            .definition_id(&definition.id)
            .is_some());
    }
    let physical = loaded.module_files().collect::<Vec<_>>();
    assert_eq!(
        physical.len(),
        1,
        "flattened foreign identities are not physical source files"
    );
    assert_eq!(Some(physical[0].0), original.program.modules.root.as_ref());
    assert_eq!(
        physical[0].1,
        fs::canonicalize(f.0.join("saved.low")).unwrap()
    );
    successful(f.cli(&f.0, &["check", "saved.low"]));
}

#[test]
fn separate_native_files_can_reuse_a_replacement_declaration_name() {
    let f = Fixture::new();
    f.write("left.nagi", "def score() -> i64:\n    return 1\n");
    f.write("right.nagi", "def score() -> i64:\n    return 2\n");
    f.write("main.nagi", "import \"left.nagi\" as left\nimport \"right.nagi\" as right\ndef main():\n    print(left.score() + right.score())\n");
    f.write(
        "first.low",
        "@replace generated::left::score\nfn replacement() -> i64 { return 10; }\n",
    );
    f.write(
        "second.low",
        "@replace generated::right::score\nfn replacement() -> i64 { return 20; }\n",
    );
    successful(f.cli(
        &f.0,
        &[
            "check",
            "main.nagi",
            "--native",
            "first.low",
            "--native",
            "second.low",
        ],
    ));
    let mut primary = f.checked("main.nagi").program;
    let mut native = nagic::ast::Program::default();
    for name in ["first.low", "second.low"] {
        let mut fragment = f.load(name).unwrap().program;
        nagic::modules::prepare_native_fragment(&mut fragment, &primary);
        native.modules.merge(fragment.modules).unwrap();
        native.functions.extend(fragment.functions);
    }
    check::integrate(&mut primary, native).unwrap();
    for (path, expected) in [("left.score", "10"), ("right.score", "20")] {
        let definition = primary.modules.resolve_root_path(path).unwrap();
        let function = primary
            .functions
            .iter()
            .find(|function| function.name == definition.symbol)
            .unwrap();
        let nagic::ast::S::Return(Some(value)) = &function.body[0].kind else {
            panic!("replacement must retain its returned value")
        };
        let nagic::ast::E::Int(number) = &value.kind else {
            panic!("replacement must retain its literal result")
        };
        assert_eq!(
            number, expected,
            "both native replacement bodies must reach their distinct generated targets"
        );
    }
    f.write("integrated.low", &emit::low(&primary));
    f.checked("integrated.low");
}

#[test]
fn aliases_of_one_function_cannot_be_replaced_twice_across_native_files() {
    let f = Fixture::new();
    f.write("score.nagi", "def score() -> i64:\n    return 1\n");
    f.write("main.nagi", "import \"score.nagi\" as first\nimport \"score.nagi\" as second\ndef main():\n    print(first.score() + second.score())\n");
    f.write(
        "first.low",
        "@replace generated::first::score\nfn replacement() -> i64 { return 10; }\n",
    );
    f.write(
        "second.low",
        "@replace generated::second::score\nfn replacement() -> i64 { return 20; }\n",
    );
    let output = f.cli(
        &f.0,
        &[
            "check",
            "main.nagi",
            "--native",
            "first.low",
            "--native",
            "second.low",
        ],
    );
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("second.low:2") && error.contains("複数回replace"),
        "{error}"
    );
}

#[test]
fn native_main_replacement_keeps_the_generated_identity_and_recursive_target() {
    let f = Fixture::new();
    f.write("main.nagi", "def main() -> i64:\n    return 0\n");
    f.write(
        "replace.low",
        "@replace generated::main\nfn main() -> i64 { return main(); }\n",
    );
    successful(f.cli(&f.0, &["check", "main.nagi", "--native", "replace.low"]));
    let mut primary = f.checked("main.nagi").program;
    let original_id = primary.modules.definition("main").unwrap().id.clone();
    let native = f.load("replace.low").unwrap().program;
    check::integrate(&mut primary, native).unwrap();
    assert_eq!(primary.modules.definition("main").unwrap().id, original_id);
    assert_eq!(primary.functions.len(), 1);
    let function = &primary.functions[0];
    let nagic::ast::S::Return(Some(value)) = &function.body[0].kind else {
        panic!("fixture must retain its recursive return")
    };
    let nagic::ast::E::Call(target, _, _) = &value.kind else {
        panic!("fixture must retain its recursive call")
    };
    assert_eq!(
        target, &function.name,
        "recursive replacement calls must resolve to the shared generated definition"
    );
    assert_eq!(value.resolution, Some(nagic::ast::NameResolution::Function));
}

#[test]
fn imported_names_cannot_silently_fall_back_to_builtins_of_a_different_kind() {
    let f = Fixture::new();
    f.write(
        "library.nagi",
        "class Record:\n    value: i64\ndef make(value: i64) -> i64:\n    return value\n",
    );
    for (source, line) in [
        (
            "import \"library.nagi\" as print\ndef main():\n    print(1)\n",
            3,
        ),
        (
            "import \"library.nagi\" as print\ndef main():\n    value = print\n",
            3,
        ),
        (
            "import \"library.nagi\" as Db\ndef take(value: Db):\n    print(0)\n",
            2,
        ),
        (
            "import \"library.nagi\" as List\ndef take(value: List[i64]):\n    print(0)\n",
            2,
        ),
        (
            "from \"library.nagi\" import Record as print\ndef main():\n    print(1)\n",
            3,
        ),
        (
            "from \"library.nagi\" import make as Db\ndef take(value: Db):\n    print(0)\n",
            2,
        ),
    ] {
        f.write("main.nagi", source);
        let error = f.error("main.nagi");
        assert!(
            error.contains(&format!("main.nagi:{line}")),
            "{source}\n{error}"
        );
        assert!(
            error.contains(source.lines().nth(line - 1).unwrap()),
            "{error}"
        );
    }
    // A legitimate class alias named Db remains a class instead of selecting
    // the database builtin; the differing field makes the resolution visible.
    f.write("main.nagi", "from \"library.nagi\" import Record as Db\ndef take(value: Db) -> i64:\n    return value.value\ndef main():\n    print(take(Db(value=42)))\n");
    f.checked("main.nagi");
}

#[test]
fn legacy_classes_constructors_and_intrinsic_types_keep_their_original_namespaces() {
    let f = Fixture::new();
    f.write("main.nagi", "class some:\n    value: i64\nclass Foo:\n    value: i64\ndef Db() -> i64:\n    return 7\ndef inspect(db: Db) -> i64:\n    return Db()\ndef main():\n    option: i64? = some(1)\n    item = some(value=2)\n    Foo = 1\n    record = Foo(value=2)\n    print(item.value + record.value + Foo + Db())\n");
    let loaded = f.checked("main.nagi");
    let definition = loaded
        .program
        .modules
        .definitions
        .iter()
        .find(|definition| definition.id.name == "inspect")
        .unwrap();
    let function = loaded
        .program
        .functions
        .iter()
        .find(|function| function.name == definition.symbol)
        .unwrap();
    assert_eq!(
        function.params[0].1 .0, "Db",
        "the parameter must retain the intrinsic database type despite the function named Db"
    );
    f.write("legacy.low", &emit::low(&loaded.program));
    f.checked("legacy.low");
    successful(f.cli(&f.0, &["check", "main.nagi"]));

    f.write(
        "library.nagi",
        "class Record:\n    value: i64\ndef make(value: i64) -> i64:\n    return value\n",
    );
    for source in [
        "from \"library.nagi\" import Record as print\ndef main():\n    print(1)\n",
        "import \"library.nagi\" as library\ndef main():\n    library.Record(1)\n",
        "from \"library.nagi\" import make as Db\ndef inspect(db: Db):\n    print(0)\n",
    ] {
        f.write("invalid.nagi", source);
        let error = f.error("invalid.nagi");
        assert!(error.contains("invalid.nagi:"), "{error}");
    }
}

#[test]
fn renamed_local_main_diagnostics_keep_source_spelling_after_program_is_taken() {
    let f = Fixture::new();
    f.write("main.nagi", "def main():\n    main = 1\n    main()\n");
    let mut loaded = f.load("main.nagi").unwrap();
    let mut program = std::mem::take(&mut loaded.program);
    let error = check::check(&mut program).unwrap_err();
    let diagnostic = loaded.diagnostic(&error);
    assert!(
        diagnostic.lines().next().unwrap().contains("main"),
        "the diagnostic message must identify the source variable: {diagnostic}"
    );
    assert!(!diagnostic.contains("__nagi_local_"), "{diagnostic}");
    assert!(
        diagnostic.contains("main.nagi:3") && diagnostic.contains("3 |     main()"),
        "{diagnostic}"
    );
}

#[test]
fn the_checker_rejects_parser_only_alias_imports_until_the_source_loader_resolves_them() {
    for (source, high) in [
        (
            "import \"library.nagi\" as library\ndef main():\n    print(0)\n",
            true,
        ),
        (
            "from \"library.nagi\" import make as helper\ndef main():\n    print(0)\n",
            true,
        ),
        (
            "import \"library.low\" as library;\nfn main() -> unit { print(0); }\n",
            false,
        ),
        (
            "from \"library.low\" import make as helper;\nfn main() -> unit { print(0); }\n",
            false,
        ),
    ] {
        let mut program = nagic::parser::parse(source, high).unwrap();
        let error = check::check(&mut program)
            .expect_err("unresolved aliases must not disappear merely because they are unused");
        assert!(error.contains("import"), "{source}\n{error}");
    }
}

#[test]
fn corrupted_low_identity_metadata_cannot_rename_or_invent_definitions() {
    let f = Fixture::new();
    f.write("original.nagi", "def answer() -> i64:\n    return 42\n");
    let generated = emit::low(&f.checked("original.nagi").program);
    let (header_line, header) = generated
        .lines()
        .enumerate()
        .find_map(|(line, text)| {
            text.strip_prefix("# nagi-modules-v1 ")
                .map(|header| (line, header))
        })
        .expect("generated Low must retain definition identity metadata");
    let metadata: serde_json::Value = serde_json::from_str(header).unwrap();
    let mut bad_symbol = metadata.clone();
    bad_symbol["definitions"][0]["symbol"] = "invented".into();
    let mut bad_path = metadata.clone();
    bad_path["modules"][0]["path"] = "../other.nagi".into();
    let mut bad_root = metadata.clone();
    bad_root["root"] = f.0.join("missing.nagi").display().to_string().into();
    let mut unknown_field = metadata;
    unknown_field["invented_field"] = true.into();
    for (metadata, rejected_part) in [
        (bad_symbol, "symbol"),
        (bad_path, "ID/path"),
        (bad_root, "root"),
        (unknown_field, "invented_field"),
    ] {
        let mutated = generated
            .lines()
            .enumerate()
            .map(|(line, text)| {
                if line == header_line {
                    format!("# nagi-modules-v1 {metadata}")
                } else {
                    text.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        f.write("broken.low", &mutated);
        let error = f
            .load("broken.low")
            .err()
            .expect("corrupt identity metadata must be rejected by the loader");
        assert!(error.contains(rejected_part), "{error}");
    }
}

#[test]
fn native_import_aliases_remain_local_while_own_functions_are_available_to_high() {
    let f = Fixture::new();
    f.write(
        "library.low",
        "record Order { number: i64; }\nfn secret() -> i64 { return 42; }\n",
    );
    f.write("native.low", "from \"library.low\" import secret as helper;\nfrom \"library.low\" import Order as PrivateOrder;\nfn visible() -> i64 { let value = PrivateOrder(number=helper()); return value.number; }\n");
    f.write("main.nagi", "def main():\n    print(visible())\n");
    successful(f.cli(&f.0, &["check", "main.nagi", "--native", "native.low"]));
    for source in [
        "def main():\n    print(helper())\n",
        "def main():\n    value = PrivateOrder(number=42)\n    print(value.number)\n",
    ] {
        f.write("main.nagi", source);
        let output = f.cli(&f.0, &["check", "main.nagi", "--native", "native.low"]);
        assert!(
            !output.status.success(),
            "native from-import aliases must remain private: {source}"
        );
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("main.nagi:2"), "{error}");
        assert!(error.contains(source.lines().nth(1).unwrap()), "{error}");
    }
}

#[test]
fn high_and_native_modules_can_use_the_same_private_namespace_spelling() {
    let f = Fixture::new();
    f.write(
        "orders.nagi",
        "class Order:\n    high: i64\ndef score(value: Order) -> i64:\n    return value.high\n",
    );
    f.write(
        "orders.low",
        "record Order { low: i64; }\nfn score(value: Order) -> i64 { return value.low; }\n",
    );
    f.write("native.low", "import \"orders.low\" as orders;\nfn native_score() -> i64 { let value = orders.Order(low=2); return orders.score(value); }\n");
    f.write("main.nagi", "import \"orders.nagi\" as orders\ndef main():\n    value = orders.Order(high=40)\n    print(orders.score(value) + native_score())\n");
    successful(f.cli(&f.0, &["check", "main.nagi", "--native", "native.low"]));
}

#[test]
fn foreign_functions_keep_their_builtin_scope_when_root_aliases_use_builtin_names() {
    let f = Fixture::new();
    f.write("library.nagi", "def hello():\n    print(7)\n");
    f.write("main.nagi", "import \"library.nagi\" as print\nimport \"library.nagi\" as library\ndef main():\n    library.hello()\n");
    let loaded = f.checked("main.nagi");
    let hello_id = loaded
        .program
        .modules
        .resolve_root_path("library.hello")
        .unwrap()
        .id
        .clone();
    let verify = |program: &nagic::ast::Program| {
        let definition = program.modules.definition_id(&hello_id).unwrap();
        let hello = program
            .functions
            .iter()
            .find(|function| function.name == definition.symbol)
            .unwrap();
        let nagic::ast::S::Expr(call) = &hello.body[0].kind else {
            panic!("foreign hello must retain its call")
        };
        let nagic::ast::E::Call(name, _, _) = &call.kind else {
            panic!("foreign hello must retain its call")
        };
        assert_eq!(name, "print");
        assert_eq!(call.resolution, Some(nagic::ast::NameResolution::Builtin));
    };
    verify(&loaded.program);
    let generated = emit::low(&loaded.program);
    let mut parsed = nagic::parser::parse(&generated, false).unwrap();
    check::check(&mut parsed).unwrap();
    verify(&parsed);
    f.write("saved.low", &generated);
    verify(&f.checked("saved.low").program);
    successful(f.cli(&f.0, &["check", "main.nagi"]));
    successful(f.cli(&f.0, &["check", "saved.low"]));
}

#[test]
fn every_explicit_native_root_gets_high_aliases_without_overwriting_own_or_dependency_scopes() {
    let f = Fixture::new();
    f.write("orders.nagi", "class Order:\n    high: i64\n");
    f.write("library.nagi", "def unused():\n    print(0)\n");
    f.write("orders.low", "record Order { low: i64; }\n");
    f.write("dependency.low", "fn hello() -> unit { print(7); }\n");
    f.write("first.low", "import \"dependency.low\" as dependency;\nfn first(value: orders.Order) -> i64 { dependency.hello(); return value.high; }\n");
    f.write(
        "second.low",
        "fn second(value: orders.Order) -> i64 { return value.high + 1; }\n",
    );
    f.write("own.low", "import \"orders.low\" as orders;\nfn own() -> i64 { let value = orders.Order(low=2); return value.low; }\n");
    f.write("main.nagi", "import \"orders.nagi\" as orders\nimport \"library.nagi\" as print\ndef main():\n    value = orders.Order(high=40)\n    assert_true(first(value) + second(value) + own() == 83)\n");
    successful(f.cli(
        &f.0,
        &[
            "check",
            "main.nagi",
            "--native",
            "first.low",
            "--native",
            "second.low",
            "--native",
            "own.low",
        ],
    ));
    let mut primary = f.load("main.nagi").unwrap().program;
    let high_order = primary
        .modules
        .resolve_root_path("orders.Order")
        .unwrap()
        .clone();
    let mut native = nagic::ast::Program::default();
    for name in ["first.low", "second.low", "own.low"] {
        let mut fragment = f.load(name).unwrap().program;
        nagic::modules::prepare_native_fragment(&mut fragment, &primary);
        native.modules.merge_native(fragment.modules).unwrap();
        native.classes.extend(fragment.classes);
        native.functions.extend(fragment.functions);
    }
    check::integrate(&mut primary, native).unwrap();
    for name in ["first", "second"] {
        let definition = primary.modules.resolve_root_path(name).unwrap();
        let function = primary
            .functions
            .iter()
            .find(|function| function.name == definition.symbol)
            .unwrap();
        assert_eq!(
            function.params[0].1,
            nagic::ast::Type::named(&high_order.symbol),
            "native root {name} must use the same High class identity"
        );
    }
    let own_order = primary
        .modules
        .definitions
        .iter()
        .find(|definition| definition.id.name == "Order" && definition.id != high_order.id)
        .unwrap();
    let own_definition = primary.modules.resolve_root_path("own").unwrap();
    let own = primary
        .functions
        .iter()
        .find(|function| function.name == own_definition.symbol)
        .unwrap();
    assert_eq!(
        own.body[0].binding_type,
        Some(nagic::ast::Type::named(&own_order.symbol)),
        "the native file's own orders namespace must override its High fallback"
    );
    let hello_definition = primary
        .modules
        .definitions
        .iter()
        .find(|definition| definition.id.name == "hello")
        .unwrap();
    let hello = primary
        .functions
        .iter()
        .find(|function| function.name == hello_definition.symbol)
        .unwrap();
    let nagic::ast::S::Expr(call) = &hello.body[0].kind else {
        panic!("dependency hello must retain print")
    };
    assert_eq!(
        call.resolution,
        Some(nagic::ast::NameResolution::Builtin),
        "the native dependency must keep its own builtin scope"
    );
    f.write("integrated.low", &emit::low(&primary));
    f.checked("integrated.low");

    f.write("dependency.low", "fn hello() -> unit { print(7); }\nfn hidden(value: orders.Order) -> i64 { return value.high; }\n");
    let output = f.cli(
        &f.0,
        &[
            "check",
            "main.nagi",
            "--native",
            "first.low",
            "--native",
            "second.low",
            "--native",
            "own.low",
        ],
    );
    assert!(!output.status.success(), "a native dependency must not inherit the explicit native root's private High module aliases");
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("dependency.low:2") && error.contains("orders.Order"),
        "{error}"
    );
}

#[test]
fn omitted_unit_returns_keep_builtin_identity_while_explicit_namespace_types_are_rejected() {
    let f = Fixture::new();
    f.write("library.nagi", "def score() -> i64:\n    return 3\n");
    for imports in [
        "import \"library.nagi\" as unit\n",
        "class unit:\n    value: i64\n",
    ] {
        f.write(
            "main.nagi",
            &format!("{imports}def main():\n    print(1)\n"),
        );
        let loaded = f.checked("main.nagi");
        let main = loaded
            .program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        assert_eq!(main.ret, nagic::ast::Type::named("unit"));
        f.write("saved.low", &emit::low(&loaded.program));
        let saved = f.checked("saved.low");
        assert_eq!(
            saved
                .program
                .functions
                .iter()
                .find(|function| function.name == "main")
                .unwrap()
                .ret,
            nagic::ast::Type::named("unit")
        );
    }
    f.write(
        "main.nagi",
        "import \"library.nagi\" as unit\ndef main() -> unit:\n    print(1)\n",
    );
    let error = f.error("main.nagi");
    assert!(
        error.contains("main.nagi:2") && error.contains("unit"),
        "{error}"
    );
}

#[test]
fn qualified_replacements_compare_definition_identity_and_report_low_sources() {
    let f = Fixture::new();
    f.write(
        "orders.nagi",
        "class Order:\n    number: i64\ndef score(value: Order) -> i64:\n    return value.number\n",
    );
    f.write("main.nagi", "import \"orders.nagi\" as orders\nfrom \"orders.nagi\" import Order as SavedOrder\ndef main():\n    print(orders.score(SavedOrder(number=41)))\n");
    f.write("replace.low", "@replace generated::orders::score\nfn replacement(value: SavedOrder) -> i64 { return value.number + 1; }\n");
    successful(f.cli(&f.0, &["check", "main.nagi", "--native", "replace.low"]));

    f.write("wrong.low", "record Order { number: i64; }\n");
    f.write("replace.low", "import \"wrong.low\" as wrong;\n@replace generated::orders::score\nfn replacement(value: wrong.Order) -> i64 { return value.number + 1; }\n");
    let output = f.cli(&f.0, &["check", "main.nagi", "--native", "replace.low"]);
    assert!(
        !output.status.success(),
        "a different file's Order cannot replace the orders.Order parameter"
    );
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("replace.low:3") && error.contains("一致しません"),
        "{error}"
    );
    assert!(
        error.contains("fn replacement(value: wrong.Order)"),
        "{error}"
    );

    f.write("replace.low", "@replace generated::orders::missing\nfn replacement(value: SavedOrder) -> i64 { return value.number; }\n");
    let output = f.cli(&f.0, &["check", "main.nagi", "--native", "replace.low"]);
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("replace.low:2") && error.contains("replace対象"),
        "{error}"
    );
}

#[test]
fn project_aliases_cross_low_replacement_and_rust_record_bridges_from_an_external_cwd() {
    let f = Fixture::new();
    f.write(
        "project/nagi.toml",
        "entry='src/main.nagi'\nnative=['native/replace.low']\n[rust]\nfile='native/adapter.rs'\n",
    );
    f.write("project/src/orders.nagi", "class Order:\n    type: i64\n    __nagi_ident_0: i64\ndef score(value: Order) -> i64:\n    return value.type\n");
    f.write("project/src/main.nagi", "import \"orders.nagi\" as orders\nfrom \"orders.nagi\" import Order as SavedOrder\n@rust(\"native::verify\")\nextern def verify(value: SavedOrder)\ndef main():\n    value = SavedOrder(type=41, __nagi_ident_0=7)\n    assert_true(orders.score(value) == 42)\n    verify(orders.Order(type=41, __nagi_ident_0=7))\n    print(\"orders.Order SavedOrder generated::orders::score unchanged\")\n");
    f.write("project/native/replace.low", "@replace generated::orders::score\nfn replacement(value: orders.Order) -> i64 { return value.type + 1; }\n");
    f.write("project/native/adapter.rs", r#"pub fn verify(value: super::orders::Order) {
    let same_type: super::SavedOrder = value;
    let json = nagi_runtime::serde_json::to_value(&same_type).unwrap();
    assert_eq!(json, nagi_runtime::serde_json::json!({"type":41,"__nagi_ident_0":7}));
    let decoded: super::orders::Order = nagi_runtime::serde_json::from_value(json.clone()).unwrap();
    assert_eq!(nagi_runtime::serde_json::to_value(decoded).unwrap(), json);
    assert_eq!(<super::SavedOrder as nagi_runtime::FromRow>::columns(), &["type", "__nagi_ident_0"]);
    let db = nagi_runtime::rusqlite::Connection::open_in_memory().unwrap();
    let from_db = db.query_row("SELECT 41 AS 'type', 7 AS '__nagi_ident_0'", [], |row| <super::orders::Order as nagi_runtime::FromRow>::read(row, &[0, 1])).unwrap();
    assert_eq!(nagi_runtime::serde_json::to_value(from_db).unwrap(), json);
    println!("verified module identity and original names");
}
"#);
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("native-target")
        });
    let output = Command::new(env!("CARGO_BIN_EXE_nagic"))
        .current_dir(&f.0)
        .args(["run", "--project", "project"])
        .env("NAGI_NATIVE_TARGET_DIR", target)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap();
    let stdout = successful(output);
    assert!(
        stdout
            .lines()
            .any(|line| line == "verified module identity and original names"),
        "{stdout}"
    );
    assert!(
        stdout
            .lines()
            .any(|line| line == "orders.Order SavedOrder generated::orders::score unchanged"),
        "{stdout}"
    );
}
