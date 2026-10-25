//! Optional initial resolution input, never a native cache or Cargo bypass.
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const DIRECTORY: &str = "runtime-seed";
const FILES: [&str; 5] = [
    "runtime-seed/Cargo.lock",
    "runtime/Cargo.toml",
    "runtime-core/Cargo.toml",
    "runtime-core/build.rs",
    "runtime/src/lib.rs",
];

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Digest {
    sha256: String,
    bytes: u64,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    schema_version: u64,
    runtime_version: String,
    files: BTreeMap<String, Digest>,
}

pub(crate) struct Selection {
    pub bytes: Option<Vec<u8>>,
    pub reason: &'static str,
    pub requested: bool,
}
impl Selection {
    pub fn disabled() -> Self {
        Self {
            bytes: None,
            reason: "not_requested",
            requested: false,
        }
    }
    pub fn report(&self, source: &str) -> serde_json::Value {
        serde_json::json!({"requested":self.requested,"source":source,
            "reason":if source == "output" { "existing_output_lock" } else { self.reason }})
    }
}

/// Protect this declared installation data even when an existing output wins.
pub(crate) fn known_inputs(root: &Path) -> Vec<PathBuf> {
    let directory = root.join(DIRECTORY);
    let descriptor = directory.join("seed.json");
    match fs::symlink_metadata(&descriptor) {
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return vec![]
        }
        Err(_) => return vec![descriptor], // Existing outputs must not ignore unknown identity.
        Ok(_) => {}
    }
    [descriptor, directory.join("Cargo.lock")]
        .into_iter()
        .filter(|path| match fs::metadata(path) {
            Ok(metadata) => metadata.is_file(),
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        })
        .collect()
}

fn plain(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| {
        format!(
            "Cannot inspect bundled lock seed input {}: {e}",
            path.display()
        )
    })?;
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    if metadata.file_type().is_symlink()
        || reparse
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err(format!(
            "Bundled lock seed input must be a plain {}: {}",
            if directory {
                "directory"
            } else {
                "regular file"
            },
            path.display()
        ));
    }
    Ok(())
}
fn read(root: &Path, relative: &str, maximum: u64) -> Result<Vec<u8>, String> {
    let path = root.join(relative);
    let mut parent = path.parent();
    while let Some(directory) = parent {
        if directory == root {
            break;
        }
        plain(directory, true)?;
        parent = directory.parent();
    }
    plain(&path, false)?;
    let mut bytes = vec![];
    fs::File::open(&path)
        .and_then(|f| f.take(maximum + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("Cannot read bundled lock seed: {e}"))?;
    if bytes.len() as u64 > maximum {
        return Err("Bundled lock seed exceeds its input limit".into());
    }
    Ok(bytes)
}

pub(crate) fn select(
    root: &Path,
    out: &Path,
    requested: bool,
    incremental: bool,
    native: bool,
    adapter: bool,
    dependencies: bool,
) -> Result<Selection, String> {
    let fallback = |reason| Selection {
        bytes: None,
        reason,
        requested,
    };
    match fs::metadata(out.join("Cargo.lock")) {
        Ok(_) => return Ok(fallback("existing_output_lock")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Cannot inherit Cargo.lock: {e}")),
    }
    if !requested {
        return Ok(Selection::disabled());
    }
    for (condition, reason) in [
        (incremental, "incremental_requested"),
        (native, "native_input"),
        (adapter, "rust_adapter"),
        (dependencies, "rust_dependencies"),
    ] {
        if condition {
            return Ok(fallback(reason));
        }
    }
    let directory = root.join(DIRECTORY);
    let descriptor = directory.join("seed.json");
    match fs::symlink_metadata(&descriptor) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(fallback("runtime_unsupported"))
        }
        Err(e) => return Err(format!("Cannot inspect bundled lock seed: {e}")),
        Ok(_) => {}
    }
    plain(&directory, true)?;
    let value: serde_json::Value =
        serde_json::from_slice(&read(root, "runtime-seed/seed.json", 1024 * 1024)?)
            .map_err(|e| format!("Invalid bundled lock seed descriptor: {e}"))?;
    let schema = value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .ok_or("Bundled lock seed has no schema version")?;
    if schema != 1 {
        return Ok(fallback("runtime_unsupported"));
    }
    let data: Descriptor = serde_json::from_value(value)
        .map_err(|e| format!("Invalid bundled lock seed descriptor: {e}"))?;
    let mut contents: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    for (file, name) in [
        ("runtime/Cargo.toml", "nagi-runtime"),
        ("runtime-core/Cargo.toml", "nagi-runtime-core"),
    ] {
        let bytes = read(root, file, 1024 * 1024)?;
        let value: toml::Value =
            toml::from_str(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
                .map_err(|e| format!("Invalid bundled runtime manifest: {e}"))?;
        contents.insert(file, bytes);
        if value
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(toml::Value::as_str)
            != Some(name)
            || value
                .get("package")
                .and_then(|p| p.get("version"))
                .and_then(toml::Value::as_str)
                != Some(data.runtime_version.as_str())
        {
            return Err("Bundled lock seed runtime identity differs".into());
        }
        if name == "nagi-runtime-core"
            && (value
                .get("lib")
                .and_then(|v| v.get("path"))
                .and_then(toml::Value::as_str)
                != Some("../runtime/src/lib.rs")
                || value
                    .get("package")
                    .and_then(|v| v.get("build"))
                    .and_then(toml::Value::as_str)
                    != Some("build.rs")
                || value
                    .get("features")
                    .and_then(|v| v.get("core"))
                    .and_then(toml::Value::as_array)
                    .is_none_or(|v| !v.is_empty()))
        {
            return Err("Bundled lock seed core layout differs".into());
        }
    }
    if data.runtime_version != env!("CARGO_PKG_VERSION") {
        return Ok(fallback("runtime_unsupported"));
    }
    if data.schema_version != 1
        || data
            .files
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>()
            != FILES.into_iter().collect()
    {
        return Err("Bundled lock seed has an unexpected input set".into());
    }
    let entries = fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .map(|e| e.map(|e| e.file_name()))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()
        .map_err(|e| e.to_string())?;
    if entries
        != [
            std::ffi::OsString::from("Cargo.lock"),
            std::ffi::OsString::from("seed.json"),
        ]
        .into_iter()
        .collect()
    {
        return Err("Bundled lock seed has an unexpected directory layout".into());
    }
    for path in FILES {
        let expected = &data.files[path];
        if !contents.contains_key(path) {
            let maximum = if path == "runtime/src/lib.rs" {
                64 * 1024 * 1024
            } else if path == "runtime-seed/Cargo.lock" {
                8 * 1024 * 1024
            } else {
                1024 * 1024
            };
            contents.insert(path, read(root, path, maximum)?);
        }
        let bytes = &contents[path];
        let actual =
            serde_json::json!({"sha256":crate::build_analysis::digest(bytes),"bytes":bytes.len()});
        if expected.sha256.len() != 64
            || !expected.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || actual["sha256"].as_str() != Some(expected.sha256.as_str())
            || actual["bytes"].as_u64() != Some(expected.bytes)
        {
            return Err(format!("Bundled lock seed input differs: {path}"));
        }
    }
    let bytes = contents
        .remove("runtime-seed/Cargo.lock")
        .expect("fixed seed input was read");
    let lock: toml::Value = toml::from_str(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Invalid bundled Cargo.lock: {e}"))?;
    for name in ["nagi-runtime", "nagi-runtime-core"] {
        if !lock
            .get("package")
            .and_then(toml::Value::as_array)
            .is_some_and(|packages| {
                packages.iter().any(|p| {
                    p.get("name").and_then(toml::Value::as_str) == Some(name)
                        && p.get("version").and_then(toml::Value::as_str)
                            == Some(data.runtime_version.as_str())
                        && p.get("source").is_none()
                })
            })
        {
            return Err("Bundled Cargo.lock has no matching runtime package".into());
        }
    }
    Ok(Selection {
        bytes: Some(bytes),
        reason: "bundled_seed",
        requested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = loop {
                let p = std::env::temp_dir().join(format!(
                    "nagi-seed-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&p) {
                    Ok(()) => break p,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => panic!("{e}"),
                }
            };
            let f = Self(root);
            f.write(
                "runtime/Cargo.toml",
                &format!(
                    "[package]\nname='nagi-runtime'\nversion='{}'\nedition='2021'\n",
                    env!("CARGO_PKG_VERSION")
                ),
            );
            f.write("runtime-core/Cargo.toml", &format!("[package]\nname='nagi-runtime-core'\nversion='{}'\nedition='2021'\nbuild='build.rs'\n[lib]\npath='../runtime/src/lib.rs'\n[features]\ncore=[]\n", env!("CARGO_PKG_VERSION")));
            f.write("runtime-core/build.rs", "fn main() {}\n");
            f.write("runtime/src/lib.rs", "");
            f.write("runtime-seed/Cargo.lock", &format!("version=3\n[[package]]\nname='nagi-runtime'\nversion='{}'\n[[package]]\nname='nagi-runtime-core'\nversion='{}'\n", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_VERSION")));
            f.refresh();
            f
        }
        fn write(&self, name: &str, value: &str) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value).unwrap();
        }
        fn refresh(&self) {
            let files: BTreeMap<_, _> = FILES
                .into_iter()
                .map(|name| {
                    (
                        name,
                        crate::build_analysis::file_digest(&self.0.join(name)).unwrap(),
                    )
                })
                .collect();
            self.write("runtime-seed/seed.json", &serde_json::json!({"schema_version":1,"runtime_version":env!("CARGO_PKG_VERSION"),"files":files}).to_string());
        }
        fn select(&self) -> Result<Selection, String> {
            select(
                &self.0,
                &self.0.join("out"),
                true,
                false,
                false,
                false,
                false,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn verified_seed_and_existing_or_disabled_resolution_have_distinct_authority() {
        let f = Fixture::new();
        assert!(f.select().unwrap().bytes.is_some());
        f.write("runtime-seed/seed.json", "malformed unused");
        assert_eq!(
            select(&f.0, &f.0.join("out"), false, false, false, false, false)
                .unwrap()
                .reason,
            "not_requested"
        );
        f.write("out/Cargo.lock", "corrupt caller lock");
        let selected = f.select().unwrap();
        assert_eq!(selected.reason, "existing_output_lock");
        assert!(selected.bytes.is_none());
    }
    #[test]
    fn every_declared_selected_corruption_fails_without_resolver_retry() {
        for variant in [
            "descriptor",
            "lock-hash",
            "lock-syntax",
            "missing",
            "extra",
            "manifest",
            "core-layout",
            "unknown-key",
        ] {
            let f = Fixture::new();
            match variant {
                "descriptor" => f.write("runtime-seed/seed.json", "{"),
                "lock-hash" => f.write("runtime-seed/Cargo.lock", "changed"),
                "lock-syntax" => {
                    f.write("runtime-seed/Cargo.lock", "invalid [[");
                    f.refresh();
                }
                "missing" => fs::remove_file(f.0.join("runtime-core/build.rs")).unwrap(),
                "extra" => f.write("runtime-seed/extra", "not allowed"),
                "manifest" => {
                    f.write(
                        "runtime/Cargo.toml",
                        "[package]\nname='wrong'\nversion='0.2.1'\n",
                    );
                    f.refresh();
                }
                "core-layout" => {
                    let p = f.0.join("runtime-core/Cargo.toml");
                    fs::write(
                        &p,
                        fs::read_to_string(&p)
                            .unwrap()
                            .replace("../runtime/src/lib.rs", "../elsewhere.rs"),
                    )
                    .unwrap();
                    f.refresh();
                }
                "unknown-key" => {
                    let p = f.0.join("runtime-seed/seed.json");
                    let mut value: serde_json::Value =
                        serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
                    value["extra"] = true.into();
                    fs::write(p, value.to_string()).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(f.select().is_err(), "{variant}");
        }
    }
    #[test]
    fn unsupported_schema_and_genuine_other_version_fall_back_but_mislabeled_identity_fails() {
        let f = Fixture::new();
        let p = f.0.join("runtime-seed/seed.json");
        let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        v["schema_version"] = 2.into();
        fs::write(&p, v.to_string()).unwrap();
        assert_eq!(f.select().unwrap().reason, "runtime_unsupported");
        v["schema_version"] = 1.into();
        v["runtime_version"] = "99.0.0".into();
        fs::write(&p, v.to_string()).unwrap();
        assert!(f.select().is_err());
        for name in ["runtime/Cargo.toml", "runtime-core/Cargo.toml"] {
            let p = f.0.join(name);
            fs::write(
                &p,
                fs::read_to_string(&p)
                    .unwrap()
                    .replace(env!("CARGO_PKG_VERSION"), "99.0.0"),
            )
            .unwrap();
        }
        assert_eq!(f.select().unwrap().reason, "runtime_unsupported");
        fs::remove_file(&p).unwrap();
        assert_eq!(f.select().unwrap().reason, "runtime_unsupported");
    }
    #[test]
    fn oversized_selected_descriptor_and_fixed_inputs_are_bounded() {
        let f = Fixture::new();
        f.write("runtime-seed/seed.json", &" ".repeat(1024 * 1024 + 1));
        assert!(f.select().err().unwrap().contains("input limit"));
        f.refresh();
        assert!(read(&f.0, "runtime/Cargo.toml", 3)
            .err()
            .unwrap()
            .contains("input limit"));
        assert!(
            select(&f.0, &f.0.join("out"), false, false, false, false, false)
                .unwrap()
                .bytes
                .is_none()
        );
    }

    #[test]
    fn opaque_boundaries_fall_back_before_reading_unrelated_seed_content() {
        let f = Fixture::new();
        f.write("runtime-seed/seed.json", "bad");
        for (flags, reason) in [
            ([true, false, false, false], "incremental_requested"),
            ([false, true, false, false], "native_input"),
            ([false, false, true, false], "rust_adapter"),
            ([false, false, false, true], "rust_dependencies"),
        ] {
            assert_eq!(
                select(
                    &f.0,
                    &f.0.join("out"),
                    true,
                    flags[0],
                    flags[1],
                    flags[2],
                    flags[3]
                )
                .unwrap()
                .reason,
                reason
            );
        }
    }
}
