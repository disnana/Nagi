//! Whole outputs through the actual resolver with stable logical identities.
//! Physical source loading, diagnostics and native execution remain integration tests.
use crate::{
    ast::{ImportSource, ModuleId, Program},
    check, emit, lexer, modules, parser,
    source::SourceProvenance,
    stdlib,
};

fn resolved_high(text: &str, identity: &str) -> Program {
    let mut program = parser::parse(text, true).unwrap();
    assert!(program.modules.is_empty());
    assert!(program.imports.is_empty());
    let imports = std::mem::take(&mut program.module_imports)
        .into_iter()
        .map(|import| {
            assert_eq!(
                import.source,
                ImportSource::Standard,
                "fixture must not require a filesystem loader"
            );
            let target = stdlib::module(&import.path).expect("registered standard import");
            (import, target)
        })
        .collect();
    let root = ModuleId(identity.into());
    let unit = modules::ModuleUnit {
        id: root.clone(),
        program,
        imports,
        tokens: lexer::lex(text, true).unwrap(),
        offset: 0,
    };
    let mut program = modules::resolve(vec![unit], root).unwrap();
    modules::validate(&program).unwrap();
    check::check(&mut program).unwrap();
    program
}
fn golden(text: &str, identity: &str, expected_low: &str, expected_rust: &str) {
    let high = resolved_high(text, identity);
    let low = emit::low(&high);
    let saved = parser::parse(&low, false).unwrap();
    // Independently finalize the saved Low; never substitute High's checked facts.
    let saved = check::finalize(
        saved,
        Program::default(),
        SourceProvenance::user_low_unmapped(),
    )
    .unwrap();
    let direct = check::finalize(
        high,
        Program::default(),
        SourceProvenance::user_low_unmapped(),
    )
    .unwrap();
    let rust = emit::rust(&direct).unwrap();
    let saved_rust = emit::rust(&saved).unwrap();
    assert!(
        rust.as_bytes() == saved_rust.as_bytes(),
        "{identity}: direct/independent Low Rust mismatch"
    );
    // Capture checked generator output for a deliberate source-contract update.
    // Assertions remain enabled; capture never counts a changed golden as pass.
    if let Some(directory) = std::env::var_os("NAGI_GOLDEN_CAPTURE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let name = std::path::Path::new(identity)
            .parent()
            .unwrap()
            .file_name()
            .unwrap();
        let target = directory.join(name);
        std::fs::write(target.with_extension("low"), &low).unwrap();
        std::fs::write(target.with_extension("rs"), &rust).unwrap();
    }
    assert!(
        low.as_bytes() == expected_low.as_bytes(),
        "{identity}: full Low golden mismatch"
    );
    assert!(
        rust.as_bytes() == expected_rust.as_bytes(),
        "{identity}: full Rust golden mismatch"
    );
}

#[test]
fn http_inspection_golden() {
    golden(
        include_str!("../../tests/fixtures/resource-contract/http-inspection.nagi"),
        "/nagi-golden/http-inspection/main.nagi",
        include_str!("../../tests/fixtures/resource-contract/http-inspection.low"),
        include_str!("../../tests/fixtures/resource-contract/http-inspection.rs"),
    );
}

#[test]
fn http_borrow_and_mappers_golden() {
    golden(
        include_str!("../../tests/fixtures/resource-contract/http-borrow-mappers.nagi"),
        "/nagi-golden/http-borrow-mappers/main.nagi",
        include_str!("../../tests/fixtures/resource-contract/http-borrow-mappers.low"),
        include_str!("../../tests/fixtures/resource-contract/http-borrow-mappers.rs"),
    );
}

#[test]
fn actor_data_golden() {
    golden(
        include_str!("../../tests/fixtures/resource-contract/actor-data.nagi"),
        "/nagi-golden/actor-data/main.nagi",
        include_str!("../../tests/fixtures/resource-contract/actor-data.low"),
        include_str!("../../tests/fixtures/resource-contract/actor-data.rs"),
    );
}

#[test]
fn data_derives_golden() {
    golden(
        include_str!("../../tests/fixtures/resource-contract/data-derives.nagi"),
        "/nagi-golden/data-derives/main.nagi",
        include_str!("../../tests/fixtures/resource-contract/data-derives.low"),
        include_str!("../../tests/fixtures/resource-contract/data-derives.rs"),
    );
}
