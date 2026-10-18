//! Small child-process fixtures for the JWT11 process-global provider binding.
//!
//! These tests exercise provider installation ordering only. HS256 is used solely to trigger
//! JWT11's public feature-selected auto-initializer in one fresh child; it is not a G3 or Ed25519
//! acceptance test. Actual Verifier configuration is used. No key, token, pointer, or provider internals are printed.

use std::env;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

use super::{provider_binding, Limits, Startup, Verifier};
use crate::auth::FailureKind;

const CHILD_CASE: &str = "NAGI_G3_PROVIDER_PROCESS_CASE";

fn startup(finite: bool) -> Startup {
    Startup {
        issuer: "owned-issuer".into(),
        audience: "owned-audience".into(),
        keys: vec![(
            "primary".into(),
            vec![
                0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
                0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
                0xf7, 0x07, 0x51, 0x1a,
            ],
        )],
        limits: Limits {
            token_bytes: if finite { 2048 } else { 0 },
            header_bytes: 256,
            claim_bytes: 768,
            depth: 8,
            audiences: 3,
            kid_bytes: 16,
            keys: 2,
            max_ttl: Duration::from_secs(30),
        },
    }
}
fn checked_fixture_startup(finite: bool) -> Result<(), FailureKind> {
    Verifier::checked_startup(startup(finite))
        .map(|_| ())
        .map_err(|e| e.kind())
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn run_bounded_child(case: &str, suite_deadline: Instant) {
    assert!(
        Instant::now() < suite_deadline,
        "provider child suite watchdog expired"
    );
    let executable = env::current_exe().expect("test executable path");
    let child = Command::new(executable)
        .arg("--exact")
        .arg(format!(
            "{}::child_case",
            module_path!().split_once("::").unwrap().1
        ))
        .env_clear()
        .env(CHILD_CASE, case)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start isolated provider child");
    let mut child = OwnedChild(child);

    let deadline = std::cmp::min(suite_deadline, Instant::now() + Duration::from_secs(5));
    loop {
        match child.0.try_wait() {
            Ok(Some(status)) => {
                assert!(
                    status.code() == Some(42),
                    "isolated provider scenario failed: {case}"
                );
                return;
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => stop_child(&mut child.0, case),
            Err(_) => stop_child(&mut child.0, case),
        }
    }
}

fn stop_child(child: &mut Child, case: &str) -> ! {
    let _ = child.kill();
    let _ = child.wait();
    panic!("isolated provider child exceeded its bounded watchdog: {case}");
}

#[test]
fn provider_scenarios_use_fresh_bounded_processes() {
    let suite_deadline = Instant::now() + Duration::from_secs(35);
    for case in [
        "cold_invalid_then_reuse",
        "prior_explicit_aws",
        "prior_jwt_auto_init",
        "two_nagi_racers",
        "nagi_external_race",
        "postinstall_cannot_replace",
    ] {
        run_bounded_child(case, suite_deadline);
    }
}

#[test]
fn child_case() {
    let Ok(case) = env::var(CHILD_CASE) else {
        return;
    };
    match case.as_str() {
        "cold_invalid_then_reuse" => {
            assert_eq!(
                checked_fixture_startup(false),
                Err(FailureKind::InvalidRequest)
            );
            assert!(checked_fixture_startup(true).is_ok());
            assert_eq!(
                checked_fixture_startup(false),
                Err(FailureKind::InvalidRequest)
            );
            let mut invalid_key = startup(true);
            invalid_key.keys[0].1.pop();
            assert_eq!(
                Verifier::checked_startup(invalid_key).err().unwrap().kind(),
                FailureKind::InvalidRequest
            );
            assert!(checked_fixture_startup(true).is_ok());
        }
        "prior_explicit_aws" => {
            assert!(DEFAULT_PROVIDER.install_default().is_ok());
            assert_eq!(
                provider_binding::require_provider_proof().err(),
                Some(provider_binding::ProviderConflict::AlreadyInitialized)
            );
            assert_eq!(checked_fixture_startup(true), Err(FailureKind::Unavailable));
        }
        "prior_jwt_auto_init" => {
            let fixture_key = EncodingKey::from_secret(b"owned-provider-order-fixture");
            assert!(encode(&Header::new(Algorithm::HS256), &(), &fixture_key).is_ok());
            assert_eq!(
                provider_binding::require_provider_proof().err(),
                Some(provider_binding::ProviderConflict::AlreadyInitialized)
            );
            assert_eq!(checked_fixture_startup(true), Err(FailureKind::Unavailable));
        }
        "two_nagi_racers" => {
            let barrier = Arc::new(Barrier::new(3));
            let mut workers = Vec::new();
            for _ in 0..2 {
                let gate = Arc::clone(&barrier);
                workers.push(thread::spawn(move || {
                    gate.wait();
                    checked_fixture_startup(true).is_ok()
                }));
            }
            barrier.wait();
            for worker in workers {
                assert!(worker.join().expect("Nagi provider worker"));
            }
        }
        "nagi_external_race" => {
            let barrier = Arc::new(Barrier::new(3));
            let nagi_gate = Arc::clone(&barrier);
            let external_gate = Arc::clone(&barrier);
            let nagi = thread::spawn(move || {
                nagi_gate.wait();
                checked_fixture_startup(true).is_ok()
            });
            let external = thread::spawn(move || {
                external_gate.wait();
                DEFAULT_PROVIDER.install_default().is_ok()
            });
            barrier.wait();
            let nagi_installed = nagi.join().expect("Nagi installer");
            let external_installed = external.join().expect("external installer");
            assert_ne!(nagi_installed, external_installed);
            if external_installed {
                assert_eq!(
                    provider_binding::require_provider_proof().err(),
                    Some(provider_binding::ProviderConflict::AlreadyInitialized)
                );
            }
        }
        "postinstall_cannot_replace" => {
            assert!(checked_fixture_startup(true).is_ok());
            assert!(DEFAULT_PROVIDER.install_default().is_err());
            assert!(checked_fixture_startup(true).is_ok());
        }
        _ => panic!("unknown provider child case"),
    }
    // Dedicated owned-child exit oracle rules out a typo silently running zero tests.
    std::process::exit(42);
}
