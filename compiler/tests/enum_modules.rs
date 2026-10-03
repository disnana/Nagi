use nagic::{ast::*, check, emit, modules, source, symbols};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nagi-enum-modules-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.0.join(name), text).unwrap();
    }
    fn load(&self, name: &str) -> Result<source::Sources, String> {
        source::load(&self.0.join(name), name.ends_with(".nagi"))
    }
    fn checked(&self, name: &str) -> source::Sources {
        let mut sources = self.load(name).unwrap();
        check::check(&mut sources.program)
            .unwrap_or_else(|error| panic!("{}", sources.diagnostic(&error)));
        sources
    }
    fn error(&self, name: &str) -> String {
        match self.load(name) {
            Err(error) => error,
            Ok(mut sources) => {
                let error =
                    check::check(&mut sources.program).expect_err("invalid program passed check");
                sources.diagnostic(&error)
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const ERRORS: &str = "enum Failure:\n    Missing\n    Invalid(message: str)\n";

#[test]
fn namespace_from_and_flat_imports_share_enum_identity_in_constructors_and_patterns() {
    let f = Fixture::new();
    f.write("errors.nagi", ERRORS);
    f.write("main.nagi", "import \"errors.nagi\" as errors\nimport \"errors.nagi\" as again\nfrom \"errors.nagi\" import Failure as AppError\nimport \"errors.nagi\"\ndef identity(value: errors.Failure) -> AppError:\n    return value\ndef describe(value: AppError) -> i64:\n    match value:\n        case errors.Failure.Missing:\n            return 0\n        case AppError.Invalid(message):\n            return len(view(message))\ndef main():\n    print(describe(identity(again.Failure.Invalid(\"bad\"))))\n    value: Failure = AppError.Missing\n    print(describe(value))\n");
    let checked = f.checked("main.nagi");
    assert_eq!(checked.program.enums.len(), 1);
    let enum_id = checked
        .program
        .modules
        .resolve_root_path("errors.Failure")
        .unwrap();
    assert_eq!(enum_id.id.kind, DefKind::Enum);
    assert_eq!(
        checked
            .program
            .modules
            .resolve_root_path("AppError")
            .unwrap()
            .id,
        enum_id.id
    );
    assert_eq!(
        checked
            .program
            .modules
            .resolve_root_path("Failure")
            .unwrap()
            .id,
        enum_id.id
    );
    assert!(checked
        .program
        .modules
        .references
        .iter()
        .any(|r| r.spelling == "errors.Failure.Missing" && r.target == enum_id.id));
    modules::validate(&checked.program).unwrap();
    f.write("saved.low", &emit::low(&checked.program));
    let saved = f.checked("saved.low");
    modules::validate(&saved.program).unwrap();
    assert_eq!(saved.program.enums[0].name, checked.program.enums[0].name);
    assert_eq!(
        saved
            .program
            .modules
            .resolve_root_path("AppError")
            .unwrap()
            .id,
        enum_id.id
    );
}

#[test]
fn different_files_have_distinct_enum_types_even_with_equal_names_and_variants() {
    let f = Fixture::new();
    f.write("left.nagi", ERRORS);
    f.write("right.nagi", ERRORS);
    for body in [
        "def take(value: left.Failure):\n    print(1)\ndef main():\n    take(right.Failure.Missing)\n",
        "def make() -> left.Failure:\n    return right.Failure.Invalid(\"wrong\")\n",
        "def describe(value: left.Failure) -> i64:\n    match value:\n        case right.Failure.Missing:\n            return 0\n        case left.Failure.Invalid(_):\n            return 1\n",
    ] {
        f.write("main.nagi", &format!("import \"left.nagi\" as left\nimport \"right.nagi\" as right\n{body}"));
        assert!(f.error("main.nagi").contains("Failure"));
    }
}

#[test]
fn enum_aliases_and_guessed_canonical_symbols_do_not_escape_module_scopes() {
    let f = Fixture::new();
    f.write("errors.nagi", ERRORS);
    f.write("facade.nagi", "from \"errors.nagi\" import Failure as Private\ndef available() -> Private:\n    return Private.Missing\n");
    let hidden = modules::symbol(&DefId {
        module: ModuleId(
            fs::canonicalize(f.0.join("errors.nagi"))
                .unwrap()
                .display()
                .to_string(),
        ),
        kind: DefKind::Enum,
        name: "Failure".into(),
    });
    for body in [
        "def main():\n    item = facade.Private.Missing\n".to_owned(),
        "def main():\n    item = Private.Missing\n".to_owned(),
        format!("def main():\n    item = {hidden}.Missing\n"),
        format!("def main():\n    item = {hidden}.Invalid(\"hidden\")\n"),
        "def describe(value: facade.Private):\n    print(1)\n".to_string(),
    ] {
        f.write(
            "main.nagi",
            &format!("import \"facade.nagi\" as facade\n{body}"),
        );
        let error = f.error("main.nagi");
        assert!(error.contains("main.nagi"), "{error}");
    }
    f.write(
        "main.nagi",
        "import \"facade.nagi\" as facade\ndef main():\n    item = facade.available()\n",
    );
    f.checked("main.nagi");
}

#[test]
fn enum_payload_types_use_the_definition_file_scope_and_survive_low() {
    let f = Fixture::new();
    f.write("record.nagi", "class Message:\n    count: i64\n");
    f.write("errors.nagi", "from \"record.nagi\" import Message as Data\nenum Failure:\n    Empty\n    WithData(value: Data)\n");
    f.write("main.nagi", "import \"errors.nagi\" as errors\nfrom \"record.nagi\" import Message\ndef describe(value: errors.Failure) -> i64:\n    match value:\n        case errors.Failure.Empty:\n            return 0\n        case errors.Failure.WithData(data):\n            return data.count\ndef main():\n    print(describe(errors.Failure.WithData(value=Message(count=7))))\n");
    let checked = f.checked("main.nagi");
    let record = checked
        .program
        .modules
        .resolve_root_path("Message")
        .unwrap();
    assert_eq!(
        checked.program.enums[0].variants[1].fields[0].1,
        Type::named(&record.symbol)
    );
    f.write("saved.low", &emit::low(&checked.program));
    let saved = f.checked("saved.low");
    assert_eq!(
        saved.program.enums[0].variants[1].fields[0].1,
        Type::named(&record.symbol)
    );
}

fn reference(index: &serde_json::Value, line: u64, column: u64) -> &serde_json::Value {
    index["references"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["location"]["file"]
                .as_str()
                .unwrap()
                .ends_with("main.nagi")
                && r["location"]["line"] == line
                && r["location"]["column"] == column
        })
        .unwrap()
}

#[test]
fn enum_symbols_keep_variant_targets_alias_signatures_and_payload_binding_types() {
    let f = Fixture::new();
    f.write("errors.nagi", ERRORS);
    f.write("main.nagi", "import \"errors.nagi\" as errors\nfrom \"errors.nagi\" import Failure as AppError\ndef describe(value: AppError) -> i64:\n    match value:\n        case errors.Failure.Missing:\n            return 0\n        case AppError.Invalid(message):\n            return len(view(message))\ndef main():\n    print(describe(errors.Failure.Invalid(\"bad\")))\n");
    let sources = f.checked("main.nagi");
    let index = symbols::index(&sources, &[&sources.program]).unwrap();
    let bindings = index["bindings"].as_array().unwrap();
    let alias = bindings
        .iter()
        .find(|b| b["name"] == "AppError" && b["file"].as_str().unwrap().ends_with("main.nagi"))
        .unwrap();
    assert_eq!(alias["definition"]["kind"], "enum");
    assert_eq!(
        alias["definition"]["variants"][1]["signature"],
        "AppError.Invalid(message: str)"
    );
    assert_eq!(
        alias["definition"]["variants"][1]["return_type"],
        "AppError"
    );
    assert_eq!(reference(&index, 5, 29)["target"]["line"], 2);
    assert_eq!(reference(&index, 5, 21)["target"]["line"], 1);
    assert_eq!(reference(&index, 7, 23)["target"]["line"], 3);
    assert_eq!(reference(&index, 8, 29)["target"]["line"], 7);
    assert!(index["locals"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["name"] == "message" && l["type"] == "str" && l["location"]["line"] == 7));
    assert!(!index["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["name"] == "Invalid"));
    let mut overlays = HashMap::new();
    overlays.insert(
        fs::canonicalize(f.0.join("errors.nagi")).unwrap(),
        "enum Failure:\n    Missing\n    Invalid(message: str, code: i64)\n".into(),
    );
    let modified = source::load_with_overlays(&f.0.join("main.nagi"), true, &overlays).unwrap();
    let changed = symbols::index(&modified, &[&modified.program]).unwrap();
    let definition = changed["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "Failure")
        .unwrap();
    assert_eq!(definition["variants"][1]["parameters"][1]["type"], "i64");
    assert_eq!(fs::read_to_string(f.0.join("errors.nagi")).unwrap(), ERRORS);
}

#[test]
fn saved_low_enum_variants_navigate_to_the_opened_file() {
    let f = Fixture::new();
    f.write("errors.nagi", ERRORS);
    f.write("main.nagi", "import \"errors.nagi\" as errors\nfrom \"errors.nagi\" import Failure as AppError\ndef main():\n    item = AppError.Invalid(\"bad\")\n");
    let checked = f.checked("main.nagi");
    f.write("saved.low", &emit::low(&checked.program));
    let saved = f.checked("saved.low");
    let index = symbols::index(&saved, &[&saved.program]).unwrap();
    let declaration = index["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == "Failure")
        .unwrap();
    assert!(declaration["variants"][1]["location"]["file"]
        .as_str()
        .unwrap()
        .ends_with("saved.low"));
    let target = &declaration["variants"][1]["location"];
    assert!(index["references"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["location"]["file"]
            .as_str()
            .unwrap()
            .ends_with("saved.low")
            && r["location"]["line"] != target["line"]
            && &r["target"] == target));
}

#[test]
fn native_fragments_reuse_high_enum_aliases_and_export_their_own_enum_identity() {
    let f = Fixture::new();
    f.write("errors.nagi", ERRORS);
    f.write("main.nagi", "import \"errors.nagi\" as errors\nfrom \"errors.nagi\" import Failure as AppError\ndef main():\n    item = native(errors.Failure.Missing)\n    own = native_own()\n");
    f.write("native.low", "enum NativeError { Broken; Detail(value: i64); }\nfn native(value: AppError) -> errors.Failure { return value; }\nfn native_own() -> NativeError { return NativeError.Detail(7); }\n");
    let mut primary = f.load("main.nagi").unwrap().program;
    let mut native = f.load("native.low").unwrap().program;
    modules::prepare_native_fragment(&mut native, &primary);
    check::integrate(&mut primary, native).unwrap();
    assert_eq!(primary.enums.len(), 2);
    let high = primary.modules.resolve_root_path("errors.Failure").unwrap();
    let native_function = primary.modules.resolve_root_path("native").unwrap();
    let function = primary
        .functions
        .iter()
        .find(|f| f.name == native_function.symbol)
        .unwrap();
    assert_eq!(function.params[0].1, Type::named(&high.symbol));
    assert_eq!(function.ret, Type::named(&high.symbol));
    let own = primary.modules.resolve_root_path("NativeError").unwrap();
    assert_eq!(own.id.kind, DefKind::Enum);
    assert_ne!(own.id, high.id);
    modules::validate(&primary).unwrap();
    f.write("integrated.low", &emit::low(&primary));
    f.checked("integrated.low");
}

#[test]
fn local_values_shadow_enum_value_syntax_but_named_constructor_uses_type_scope() {
    let f = Fixture::new();
    f.write("errors.nagi", ERRORS);
    f.write("main.nagi", "from \"errors.nagi\" import Failure as AppError\nclass Holder:\n    Missing: i64\ndef read(AppError: Holder) -> i64:\n    return AppError.Missing\ndef make() -> AppError:\n    AppError = 7\n    return AppError.Invalid(message=\"named\")\ndef main():\n    print(read(Holder(Missing=7)))\n    item = make()\n");
    f.checked("main.nagi");
    for expression in ["AppError.Missing", "AppError.Invalid(\"positional\")"] {
        f.write("main.nagi", &format!("from \"errors.nagi\" import Failure as AppError\ndef main():\n    AppError = 7\n    item = {expression}\n"));
        assert!(f.error("main.nagi").contains("main.nagi:4"));
    }
}

#[test]
fn builtin_spelled_enum_aliases_keep_intrinsic_types_distinct_in_generated_low() {
    let f = Fixture::new();
    f.write("errors.nagi", ERRORS);
    f.write("main.nagi", "from \"errors.nagi\" import Failure as i64\ndef make() -> i64:\n    return i64.Missing\ndef main():\n    value = 7\n    print(value)\n    item = make()\n");
    let checked = f.checked("main.nagi");
    let main = checked
        .program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .unwrap();
    assert_eq!(main.body[0].binding_type, Some(Type::named("i64")));
    f.write("saved.low", &emit::low(&checked.program));
    let saved = f.checked("saved.low");
    let main = saved
        .program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .unwrap();
    assert_eq!(main.body[0].binding_type, Some(Type::named("i64")));
    assert_eq!(saved.program.enums[0].name, checked.program.enums[0].name);
}
