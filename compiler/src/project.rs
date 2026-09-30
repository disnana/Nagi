//! Project configuration and CLI precedence, shared by every compiler command.
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const USAGE: &str = "nagic <check|lower|build|run|symbols> [SOURCE] [--project DIR|nagi.toml] [--no-project] [--native FILE.low] [--rust FILE.rs] [--rust-dep NAME=VERSION] [--out DIR] [--cost-report]";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    entry: String,
    #[serde(default)]
    native: Vec<String>,
    #[serde(default)]
    rust: Rust,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rust {
    file: Option<String>,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct Options {
    pub command: String,
    pub source: PathBuf,
    pub native: Vec<PathBuf>,
    pub rust_file: Option<PathBuf>,
    pub rust_dependencies: BTreeMap<String, String>,
    pub out: PathBuf,
    pub cost: bool,
    /// Read editor buffers from stdin only for the read-only symbols command.
    pub editor_input: bool,
    /// Set only when a project is selected. Plain SOURCE commands keep their cwd.
    pub project_root: Option<PathBuf>,
}

pub fn discover(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|p| p.join("nagi.toml"))
        .find(|p| p.is_file())
}

fn dependency(name: &str, version: &str) -> Result<(), String> {
    if name.replace('_', "-") == "nagi-runtime"
        || name.is_empty()
        || version.trim().is_empty()
        || !name.chars().enumerate().all(|(i, c)| {
            c == '_' || c.is_ascii_alphabetic() || i > 0 && (c.is_ascii_digit() || c == '-')
        })
    {
        return Err(format!("Rust依存の名前・versionが不正です: {name}"));
    }
    Ok(())
}

fn validate_dependencies(deps: &BTreeMap<String, String>) -> Result<(), String> {
    let mut names = std::collections::BTreeSet::new();
    for (name, version) in deps {
        dependency(name, version)?;
        if !names.insert(name.replace('-', "_")) {
            return Err(format!(
                "Rust依存の名前が重複しています (- と _ は同じ名前です): {name}"
            ));
        }
    }
    Ok(())
}

fn manifest_error(path: &Path, line: usize, message: impl std::fmt::Display) -> String {
    format!("nagi.toml: {message}\n --> {}:{line}", path.display())
}

fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path).map_err(|e| manifest_error(path, 1, e))?;
    let config: Manifest = toml::from_str(&text).map_err(|e| {
        let line = e
            .span()
            .map(|s| text[..s.start].bytes().filter(|b| *b == b'\n').count() + 1)
            .unwrap_or(1);
        manifest_error(path, line, e.message())
    })?;
    if config.entry.trim().is_empty()
        || !matches!(
            Path::new(&config.entry)
                .extension()
                .and_then(|s| s.to_str()),
            Some("nagi" | "low")
        )
    {
        return Err(manifest_error(
            path,
            1,
            "entryには.nagiまたは.lowファイルを指定してください",
        ));
    }
    if config.native.iter().any(|s| s.trim().is_empty())
        || config
            .rust
            .file
            .as_ref()
            .is_some_and(|s| s.trim().is_empty())
    {
        return Err(manifest_error(
            path,
            1,
            "native / rust.fileには空でないパスを指定してください",
        ));
    }
    validate_dependencies(&config.rust.dependencies).map_err(|e| manifest_error(path, 1, e))?;
    Ok(config)
}

/// Explicit SOURCE stays standalone unless --project is also supplied.
/// With no SOURCE, search upward from cwd for the nearest nagi.toml.
pub fn resolve(args: &[String], cwd: &Path) -> Result<Options, String> {
    let command = args.first().ok_or(USAGE)?;
    if !["check", "lower", "build", "run", "symbols"].contains(&command.as_str()) {
        return Err(format!("unknown command: {command}\n{USAGE}"));
    }
    let mut source = None;
    let mut project = None;
    let mut no_project = false;
    let mut native = vec![];
    let mut rust_file = None;
    let mut dependencies = BTreeMap::new();
    let mut out = None;
    let mut cost = false;
    let mut editor_input = false;
    let mut i = 1;
    while i < args.len() {
        let option = &args[i];
        match option.as_str() {
            "--project" | "--native" | "--rust" | "--rust-dep" | "--out" => {
                i += 1;
                let value = args
                    .get(i)
                    .filter(|v| !v.starts_with("--") && !v.is_empty())
                    .ok_or_else(|| format!("{option} requires a value"))?;
                match option.as_str() {
                    "--project" => {
                        if project.replace(cwd.join(value)).is_some() {
                            return Err("--projectは1つ指定してください".into());
                        }
                    }
                    "--native" => native.push(cwd.join(value)),
                    "--rust" => {
                        if rust_file.replace(cwd.join(value)).is_some() {
                            return Err("--rustは1ファイル指定してください".into());
                        }
                    }
                    "--out" => {
                        if out.replace(cwd.join(value)).is_some() {
                            return Err("--outは1つ指定してください".into());
                        }
                    }
                    _ => {
                        let (name, version) = value
                            .split_once('=')
                            .ok_or("--rust-dep requires NAME=VERSION")?;
                        dependency(name, version)?;
                        if dependencies
                            .insert(name.to_owned(), version.to_owned())
                            .is_some()
                        {
                            return Err(format!("Rust依存が重複しています: {name}"));
                        }
                    }
                }
            }
            "--no-project" => no_project = true,
            "--cost-report" => cost = true,
            "--editor-input" => {
                if command != "symbols" || editor_input {
                    return Err("--editor-inputはsymbolsに1回だけ指定できます".into());
                }
                editor_input = true;
            }
            x if x.starts_with('-') => return Err(format!("unknown option: {x}")),
            x => {
                if source.replace(cwd.join(x)).is_some() {
                    return Err("SOURCEは1ファイル指定してください".into());
                }
            }
        }
        i += 1;
    }
    validate_dependencies(&dependencies)?;
    if no_project && project.is_some() {
        return Err("--projectと--no-projectは同時に指定できません".into());
    }
    let manifest = if let Some(p) = project {
        Some(if p.is_dir() { p.join("nagi.toml") } else { p })
    } else if source.is_none() && !no_project {
        discover(cwd)
    } else {
        None
    };

    let mut project_root = None;
    let mut rust_dependencies = BTreeMap::new();
    if let Some(manifest) = manifest {
        let manifest = fs::canonicalize(&manifest).map_err(|e| manifest_error(&manifest, 1, e))?;
        let config = read_manifest(&manifest)?;
        let root = manifest.parent().unwrap().to_path_buf();
        if source.is_none() {
            source = Some(root.join(config.entry));
        }
        let mut configured_native: Vec<_> =
            config.native.into_iter().map(|p| root.join(p)).collect();
        configured_native.extend(native);
        native = configured_native;
        if rust_file.is_none() {
            rust_file = config.rust.file.map(|p| root.join(p));
        }
        rust_dependencies = config.rust.dependencies;
        project_root = Some(root);
    }
    rust_dependencies.extend(dependencies);
    validate_dependencies(&rust_dependencies)?;
    let source = source.ok_or(
        "SOURCEまたはnagi.tomlが必要です。nagic run --project DIRでプロジェクトを選べます",
    )?;
    let out = out.unwrap_or_else(|| {
        project_root
            .as_deref()
            .unwrap_or(cwd)
            .join("build")
            .join(source.file_stem().unwrap_or_default())
    });
    let rust_file = rust_file
        .map(|p| fs::canonicalize(&p).map_err(|e| format!("Rustファイル {}: {e}", p.display())))
        .transpose()?;
    Ok(Options {
        command: command.clone(),
        source,
        native,
        rust_file,
        rust_dependencies,
        out,
        cost,
        editor_input,
        project_root,
    })
}
