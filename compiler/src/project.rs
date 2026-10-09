//! Project configuration and CLI precedence, shared by every compiler command.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const USAGE: &str = "Usage:
  nagic <check|lower|build|run|symbols|assist> [SOURCE] [OPTIONS]
  nagic map [types|modules|calls] [SOURCE] [OPTIONS]
  nagic version

Commands:
  check    Check syntax, types, and ownership
  lower    Check and write generated Low
  build    Build a native executable
  run      Build and run a program
  symbols  Print type and definition information as JSON
  assist   Print compiler-authoritative editor assistance as JSON (internal)
  map      Map checked source types, modules, or calls (default: types)
  version  Print the compiler version

Options:
  --project DIR|nagi.toml  Select a project
  --no-project            Disable project discovery
  --native FILE.low       Add handwritten Low
  --rust FILE.rs          Add a Rust adapter
  --rust-dep NAME=VERSION  Add a Rust dependency
  --out DIR               Select the generated-source directory
  --cost-report           Write an allocation/copy cost report
  --rust-diagnostics      Include generated Rust diagnostic details (build/run)
  --editor-input          Read editor buffers from stdin (check/symbols/assist)
  --sql-schema FILE       Check literal SQL against an offline DDL snapshot (check)
  --sql-dialect sqlite    Required with --sql-schema; SQLite only
  --format FORMAT         Map output: mermaid (default), d2, json, html, svg, png
  --module NAME           Select a map module
  --focus NAME            Focus a map definition
  --depth N               Limit relationships from the focused definition
  --output FILE           Write the map to FILE instead of stdout
  --layout elk|dagre|tala  SVG/PNG layout (default: elk; requires local D2)
  -h, --help              Show this help
  -V, --version           Print the compiler version

Map notes:
  SVG/PNG require --output with a matching .svg/.png extension and local D2.
  Available layouts depend on the installed D2 version.
  architecture, dataflow, trace, cost, and --serve are not implemented.";

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
    dependencies: BTreeMap<String, RustDependency>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RustDependency {
    Version(String),
    Detailed(RustDependencyTable),
}

impl<'de> Deserialize<'de> for RustDependency {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DependencyVisitor;
        impl<'de> serde::de::Visitor<'de> for DependencyVisitor {
            type Value = RustDependency;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a version string or Rust dependency table")
            }
            fn visit_str<E: serde::de::Error>(self, version: &str) -> Result<Self::Value, E> {
                Ok(RustDependency::Version(version.to_owned()))
            }
            fn visit_string<E: serde::de::Error>(self, version: String) -> Result<Self::Value, E> {
                Ok(RustDependency::Version(version))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                RustDependencyTable::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(RustDependency::Detailed)
            }
        }
        deserializer.deserialize_any(DependencyVisitor)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RustDependencyTable {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(rename = "default-features", skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
}

#[derive(Debug)]
pub struct Options {
    pub command: String,
    pub source: PathBuf,
    pub native: Vec<PathBuf>,
    pub rust_file: Option<PathBuf>,
    pub rust_dependencies: BTreeMap<String, RustDependency>,
    pub out: PathBuf,
    pub cost: bool,
    /// Include raw generated Rust details after a mapped Nagi diagnostic.
    pub rust_diagnostics: bool,
    /// Read editor buffers from stdin for symbols/assist or an in-memory check.
    pub editor_input: bool,
    pub sql_schema: Option<PathBuf>,
    /// Set only when a project is selected. Plain SOURCE commands keep their cwd.
    pub project_root: Option<PathBuf>,
    pub(crate) manifest_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapView {
    Types,
    Modules,
    Calls,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapFormat {
    Mermaid,
    D2,
    Json,
    Html,
    Svg,
    Png,
}

impl MapFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mermaid => "mermaid",
            Self::D2 => "d2",
            Self::Json => "json",
            Self::Html => "html",
            Self::Svg => "svg",
            Self::Png => "png",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapLayout {
    Elk,
    Dagre,
    Tala,
}

impl MapLayout {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Elk => "elk",
            Self::Dagre => "dagre",
            Self::Tala => "tala",
        }
    }
}

#[derive(Debug)]
pub struct MapOptions {
    pub view: MapView,
    pub format: MapFormat,
    pub module: Option<String>,
    pub focus: Option<String>,
    pub depth: Option<usize>,
    pub output: Option<PathBuf>,
    pub layout: MapLayout,
}

/// Remove only map-specific arguments, then reuse the existing source/project
/// resolver. Adapter and native paths retain the same precedence as check.
pub fn resolve_map(args: &[String], cwd: &Path) -> Result<(Options, MapOptions), String> {
    if args.first().map(String::as_str) != Some("map") {
        return Err("resolve_map requires the map command".into());
    }
    let mut view = MapView::Types;
    let mut i = 1;
    match args.get(i).map(String::as_str) {
        Some("types") => i += 1,
        Some("modules") => {
            view = MapView::Modules;
            i += 1;
        }
        Some("calls") => {
            view = MapView::Calls;
            i += 1;
        }
        Some(name @ ("architecture" | "trace" | "dataflow" | "cost")) => {
            return Err(format!(
                "map {name} is not implemented; supported views: types, modules, calls"
            ));
        }
        _ => {}
    }
    let mut input = vec!["check".to_owned()];
    let mut format = None;
    let mut module = None;
    let mut focus = None;
    let mut depth = None;
    let mut output = None;
    let mut layout = None;
    while i < args.len() {
        let option = args[i].as_str();
        match option {
            "--format" | "--module" | "--focus" | "--depth" | "--output" | "--layout" => {
                i += 1;
                let value = args
                    .get(i)
                    .filter(|value| !value.starts_with("--") && !value.is_empty())
                    .ok_or_else(|| format!("{option} requires a value"))?;
                let duplicate = match option {
                    "--format" => {
                        let parsed = match value.as_str() {
                            "mermaid" => MapFormat::Mermaid,
                            "d2" => MapFormat::D2,
                            "json" => MapFormat::Json,
                            "html" => MapFormat::Html,
                            "svg" => MapFormat::Svg,
                            "png" => MapFormat::Png,
                            _ => return Err(format!("unsupported map format: {value}; use mermaid, d2, json, html, svg, or png")),
                        };
                        format.replace(parsed).is_some()
                    }
                    "--module" => module.replace(value.clone()).is_some(),
                    "--focus" => focus.replace(value.clone()).is_some(),
                    "--depth" => {
                        let parsed = value
                            .parse::<usize>()
                            .map_err(|_| "--depth requires a nonnegative integer".to_owned())?;
                        depth.replace(parsed).is_some()
                    }
                    "--output" => output.replace(cwd.join(value)).is_some(),
                    _ => {
                        let parsed = match value.as_str() {
                            "elk" => MapLayout::Elk,
                            "dagre" => MapLayout::Dagre,
                            "tala" => MapLayout::Tala,
                            _ => {
                                return Err(format!(
                                    "unsupported map layout: {value}; use elk, dagre, or tala"
                                ))
                            }
                        };
                        layout.replace(parsed).is_some()
                    }
                };
                if duplicate {
                    return Err(format!("{option} may be specified only once"));
                }
            }
            "--project" | "--native" | "--rust" | "--rust-dep" => {
                // Keep values intact even when a path happens to match a map
                // option or view name. resolve validates these shared options.
                input.push(args[i].clone());
                i += 1;
                input.push(
                    args.get(i)
                        .filter(|value| !value.starts_with("--") && !value.is_empty())
                        .ok_or_else(|| format!("{option} requires a value"))?
                        .clone(),
                );
            }
            "--out" => return Err(
                "map uses --output FILE; --out selects generated code for check/lower/build/run"
                    .into(),
            ),
            "--cost-report" => return Err(
                "map does not produce a cost report; use --cost-report with check/lower/build/run"
                    .into(),
            ),
            "--editor-input" => {
                return Err("--editor-input is supported only by check/symbols/assist".into())
            }
            "--serve" => return Err("map --serve is not implemented".into()),
            _ => input.push(args[i].clone()),
        }
        i += 1;
    }
    let format = format.unwrap_or(MapFormat::Mermaid);
    let image = matches!(format, MapFormat::Svg | MapFormat::Png);
    if image && output.is_none() {
        return Err("map --format svg/png requires --output FILE".into());
    }
    if layout.is_some() && !image {
        return Err("--layout is supported only with --format svg/png".into());
    }
    let mut options = resolve(&input, cwd)?;
    if options.sql_schema.is_some() {
        return Err("SQL options are supported only by check".into());
    }
    options.command = "map".into();
    Ok((
        options,
        MapOptions {
            view,
            format,
            module,
            focus,
            depth,
            output,
            layout: layout.unwrap_or(MapLayout::Elk),
        },
    ))
}

pub fn discover(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|p| p.join("nagi.toml"))
        .find(|p| p.is_file())
}

fn dependency_name(name: &str) -> Result<(), String> {
    if name.replace('_', "-") == "nagi-runtime"
        || name.is_empty()
        || !name.chars().enumerate().all(|(i, c)| {
            c == '_' || c.is_ascii_alphabetic() || i > 0 && (c.is_ascii_digit() || c == '-')
        })
    {
        return Err(format!("Rust依存の名前が不正または予約名です: {name}"));
    }
    Ok(())
}

fn dependency(name: &str, dependency: &RustDependency) -> Result<(), String> {
    dependency_name(name)?;
    let (version, path, features, package) = match dependency {
        RustDependency::Version(version) => (Some(version), None, &[][..], None),
        RustDependency::Detailed(table) => (
            table.version.as_ref(),
            table.path.as_ref(),
            table.features.as_slice(),
            table.package.as_ref(),
        ),
    };
    if version.is_none() && path.is_none() {
        return Err(format!("Rust依存 {name} にはversionまたはpathが必要です"));
    }
    if version.is_some_and(|version| version.trim().is_empty())
        || path.is_some_and(|path| {
            path.as_os_str().is_empty() || path.to_str().is_some_and(|path| path.trim().is_empty())
        })
        || features.iter().any(|feature| feature.trim().is_empty())
    {
        return Err(format!(
            "Rust依存 {name} のversion/path/featuresには空でない値を指定してください"
        ));
    }
    if let Some(package) = package {
        dependency_name(package).map_err(|error| format!("Rust依存 {name} のpackage: {error}"))?;
    }
    Ok(())
}

fn validate_dependencies(deps: &BTreeMap<String, RustDependency>) -> Result<(), String> {
    let mut names = std::collections::BTreeSet::new();
    for (name, config) in deps {
        dependency(name, config)?;
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
    if !["check", "lower", "build", "run", "symbols", "assist"].contains(&command.as_str()) {
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
    let mut rust_diagnostics = false;
    let mut editor_input = false;
    let mut sql_schema = None;
    let mut sql_dialect = None;
    let mut i = 1;
    while i < args.len() {
        let option = &args[i];
        match option.as_str() {
            "--project" | "--native" | "--rust" | "--rust-dep" | "--out" | "--sql-schema"
            | "--sql-dialect" => {
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
                    "--sql-schema" => {
                        if sql_schema.replace(cwd.join(value)).is_some() {
                            return Err("--sql-schema must be specified once".into());
                        }
                    }
                    "--sql-dialect" => {
                        if sql_dialect.replace(value.clone()).is_some() {
                            return Err("--sql-dialect must be specified once".into());
                        }
                    }
                    _ => {
                        let (name, version) = value
                            .split_once('=')
                            .ok_or("--rust-dep requires NAME=VERSION")?;
                        let config = RustDependency::Version(version.to_owned());
                        dependency(name, &config)?;
                        if dependencies.insert(name.to_owned(), config).is_some() {
                            return Err(format!("Rust依存が重複しています: {name}"));
                        }
                    }
                }
            }
            "--no-project" => no_project = true,
            "--cost-report" => cost = true,
            "--rust-diagnostics" => rust_diagnostics = true,
            "--editor-input" => {
                if !matches!(command.as_str(), "symbols" | "check" | "assist") || editor_input {
                    return Err("--editor-inputはcheck/symbols/assistに1回だけ指定できます".into());
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
    if rust_diagnostics && !matches!(command.as_str(), "build" | "run") {
        return Err("--rust-diagnostics is supported only by build/run".into());
    }
    if sql_schema.is_some() || sql_dialect.is_some() {
        if command != "check" || editor_input {
            return Err("SQL options are supported only by check without --editor-input".into());
        }
        if sql_schema.is_none() || sql_dialect.is_none() {
            return Err("--sql-schema and --sql-dialect sqlite must be specified together".into());
        }
        if sql_dialect.as_deref() != Some("sqlite") {
            return Err("unsupported SQL dialect; use --sql-dialect sqlite".into());
        }
        if !cfg!(feature = "sql-check") {
            return Err("This compiler was built without the sql-check feature; use an official compiler or rebuild with --features sql-check".into());
        }
    }
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
    let mut manifest_path = None;
    let mut rust_dependencies = BTreeMap::new();
    if let Some(manifest) = manifest {
        let manifest = fs::canonicalize(&manifest).map_err(|e| manifest_error(&manifest, 1, e))?;
        let config = read_manifest(&manifest)?;
        manifest_path = Some(manifest.clone());
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
    // Resolve only dependencies that remain after complete CLI replacement.
    // Local crates need not exist until Cargo builds the generated manifest.
    if let Some(root) = &project_root {
        for dependency in rust_dependencies.values_mut() {
            if let RustDependency::Detailed(table) = dependency {
                if let Some(path) = &mut table.path {
                    *path = root.join(&*path);
                }
            }
        }
    }
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
        rust_diagnostics,
        editor_input,
        sql_schema,
        project_root,
        manifest_path,
    })
}
