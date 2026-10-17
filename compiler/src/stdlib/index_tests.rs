use super::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

include!("index_fixture_inputs.rs");

#[test]
fn identities_match_the_legacy_registry_for_every_key_and_invalid_kind() {
    assert!(!index::legacy_enabled());
    index::with_legacy(|| {
        assert!(index::legacy_enabled());
        index::with_legacy(|| assert!(index::legacy_enabled()));
        assert!(index::legacy_enabled());
    });
    assert!(!index::legacy_enabled());
    index::first_match_fixture();
    for &resource in RESOURCES {
        let id = resource_id(resource);
        let symbol = crate::modules::symbol(&id);
        assert_eq!(self::resource(&symbol), resource_uncached(&symbol));
        assert_eq!(
            resource_named(&id.module, &id.name),
            resource_named_uncached(&id.module, &id.name)
        );
        assert!(definition(&id));
        for kind in [
            DefKind::Class,
            DefKind::Enum,
            DefKind::Function,
            DefKind::Resource,
        ] {
            let id = DefId { kind, ..id.clone() };
            assert_eq!(definition(&id), definition_uncached(&id));
        }
    }
    for &operation in OPERATIONS {
        let id = function_id(operation);
        let symbol = crate::modules::symbol(&id);
        assert_eq!(self::operation(&symbol), operation_uncached(&symbol));
        assert!(definition(&id));
        for kind in [
            DefKind::Class,
            DefKind::Enum,
            DefKind::Function,
            DefKind::Resource,
        ] {
            let id = DefId { kind, ..id.clone() };
            assert_eq!(definition(&id), definition_uncached(&id));
        }
    }
    for &module in MODULES {
        let id = ModuleId(module_info(module).id.into());
        for &resource in RESOURCES {
            let name = resource_info(resource).name;
            assert_eq!(
                resource_named(&id, name),
                resource_named_uncached(&id, name)
            );
        }
    }
    for text in [
        "",
        "Request",
        "stdlib:unknown",
        "😀",
        "__nagi_missing_symbol",
    ] {
        assert_eq!(self::resource(text), resource_uncached(text));
        assert_eq!(self::operation(text), operation_uncached(text));
        let id = DefId {
            module: ModuleId(text.into()),
            kind: DefKind::Resource,
            name: "Request".into(),
        };
        assert_eq!(definition(&id), definition_uncached(&id));
        assert_eq!(
            resource_named(&id.module, &id.name),
            resource_named_uncached(&id.module, &id.name)
        );
    }
}

#[test]
fn declarations_and_all_member_lines_match_legacy_bytes_and_order() {
    for &module in MODULES {
        let id = ModuleId(module_info(module).id.into());
        assert!(module_source(&id) == index::with_legacy(|| module_source(&id)));
        assert!(
            serde_json::to_vec(&definitions(&id)).unwrap()
                == index::with_legacy(|| serde_json::to_vec(&definitions(&id)).unwrap())
        );
    }
    for &resource in RESOURCES {
        for name in fields(resource)
            .into_iter()
            .map(|(name, _)| name)
            .chain(constants(resource).iter().map(|constant| constant.name))
            .chain(["", "missing", "kind: i64", "\n", "😀"])
        {
            assert_eq!(
                member_line(resource, name),
                index::with_legacy(|| member_line(resource, name))
            );
        }
    }
    let unknown = ModuleId("stdlib:missing".into());
    assert!(module_source(&unknown).is_empty());
    assert!(definitions(&unknown).is_empty());
    // Caller ownership remains unchanged: modifying a returned projection must
    // not mutate the retained compiled declaration or a later caller's result.
    let id = ModuleId(MODULE_ID.into());
    let expected = module_source(&id);
    let mut copy = module_source(&id);
    copy.clear();
    let mut definitions_copy = definitions(&id);
    definitions_copy[0].id.name.clear();
    assert!(module_source(&id) == expected);
    assert!(!definitions(&id)[0].id.name.is_empty());
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "nagi-index-cache-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => panic!("exclusive fixture creation failed"),
            }
        }
    }
    fn project(
        &self,
        high: bool,
        overlays: &HashMap<PathBuf, String>,
    ) -> Result<serde_json::Value, String> {
        let file = self.0.join(if high { "main.nagi" } else { "main.low" });
        let sources = crate::source::load_with_overlays(&file, high, overlays)?;
        crate::symbols::index(&sources, &[&sources.program])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn compare_fixture(name: &str, high: bool, text: &str, legacy_first: bool) {
    let fixture = Fixture::new();
    let path = fixture.0.join(if high { "main.nagi" } else { "main.low" });
    std::fs::write(&path, text).unwrap();
    let saved = HashMap::new();
    let changed = text
        .replace("ActorConfig", "ActorSettings")
        .replace("Counter", "FreshState")
        .replace("Incoming", "Packet")
        .replace("Code", "ReplyCode")
        .replace('😀', "🙂");
    let mut overlay = HashMap::new();
    overlay.insert(std::fs::canonicalize(&path).unwrap(), changed.clone());
    for overlays in [&saved, &overlay, &saved] {
        let (legacy, cached) = if legacy_first {
            (
                index::with_legacy(|| fixture.project(high, overlays)).unwrap(),
                fixture.project(high, overlays).unwrap(),
            )
        } else {
            let cached = fixture.project(high, overlays).unwrap();
            (
                index::with_legacy(|| fixture.project(high, overlays)).unwrap(),
                cached,
            )
        };
        assert!(
            serde_json::to_vec(&cached).unwrap() == serde_json::to_vec(&legacy).unwrap(),
            "whole projection differs"
        );
        assert_eq!(cached["format"], "nagi-symbols-v1");
        assert!(
            !cached["expressions"].as_array().unwrap().is_empty(),
            "typed expressions missing"
        );
        let bindings = cached["bindings"].as_array().unwrap();
        let alias = if name.starts_with("actor") {
            if overlays.is_empty() {
                "ActorConfig"
            } else {
                "ActorSettings"
            }
        } else if overlays.is_empty() {
            "Incoming"
        } else {
            "Packet"
        };
        assert!(bindings.iter().any(|binding| binding["name"] == alias));
        let stale = if name.starts_with("actor") {
            if overlays.is_empty() {
                "ActorSettings"
            } else {
                "ActorConfig"
            }
        } else if overlays.is_empty() {
            "Packet"
        } else {
            "Incoming"
        };
        assert!(!bindings.iter().any(|binding| binding["name"] == stale));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
}

#[test]
fn high_low_actor_stdlib_full_json_matches_with_overlays_and_fresh_order() {
    for (position, &(name, high, text)) in FIXTURES.iter().enumerate() {
        compare_fixture(name, high, text, position % 2 == 0);
    }
    for (high, text) in [
        (true, "import std.http.server as http\nclass Local:\n    path: i64\ndef shadow(http: Local) -> i64:\n    return http.path\ndef main():\n    print(\"😀\")\n    print(shadow(Local(path=7)))\n"),
        (false, "import std.http.server as http;\nrecord Local { path: i64; }\nfn shadow(http: Local) -> i64 { return http.path; }\nfn main() { print(\"😀\"); print(shadow(Local(path=7))); }\n"),
    ] {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join(if high { "main.nagi" } else { "main.low" }), text).unwrap();
        let cached = fixture.project(high, &HashMap::new()).unwrap();
        let legacy = index::with_legacy(|| fixture.project(high, &HashMap::new())).unwrap();
        assert!(serde_json::to_vec(&cached).unwrap() == serde_json::to_vec(&legacy).unwrap(), "shadow projection differs");
        assert!(cached["expressions"].as_array().unwrap().iter().any(|expression|
            expression["fields"].as_array().is_some_and(|fields| fields.iter().any(|field| field["name"] == "path" && field["type"] == "i64"))));
    }
}

#[test]
fn parallel_documents_keep_aliases_and_unicode_out_of_global_catalogs() {
    std::thread::scope(|scope| {
        for &(name, high, text) in &[FIXTURES[0], FIXTURES[3]] {
            scope.spawn(move || {
                assert!(!index::legacy_enabled());
                compare_fixture(name, high, text, false);
                assert!(!index::legacy_enabled());
            });
        }
    });
}

#[test]
fn invalid_documents_and_unknown_lookups_do_not_grow_retained_catalogs() {
    let before = index::retained_capacity();
    for position in 0..32 {
        let name = format!("request-only-{position}-😀");
        assert!(resource_named(&ModuleId(MODULE_ID.into()), &name).is_none());
        assert!(!definition(&DefId {
            module: ModuleId(MODULE_ID.into()),
            kind: DefKind::Function,
            name: name.clone()
        }));
        assert!(resource(&name).is_none());
        assert!(operation(&name).is_none());
        assert!(member_line(Resource::Request, &name).is_none());
        assert!(definitions(&ModuleId(name.clone())).is_empty());
        assert!(module_source(&ModuleId(name)).is_empty());
    }
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("main.nagi"), "def broken(\n").unwrap();
    let cached = fixture.project(true, &HashMap::new());
    let legacy = index::with_legacy(|| fixture.project(true, &HashMap::new()));
    assert!(cached.is_err());
    assert!(cached == legacy);
    assert_eq!(index::retained_capacity(), before);
    let panic = std::panic::catch_unwind(|| index::with_legacy(|| panic!("legacy scope test")));
    assert!(panic.is_err());
    assert!(!index::legacy_enabled());
}

struct ChildOwner(Option<Child>);
impl Drop for ChildOwner {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn cold_parallel_initialization_uses_real_cache_with_bounded_child_join() {
    const CHILD: &str = "NAGI_INDEX_COLD_CHILD";
    const RECEIPT: &str = "NAGI_INDEX_COLD_RECEIPT";
    let mode = std::env::var_os(CHILD);
    assert!(
        mode.as_ref().is_none_or(|value| value == "1"),
        "unknown cold child mode"
    );
    let child_mode = mode.is_some();
    if child_mode {
        assert!(
            std::env::args().any(|arg| arg == "--exact"),
            "invalid cold child invocation"
        );
    }
    if !child_mode {
        let receipt_owner = Fixture::new();
        let receipt = receipt_owner.0.join("cold-finished");
        let mut child = ChildOwner(Some(Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "stdlib::index_tests::cold_parallel_initialization_uses_real_cache_with_bounded_child_join"])
            .env(CHILD, "1").env(RECEIPT, &receipt).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap()));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() {
                child.0.take();
                assert!(status.success(), "cold child failed");
                assert!(
                    std::fs::read(&receipt).is_ok_and(|bytes| bytes == b"cold-ready"),
                    "cold child did not finish the exact case"
                );
                return;
            }
            if Instant::now() >= deadline {
                let process = child.0.as_mut().unwrap();
                let _ = process.kill();
                process.wait().expect("cold child was not reaped");
                child.0.take();
                panic!("cold initialization/join deadline");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    assert_eq!(std::env::var_os(CHILD).unwrap(), "1");
    assert!(index::cold(), "cold child caches were already initialized");
    assert!(!index::legacy_enabled());
    let barrier = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        for position in 0..8 {
            let barrier = &barrier;
            scope.spawn(move || {
                assert!(!index::legacy_enabled());
                barrier.wait();
                if position % 2 == 0 {
                    assert_eq!(
                        resource(&crate::modules::symbol(&resource_id(Resource::Request))),
                        Some(Resource::Request)
                    );
                }
                let id = ModuleId(module_info(MODULES[position % MODULES.len()]).id.into());
                assert!(!module_source(&id).is_empty());
                assert!(!definitions(&id).is_empty());
            });
        }
    });
    assert!(index::initialized());
    let (identities, modules, bytes) = index::retained_capacity();
    assert!(identities <= RESOURCES.len() + OPERATIONS.len());
    assert_eq!(modules, MODULES.len());
    assert!(bytes > 0);
    use std::io::Write;
    let path = std::env::var_os(RECEIPT).expect("missing cold child receipt boundary");
    let mut receipt = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    receipt.write_all(b"cold-ready").unwrap();
}
