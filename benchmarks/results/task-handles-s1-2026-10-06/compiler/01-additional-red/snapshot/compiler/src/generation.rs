//! Per-output cooperative writers and immutable successful build generations.
//! External Rust/runtime/path dependencies remain external references.
use crate::{
    diagnostics::Generated,
    source::{LoweringKind, Sources},
};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) struct OutputLock {
    _file: File,
    pub(crate) directory: PathBuf,
}
impl OutputLock {
    /// The caller must complete its initial check and non-writing input preflight
    /// first, and must repeat that preflight after this call returns.
    pub(crate) fn acquire(out: &Path) -> Result<Self, String> {
        fs::create_dir_all(out)
            .map_err(|e| format!("Cannot create output {}: {e}", out.display()))?;
        let directory = fs::canonicalize(out).map_err(|e| e.to_string())?;
        let path = directory.join(".nagi-write.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("Cannot open output lock {}: {e}", path.display()))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                eprintln!("waiting for output lock: {}", path.display());
                file.lock()
                    .map_err(|e| format!("Cannot acquire output lock {}: {e}", path.display()))?;
            }
            Err(TryLockError::Error(e)) => {
                return Err(format!(
                    "Cannot acquire output lock {}: {e}",
                    path.display()
                ))
            }
        }
        Ok(Self {
            _file: file,
            directory,
        })
    }
}

pub(crate) fn application_name(source: &Path, out: &Path) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    let stem = source.file_stem().ok_or("source has no file stem")?;
    let mut name = format!(
        "nagi-{}",
        stem.to_string_lossy()
            .chars()
            .map(|ch| {
                if ch == '_'
                    || (ch != '-' && (!ch.is_alphanumeric() || !unicode_ident::is_xid_continue(ch)))
                {
                    '-'
                } else {
                    ch
                }
            })
            .collect::<String>()
    );
    let mut identity = std::collections::hash_map::DefaultHasher::new();
    (fs::canonicalize(source).map_err(|e| e.to_string())?, out).hash(&mut identity);
    name.push_str(&format!("-{:016x}", identity.finish()));
    Ok(name)
}

pub(crate) fn latest_path(source: &Path, out: &Path) -> Result<PathBuf, String> {
    Ok(out
        .join(".nagi/apps")
        .join(application_name(source, out)?)
        .join("latest.json"))
}

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(crate) struct BuildGeneration {
    pub(crate) staging: PathBuf,
    pub(crate) bin: String,
    app: PathBuf,
    id: String,
    package: String,
    inherited_lock: Option<Vec<u8>>,
}
impl BuildGeneration {
    pub(crate) fn create(out: &Path, package: &str) -> Result<Self, String> {
        let hash = package
            .rsplit('-')
            .next()
            .filter(|hash| hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or("Build application ID has no canonical 16-hex suffix")?;
        let app = out.join(".nagi/apps").join(package);
        fs::create_dir_all(app.join("generations")).map_err(|e| e.to_string())?;
        loop {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos();
            let id = format!(
                "g-{:x}-{now:x}-{:x}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let staging = app.join("generations").join(format!(".staging-{id}"));
            match fs::create_dir(&staging) {
                Ok(()) => {
                    fs::create_dir(staging.join("src")).map_err(|e| e.to_string())?;
                    let inherited_lock = match fs::read(out.join("Cargo.lock")) {
                        Ok(lock) => Some(lock),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                        Err(e) => return Err(format!("Cannot inherit Cargo.lock: {e}")),
                    };
                    let result = Self {
                        bin: format!("nagi-{hash}-{id}"),
                        staging,
                        app,
                        id,
                        package: package.to_owned(),
                        inherited_lock,
                    };
                    if let Some(lock) = &result.inherited_lock {
                        result.write("Cargo.lock", lock)?;
                    }
                    return Ok(result);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(format!("Cannot create build generation: {e}")),
            }
        }
    }
    pub(crate) fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        fs::write(self.staging.join(name), bytes)
            .map_err(|e| format!("Cannot write generation {name}: {e}"))
    }
    pub(crate) fn snapshot(
        &self,
        sources: &Sources,
        rust: &Generated,
        low: &str,
    ) -> Result<(), String> {
        let provenance = sources.provenance();
        let files = sources.files().map(|(path, text, start)| {
            let origin = provenance.origin(start);
            Ok(serde_json::json!({"path": path.to_string_lossy(), "path_os": os_path(path), "text": text, "module": origin.module, "kind": kind(origin.kind)}))
        }).collect::<Result<Vec<_>, String>>()?;
        let lines = (1..=rust.text.lines().count()).map(|line| {
            let origin = rust.line_provenance(line);
            let path = origin.path.as_deref().map(|p| p.to_string_lossy());
            let path_os = origin.path.as_deref().map(os_path);
            Ok(serde_json::json!({"line":line,"path":path,"path_os":path_os,"source_line":origin.line,"kind":kind(origin.kind),"module":origin.module,"replacement_target":origin.replacement_target}))
        }).collect::<Result<Vec<_>, String>>()?;
        self.write("generated.low", low.as_bytes())?;
        self.json(
            "sources.json",
            &serde_json::json!({"schema_version":1,"files":files}),
        )?;
        self.json(
            "provenance.json",
            &serde_json::json!({"schema_version":1,"rust_lines":lines}),
        )
    }
    fn json(&self, file: &str, value: &serde_json::Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec_pretty(value)
            .map_err(|e| format!("Cannot serialize generation {file}: {e}"))?;
        bytes.push(b'\n');
        self.write(file, &bytes)
    }
    /// Cargo success is checked by the caller. This single consuming boundary
    /// preserves publish -> compatibility projection -> atomic latest ordering.
    pub(crate) fn finish(
        self,
        cache: &Path,
        out: &Path,
        high: bool,
        inputs: &[PathBuf],
        compatibility_manifest: &str,
    ) -> Result<PathBuf, String> {
        self.finish_lock()?;
        let executable = self.publish(cache)?;
        self.project(out, high, inputs, compatibility_manifest)?;
        self.commit_latest(inputs)?;
        Ok(executable)
    }
    fn finish_lock(&self) -> Result<(), String> {
        let Some(original) = &self.inherited_lock else {
            return Ok(());
        };
        let updated = fs::read(self.staging.join("Cargo.lock")).map_err(|e| e.to_string())?;
        let parse = |bytes: &[u8]| -> Result<toml::Value, String> {
            toml::from_str(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())
        };
        // Cargo may reserialize the lock in a new workspace even when resolution
        // is identical. Keep caller bytes/comments in that case; changed
        // resolution always uses Cargo's updated lock without comment heuristics.
        if parse(original)? == parse(&updated)? {
            self.write("Cargo.lock", original)?;
        }
        Ok(())
    }
    /// Copy the mutable cache executable and publish this directory exactly once.
    /// Compatibility projection and metadata commit are separate operations.
    fn publish(&self, cache: &Path) -> Result<PathBuf, String> {
        let executable = format!("{}{}", self.bin, std::env::consts::EXE_SUFFIX);
        fs::copy(cache, self.staging.join(&executable))
            .map_err(|e| format!("Cannot copy generation executable: {e}"))?;
        let published = self.app.join("generations").join(&self.id);
        if published.exists() {
            return Err("Build generation already exists".into());
        }
        fs::rename(&self.staging, &published)
            .map_err(|e| format!("Cannot publish generation: {e}"))?;
        Ok(published.join(executable))
    }
    fn project(
        &self,
        out: &Path,
        high: bool,
        inputs: &[PathBuf],
        compatibility_manifest: &str,
    ) -> Result<(), String> {
        let published = self.app.join("generations").join(&self.id);
        let names: &[&str] = if high {
            &["generated.low", "src/main.rs", "Cargo.toml", "Cargo.lock"]
        } else {
            &["src/main.rs", "Cargo.toml", "Cargo.lock"]
        };
        let mut paths: Vec<_> = names.iter().map(|name| out.join(name)).collect();
        paths.push(self.app.join("latest.json"));
        crate::output::protect(&paths, inputs, "Generated").map_err(|e| {
            format!("Compatibility projection may be partial; latest unchanged: {e}")
        })?;
        for name in names {
            let result = if *name == "Cargo.toml" {
                fs::write(out.join(name), compatibility_manifest).map(|_| ())
            } else {
                fs::copy(published.join(name), out.join(name)).map(|_| ())
            };
            result.map_err(|e| format!("Compatibility projection may be partial; latest unchanged: cannot update {name}: {e}"))?;
        }
        Ok(())
    }
    fn commit_latest(&self, inputs: &[PathBuf]) -> Result<(), String> {
        crate::output::protect(&[self.app.join("latest.json")], inputs, "Generated")?;
        let prefix = format!("generations/{}/", self.id);
        let executable = format!("{prefix}{}{}", self.bin, std::env::consts::EXE_SUFFIX);
        let metadata = serde_json::json!({"schema_version":1,"app_id":self.package,"generation":self.id,
            "executable":executable,"manifest":format!("{prefix}Cargo.toml"),"low":format!("{prefix}generated.low"),
            "rust":format!("{prefix}src/main.rs"),"sources":format!("{prefix}sources.json"),
            "provenance":format!("{prefix}provenance.json"),"lock":format!("{prefix}Cargo.lock")});
        let bytes = serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?;
        let temp = self.app.join(format!(".latest-{}.tmp", self.id));
        {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp)
                .map_err(|e| {
                    format!("Cannot create latest metadata; prior latest unchanged: {e}")
                })?;
            file.write_all(&bytes)
                .and_then(|_| file.write_all(b"\n"))
                .map_err(|e| {
                    format!("Cannot write latest metadata; prior latest unchanged: {e}")
                })?;
        }
        fs::rename(temp, self.app.join("latest.json"))
            .map_err(|e| format!("Cannot replace latest metadata; prior latest unchanged: {e}"))
    }
}
// Display mirrors existing diagnostics; raw OS units preserve exact location
// even when it has no Unicode representation. Module IDs retain their existing
// compiler representation and are not reconstructed from a lossy display path.
#[cfg(unix)]
fn os_path(path: &Path) -> serde_json::Value {
    use std::os::unix::ffi::OsStrExt;
    serde_json::json!({"encoding":"unix_bytes","units":path.as_os_str().as_bytes()})
}
#[cfg(windows)]
fn os_path(path: &Path) -> serde_json::Value {
    use std::os::windows::ffi::OsStrExt;
    let units: Vec<_> = path.as_os_str().encode_wide().collect();
    serde_json::json!({"encoding":"windows_utf16","units":units})
}
#[cfg(not(any(unix, windows)))]
fn os_path(path: &Path) -> serde_json::Value {
    serde_json::json!({"encoding":"encoded_bytes","units":path.as_os_str().as_encoded_bytes()})
}
fn kind(kind: LoweringKind) -> &'static str {
    match kind {
        LoweringKind::GeneratedLow => "generated_low",
        LoweringKind::UserLow => "user_low",
        LoweringKind::NativeLow => "native_low",
        LoweringKind::Replacement => "replacement",
        LoweringKind::Synthetic => "synthetic",
        LoweringKind::Unknown => "unknown",
    }
}
