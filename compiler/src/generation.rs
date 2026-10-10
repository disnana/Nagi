//! Per-output cooperative writers and immutable successful build generations.
//! External Rust/runtime/path dependencies remain external references.
use crate::{
    diagnostics::Generated,
    source::{LoweringKind, Sources},
};
use std::{
    collections::BTreeSet,
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
    temporary_run: bool,
}
impl BuildGeneration {
    /// Only explicitly temporary IDE runs opt in. Ordinary builds retain the
    /// immutable executable/snapshot contract of ADR 007.
    pub(crate) fn temporary_run_guard(&mut self) -> Result<String, String> {
        self.temporary_run = true;
        self.json(
            "run-retention.json",
            &serde_json::json!({
                "schema_version": 2, "app_id": self.package, "generation": self.id
            }),
        )?;
        Ok(include_str!("generation_guard.rs.inc")
            .replace("@GENERATION@", &format!("{:?}", self.id))
            .replace("@APPLICATION@", &format!("{:?}", self.package))
            .replace(
                "@READY@",
                &format!("{:?}", ready_header(&self.package, &self.id)),
            ))
    }
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
                        temporary_run: false,
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
        // A later completion may collect this app or another app's generations.
        // Preserve the input generations and file identities of every remaining
        // published snapshot, including ordinary builds and orphan native runs.
        let (references, identities) = input_references(out, inputs)?;
        self.json(
            "generation-inputs.json",
            &serde_json::json!({
                "schema_version": 1, "app_id": self.package, "generation": self.id,
                "references": references, "identities": identities
            }),
        )?;
        if self.temporary_run {
            let records = self.app.join("run-retention");
            fs::create_dir_all(&records).map_err(|e| e.to_string())?;
            require_directory(&records)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(records.join(format!("{}.lease", self.id)))
                .map_err(|e| format!("Cannot create run lease: {e}"))?;
            file.write_all(ready_header(&self.package, &self.id).as_bytes())
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
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

/// A kernel lease protects managed output across compiler/IDE termination.
/// The native entry owns an independent shared lease until process termination.
pub(crate) struct RunActivity {
    out: PathBuf,
    app: PathBuf,
    pin: File,
    id: String,
}
impl RunActivity {
    /// Pin the chosen executable before releasing the publication writer lock.
    pub(crate) fn begin(binary: &Path, out: &Path) -> Result<Self, String> {
        let generation = binary.parent().ok_or("Run generation has no directory")?;
        let app = generation
            .parent()
            .and_then(Path::parent)
            .ok_or("Run generation has no application")?;
        let id = generation
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|id| valid_generation(id))
            .ok_or("Invalid run generation")?;
        let app_name = app
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid application ID")?;
        let mut pin = open_lease(&app.join("run-retention").join(format!("{id}.lease")))?;
        pin.lock_shared()
            .map_err(|e| format!("Cannot pin run generation: {e}"))?;
        if read_lease(&mut pin)? != ready_header(app_name, id).as_bytes() {
            return Err("Run generation is retired or its lease is invalid".into());
        }
        Ok(Self {
            out: out.to_owned(),
            app: app.to_owned(),
            pin,
            id: id.to_owned(),
        })
    }

    /// Successful run completion attempts bounded reclamation. Cleanup failure
    /// is a warning, never a rewrite of the observed application's exit status.
    pub(crate) fn complete(self, retire: bool, inputs: &[PathBuf]) -> Result<(), String> {
        let _writer = OutputLock::acquire(&self.out)?;
        drop(self.pin);
        if !retire {
            return Ok(());
        }
        for directory in [
            self.out.join(".nagi"),
            self.out.join(".nagi/apps"),
            self.app.clone(),
            self.app.join("run-retention"),
            self.app.join("generations"),
        ] {
            require_directory(&directory)?;
        }
        let latest = read_small_json(&self.app.join("latest.json"))?;
        let app_name = self
            .app
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid application ID")?;
        let latest_id = latest["generation"]
            .as_str()
            .filter(|id| valid_generation(id))
            .ok_or("Cannot verify latest generation")?;
        if latest["schema_version"] != 1 || latest["app_id"] != app_name {
            return Err("Cannot verify latest application".into());
        }
        let generations = self.app.join("generations");
        let completed = generations.join(&self.id).join("run-success.json");
        crate::output::protect(std::slice::from_ref(&completed), inputs, "Run cleanup")?;
        let mut success = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&completed)
            .map_err(|e| format!("Cannot record successful run completion: {e}"))?;
        success
            .write_all(
                serde_json::to_vec(&serde_json::json!({
                    "schema_version": 1, "app_id": app_name, "generation": self.id
                }))
                .map_err(|e| e.to_string())?
                .as_slice(),
            )
            .and_then(|_| success.sync_all())
            .map_err(|e| format!("Cannot persist successful run completion: {e}"))?;
        drop(success);
        // Compilation/publish latest is not evidence that its program succeeded.
        // An older run finishing after a pending/failed newer run must not
        // discard the last good run. A later successful latest resumes cleanup.
        let latest_success = generations.join(latest_id).join("run-success.json");
        if !latest_success.try_exists().map_err(|e| e.to_string())? {
            return Ok(());
        }
        let latest_success = read_small_json(&latest_success)?;
        if latest_success["schema_version"] != 1
            || latest_success["app_id"] != app_name
            || latest_success["generation"] != latest_id
        {
            return Err("Cannot verify latest run success".into());
        }
        require_directory(&generations.join(latest_id))?;
        let hash = app_name
            .rsplit('-')
            .next()
            .ok_or("Invalid application ID")?;
        let expected = format!(
            "generations/{latest_id}/nagi-{hash}-{latest_id}{}",
            std::env::consts::EXE_SUFFIX
        );
        let executable = self.app.join(&expected);
        if latest["executable"] != expected
            || fs::symlink_metadata(&executable)
                .is_ok_and(|metadata| !metadata.is_file() || is_link(&metadata))
            || !executable.is_file()
        {
            return Err("Cannot verify latest executable".into());
        }
        let inputs = inputs
            .iter()
            .map(fs::canonicalize)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Cannot verify cleanup input identities: {e}"))?;
        let (referenced, input_identities) = retained_input_references(&self.out)?;
        let mut retired = 0;
        // Scan the managed journal, not the generation directory. Old/unmarked
        // and staging artifacts cannot starve new eligible entries. Bound actual
        // deletions rather than selecting the first N arbitrary directory names.
        for entry in fs::read_dir(self.app.join("run-retention")).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let Some(id) = name
                .to_str()
                .and_then(|name| name.strip_suffix(".lease"))
                .filter(|id| valid_generation(id))
            else {
                continue;
            };
            if id == latest_id {
                continue;
            }
            let path = generations.join(id);
            if referenced.contains(&path) || inputs.iter().any(|input| input.starts_with(&path)) {
                continue;
            }
            let Ok(mut lease) = open_lease(&entry.path()) else {
                continue;
            };
            match lease.try_lock() {
                Ok(()) => {}
                Err(TryLockError::WouldBlock) => continue,
                Err(TryLockError::Error(error)) => {
                    return Err(format!("Cannot determine run lease state: {error}"))
                }
            }
            // Input export is recorded under a shared lease. Recheck only
            // after exclusive acquisition, so export admission cannot race GC.
            if entry
                .path()
                .with_extension("exported")
                .try_exists()
                .map_err(|e| e.to_string())?
            {
                continue;
            }
            let Ok(header) = read_lease(&mut lease) else {
                // A malformed individual journal is retained, not a reason to
                // starve other well-formed completed generations.
                continue;
            };
            let ready = ready_header(app_name, id);
            let tombstone = retired_header(app_name, id);
            if header != ready.as_bytes() && header != tombstone.as_bytes() {
                continue;
            }
            let present = match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    if !metadata.is_dir() || is_link(&metadata) || !plain_tree(&path, 0, &mut 4096)
                    {
                        continue;
                    }
                    true
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => return Err(error.to_string()),
            };
            crate::output::protect(&[entry.path()], &inputs, "Run cleanup")?;
            let identity = crate::output::identity(&lease).map_err(|e| e.to_string())?;
            if input_identities.contains(&identity)
                || crate::output::identity_and_links(&lease)
                    .map_err(|e| e.to_string())?
                    .2
                    > 1
            {
                // A latest/active snapshot may read an external hard-link to
                // this journal; tombstoning it would mutate its input bytes.
                continue;
            }
            // The journal is outside the deleted tree. A failed or interrupted
            // deletion retains exact ownership evidence for next-run recovery.
            use std::io::Seek;
            lease.rewind().map_err(|e| e.to_string())?;
            lease
                .write_all(tombstone.as_bytes())
                .map_err(|e| e.to_string())?;
            lease.sync_all().map_err(|e| e.to_string())?;
            if present {
                fs::remove_dir_all(&path)
                    .map_err(|e| format!("Cannot retire run generation {}: {e}", path.display()))?;
            }
            // Keep the exclusive lease through deletion. A native process that
            // opened this inode before retirement reads X after acquiring shared.
            drop(lease);
            fs::remove_file(entry.path())
                .map_err(|e| format!("Cannot remove finished run journal: {e}"))?;
            retired += 1;
            if retired == 128 {
                break;
            }
        }
        if retired != 0 {
            eprintln!("retired Nagi run generations: {retired}");
        }
        Ok(())
    }
}

/// Exported immutable inputs may be owned by another output root indefinitely.
/// Promote them to the ordinary retained-artifact policy under their own shared
/// lease. No global directory discovery, PID guess, or cross-output writer-lock
/// ordering is needed. This is deliberately conservative, not global GC.
pub(crate) fn preserve_exported_inputs(
    out: &Path,
    inputs: &[PathBuf],
) -> Result<Vec<File>, String> {
    let mut pins = Vec::new();
    let mut seen = BTreeSet::new();
    for input in inputs {
        let canonical = fs::canonicalize(input).map_err(|e| e.to_string())?;
        let Some((generation, app)) = canonical.ancestors().find_map(|path| {
            let id = path.file_name()?.to_str()?;
            let generations = path.parent()?;
            let app = generations.parent()?;
            let apps = app.parent()?;
            let namespace = apps.parent()?;
            (valid_generation(id)
                && generations.file_name() == Some(std::ffi::OsStr::new("generations"))
                && app.file_name()?.to_str().is_some_and(valid_application)
                && apps.file_name() == Some(std::ffi::OsStr::new("apps"))
                && namespace.file_name() == Some(std::ffi::OsStr::new(".nagi")))
            .then_some((path, app))
        }) else {
            continue;
        };
        if app.parent().and_then(Path::parent).and_then(Path::parent) == Some(out)
            || !seen.insert(generation.to_owned())
        {
            continue;
        }
        let id = generation
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid input generation")?;
        let app_name = app
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid input application")?;
        let records = app.join("run-retention");
        let journal = records.join(format!("{id}.lease"));
        if !journal.try_exists().map_err(|e| e.to_string())? {
            continue;
        }
        for directory in [generation, app, &records] {
            require_directory(directory)?;
        }
        let mut pin = open_lease(&journal)?;
        pin.lock_shared()
            .map_err(|e| format!("Cannot pin exported input: {e}"))?;
        if read_lease(&mut pin)? != ready_header(app_name, id).as_bytes() {
            return Err("Managed input generation is retired or its lease is invalid".into());
        }
        let exported = journal.with_extension("exported");
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&exported)
        {
            Ok(mut marker) => {
                marker
                    .write_all(format!("P:NAGI-RUN-2:{app_name}:{id}\n").as_bytes())
                    .and_then(|_| marker.sync_all())
                    .map_err(|e| e.to_string())?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&exported).map_err(|e| e.to_string())?;
                if !metadata.is_file() || is_link(&metadata) {
                    return Err("Exported input marker is not a plain file".into());
                }
            }
            Err(error) => return Err(format!("Cannot preserve exported input: {error}")),
        }
        pins.push(pin);
    }
    Ok(pins)
}

/// Store namespace references rather than lossy absolute path strings. File
/// identities additionally protect aliases outside the generation namespace.
type GenerationReferences = BTreeSet<(String, String)>;
type InputIdentities = BTreeSet<(u64, u64)>;
fn input_references(
    out: &Path,
    inputs: &[PathBuf],
) -> Result<(GenerationReferences, InputIdentities), String> {
    let mut references = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let namespace = out.join(".nagi/apps");
    for input in inputs {
        let canonical = fs::canonicalize(input).map_err(|e| e.to_string())?;
        let file = File::open(&canonical).map_err(|e| e.to_string())?;
        identities.insert(crate::output::identity(&file).map_err(|e| e.to_string())?);
        let Ok(relative) = canonical.strip_prefix(&namespace) else {
            continue;
        };
        let mut components = relative.iter();
        let Some(app) = components.next().and_then(|name| name.to_str()) else {
            continue;
        };
        if !valid_application(app) || components.next() != Some(std::ffi::OsStr::new("generations"))
        {
            continue;
        }
        let Some(id) = components
            .next()
            .and_then(|name| name.to_str())
            .filter(|id| valid_generation(id))
        else {
            continue;
        };
        references.insert((app.to_owned(), id.to_owned()));
    }
    Ok((references, identities))
}

/// Conservative snapshot dependencies are held until their owning generation
/// disappears. Collect before deletion: releasing an old dependency may take
/// one additional successful sweep, avoiding an order-dependent GC decision.
fn retained_input_references(out: &Path) -> Result<(BTreeSet<PathBuf>, InputIdentities), String> {
    #[derive(serde::Deserialize)]
    struct Record {
        schema_version: u32,
        app_id: String,
        generation: String,
        references: Vec<(String, String)>,
        identities: Vec<(u64, u64)>,
    }
    let namespace = out.join(".nagi/apps");
    let mut references = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for app in fs::read_dir(&namespace).map_err(|e| e.to_string())? {
        let app = app.map_err(|e| e.to_string())?;
        let Some(app_name) = app
            .file_name()
            .to_str()
            .map(str::to_owned)
            .filter(|name| valid_application(name))
        else {
            continue;
        };
        require_directory(&app.path())?;
        let generations = app.path().join("generations");
        if !generations.try_exists().map_err(|e| e.to_string())? {
            continue;
        }
        require_directory(&generations)?;
        for generation in fs::read_dir(generations).map_err(|e| e.to_string())? {
            let generation = generation.map_err(|e| e.to_string())?;
            let Some(id) = generation
                .file_name()
                .to_str()
                .map(str::to_owned)
                .filter(|id| valid_generation(id))
            else {
                continue;
            };
            require_directory(&generation.path())?;
            let record = generation.path().join("generation-inputs.json");
            if !record.try_exists().map_err(|e| e.to_string())? {
                // Missing dependencies cannot be read as "no dependencies" for
                // a ready managed artifact. A retired X journal, however, is
                // enough to resume a tree whose metadata was already deleted.
                let journal_directory = app.path().join("run-retention");
                let journal = journal_directory.join(format!("{id}.lease"));
                if journal.try_exists().map_err(|e| e.to_string())? {
                    require_directory(&journal_directory)?;
                    let mut lease = open_lease(&journal)?;
                    if read_lease(&mut lease)? != retired_header(&app_name, &id).as_bytes() {
                        return Err(
                            "Ready managed generation is missing input protection metadata".into(),
                        );
                    }
                }
                continue; // Previously published/unmarked generations stay untouched.
            }
            let record: Record = serde_json::from_value(read_small_json(&record)?)
                .map_err(|e| format!("Cannot verify generation inputs: {e}"))?;
            if record.schema_version != 1 || record.app_id != app_name || record.generation != id {
                return Err("Cannot verify generation input identity".into());
            }
            for (app, id) in record.references {
                if !valid_application(&app) || !valid_generation(&id) {
                    return Err("Cannot verify referenced input generation".into());
                }
                references.insert(namespace.join(app).join("generations").join(id));
            }
            identities.extend(record.identities);
        }
    }
    Ok((references, identities))
}

fn valid_application(app: &str) -> bool {
    app.starts_with("nagi-")
        && !app.contains(['/', '\\', ':'])
        && app.rsplit('-').next().is_some_and(|hash| {
            hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

fn ready_header(app: &str, id: &str) -> String {
    format!("R:NAGI-RUN-2:{app}:{id}\n")
}
fn retired_header(app: &str, id: &str) -> String {
    format!("X:NAGI-RUN-2:{app}:{id}\n")
}
fn open_lease(path: &Path) -> Result<File, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || is_link(&metadata) {
        return Err("Run lease is not a plain file".into());
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("Cannot open run lease: {e}"))
}
fn read_lease(file: &mut File) -> Result<Vec<u8>, String> {
    use std::io::{Read, Seek};
    file.rewind().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4096 {
        return Err("Run lease exceeds 4 KiB".into());
    }
    Ok(bytes)
}

fn valid_generation(id: &str) -> bool {
    let mut parts = id.split('-');
    parts.next() == Some("g")
        && (0..3).all(|_| {
            parts.next().is_some_and(|part| {
                !part.is_empty() && part.bytes().all(|ch| ch.is_ascii_hexdigit())
            })
        })
        && parts.next().is_none()
}
fn require_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if metadata.is_dir() && !is_link(&metadata) {
        Ok(())
    } else {
        Err(format!(
            "Cleanup requires a plain directory: {}",
            path.display()
        ))
    }
}
fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions are reparse points too, even when is_symlink is false.
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn read_small_json(path: &Path) -> Result<serde_json::Value, String> {
    use std::io::Read;
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || is_link(&metadata) {
        return Err("Cleanup metadata is not a plain file".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Cleanup metadata exceeds 64 KiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn plain_tree(path: &Path, depth: usize, remaining: &mut usize) -> bool {
    if depth > 64 || *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if is_link(&metadata) {
        return false;
    }
    if metadata.is_file() {
        return true;
    }
    if !metadata.is_dir() {
        return false;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    for entry in entries {
        let Ok(entry) = entry else { return false };
        if !plain_tree(&entry.path(), depth + 1, remaining) {
            return false;
        }
    }
    true
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
