//! Compile fresh unedited generated Rust in a copy of the ordinary runtime.
//! The copied production files stay byte-identical. A test-only crate-root
//! wrapper exposes the original cfg(test) owned-loopback TLS fixture.
//! No production IP exception, runtime feature or TLS bypass is introduced.
use super::native_triple::{checked_emission, Fixture};
use nagic::emit;
use std::{fs, path::Path, process::Command, time::Duration};
#[allow(dead_code)]
#[path = "conformance.rs"]
mod conformance;

fn preserve_process_logs(fixture: &Path, label: &str, stage: &str) {
    if let Some(artifact) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
        let artifact = std::path::PathBuf::from(artifact);
        fs::create_dir_all(&artifact).unwrap();
        for stream in ["stdout", "stderr"] {
            let source = fixture.join(format!("{label}-{stage}.{stream}"));
            if source.is_file() {
                fs::copy(
                    source,
                    artifact.join(format!("http-client-{label}-{stage}.{stream}")),
                )
                .unwrap();
            }
        }
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        assert!(!kind.is_symlink(), "runtime fixture cannot follow symlinks");
        let output = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &output);
        } else {
            assert!(kind.is_file());
            fs::copy(entry.path(), &output).unwrap();
            assert_eq!(fs::read(entry.path()).unwrap(), fs::read(&output).unwrap());
        }
    }
}

fn assert_locked_dependencies(approved: &str, actual: &str) {
    let packages = |text: &str| {
        let lock: toml::Value = toml::from_str(text).unwrap();
        lock["package"]
            .as_array()
            .unwrap()
            .iter()
            .map(|package| {
                (
                    package["name"].as_str().unwrap().to_owned(),
                    package["version"].as_str().unwrap().to_owned(),
                    package
                        .get("source")
                        .map(|source| source.as_str().unwrap().to_owned()),
                )
            })
            .collect::<std::collections::HashSet<_>>()
    };
    let approved = packages(approved);
    for package in packages(actual) {
        assert!(
            approved.contains(&package),
            "native fixture changed locked dependency: {package:?}"
        );
    }
}

pub(super) fn run_three(high: &str, low: &str) {
    let fixture = Fixture::new();
    fixture.write("main.nagi", high);
    let high = fixture.checked("main.nagi").expect("High check");
    fixture.write("saved.low", &emit::low(&high));
    fixture.write("manual.low", low);
    fs::remove_file(fixture.0.join("main.nagi")).unwrap();
    let saved = fixture
        .checked("saved.low")
        .expect("saved Low without High");
    let manual = fixture
        .checked("manual.low")
        .expect("handwritten Low check");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    copy_tree(&repo.join("runtime"), &fixture.0.join("runtime"));
    let mut workspace: toml::Value =
        toml::from_str(&fs::read_to_string(repo.join("Cargo.toml")).unwrap()).unwrap();
    workspace["workspace"]["members"] =
        toml::Value::Array(vec![toml::Value::String("runtime".into())]);
    fixture.write("Cargo.toml", &toml::to_string(&workspace).unwrap());
    let approved_lock = fs::read_to_string(repo.join("Cargo.lock")).unwrap();
    fixture.write("Cargo.lock", &approved_lock);
    let original_manifest = fs::read_to_string(repo.join("runtime/Cargo.toml")).unwrap();
    fixture.write(
        "runtime/Cargo.toml",
        &format!("{original_manifest}\n[lib]\npath = \"src/lib_native_oracle.rs\"\n"),
    );
    for (label, program) in [("high", high), ("saved-low", saved), ("manual-low", manual)] {
        let generated =
            emit::rust(&checked_emission::seal(&program)).expect("sealed Rust emission");
        let symbol = |name| {
            program
                .modules
                .definitions
                .iter()
                .find(|d| d.id.name == name)
                .expect("checked definition")
                .symbol
                .as_str()
        };
        let exports: Vec<_> = program
            .modules
            .definitions
            .iter()
            .filter(|d| {
                Some(&d.id.module) == program.modules.root.as_ref()
                    && matches!(
                        d.id.kind,
                        nagic::ast::DefKind::Class
                            | nagic::ast::DefKind::Enum
                            | nagic::ast::DefKind::Function
                    )
                    && d.id.name != "main"
            })
            .map(|d| d.symbol.as_str())
            .collect();
        let wrapper = format!("extern crate self as nagi_runtime;\ninclude!(\"lib.rs\");\n#[path=\"generated_nagi.rs\"] mod generated;\npub use generated::{{{}}};\n#[cfg(test)] mod generated_operation_oracle {{\n{}\n}}\n", exports.join(","), ORACLE);
        // Adapt only the native oracle by checked canonical symbol identity.
        // generated_nagi.rs is exactly the emitter output, including aliases.
        let wrapper = wrapper
            .replace("__CALL_PUBLIC__", symbol("public_call"))
            .replace("__CALL_PROTECTED__", symbol("protected_call"))
            .replace("__BODY__", symbol("body"))
            .replace("__HEADERS__", symbol("headers"))
            .replace("__LOCAL_HEADERS__", symbol("local_headers"))
            .replace("__TAKE__", symbol("take"))
            .replace("__STATUS__", symbol("response_status"))
            .replace("__STATE__", symbol("State"))
            .replace("__PERMISSION__", symbol("Permission"));
        fixture.write("runtime/src/generated_nagi.rs", &generated);
        fixture.write("runtime/src/lib_native_oracle.rs", &wrapper);
        if let Some(artifact) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
            let artifact = std::path::PathBuf::from(artifact);
            fs::create_dir_all(&artifact).unwrap();
            fs::write(
                artifact.join(format!("http-client-{label}-generated.rs")),
                &generated,
            )
            .unwrap();
            fs::write(
                artifact.join(format!("http-client-{label}-native-wrapper.rs")),
                &wrapper,
            )
            .unwrap();
        }
        let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
            .or_else(|| std::env::var_os("CARGO_TARGET_DIR"))
            .expect("native test cache must be explicit");
        let mut build = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        build
            .current_dir(&fixture.0)
            .args([
                "test",
                "--offline",
                "-p",
                "nagi-runtime",
                "--lib",
                "--no-run",
                "--message-format=json",
            ])
            .env("CARGO_TARGET_DIR", &target);
        let compiled = conformance::bounded_process(
            &mut build,
            &format!("{label}-build"),
            Duration::from_secs(180),
            &fixture.0,
        );
        preserve_process_logs(&fixture.0, label, "build");
        compiled.unwrap_or_else(|failure| panic!("native Rust build: {failure:?}"));
        assert_locked_dependencies(
            &approved_lock,
            &fs::read_to_string(fixture.0.join("Cargo.lock")).unwrap(),
        );
        let events = fs::read_to_string(fixture.0.join(format!("{label}-build.stdout"))).unwrap();
        let binary = events
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find_map(|e| {
                (e["reason"] == "compiler-artifact"
                    && e["target"]["name"] == "nagi_runtime"
                    && e["profile"]["test"] == true)
                    .then(|| e["executable"].as_str().map(str::to_owned))
                    .flatten()
            })
            .expect("actual runtime fixture executable");
        let mut native = Command::new(binary);
        native.args([
            "--exact",
            "generated_operation_oracle::real_tls_and_grant",
            "--nocapture",
        ]);
        let ran = conformance::bounded_process(
            &mut native,
            &format!("{label}-native"),
            Duration::from_secs(20),
            &fixture.0,
        );
        preserve_process_logs(&fixture.0, label, "native");
        ran.unwrap_or_else(|failure| panic!("native TLS/Grant execution: {failure:?}"));
        let output = fs::read(fixture.0.join(format!("{label}-native.stdout"))).unwrap();
        // This executable contains all stock runtime tests too, so the exact
        // oracle intentionally filters them. Require the one selected native
        // oracle to execute; a successful zero-test invocation cannot pass.
        let output = String::from_utf8(output).unwrap();
        let summaries: Vec<_> = output
            .lines()
            .filter(|line| line.starts_with("test result:"))
            .collect();
        assert_eq!(summaries.len(), 1, "{output}");
        assert!(
            summaries[0]
                .starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "),
            "{output}"
        );
        assert_eq!(
            fs::read_to_string(fixture.0.join("runtime/src/generated_nagi.rs")).unwrap(),
            generated
        );
        assert_eq!(
            fs::read(repo.join("runtime/src/lib.rs")).unwrap(),
            fs::read(fixture.0.join("runtime/src/lib.rs")).unwrap()
        );
        if let Some(artifact) = std::env::var_os("NAGI_TEST_ARTIFACT_DIR") {
            let artifact = std::path::PathBuf::from(artifact);
            for stage in ["build", "native"] {
                for stream in ["stdout", "stderr"] {
                    fs::copy(
                        fixture.0.join(format!("{label}-{stage}.{stream}")),
                        artifact.join(format!("http-client-{label}-{stage}.{stream}")),
                    )
                    .unwrap();
                }
            }
        }
    }
}

const ORACLE: &str = r#"
use crate::{auth, http_client as client, http_server as http};
use client::startup::*;
use crate::http_client::startup_tests::{Fixture, fixed_headers, limits};
use std::{sync::{Arc,atomic::Ordering}, time::{Duration,Instant}};
use tokio::{net::{TcpListener,TcpStream},io::{AsyncReadExt,AsyncWriteExt},sync::oneshot};
fn freeze(input: Vec<u8>, limits: &InputLimits) -> Result<FrozenInput,client::Failure> { FrozenInput::checked(input,limits) }
fn prepare_public(state: &super::__STATE__, input: FrozenInput, limits: &RequestLimits) -> Result<PreparedRequest,client::Failure> {
 assert_eq!(state.n, 42); PreparedRequest::public(&["items","a/b"],&[("name","x&y")],input.into_bytes(),limits)
}
fn prepare_protected(state: &super::__STATE__, bound: BoundValues, input: FrozenInput, limits: &RequestLimits) -> Result<PreparedRequest,client::Failure> {
 assert_eq!(state.n, 42); assert_eq!((bound.subject(),bound.target()),(7,91));
 let subject=bound.subject().to_string();let target=bound.target().to_string();
 PreparedRequest::protected(bound,&["subjects",&subject,"targets",&target],&[],input.into_bytes(),limits)
}
fn inspect(response: client::Response, status:i64) {
 assert_eq!(super::__STATUS__(&response),status);
 let name=String::from("x-raw");let headers=super::__HEADERS__(&response,&name).unwrap();
 assert_eq!(headers,vec![&b"\xff"[..]]);
 assert_eq!(super::__LOCAL_HEADERS__(&response).unwrap(),vec![&b"a"[..],&b"b"[..]]);
 let bytes=super::__BODY__(&response);assert_eq!(bytes,[0,255,b'x']);let pointer=bytes.as_ptr();
 let moved=super::__TAKE__(response);assert_eq!(moved,[0,255,b'x']);assert_eq!(pointer,moved.as_ptr());
}
struct AppState { operation: client::ProtectedOperation<super::__STATE__,super::__PERMISSION__,Vec<u8>> }
async fn verify(_:http::Request,_:Arc<AppState>)->Result<auth::VerifiedIdentity,auth::Failure>{auth::VerifiedIdentity::from_verified(7,Instant::now()+Duration::from_secs(5))}
async fn authorize(scope:auth::AuthScope,_:http::Request,_:Arc<AppState>)->Result<auth::Grant<super::__PERMISSION__>,auth::Failure>{auth::Grant::from_authorized(scope,91)}
async fn handler(_:http::Request,state:Arc<AppState>,grant:auth::Grant<super::__PERMISSION__>)->Result<http::Response,crate::Error>{
 let response=super::__CALL_PROTECTED__(&state.operation,grant,b"dto".to_vec()).await.map_err(|_|crate::Error::internal("outbound failure"))?;
 inspect(response,403);Ok(http::text(http::Status::OK,"bound"))
}
#[tokio::test]
async fn real_tls_and_grant(){
 // Only destination admission to this owned loopback service is a cfg(test)
 // seam. Compiler output, TLS, Grant, structured request, driver and joins are real.
 let fixture=Fixture::new(false).await;let mut catalog=StartupCatalog::new(1).unwrap();
 let spec=PublicSpec::checked(Arc::clone(&fixture.service),1,hyper::Method::POST,limits(),fixed_headers()).unwrap();
 let operation=register_public(&mut catalog,spec,Arc::new(super::__STATE__{n:42}),freeze,prepare_public).unwrap();let mut owner=catalog.freeze();
 let response=super::__CALL_PUBLIC__(&operation,b"dto".to_vec()).await;
 owner.shutdown().await; assert_eq!(fixture.dns_calls.load(Ordering::SeqCst),2);assert_eq!(fixture.http_calls.load(Ordering::SeqCst),1);fixture.finish().await;
 inspect(response.unwrap(),302);
 let mut fixture=Fixture::new(true).await;let mut catalog=StartupCatalog::new(1).unwrap();
 let spec=ProtectedSpec::checked(Arc::clone(&fixture.service),1,hyper::Method::POST,limits(),fixed_headers(),fixture.credential.take()).unwrap();
 let operation=register_protected(&mut catalog,spec,Arc::new(super::__STATE__{n:42}),freeze,prepare_protected).unwrap();let mut owner=catalog.freeze();
 let app=http::route(http::app_default(AppState{operation}),http::Method::GET,"/",http::authorized_policy(verify,authorize),handler).unwrap();
 let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let endpoint=listener.local_addr().unwrap();let(stop,stopped)=oneshot::channel();
 let server=tokio::spawn(http::serve_listener(listener,app,http::authority(http::default_options(),"https://localhost",vec!["localhost".into()],1,128).unwrap(),async{let _=stopped.await;}));
 let mut stream=TcpStream::connect(endpoint).await.unwrap();stream.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer fixture\r\nConnection: close\r\n\r\n").await.unwrap();
 let mut wire=Vec::new();let received=tokio::time::timeout(Duration::from_secs(3),stream.read_to_end(&mut wire)).await;
 let _=stop.send(());server.await.unwrap().unwrap();owner.shutdown().await;
 let dns=fixture.dns_calls.load(Ordering::SeqCst);let http_calls=fixture.http_calls.load(Ordering::SeqCst);fixture.finish().await;
 received.unwrap().unwrap();assert!(wire.starts_with(b"HTTP/1.1 200"));assert!(wire.ends_with(b"bound"));assert_eq!((dns,http_calls),(2,1));
}
"#;
