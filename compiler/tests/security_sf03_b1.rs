//! B1 compiler surface: stage-separated High, saved Low, handwritten Low and runtime validation.
#[path = "support/native_triple.rs"]
mod native_triple;
use nagic::{check, emit, source};
use native_triple::checked_emission;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/security-sf03-b1")
        .join(name)
}

#[test]
fn guarded_registration_checks_and_emits_in_three_independent_forms() {
    let fixture = native_triple::Fixture::new();
    let high = fs::read_to_string(fixture_path("guarded.nagi")).unwrap();
    let manual = fs::read_to_string(fixture_path("guarded.low")).unwrap();
    fixture.write("guarded.nagi", &high);
    let high_program = fixture
        .checked("guarded.nagi")
        .expect("High parse and checker");
    fixture.write("saved.low", &emit::low(&high_program));
    fixture.write("manual.low", &manual);
    let manual_program = fixture
        .checked("manual.low")
        .expect("handwritten Low parse and checker");
    fs::remove_file(fixture.0.join("guarded.nagi")).unwrap();
    let saved_program = fixture
        .checked("saved.low")
        .expect("saved Low after High deletion");
    let high_rust =
        emit::rust(&checked_emission::seal(&high_program)).expect("sealed High emission");
    let saved_rust =
        emit::rust(&checked_emission::seal(&saved_program)).expect("sealed saved Low emission");
    let manual_rust =
        emit::rust(&checked_emission::seal(&manual_program)).expect("sealed manual Low emission");
    assert_eq!(high_rust, saved_rust);
    assert!(manual_rust.contains("::nagi_runtime::auth::csrf::open"));
    assert!(manual_rust.contains("::nagi_runtime::http_server::session_csrf_policy"));
    assert!(manual_rust.contains("::nagi_runtime::http_server::session_csrf_tokens"));
}

fn reject_high_and_saved_low(high: &str, source_line: usize, reason: &str) {
    let fixture = native_triple::Fixture::new();
    fixture.write("bad.nagi", high);
    let mut loaded =
        source::load(&fixture.0.join("bad.nagi"), true).expect("negative High must parse");
    let generated = emit::low_with_lines(&loaded.program);
    let saved_line = (1..=generated.text.lines().count())
        .find(|&line| {
            generated.line_origin(line).is_some_and(|global| {
                let origin = loaded.provenance().origin(global);
                origin.line == Some(source_line)
                    && origin
                        .path
                        .as_ref()
                        .is_some_and(|p| p.ends_with("bad.nagi"))
            })
        })
        .expect("offending High line must map to Low");
    fixture.write("saved.low", &generated.text);
    let error = check::check(&mut loaded.program).expect_err("High must reject by checker");
    assert!(error.contains(reason), "High expected {reason:?}: {error}");
    assert!(loaded
        .diagnostic(&error)
        .contains(&format!("bad.nagi:{source_line}\n")));
    fs::remove_file(fixture.0.join("bad.nagi")).unwrap();
    let mut low = source::load(&fixture.0.join("saved.low"), false)
        .expect("saved Low must parse independently");
    let error = check::check(&mut low.program).expect_err("saved Low must reject by checker");
    assert!(error.contains(reason), "Low expected {reason:?}: {error}");
    assert!(low
        .diagnostic(&error)
        .contains(&format!("saved.low:{saved_line}\n")));
}

#[test]
fn csrf_wrong_types_and_opaque_construction_reject_in_checker() {
    reject_high_and_saved_low(
        "import std.auth.csrf as csrf\nimport std.auth.session as session\nasync def bad(store: view[session.Store], config: csrf.Options) -> Result[csrf.CsrfStore, csrf.Failure]:\n    return await csrf.open(store, config, store)\ndef main():\n    print(0)\n",
        4,
        "型",
    );
    reject_high_and_saved_low(
        "import std.auth.session as session\nimport std.http.server as http\nclass State:\n    store: session.Store\ndef bad(store: view[session.Store]) -> Result[http.App[State, Error], Error]:\n    app = http.app_default[State](State(store=session.clone_store(store)))\n    return http.session_csrf_tokens[State, Error](app, \"/_csrf\", session.clone_store(store))\ndef main():\n    print(0)\n",
        7,
        "型",
    );
    reject_high_and_saved_low(
        "import std.auth.csrf as csrf\ndef bad() -> csrf.Options:\n    return csrf.Options()\ndef main():\n    print(0)\n",
        3,
        "opaque",
    );
}

#[test]
fn guarded_session_registration_and_runtime_identity_are_native_in_three_forms() {
    let fixture = native_triple::Fixture::new();
    let high = fs::read_to_string(fixture_path("guarded.nagi")).unwrap();
    let manual = fs::read_to_string(fixture_path("guarded.low")).unwrap();
    let assertions = r#"
#[test]
fn actual_csrf_startup_guard_identity_and_plain_unsafe_refusal() {
    static NEXT: ::std::sync::atomic::AtomicU64 = ::std::sync::atomic::AtomicU64::new(0);
    let directory=loop {
        let n=NEXT.fetch_add(1,::std::sync::atomic::Ordering::Relaxed);
        let p=::std::env::temp_dir().join(format!("sf03-b1-native-owned-{}-{n}",::std::process::id()));
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
        let pool=::nagi_runtime::sqlite::open(owned.0.join("first.sqlite").to_str().unwrap(),::nagi_runtime::sqlite::options(2,4,1000,1000).unwrap()).await.unwrap();
        let store=::nagi_runtime::auth::session::open(&pool,::nagi_runtime::auth::session::options(4,5,72,1,10000,100000,5000,2).unwrap(),::nagi_runtime::auth::session::cookie_options("__Host-owned",::nagi_runtime::auth::session::SameSite::Strict).unwrap()).await.unwrap();
        let http=::nagi_runtime::http_server::authority(::nagi_runtime::http_server::security_timeout(::nagi_runtime::http_server::options(256,1000,1000,1000).unwrap(),1000).unwrap(),"https://localhost",vec!["localhost".to_owned()],1,128).unwrap();
        let csrf=open_csrf(&store,&http).await.unwrap();
        let cloned=clone_csrf(&csrf);
        assert!(guarded_registration(&store,&cloned).is_ok());
        let plain=unsafe_registration(&store);
        match plain {
            Err(e)=>assert!(matches!(e.kind,::nagi_runtime::ErrorKind::Invalid) && e.message.contains("CSRF")),
            Ok(_)=>panic!("unguarded unsafe Session registration succeeded"),
        }
        assert!(double_guard(&store,&csrf).is_err());
        let other_pool=::nagi_runtime::sqlite::open(owned.0.join("second.sqlite").to_str().unwrap(),::nagi_runtime::sqlite::options(2,4,1000,1000).unwrap()).await.unwrap();
        let other=::nagi_runtime::auth::session::open(&other_pool,::nagi_runtime::auth::session::options(4,5,72,1,10000,100000,5000,2).unwrap(),::nagi_runtime::auth::session::cookie_options("__Host-other",::nagi_runtime::auth::session::SameSite::Strict).unwrap()).await.unwrap();
        assert!(guarded_registration(&other,&csrf).is_err());
        let failure=::nagi_runtime::auth::csrf::options(0,128,1,10000,2).unwrap_err();
        assert_eq!(inspect_failure(&failure),0);
        drop(other);
        ::nagi_runtime::sqlite::close(&other_pool,2000).await.unwrap();
        drop(other_pool);drop(csrf);drop(store);
        ::nagi_runtime::sqlite::close(&pool,2000).await.unwrap();
    });
}
"#;
    fixture.run_three_with_assertions("sf03-b1-guarded", &high, &manual, "", |program| {
        let resolve = |name| {
            program
                .modules
                .resolve_root_path(name)
                .expect("fixture function")
                .symbol
                .clone()
        };
        let mut text = assertions.to_owned();
        for name in [
            "open_csrf",
            "clone_csrf",
            "guarded_registration",
            "unsafe_registration",
            "double_guard",
            "inspect_failure",
        ] {
            text = text.replace(&format!("{name}("), &format!("{}(", resolve(name)));
        }
        text
    });
}

#[test]
fn csrf_copy_move_and_message_borrow_escape_reject_in_checker() {
    reject_high_and_saved_low(
        "import std.auth.csrf as csrf\ndef bad(value: csrf.CsrfStore):\n    other = copy(view(value))\ndef main():\n    print(0)\n",
        3,
        "非Clone",
    );
    reject_high_and_saved_low(
        "import std.auth.csrf as csrf\nimport std.ownership as ownership\ndef bad(value: csrf.CsrfStore) -> csrf.CsrfStore:\n    first = ownership.move(value)\n    return ownership.move(value)\ndef main():\n    print(0)\n",
        5,
        "move後",
    );
    reject_high_and_saved_low(
        "import std.auth.csrf as csrf\ndef bad(value: csrf.Failure) -> view[str]:\n    return csrf.message(view(value))\ndef main():\n    print(0)\n",
        3,
        "escapes",
    );
}

#[test]
fn handwritten_low_negatives_reach_checker_and_keep_primary_lines() {
    for (name, text, line, reason) in [
        (
            "wrong-type",
            "import std.auth.csrf as csrf;\nimport std.auth.session as session;\nasync fn bad(store: view[session.Store],config: csrf.Options) -> Result[csrf.CsrfStore,csrf.Failure] { return await csrf.open(store,config,store); }\nfn main() -> unit { print(0); }\n",
            3,
            "型",
        ),
        (
            "opaque",
            "import std.auth.csrf as csrf;\nfn bad() -> csrf.Options { return csrf.Options(); }\nfn main() -> unit { print(0); }\n",
            2,
            "opaque",
        ),
        (
            "copy",
            "import std.auth.csrf as csrf;\nfn bad(value: csrf.CsrfStore) -> unit { let other = copy(view(value)); }\nfn main() -> unit { print(0); }\n",
            2,
            "非Clone",
        ),
        (
            "move",
            "import std.auth.csrf as csrf;\nimport std.ownership as ownership;\nfn bad(value: csrf.CsrfStore) -> csrf.CsrfStore { let first = ownership.move(value); return ownership.move(value); }\nfn main() -> unit { print(0); }\n",
            3,
            "move後",
        ),
        (
            "message",
            "import std.auth.csrf as csrf;\nfn bad(value: csrf.Failure) -> view[str] { return csrf.message(view(value)); }\nfn main() -> unit { print(0); }\n",
            2,
            "escapes",
        ),
    ] {
        let fixture = native_triple::Fixture::new();
        fixture.write("manual.low", text);
        let mut loaded = source::load(&fixture.0.join("manual.low"), false)
            .unwrap_or_else(|error| panic!("{name}: manual Low parse: {error}"));
        let error = check::check(&mut loaded.program).expect_err("manual Low must reject in checker");
        assert!(error.contains(reason), "{name}: expected {reason:?}: {error}");
        assert!(loaded.diagnostic(&error).contains(&format!("manual.low:{line}\n")), "{name}: {error}");
    }
}
