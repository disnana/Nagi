//! Immutable generated-source reuse. Cargo alone decides native freshness.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) struct GeneratedBody {
    pub(crate) directory: PathBuf,
    pub(crate) digest: String,
    pub(crate) package: String,
    pub(crate) manifest: String,
    pub(crate) runtime: PathBuf,
    pub(crate) hit: bool,
    pub(crate) unit_id: Option<String>,
    pub(crate) units: Vec<GeneratedBody>,
}

fn plain(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| format!("Generated body {}: {e}", path.display()))?;
    #[cfg(windows)]
    let link = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let link = metadata.file_type().is_symlink();
    if link
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err(format!(
            "Generated body requires a plain {}: {}",
            if directory { "directory" } else { "file" },
            path.display()
        ));
    }
    Ok(())
}

fn same_bytes(path: &Path, expected: &[u8]) -> Result<(), String> {
    plain(path, false)?;
    let file = File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err(format!(
            "Generated body requires a regular file: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.take(expected.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes != expected {
        return Err(format!(
            "Generated body cache integrity mismatch: {}; prior latest unchanged",
            path.display()
        ));
    }
    Ok(())
}

static NEXT: AtomicU64 = AtomicU64::new(0);

fn exact_entries(directory: &Path, expected: &[&str]) -> Result<(), String> {
    let actual: BTreeSet<OsString> = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    if actual != expected.iter().map(OsString::from).collect() {
        return Err(format!(
            "Generated body cache has unexpected or missing entries: {}; prior latest unchanged",
            directory.display()
        ));
    }
    Ok(())
}

fn exact_layout(directory: &Path) -> Result<(), String> {
    plain(directory, true)?;
    plain(&directory.join("src"), true)?;
    exact_entries(directory, &["Cargo.toml", "src"])?;
    exact_entries(&directory.join("src"), &["lib.rs"])
}

/// Compatibility writers (including check/lower and nonincremental builds)
/// must not mutate retained bodies through a user-created hardlink or symlink.
/// Unknown/broken entries are not adopted as cache entries or written here.
pub(crate) fn protect_outputs(out: &Path, outputs: &[PathBuf]) -> Result<(), String> {
    let apps = out.join(".nagi/apps");
    if !apps.exists() || outputs.is_empty() {
        return Ok(());
    }
    let mut retained = Vec::new();
    for app in fs::read_dir(&apps).map_err(|e| e.to_string())? {
        let app = app.map_err(|e| e.to_string())?.path();
        if plain(&app, true).is_err() {
            continue;
        }
        let code = app.join("code");
        if plain(&code, true).is_err() {
            continue;
        }
        for entry in fs::read_dir(code).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.len() != 64
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                continue;
            }
            let directory = entry.path();
            if plain(&directory, true).is_err() || plain(&directory.join("src"), true).is_err() {
                continue;
            }
            for file in [directory.join("Cargo.toml"), directory.join("src/lib.rs")] {
                if plain(&file, false).is_ok() {
                    retained.push(file);
                }
            }
        }
    }
    crate::output::protect(outputs, &retained, "Generated body")
}

impl GeneratedBody {
    /// Caller holds the canonical output lock. This key identifies generated
    /// source bytes, NOT native inputs (runtime, compiler flags, toolchain, etc.).
    pub(crate) fn prepare(
        out: &Path,
        app: &str,
        runtime: &Path,
        rust: &str,
        inputs: &[PathBuf],
    ) -> Result<Self, String> {
        #[derive(serde::Serialize)]
        struct Runtime<'a> {
            path: &'a Path,
        }
        #[derive(serde::Serialize)]
        struct Dependencies<'a> {
            #[serde(rename = "nagi-runtime")]
            runtime: Runtime<'a>,
        }
        #[derive(serde::Serialize)]
        struct Package<'a> {
            name: &'a str,
            version: &'static str,
            edition: &'static str,
            build: bool,
            autobins: bool,
            autoexamples: bool,
            autotests: bool,
            autobenches: bool,
        }
        #[derive(serde::Serialize)]
        struct Lib {
            name: &'static str,
            path: &'static str,
        }
        #[derive(serde::Serialize)]
        struct Manifest<'a> {
            package: Package<'a>,
            lib: Lib,
            workspace: toml::Table,
            dependencies: Dependencies<'a>,
        }
        let runtime = fs::canonicalize(runtime).map_err(|e| e.to_string())?;
        let package = format!(
            "nagi-body-{}",
            app.rsplit('-').next().ok_or("missing app hash")?
        );
        let manifest = toml::to_string(&Manifest {
            package: Package {
                name: &package,
                version: "0.1.0",
                edition: "2021",
                build: false,
                autobins: false,
                autoexamples: false,
                autotests: false,
                autobenches: false,
            },
            lib: Lib {
                name: "nagi_app_body",
                path: "src/lib.rs",
            },
            workspace: toml::Table::new(),
            dependencies: Dependencies {
                runtime: Runtime { path: &runtime },
            },
        })
        .map_err(|e| format!("Cannot serialize generated body: {e}"))?;
        Self::publish(out, app, runtime, rust, inputs, package, manifest, None)
    }

    /// Private, closed manifest grammar for schema 3. Dependencies have already
    /// been checked and published from this invocation's sealed plan.
    pub(crate) fn prepare_unit(
        out: &Path,
        app: &str,
        runtime: &Path,
        rust: &str,
        inputs: &[PathBuf],
        id: Option<&str>,
        dependencies: &[&GeneratedBody],
    ) -> Result<Self, String> {
        if rust.len() > 64 * 1024 * 1024 {
            return Err("Generated unit exceeds 64 MiB".into());
        }
        let suffix = app.rsplit('-').next().ok_or("missing app hash")?;
        let (package, library) = match id {
            Some(id) => (
                format!("nagi-unit-{suffix}-{id}"),
                format!("nagi_unit_{id}"),
            ),
            None => (format!("nagi-body-{suffix}"), "nagi_app_body".to_owned()),
        };
        let mut manifest = format!(
            "[package]\nname = {}\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = false\nautobins = false\nautoexamples = false\nautotests = false\nautobenches = false\n[lib]\nname = {}\npath = \"src/lib.rs\"\n[workspace]\n[dependencies.nagi-runtime]\npath = {}\n",
            toml_string(&package)?, toml_string(&library)?, path_string(runtime)?,
        );
        let mut aliases = BTreeSet::new();
        for dep in dependencies {
            let id = dep.unit_id.as_ref().ok_or("Expected unit dependency")?;
            if !aliases.insert(id) {
                return Err("Duplicate logical unit dependency".into());
            }
            manifest.push_str(&format!(
                "[dependencies.nagi_unit_{id}]\npath = {}\npackage = {}\n",
                path_string(&dep.directory)?,
                toml_string(&dep.package)?,
            ));
        }
        Self::publish(
            out,
            app,
            runtime.to_path_buf(),
            rust,
            inputs,
            package,
            manifest,
            id.map(str::to_owned),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn publish(
        out: &Path,
        app: &str,
        runtime: PathBuf,
        rust: &str,
        inputs: &[PathBuf],
        package: String,
        manifest: String,
        unit_id: Option<String>,
    ) -> Result<Self, String> {
        let mut hash = Sha256::new();
        hash.update(b"nagi-generated-body-v1\0");
        for bytes in [manifest.as_bytes(), rust.as_bytes()] {
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        }
        let digest = format!("{:x}", hash.finalize());
        // Do not follow a user-installed link into another cache or input tree.
        let mut parent = out.to_path_buf();
        for part in [".nagi", "apps", app, "code"] {
            parent.push(part);
            match fs::create_dir(&parent) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(format!("Cannot create generated body cache: {e}")),
            }
            plain(&parent, true)?;
        }
        let directory = parent.join(&digest);
        let files = [directory.join("Cargo.toml"), directory.join("src/lib.rs")];
        // Inspect kinds before protect(), whose identity lookup opens files.
        let hit = match fs::symlink_metadata(&directory) {
            Ok(_) => {
                exact_layout(&directory)?;
                same_bytes(&files[0], manifest.as_bytes())?;
                same_bytes(&files[1], rust.as_bytes())?;
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.to_string()),
        };
        crate::output::protect(&files, inputs, "Generated body")?;
        if !hit {
            // Exclusive creation handles old orphan staging directories without
            // overwriting or adopting their contents. Never repair a bad hit.
            let staging = loop {
                let p = parent.join(format!(
                    ".staging-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&p) {
                    Ok(()) => break p,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => return Err(e.to_string()),
                }
            };
            fs::create_dir(staging.join("src")).map_err(|e| e.to_string())?;
            fs::write(staging.join("Cargo.toml"), &manifest).map_err(|e| e.to_string())?;
            fs::write(staging.join("src/lib.rs"), rust).map_err(|e| e.to_string())?;
            exact_layout(&staging)?;
            // Same-output cooperative writers are serialized. No existing
            // content-addressed entry is ever modified or removed.
            fs::rename(&staging, &directory)
                .map_err(|e| format!("Cannot publish generated body: {e}"))?;
        }
        Ok(Self {
            directory,
            digest,
            package,
            manifest,
            runtime,
            hit,
            unit_id,
            units: Vec::new(),
        })
    }
}

pub(crate) fn toml_string(value: &str) -> Result<String, String> {
    serde_json::to_string(value)
        .map(|s| s.replace('\u{7f}', "\\u007f"))
        .map_err(|e| e.to_string())
}
pub(crate) fn path_string(value: &Path) -> Result<String, String> {
    toml_string(value.to_str().ok_or("Generated paths require UTF-8")?)
}
