//! Purpose expectations: load/parse, checker, sealed emission and native are distinct.
//! No absent API or incidental parse failure counts as contract acceptance.
#[path = "support/checked_emission.rs"]
mod checked_emission;
#[path = "support/native_triple.rs"]
mod native_triple;
use nagic::{check, emit, source};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/security-sf02")
}
fn cases() -> Vec<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(fixtures().join("expectations.json")).unwrap())
        .unwrap()
}
fn rejection(path: &Path, high: bool, reason: &str, expected_line: usize) {
    let mut loaded =
        source::load(path, high).expect("load/parse must succeed before purpose rejection");
    let error = check::check(&mut loaded.program).expect_err("Session contract rejection required");
    assert!(
        error.contains(reason),
        "{}: expected diagnostic containing {reason:?}; got {error}",
        path.display()
    );
    assert!(!error.contains("internal compiler error"), "{error}");
    let diagnostic = loaded.diagnostic(&error);
    let name = path.file_name().unwrap().to_str().unwrap();
    assert!(
        diagnostic.contains(&format!("{name}:{expected_line}\n")),
        "{diagnostic}"
    );
}
#[test]
fn session_positive_high_independent_saved_and_handwritten_low_emit() {
    for case in cases()
        .into_iter()
        .filter(|c| c["status"] == "check-emit-accept")
    {
        let name = case["name"].as_str().unwrap();
        let fixture = native_triple::Fixture::new();
        fixture.write(
            "positive.nagi",
            &fs::read_to_string(fixtures().join(format!("{name}.nagi"))).unwrap(),
        );
        let program = fixture
            .checked("positive.nagi")
            .expect("positive High check");
        fixture.write("saved.low", &emit::low(&program));
        fixture.write(
            "manual.low",
            &fs::read_to_string(fixtures().join(format!("{name}.low"))).unwrap(),
        );
        // Saved Low retains no dependency on the original High file.
        fs::remove_file(fixture.0.join("positive.nagi")).unwrap();
        let saved = fixture
            .checked("saved.low")
            .unwrap_or_else(|error| panic!("{name}: saved Low independent check: {error}"));
        let manual = fixture
            .checked("manual.low")
            .expect("manual Low independent check");
        let rust = emit::rust(&checked_emission::seal(&program)).expect("sealed High emission");
        assert_eq!(rust, emit::rust(&checked_emission::seal(&saved)).unwrap());
        let manual_rust =
            emit::rust(&checked_emission::seal(&manual)).expect("sealed manual emission");
        assert!(rust.contains("::nagi_runtime::auth::session::") || name == "renamed");
        assert!(manual_rust.contains("::nagi_runtime::auth::session::") || name == "renamed");
    }
}
#[test]
fn session_inferred_foreign_types_survive_low_without_importing_names() {
    let fixture = native_triple::Fixture::new();
    fixture.write("main.nagi", "import std.auth.session as session\ndef inspect(failure: view[session.Failure]) -> i64:\n    kind = session.kind(failure)\n    outcome = session.outcome(failure)\n    return 0\ndef main():\n    print(0)\n");
    let high = fixture.checked("main.nagi").expect("Session-only import");
    fixture.write("saved.low", &emit::low(&high));
    fs::remove_file(fixture.0.join("main.nagi")).unwrap();
    let saved = fixture
        .checked("saved.low")
        .expect("foreign inferred types persist");
    for program in [&high, &saved] {
        for absent in [
            "Outcome",
            "Pool",
            "FailureKind",
            "AuthScope",
            "Response",
            "sqlite",
            "auth",
            "http",
        ] {
            assert!(
                program.modules.resolve_root_path(absent).is_none(),
                "implicit user binding {absent}"
            );
        }
        emit::rust(&checked_emission::seal(program)).expect("canonical foreign type emission");
    }
}
#[test]
fn session_negative_high_saved_manual_require_checker_and_primary_line() {
    for case in cases()
        .into_iter()
        .filter(|c| c["status"] == "checker-reject")
    {
        let name = case["name"].as_str().unwrap();
        let reason = case["reason"].as_str().unwrap();
        let high_line = case["high_line"].as_u64().unwrap() as usize;
        let manual_line = case["manual_line"].as_u64().unwrap() as usize;
        let fixture = native_triple::Fixture::new();
        let text = fs::read_to_string(fixtures().join(format!("{name}.nagi"))).unwrap();
        fixture.write("negative.nagi", &text);
        let loaded =
            source::load(&fixture.0.join("negative.nagi"), true).expect("negative High load/parse");
        let generated = emit::low_with_lines(&loaded.program);
        // Expected saved Low line comes from fixed offending High source line and
        // explicit lowering lineage, never from the observed failure/diagnostic.
        let expected_saved = (1..=generated.text.lines().count())
            .find(|&line| {
                generated.line_origin(line).is_some_and(|global| {
                    let origin = loaded.provenance().origin(global);
                    origin.line == Some(high_line)
                        && origin
                            .path
                            .as_ref()
                            .is_some_and(|p| p.ends_with("negative.nagi"))
                })
            })
            .expect("offending source line must have explicit Low origin");
        fixture.write("saved.low", &generated.text);
        fixture.write(
            "manual.low",
            &fs::read_to_string(fixtures().join(format!("{name}.low"))).unwrap(),
        );
        rejection(&fixture.0.join("negative.nagi"), true, reason, high_line);
        fs::remove_file(fixture.0.join("negative.nagi")).unwrap();
        rejection(&fixture.0.join("saved.low"), false, reason, expected_saved);
        rejection(&fixture.0.join("manual.low"), false, reason, manual_line);
    }
}
#[test]
fn session_startup_and_explicit_shared_store_are_native_in_three_forms() {
    let fixture = native_triple::Fixture::new();
    let high = fs::read_to_string(fixtures().join("startup.nagi")).unwrap();
    let low = fs::read_to_string(fixtures().join("startup.low")).unwrap();
    let assertions = r#"
#[test]
fn actual_persistent_store_startup_and_pool_close() {
    static NEXT: ::std::sync::atomic::AtomicU64 = ::std::sync::atomic::AtomicU64::new(0);
    let directory=loop {
        let n=NEXT.fetch_add(1,::std::sync::atomic::Ordering::Relaxed);
        let p=::std::env::temp_dir().join(format!("sf02-native-owned-{}-{n}",::std::process::id()));
        match ::std::fs::create_dir(&p) {
            Ok(())=>break p,
            Err(e) if e.kind()==::std::io::ErrorKind::AlreadyExists=>continue,
            Err(_)=>panic!("owned directory unavailable"),
        }
    };
    struct Owned(::std::path::PathBuf);
    impl Drop for Owned { fn drop(&mut self){let _=::std::fs::remove_dir_all(&self.0);} }
    let owned=Owned(directory);
    let runtime=::tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let path=owned.0.join("owned.sqlite");
        let pool=::nagi_runtime::sqlite::open(path.to_str().unwrap(),::nagi_runtime::sqlite::options(1,4,1000,1000).unwrap()).await.unwrap();
        let store=startup(&pool).await.unwrap();
        let state=shared_state(&store);
        drop(state);drop(store);
        ::nagi_runtime::sqlite::close(&pool,2000).await.unwrap();
        assert!(startup(&pool).await.is_err());
    });
}
"#;
    fixture.run_three_with_assertions("sf02-startup", &high, &low, "", |program| {
        let resolve = |name| {
            program
                .modules
                .resolve_root_path(name)
                .expect("canonical definition in this source form")
                .symbol
                .clone()
        };
        assertions
            .replace("startup(&pool)", &format!("{}(&pool)", resolve("startup")))
            .replace(
                "shared_state(&store)",
                &format!("{}(&store)", resolve("shared_state")),
            )
    });
}
