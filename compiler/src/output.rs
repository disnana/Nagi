//! Generated files must never replace an input through another path name.

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

struct ExistingFile {
    canonical: PathBuf,
    identity: (u64, u64),
    // Keep the handle used for identity lookup alive through the comparison.
    _file: File,
}

impl ExistingFile {
    fn open(path: &Path) -> std::io::Result<Self> {
        let canonical = fs::canonicalize(path)?;
        let file = File::open(&canonical)?;
        let identity = identity(&file)?;
        Ok(Self {
            canonical,
            identity,
            _file: file,
        })
    }
}

pub(crate) fn protect(outputs: &[PathBuf], inputs: &[PathBuf], kind: &str) -> Result<(), String> {
    let mut existing: Vec<(&PathBuf, ExistingFile)> = Vec::new();
    for output in outputs {
        match ExistingFile::open(output) {
            Ok(file) => {
                if let Some((other, _)) = existing.iter().find(|(_, other)| {
                    other.canonical == file.canonical || other.identity == file.identity
                }) {
                    return Err(format!(
                        "{kind} output files must be distinct: {} and {}",
                        other.display(),
                        output.display()
                    ));
                }
                existing.push((output, file));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if fs::symlink_metadata(output)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                {
                    return Err(format!(
                        "Cannot resolve output symlink: {}",
                        output.display()
                    ));
                }
            }
            Err(error) => {
                return Err(format!(
                    "Cannot access {} output {}: {error}",
                    kind.to_ascii_lowercase(),
                    output.display()
                ));
            }
        }
    }
    if existing.is_empty() {
        return Ok(());
    }
    for input in inputs {
        let file = ExistingFile::open(input)
            .map_err(|error| format!("Cannot access input {}: {error}", input.display()))?;
        if existing.iter().any(|(_, output)| {
            output.canonical == file.canonical || output.identity == file.identity
        }) {
            return Err(format!(
                "{kind} output cannot overwrite input file: {}",
                input.display()
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn identity(file: &File) -> std::io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn identity(file: &File) -> std::io::Result<(u64, u64)> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};

    #[repr(C)]
    #[derive(Default)]
    struct FileInformation {
        attributes: u32,
        creation_time: [u32; 2],
        access_time: [u32; 2],
        write_time: [u32; 2],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "GetFileInformationByHandle"]
        fn file_information_by_handle(
            handle: *mut c_void,
            information: *mut FileInformation,
        ) -> i32;
    }
    let mut information = FileInformation::default();
    // The handle belongs to a live File and the output matches the Win32 ABI.
    if unsafe { file_information_by_handle(file.as_raw_handle(), &mut information) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((
        u64::from(information.volume),
        (u64::from(information.index_high) << 32) | u64::from(information.index_low),
    ))
}

#[cfg(not(any(unix, windows)))]
fn identity(_file: &File) -> std::io::Result<(u64, u64)> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "file identity is unavailable on this platform",
    ))
}

pub(crate) fn assets(program: &crate::ast::Program) -> Vec<PathBuf> {
    use crate::ast::{Expr, NameResolution, Stmt, E, S};
    fn expr(expression: &Expr, assets: &mut Vec<PathBuf>) {
        match &expression.kind {
            E::Call(name, _, args) => {
                if name == "include_text"
                    && matches!(expression.resolution, None | Some(NameResolution::Builtin))
                    && args.len() == 1
                {
                    if let E::Str(path) = &args[0].kind {
                        if Path::new(path).is_absolute() {
                            assets.push(PathBuf::from(path));
                        }
                    }
                }
                for arg in args {
                    expr(arg, assets);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expr(a, assets);
                expr(b, assets);
            }
            E::Unary(_, a) | E::Try(a) | E::Await(a) | E::Field(a, _) => expr(a, assets),
            E::Record(_, fields) => {
                for (_, value) in fields {
                    expr(value, assets);
                }
            }
            E::List(values) => {
                for value in values {
                    expr(value, assets);
                }
            }
            _ => {}
        }
    }
    fn block(statements: &[Stmt], assets: &mut Vec<PathBuf>) {
        for statement in statements {
            match &statement.kind {
                S::Assign { value, .. }
                | S::SpawnBind { value, .. }
                | S::Expr(value)
                | S::Spawn(value)
                | S::Return(Some(value)) => expr(value, assets),
                S::If(condition, yes, no) => {
                    expr(condition, assets);
                    block(yes, assets);
                    block(no, assets);
                }
                S::While(condition, body) | S::For(_, condition, body) => {
                    expr(condition, assets);
                    block(body, assets);
                }
                S::Scope(body) => block(body, assets),
                S::Match(value, arms) => {
                    expr(value, assets);
                    for arm in arms {
                        block(&arm.body, assets);
                    }
                }
                S::Return(None) => {}
            }
        }
    }
    let mut assets = Vec::new();
    for function in &program.functions {
        block(&function.body, &mut assets);
    }
    assets.sort();
    assets.dedup();
    assets
}
