//! Syntax-stage validation of planned SQLite contract inputs.
//! The SQLite module and semantic harness are not registered. Passing this test
//! proves parseability and fixture locations, not Tx/API/SQL acceptance.
use nagic::{lexer, parser};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sqlite-contract")
}

fn matrix() -> Value {
    serde_json::from_str(include_str!("fixtures/sqlite-contract/matrix.json"))
        .expect("valid planned contract matrix JSON")
}

fn relative_input(root: &Path, name: &str, extension: &str) -> PathBuf {
    let path = Path::new(name);
    let mut components = path.components();
    assert!(matches!(components.next(), Some(Component::Normal(_))));
    assert!(
        components.next().is_none(),
        "input must be a fixture filename: {name}"
    );
    assert_eq!(
        path.extension().and_then(|value| value.to_str()),
        Some(extension)
    );
    root.join(path)
}

#[test]
fn every_planned_high_and_handwritten_low_input_parses_before_module_resolution() {
    let root = fixture_root();
    let matrix = matrix();
    assert_eq!(matrix["schema_version"], 1);
    assert_eq!(matrix["status"], "planned_unwired");
    assert_eq!(matrix["execution"]["harness_registered"], false);
    let mut ids = BTreeSet::new();
    let mut registered = BTreeSet::new();
    let cases = matrix["cases"].as_array().expect("matrix cases");
    assert!(!cases.is_empty());
    for case in cases {
        let id = case["id"].as_str().expect("case ID");
        assert!(ids.insert(id), "duplicate case {id}");
        assert_eq!(case["status"], "planned_unwired", "{id}");
        assert!(
            case["validation_result"].is_null(),
            "{id}: semantic result must stay unset"
        );
        assert!(matches!(
            case["expected_after_public_vertical_slice"].as_str(),
            Some("pass" | "fail")
        ));
        for (mode, high, extension) in [("high", true, "nagi"), ("user_low", false, "low")] {
            let name = case["inputs"][mode].as_str().expect("input filename");
            let path = relative_input(&root, name, extension);
            assert!(registered.insert(name.to_owned()), "duplicate input {name}");
            let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{name}: {error}"));
            let program = parser::parse(&text, high)
                .unwrap_or_else(|error| panic!("{name}: syntax-stage rejection: {error}"));
            assert!(
                program
                    .module_imports
                    .iter()
                    .any(|import| import.path == "std.db.sqlite"),
                "{name}: missing planned SQLite import"
            );
            assert!(
                program.modules.root.is_none(),
                "{name}: parser result must remain unresolved"
            );
        }
    }
    let on_disk: BTreeSet<_> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("nagi" | "low")
            )
        })
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        registered, on_disk,
        "every source input must have a matrix case"
    );
}

#[test]
fn planned_rejection_anchors_identify_original_fixture_lines_and_lexer_tokens() {
    let root = fixture_root();
    let matrix = matrix();
    for case in matrix["cases"].as_array().unwrap() {
        if case["expected_after_public_vertical_slice"] != "fail" {
            continue;
        }
        for (mode, high, extension) in [("high", true, "nagi"), ("user_low", false, "low")] {
            let name = case["inputs"][mode].as_str().unwrap();
            let text = fs::read_to_string(relative_input(&root, name, extension)).unwrap();
            let origin = &case["expected_origin"][mode];
            let line_number =
                usize::try_from(origin["line"].as_u64().expect("original line")).unwrap();
            let anchor = origin["anchor"].as_str().expect("original token anchor");
            let hits: Vec<_> = text
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains(anchor))
                .collect();
            assert_eq!(hits.len(), 1, "{name}: anchor must be unique: {anchor}");
            let (index, line) = hits[0];
            assert_eq!(index + 1, line_number, "{name}: stale rejection line");
            let byte_column = line.find(anchor).unwrap();
            assert_eq!(
                byte_column + 1,
                usize::try_from(origin["column_utf8_for_fixture_review"].as_u64().unwrap())
                    .unwrap(),
                "{name}: stale fixture-review column"
            );
            let character_column = line[..byte_column].chars().count() + 1;
            let tokens = lexer::lex(&text, high).unwrap();
            assert!(
                tokens.iter().any(|token| token.line == line_number
                    && token.col == character_column
                    && matches!(token.kind, lexer::K::Id(_))),
                "{name}: anchor must start on an original identifier/keyword token"
            );
        }
    }
}
