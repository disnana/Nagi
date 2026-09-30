use crate::ast::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;

struct SourceFile {
    path: PathBuf,
    source: String,
    start: usize,
    lines: usize,
}
#[derive(Default)]
pub struct Sources {
    pub program: Program,
    pub text: String,
    files: Vec<SourceFile>,
    bytes: usize,
}

pub fn diagnostic(path: &Path, source: &str, message: &str) -> String {
    let line = message
        .strip_prefix("line ")
        .and_then(|s| s.split(':').next())
        .and_then(|s| s.parse::<usize>().ok());
    if let Some(line) = line {
        format!("error: {message}\n --> {}:{line}\n {line} | {}\n help: 型注釈、所有権、scope、明示copyを確認してください", path.display(), source.lines().nth(line.saturating_sub(1)).unwrap_or(""))
    } else {
        format!("error: {message}\n --> {}", path.display())
    }
}

impl Sources {
    pub fn files(&self) -> impl Iterator<Item = (&Path, &str, usize)> {
        self.files
            .iter()
            .map(|f| (f.path.as_path(), f.source.as_str(), f.start))
    }
    pub fn append(&mut self, mut other: Sources) -> Program {
        let offset = self.text.lines().count();
        shift(&mut other.program, offset);
        for file in &mut other.files {
            file.start += offset;
        }
        self.files.extend(other.files);
        self.text.push_str(&other.text);
        self.bytes += other.bytes;
        other.program
    }
    pub fn diagnostic(&self, message: &str) -> String {
        let line = message
            .strip_prefix("line ")
            .and_then(|s| s.split(':').next())
            .and_then(|s| s.parse::<usize>().ok());
        if let Some(line) = line {
            if let Some(file) = self
                .files
                .iter()
                .find(|f| line >= f.start && line < f.start + f.lines)
            {
                let local = line - file.start + 1;
                return diagnostic(
                    &file.path,
                    &file.source,
                    &message.replacen(&format!("line {line}:"), &format!("line {local}:"), 1),
                );
            }
        }
        message.to_owned()
    }
}

pub fn load(path: &Path, high: bool) -> Result<Sources, String> {
    load_with_overlays(path, high, &HashMap::new())
}

/// Overlays use canonical existing file paths and never alter files on disk.
pub fn load_with_overlays(
    path: &Path,
    high: bool,
    overlays: &HashMap<PathBuf, String>,
) -> Result<Sources, String> {
    fn visit(
        path: &Path,
        high: bool,
        out: &mut Sources,
        stack: &mut HashSet<PathBuf>,
        seen: &mut HashSet<PathBuf>,
        overlays: &HashMap<PathBuf, String>,
    ) -> Result<(), String> {
        let path = std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if stack.contains(&path) {
            return Err(format!("循環import: {}", path.display()));
        }
        if seen.contains(&path) {
            return Ok(());
        }
        if stack.len() >= 64 || seen.len() + stack.len() >= 128 {
            return Err("importの深さまたはファイル数の上限を超えました".into());
        }
        let source = if let Some(text) = overlays.get(&path) {
            text.clone()
        } else {
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?
        };
        out.bytes += source.len();
        if out.bytes > 8_000_000 {
            return Err("importを含むソースの合計は8 MBまでです".into());
        }
        let mut program =
            crate::parser::parse(&source, high).map_err(|e| diagnostic(&path, &source, &e))?;
        stack.insert(path.clone());
        for (file, line) in &program.imports {
            let dependency = Path::new(file);
            let expected = if high { "nagi" } else { "low" };
            if dependency.is_absolute()
                || dependency.extension().and_then(|s| s.to_str()) != Some(expected)
            {
                return Err(diagnostic(
                    &path,
                    &source,
                    &format!(
                        "line {line}: importは相対パスの.{expected}ファイルを指定してください"
                    ),
                ));
            }
            let dependency = path.parent().unwrap().join(dependency);
            visit(&dependency, high, out, stack, seen, overlays).map_err(|e| {
                if e.starts_with("error:") {
                    e
                } else {
                    diagnostic(&path, &source, &format!("line {line}: {e}"))
                }
            })?;
        }
        resolve_assets(&mut program, &path).map_err(|e| diagnostic(&path, &source, &e))?;
        let offset = out.text.lines().count();
        shift(&mut program, offset);
        program.imports.clear();
        out.program.classes.extend(program.classes);
        out.program.functions.extend(program.functions);
        out.text.push_str(&source);
        if !source.ends_with('\n') {
            out.text.push('\n');
        }
        out.files.push(SourceFile {
            path: path.clone(),
            lines: source.lines().count().max(1),
            start: offset + 1,
            source,
        });
        stack.remove(&path);
        seen.insert(path);
        Ok(())
    }
    let mut out = Sources::default();
    visit(
        path,
        high,
        &mut out,
        &mut HashSet::new(),
        &mut HashSet::new(),
        overlays,
    )?;
    Ok(out)
}

fn shift(program: &mut Program, offset: usize) {
    fn expr(e: &mut Expr, offset: usize) {
        e.line += offset;
        match &mut e.kind {
            E::Call(_, _, args) | E::List(args) => {
                for arg in args {
                    expr(arg, offset);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expr(a, offset);
                expr(b, offset);
            }
            E::Unary(_, a) | E::Try(a) | E::Await(a) | E::Field(a, _) => expr(a, offset),
            E::Record(_, fields) => {
                for (_, value) in fields {
                    expr(value, offset);
                }
            }
            _ => {}
        }
    }
    fn block(stmts: &mut [Stmt], offset: usize) {
        for stmt in stmts {
            stmt.line += offset;
            match &mut stmt.kind {
                S::Assign { value, .. }
                | S::Expr(value)
                | S::Spawn(value)
                | S::Return(Some(value)) => expr(value, offset),
                S::If(condition, yes, no) => {
                    expr(condition, offset);
                    block(yes, offset);
                    block(no, offset);
                }
                S::While(condition, body) | S::For(_, condition, body) => {
                    expr(condition, offset);
                    block(body, offset);
                }
                S::Scope(body) => block(body, offset),
                S::Match(value, arms) => {
                    expr(value, offset);
                    for arm in arms {
                        arm.line += offset;
                        block(&mut arm.body, offset);
                    }
                }
                S::Return(None) => {}
            }
        }
    }
    for class in &mut program.classes {
        class.line += offset;
    }
    for function in &mut program.functions {
        function.line += offset;
        block(&mut function.body, offset);
    }
}

pub fn resolve_assets(program: &mut Program, source: &Path) -> Result<(), String> {
    fn expr(e: &mut Expr, source: &Path) -> Result<(), String> {
        match &mut e.kind {
            E::Call(name, _, args) => {
                if name == "include_text" && args.len() == 1 {
                    if let E::Str(file) = &mut args[0].kind {
                        let path = source.parent().unwrap_or(Path::new(".")).join(&*file);
                        let resolved = std::fs::canonicalize(&path).map_err(|err| {
                            format!("line {}: include_text {}: {err}", e.line, path.display())
                        })?;
                        std::fs::read_to_string(&resolved).map_err(|err| {
                            format!("line {}: include_text {}: {err}", e.line, path.display())
                        })?;
                        *file = resolved.display().to_string();
                    }
                }
                for arg in args {
                    expr(arg, source)?;
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expr(a, source)?;
                expr(b, source)?;
            }
            E::Unary(_, a) | E::Try(a) | E::Await(a) | E::Field(a, _) => expr(a, source)?,
            E::Record(_, fields) => {
                for (_, value) in fields {
                    expr(value, source)?;
                }
            }
            E::List(values) => {
                for value in values {
                    expr(value, source)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn block(stmts: &mut [Stmt], source: &Path) -> Result<(), String> {
        for stmt in stmts {
            match &mut stmt.kind {
                S::Assign { value, .. }
                | S::Expr(value)
                | S::Spawn(value)
                | S::Return(Some(value)) => expr(value, source)?,
                S::If(condition, yes, no) => {
                    expr(condition, source)?;
                    block(yes, source)?;
                    block(no, source)?;
                }
                S::While(condition, body) | S::For(_, condition, body) => {
                    expr(condition, source)?;
                    block(body, source)?;
                }
                S::Scope(body) => block(body, source)?,
                S::Match(value, arms) => {
                    expr(value, source)?;
                    for arm in arms {
                        block(&mut arm.body, source)?;
                    }
                }
                S::Return(None) => {}
            }
        }
        Ok(())
    }
    for function in &mut program.functions {
        block(&mut function.body, source)?;
    }
    Ok(())
}
