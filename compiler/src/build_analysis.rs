//! Observation-only incremental foundation. These digests never authorize
//! skipping check, emission, Cargo, or publication. Cargo owns native inputs.
use crate::{ast::*, source::Sources};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn file_digest(path: &Path) -> Result<Value, String> {
    let mut file = File::open(path).map_err(|e| format!("Cannot hash {}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok(json!({"sha256":format!("{:x}", hash.finalize()), "bytes":bytes}))
}
fn ty(value: &Type) -> Value {
    json!([value.0, value.1.iter().map(ty).collect::<Vec<_>>()])
}
fn fields(values: &[(String, Type)]) -> Value {
    json!(values
        .iter()
        .map(|(name, value)| json!([name, ty(value)]))
        .collect::<Vec<_>>())
}
fn value_digest(value: &Value) -> String {
    digest(&serde_json::to_vec(value).expect("JSON values serialize"))
}

pub(crate) fn analyze(program: &Program, sources: &Sources, rust: &str, profile: &str) -> Value {
    let mut interface = BTreeMap::new();
    let mut bodies = BTreeMap::new();
    for class in &program.classes {
        interface.insert(format!("type:{}", class.name), fields(&class.fields));
    }
    for enumeration in &program.enums {
        interface.insert(
            format!("enum:{}", enumeration.name),
            json!(enumeration
                .variants
                .iter()
                .map(|v| json!([v.name, fields(&v.fields)]))
                .collect::<Vec<_>>()),
        );
    }
    for function in &program.functions {
        interface.insert(
            format!("fn:{}", function.name),
            json!({
                "params":fields(&function.params), "return":ty(&function.ret),
                "async":function.asynchronous, "extern":function.external, "attrs":function.attrs
            }),
        );
        // Use the compiler's canonical Low emitter, never Debug of checked facts
        // or source spans. No module transport header/physical line numbers.
        let unit = Program {
            functions: vec![function.clone()],
            ..Program::default()
        };
        bodies.insert(
            function.name.clone(),
            digest(crate::emit::low(&unit).as_bytes()),
        );
    }
    let interface_hash = value_digest(&json!(interface));
    let semantic_hash = value_digest(&json!([interface_hash, bodies]));
    json!({
        "schema_version":1, "compiler_version":env!("CARGO_PKG_VERSION"), "profile":profile,
        "scope":"observed_nagi_only_not_a_native_cache_key",
        "path_scope":"canonical_module_paths_not_portable_across_checkouts",
        "interface_sha256":interface_hash, "semantic_sha256":semantic_hash,
        "rust_sha256":digest(rust.as_bytes()), "functions":bodies,
        "sources":sources.files().map(|(_, text, _)| json!({"sha256":digest(text.as_bytes()), "bytes":text.len()})).collect::<Vec<_>>(),
        "modules":crate::graph::modules(program, sources),
        "calls":crate::graph::calls(program, sources),
        "types":crate::graph::types(program, sources),
        "test_selection":"disabled_full_required_checks",
        "native_reuse":"cargo_only"
    })
}

/// A bounded, untrusted observation. Invalid/missing data is never a cache hit.
fn read_json(path: &Path) -> Option<Value> {
    let file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
pub(crate) fn previous(app: &Path) -> Option<Value> {
    let latest = read_json(&app.join("latest.json"))?;
    if latest.get("schema_version")?.as_u64()? != 1 {
        return None;
    }
    let generation = latest.get("generation")?.as_str()?;
    if !generation.starts_with("g-")
        || generation.len() > 128
        || !generation
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-' || b == b'g')
    {
        return None;
    }
    let value = read_json(
        &app.join("generations")
            .join(generation)
            .join("build-analysis.json"),
    )?;
    (value.get("schema_version")?.as_u64()? == 1).then_some(value)
}
pub(crate) fn change(previous: Option<&Value>, current: &Value) -> &'static str {
    let Some(previous) = previous else {
        return "no_previous_analysis";
    };
    for key in ["interface_sha256", "semantic_sha256", "rust_sha256"] {
        if previous
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return "invalid_previous_analysis";
        }
    }
    if previous["compiler_version"] != current["compiler_version"] {
        "compiler_version_changed"
    } else if previous["profile"] != current["profile"] {
        "profile_changed"
    } else if previous["interface_sha256"] != current["interface_sha256"] {
        "interface_changed"
    } else if previous["semantic_sha256"] != current["semantic_sha256"] {
        "implementation_changed"
    } else if previous["rust_sha256"] != current["rust_sha256"] {
        "generated_rust_changed"
    } else {
        "observed_nagi_unchanged"
    }
}

pub(crate) struct Stats {
    pub(crate) cargo_fresh: u64,
    pub(crate) cargo_compiled: u64,
}
impl Stats {
    pub(crate) fn new() -> Self {
        Self {
            cargo_fresh: 0,
            cargo_compiled: 0,
        }
    }
    pub(crate) fn cargo_message(&mut self, line: &str) {
        if let Ok(message) = serde_json::from_str::<Value>(line) {
            if message["reason"] == "compiler-artifact" {
                match message["fresh"].as_bool() {
                    Some(true) => self.cargo_fresh += 1,
                    Some(false) => self.cargo_compiled += 1,
                    None => {}
                }
            }
        }
    }
}
