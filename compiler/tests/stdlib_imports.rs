use nagic::{ast::*, emit, modules, parser, source, stdlib};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nagi std imports {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }
    fn load(&self, name: &str) -> source::Sources {
        source::load(&self.0.join(name), name.ends_with(".nagi"))
            .unwrap_or_else(|error| panic!("{name}: {error}"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn imported(f: &Fixture) -> source::Sources {
    f.write(
        "main.nagi",
        "import std.http.server as http\nfrom std.http.server import Status as Code\n\
         def identity(request: http.Request) -> http.Request:\n    return request\n\
         def status() -> Code:\n    return http.Status.OK\n",
    );
    f.load("main.nagi")
}

#[test]
fn parser_distinguishes_standard_names_from_quoted_files_and_keeps_alias_spans() {
    for high in [true, false] {
        let source = "import std.http.server as http;\nfrom std.http.server import Status as Code;\nimport \"std.http.server.nagi\" as local;\n";
        let parsed = parser::parse(source, high).unwrap();
        assert_eq!(parsed.module_imports.len(), 3);
        let standard = &parsed.module_imports[0];
        assert_eq!(standard.path, stdlib::MODULE_NAME);
        assert_eq!(standard.source, ImportSource::Standard);
        assert_eq!(standard.span.end - standard.span.start, 8);
        assert!(
            matches!(&standard.kind, ImportKind::Module { alias, alias_span }
            if alias == "http" && alias_span.end - alias_span.start == 1)
        );
        assert!(
            matches!(&parsed.module_imports[1].kind, ImportKind::Names(names)
            if names[0].name == "Status" && names[0].alias == "Code"
                && names[0].name_span.end - names[0].name_span.start == 1)
        );
        assert_eq!(parsed.module_imports[2].source, ImportSource::File);
        assert_eq!(parsed.module_imports[2].path, "std.http.server.nagi");
        assert!(parsed.imports.is_empty());
        let low = emit::low(&parsed);
        assert!(low.contains("import std.http.server as http;"), "{low}");
        assert!(low.contains("from std.http.server import Status as Code;"));
        let saved = parser::parse(&low, false).unwrap();
        assert_eq!(saved.module_imports[0].source, ImportSource::Standard);
        assert_eq!(saved.module_imports[2].source, ImportSource::File);
    }
}

#[test]
fn comma_separated_from_imports_preserve_each_name_alias_and_source_span() {
    for high in [true, false] {
        for module in ["std.http.server", "\"models.nagi\""] {
            let source =
                format!("from {module} import Request as Input, Response as Output, Status;\n");
            let tokens = nagic::lexer::lex(&source, high).unwrap();
            let parsed = parser::parse(&source, high).unwrap();
            let ImportKind::Names(names) = &parsed.module_imports[0].kind else {
                panic!("expected named import");
            };
            assert_eq!(names.len(), 3);
            for (item, (name, alias)) in names.iter().zip([
                ("Request", "Input"),
                ("Response", "Output"),
                ("Status", "Status"),
            ]) {
                assert_eq!(item.name, name);
                assert_eq!(item.alias, alias);
                assert_eq!(item.name_span.end - item.name_span.start, 1);
                assert_eq!(item.alias_span.end - item.alias_span.start, 1);
                assert_eq!(
                    tokens[item.name_span.start].kind,
                    nagic::lexer::K::Id(name.into())
                );
                assert_eq!(
                    tokens[item.alias_span.start].kind,
                    nagic::lexer::K::Id(alias.into())
                );
            }
            assert_eq!(names[2].name_span, names[2].alias_span);
            let low = emit::low(&parsed);
            let saved = parser::parse(&low, false).unwrap();
            let saved_names: Vec<_> = saved
                .module_imports
                .iter()
                .flat_map(|import| match &import.kind {
                    ImportKind::Names(names) => names
                        .iter()
                        .map(|item| (item.name.as_str(), item.alias.as_str()))
                        .collect::<Vec<_>>(),
                    _ => panic!("expected named import"),
                })
                .collect();
            assert_eq!(
                saved_names,
                [
                    ("Request", "Input"),
                    ("Response", "Output"),
                    ("Status", "Status")
                ]
            );
            assert!(saved
                .module_imports
                .iter()
                .all(|import| import.source == parsed.module_imports[0].source));
        }
    }
}

#[test]
fn multiple_from_aliases_keep_registry_identity_in_independently_loaded_low() {
    let f = Fixture::new();
    f.write("main.nagi", "from std.http.server import Request as Input, Response as Output, Status, empty as make_empty\ndef identity(request: Input) -> Input:\n    return request\ndef response() -> Output:\n    return make_empty(Status.OK)\n");
    let loaded = f.load("main.nagi");
    modules::validate(&loaded.program).unwrap();
    assert_eq!(
        loaded.program.functions[0].params[0].1,
        stdlib::resource_type(stdlib::Resource::Request, vec![])
    );
    assert_eq!(
        loaded.program.functions[1].ret,
        stdlib::resource_type(stdlib::Resource::Response, vec![])
    );
    let references = &loaded.program.modules.references;
    assert!(references
        .iter()
        .any(|reference| reference.spelling == "Input"
            && reference.target == stdlib::resource_id(stdlib::Resource::Request)));
    assert!(references
        .iter()
        .any(|reference| reference.spelling == "Output"
            && reference.target == stdlib::resource_id(stdlib::Resource::Response)));
    let low = emit::low(&loaded.program);
    f.write("saved.low", &low);
    let saved = f.load("saved.low");
    modules::validate(&saved.program).unwrap();
    assert_eq!(
        saved.program.functions[0].params[0].1,
        loaded.program.functions[0].params[0].1
    );
    assert_eq!(
        saved.program.functions[1].ret,
        loaded.program.functions[1].ret
    );
}

#[test]
fn from_list_reuses_existing_conflict_and_same_definition_rules() {
    let f = Fixture::new();
    f.write("main.nagi", "from std.http.server import Request as Input, Request as Input\ndef identity(request: Input) -> Input:\n    return request\n");
    modules::validate(&f.load("main.nagi").program).unwrap();
    for statement in [
        "from std.http.server import Request as Input, Response as Input",
        "from std.http.server import Request as Input, Missing as Other",
    ] {
        f.write("main.nagi", &format!("{statement}\n"));
        let error = match source::load(&f.0.join("main.nagi"), true) {
            Ok(_) => panic!("accepted {statement}"),
            Err(error) => error,
        };
        assert!(error.contains("main.nagi:1"), "{error}");
    }
}

#[test]
fn malformed_standard_imports_do_not_fall_back_to_file_lookup() {
    for high in [true, false] {
        for source in [
            "import std.http.server",
            "import std.http.server as",
            "from std.http.server import",
            "from std.http.server import Status,",
            "from std.http.server import Status,, Request",
            "from std.http.server import Status, Request as",
            "from std.http.server import *",
            "import http.server as http",
            "import std..server as http",
        ] {
            assert!(parser::parse(source, high).is_err(), "{source}");
        }
    }
    let f = Fixture::new();
    f.write("main.nagi", "import std.missing as missing\n");
    let error = match source::load(&f.0.join("main.nagi"), true) {
        Ok(_) => panic!("accepted missing registry module"),
        Err(error) => error,
    };
    assert!(error.contains("main.nagi:1"), "{error}");
    assert!(error.contains("std.missing"), "{error}");
    assert!(!error.contains("No such file"), "{error}");
}

#[test]
fn standard_imports_ignore_local_lookalikes_and_unsaved_overlays() {
    let f = Fixture::new();
    let fake = f.write("std/http/server.nagi", "class Request:\n    fake: i64\n");
    let root = f.write(
        "main.nagi",
        "import std.http.server as http\ndef identity(request: http.Request) -> http.Request:\n    return request\n",
    );
    let overlays = HashMap::from([(fs::canonicalize(fake).unwrap(), "invalid source".into())]);
    let loaded = source::load_with_overlays(&root, true, &overlays).unwrap();
    assert_eq!(loaded.files().count(), 1);
    assert!(loaded.program.classes.is_empty());
    assert_eq!(loaded.program.functions.len(), 1);
    let request = stdlib::resource_type(stdlib::Resource::Request, vec![]);
    assert_eq!(loaded.program.functions[0].params[0].1, request);
    assert!(loaded
        .program
        .modules
        .modules
        .iter()
        .any(|module| module.id.0 == stdlib::MODULE_ID && module.path == stdlib::MODULE_ID));
    modules::validate(&loaded.program).unwrap();
}

#[test]
fn registered_resources_and_functions_are_separate_from_source_definitions() {
    let f = Fixture::new();
    let loaded = imported(&f);
    let metadata = &loaded.program.modules;
    let standard = stdlib::module(stdlib::MODULE_NAME).unwrap();
    let definitions = metadata.exports(&standard).collect::<Vec<_>>();
    assert_eq!(definitions.len(), stdlib::definitions(&standard).len());
    assert!(definitions
        .iter()
        .any(|definition| definition.id.kind == DefKind::Resource));
    assert!(definitions
        .iter()
        .any(|definition| definition.id.kind == DefKind::Function));
    assert_eq!(loaded.program.functions.len(), 2);
    assert!(loaded.program.classes.is_empty());
    let request = stdlib::resource_type(stdlib::Resource::Request, vec![]);
    assert_eq!(loaded.program.functions[0].params[0].1, request);
    let status = stdlib::resource_type(stdlib::Resource::Status, vec![]);
    assert_eq!(loaded.program.functions[1].ret, status);
    assert!(metadata.references.iter().any(|reference| reference.target
        == stdlib::resource_id(stdlib::Resource::Status)
        && reference.spelling == "http.Status.OK"));
    modules::validate(&loaded.program).unwrap();
}

#[test]
fn constant_fields_and_function_aliases_resolve_without_ignoring_local_shadowing() {
    let f = Fixture::new();
    f.write(
        "main.nagi",
        "import std.http.server as http\nfrom std.http.server import status as make_status\n\
         class Holder:\n    value: i64\n\
         def code() -> i64:\n    return http.Status.OK.value\n\
         def selected(value: i64) -> Result[http.Status, Error]:\n    return make_status(value)\n\
         def local(http: Holder) -> i64:\n    return http.value\n",
    );
    let loaded = f.load("main.nagi");
    let S::Return(Some(code)) = &loaded.program.functions[0].body[0].kind else {
        panic!("expected return");
    };
    let status = modules::symbol(&stdlib::resource_id(stdlib::Resource::Status));
    assert!(matches!(&code.kind, E::Field(constant, name)
        if name == "value" && matches!(&constant.kind, E::Field(parent, member)
            if member == "OK" && matches!(&parent.kind, E::Name(name) if name == &status))));
    let S::Return(Some(selected)) = &loaded.program.functions[1].body[0].kind else {
        panic!("expected return");
    };
    let operation = modules::symbol(&stdlib::function_id(stdlib::Operation::Status));
    assert!(matches!(&selected.kind, E::Call(name, _, _) if name == &operation));
    let S::Return(Some(local)) = &loaded.program.functions[2].body[0].kind else {
        panic!("expected return");
    };
    assert!(matches!(&local.kind, E::Field(parent, name)
        if name == "value" && matches!(&parent.kind, E::Name(name) if name == "http")));
}

#[test]
fn saved_low_keeps_registry_identity_without_standard_source_files() {
    let f = Fixture::new();
    let loaded = imported(&f);
    let low = emit::low(&loaded.program);
    assert!(
        !low.contains("import std.http.server"),
        "resolved imports should be metadata"
    );
    let parsed = parser::parse(&low, false).unwrap();
    modules::validate(&parsed).unwrap();
    f.write("saved.low", &low);
    let saved = f.load("saved.low");
    assert_eq!(saved.files().count(), 1);
    assert_eq!(
        saved.program.functions[0].params[0].1,
        loaded.program.functions[0].params[0].1
    );
    let standard = stdlib::module(stdlib::MODULE_NAME).unwrap();
    for definition in stdlib::definitions(&standard) {
        let retained = saved.program.modules.definition_id(&definition.id).unwrap();
        assert_eq!(retained.symbol, definition.symbol);
        assert_eq!(retained.line, definition.line);
    }
    assert!(saved
        .program
        .modules
        .references
        .iter()
        .filter(|reference| stdlib::is_registered_module(&reference.target.module))
        .all(|reference| !stdlib::is_registered_module(&reference.module)));
    modules::validate(&saved.program).unwrap();
}

#[test]
fn repeated_standard_imports_do_not_shift_registry_declaration_lines() {
    let f = Fixture::new();
    f.write(
        "helper.nagi",
        "from std.http.server import Status as Code\ndef status() -> Code:\n    return Code.OK\n",
    );
    f.write("main.nagi", "import \"helper.nagi\" as helper\nimport std.http.server as http\ndef status() -> http.Status:\n    return helper.status()\n");
    let mut loaded = f.load("main.nagi");
    modules::synchronize(&mut loaded.program);
    let standard = stdlib::module(stdlib::MODULE_NAME).unwrap();
    for definition in stdlib::definitions(&standard) {
        assert_eq!(
            loaded
                .program
                .modules
                .definition_id(&definition.id)
                .unwrap()
                .line,
            definition.line
        );
    }
    let low = emit::low(&loaded.program);
    f.write("dependency.low", &low);
    f.write(
        "main.low",
        "import \"dependency.low\" as dependency;\nfn entry() {}\n",
    );
    let nested = f.load("main.low");
    modules::validate(&nested.program).unwrap();
    for definition in stdlib::definitions(&standard) {
        assert_eq!(
            nested
                .program
                .modules
                .definition_id(&definition.id)
                .unwrap()
                .line,
            definition.line
        );
    }
}

#[test]
fn low_metadata_cannot_forge_standard_ids_or_implement_registry_functions() {
    let f = Fixture::new();
    let loaded = imported(&f);
    let original = &loaded.program;
    let mut wrong = original.clone();
    let standard = wrong
        .modules
        .modules
        .iter_mut()
        .find(|module| module.id.0 == stdlib::MODULE_ID)
        .unwrap();
    standard.path = "/tmp/std/http/server.nagi".into();
    assert!(modules::validate(&wrong).is_err());

    let mut wrong = original.clone();
    let definition = wrong
        .modules
        .definitions
        .iter_mut()
        .find(|definition| definition.id.kind == DefKind::Resource)
        .unwrap();
    definition.id.name = "ForgedResource".into();
    definition.symbol = modules::symbol(&definition.id);
    assert!(modules::validate(&wrong).is_err());

    let mut wrong = original.clone();
    let definition = wrong
        .modules
        .definitions
        .iter_mut()
        .find(|definition| definition.id.kind == DefKind::Resource)
        .unwrap();
    definition.id.kind = DefKind::Class;
    definition.symbol = modules::symbol(&definition.id);
    assert!(modules::validate(&wrong).is_err());

    let mut wrong = original.clone();
    let forged = stdlib::function_id(stdlib::Operation::Empty);
    let mut function = wrong.functions[0].clone();
    function.name = modules::symbol(&forged);
    wrong.functions.push(function);
    assert!(modules::validate(&wrong).is_err());

    let mut wrong = original.clone();
    let id = stdlib::resource_id(stdlib::Resource::Status);
    wrong
        .modules
        .definitions
        .retain(|definition| definition.id != id);
    wrong.modules.bindings.retain(
        |binding| !matches!(&binding.target, BindingTarget::Definition(target) if target == &id),
    );
    wrong
        .modules
        .references
        .retain(|reference| reference.target != id);
    assert!(modules::validate(&wrong).is_err());

    let mut wrong = original.clone();
    wrong.modules.root = Some(ModuleId(stdlib::MODULE_ID.into()));
    assert!(modules::validate(&wrong).is_err());
}

#[test]
fn low_metadata_rejects_native_paths_capabilities_and_unimported_canonical_names() {
    let f = Fixture::new();
    let loaded = imported(&f);
    let mut metadata = serde_json::to_value(&loaded.program.modules).unwrap();
    let index = metadata["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .position(|definition| definition["id"]["kind"] == "Resource")
        .unwrap();
    metadata["definitions"][index]["rust_path"] = serde_json::json!("arbitrary::Request");
    let source = format!("# nagi-modules-v1 {metadata}\n");
    assert!(parser::parse(&source, false).is_err());
    metadata["definitions"][index]
        .as_object_mut()
        .unwrap()
        .remove("rust_path");
    metadata["definitions"][index]["serde"] = serde_json::json!(true);
    assert!(parser::parse(&format!("# nagi-modules-v1 {metadata}\n"), false).is_err());

    let private = modules::symbol(&stdlib::resource_id(stdlib::Resource::Request));
    f.write(
        "private.nagi",
        &format!("def identity(request: {private}) -> {private}:\n    return request\n"),
    );
    assert!(source::load(&f.0.join("private.nagi"), true).is_err());
    f.write("private.nagi", &format!("from std.http.server import Request as Input\ndef identity(request: {private}) -> Input:\n    return request\n"));
    assert!(source::load(&f.0.join("private.nagi"), true).is_err());
}
