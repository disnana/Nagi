use crate::ast::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;

struct SourceFile {
    path: PathBuf,
    module: ModuleId,
    source: String,
    start: usize,
    lines: usize,
    tokens: Vec<crate::lexer::Token>,
}
#[derive(Default)]
pub struct Sources {
    pub program: Program,
    pub text: String,
    files: Vec<SourceFile>,
    bytes: usize,
    identifiers: HashMap<String, String>,
}

pub struct Location<'a> {
    pub path: &'a Path,
    pub line: usize,
    pub text: &'a str,
}

pub fn diagnostic(path: &Path, source: &str, message: &str) -> String {
    let line = message
        .strip_prefix("line ")
        .and_then(|s| s.split(':').next())
        .and_then(|s| s.parse::<usize>().ok());
    if let Some(line) = line {
        format!(
            "error: {message}\n --> {}:{line}\n {line} | {}",
            path.display(),
            source.lines().nth(line.saturating_sub(1)).unwrap_or("")
        )
    } else {
        format!("error: {message}\n --> {}", path.display())
    }
}

impl Sources {
    pub fn location(&self, line: usize) -> Option<Location<'_>> {
        let file = self
            .files
            .iter()
            .find(|f| line >= f.start && line < f.start + f.lines)?;
        let local = line - file.start + 1;
        Some(Location {
            path: &file.path,
            line: local,
            text: file.source.lines().nth(local - 1).unwrap_or(""),
        })
    }
    pub fn files(&self) -> impl Iterator<Item = (&Path, &str, usize)> {
        self.files
            .iter()
            .map(|f| (f.path.as_path(), f.source.as_str(), f.start))
    }
    /// A serialized root can retain its logical ID in a new physical Low file.
    /// Foreign flattened identities do not become loaded source files.
    pub fn module_files(&self) -> impl Iterator<Item = (&ModuleId, &Path)> {
        self.files
            .iter()
            .map(|file| (&file.module, file.path.as_path()))
    }
    pub fn append(&mut self, mut other: Sources) -> Program {
        let offset = self.text.lines().count();
        shift(&mut other.program, offset);
        for definition in &mut other.program.modules.definitions {
            definition.line += offset;
        }
        for binding in &mut other.program.modules.bindings {
            binding.line += offset;
        }
        for reference in &mut other.program.modules.references {
            reference.line += offset;
        }
        for file in &mut other.files {
            file.start += offset;
        }
        self.files.extend(other.files);
        self.identifiers.extend(other.identifiers);
        self.text.push_str(&other.text);
        self.bytes += other.bytes;
        other.program
    }
    pub fn diagnostic(&self, message: &str) -> String {
        // Replace complete diagnostic identifiers in one pass. Inserted user
        // spellings are never interpreted as another compiler-generated name.
        let mut readable = String::new();
        let mut start = 0;
        while start < message.len() {
            let ch = message[start..].chars().next().expect("character boundary");
            if ch.is_ascii_alphanumeric() || ch == '_' {
                let mut end = start + 1;
                while end < message.len()
                    && (message.as_bytes()[end].is_ascii_alphanumeric()
                        || message.as_bytes()[end] == b'_')
                {
                    end += 1;
                }
                let token = &message[start..end];
                readable.push_str(
                    self.identifiers
                        .get(token)
                        .map(String::as_str)
                        .unwrap_or(token),
                );
                start = end;
            } else {
                readable.push(ch);
                start += ch.len_utf8();
            }
        }
        let message = readable.as_str();
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

    fn collect_identifiers(&mut self) {
        fn binding(
            files: &[SourceFile],
            names: &mut HashMap<String, String>,
            line: usize,
            span: Span,
            resolved: &str,
        ) {
            if span.end != span.start + 1 {
                return;
            }
            let Some(file) = files
                .iter()
                .find(|f| line >= f.start && line < f.start + f.lines)
            else {
                return;
            };
            let Some(crate::lexer::Token {
                kind: crate::lexer::K::Id(original),
                ..
            }) = file.tokens.get(span.start)
            else {
                return;
            };
            if original != resolved {
                names.insert(resolved.to_owned(), original.clone());
            }
        }
        fn block(files: &[SourceFile], names: &mut HashMap<String, String>, stmts: &[Stmt]) {
            for stmt in stmts {
                match &stmt.kind {
                    S::Assign { name, .. } => {
                        if let Some(span) = stmt.binding_span {
                            binding(files, names, stmt.line, span, name);
                        }
                    }
                    S::For(name, _, body) => {
                        if let Some(span) = stmt.binding_span {
                            binding(files, names, stmt.line, span, name);
                        }
                        block(files, names, body);
                    }
                    S::If(_, yes, no) => {
                        block(files, names, yes);
                        block(files, names, no);
                    }
                    S::While(_, body) | S::Scope(body) => block(files, names, body),
                    S::Match(_, arms) => {
                        for arm in arms {
                            for item in arm.pattern.bindings() {
                                if let Some(name) = &item.name {
                                    binding(files, names, arm.line, item.span, name);
                                }
                            }
                            block(files, names, &arm.body);
                        }
                    }
                    _ => {}
                }
            }
        }
        for def in &self.program.modules.definitions {
            self.identifiers.insert(
                def.symbol.clone(),
                crate::modules::display_symbol(&def.symbol),
            );
            let canonical = crate::modules::symbol(&def.id);
            self.identifiers.insert(
                canonical.clone(),
                crate::modules::display_symbol(&canonical),
            );
        }
        for function in &self.program.functions {
            for ((name, _), span) in function.params.iter().zip(&function.parameter_spans) {
                binding(
                    &self.files,
                    &mut self.identifiers,
                    function.line,
                    *span,
                    name,
                );
            }
            block(&self.files, &mut self.identifiers, &function.body);
        }
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
        units: &mut Vec<crate::modules::ModuleUnit>,
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
        let imports = if program.module_imports.is_empty() {
            program
                .imports
                .iter()
                .map(|(path, line)| ModuleImport {
                    path: path.clone(),
                    line: *line,
                    span: Span::default(),
                    kind: ImportKind::Flat,
                })
                .collect::<Vec<_>>()
        } else {
            program.module_imports.clone()
        };
        let mut resolved_imports = vec![];
        for import in imports {
            let file = &import.path;
            let line = &import.line;
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
            let canonical = std::fs::canonicalize(&dependency).map_err(|e| {
                diagnostic(
                    &path,
                    &source,
                    &format!("line {line}: {}: {e}", dependency.display()),
                )
            })?;
            visit(units, &canonical, high, out, stack, seen, overlays).map_err(|e| {
                if e.starts_with("error:") {
                    e
                } else {
                    diagnostic(&path, &source, &format!("line {line}: {e}"))
                }
            })?;
            resolved_imports.push((import, ModuleId(canonical.display().to_string())));
        }
        let offset = out.text.lines().count();
        shift(&mut program, offset);
        for (import, _) in &mut resolved_imports {
            import.line += offset;
        }
        program.imports.clear();
        program.module_imports.clear();
        let tokens =
            crate::lexer::lex(&source, high).map_err(|e| diagnostic(&path, &source, &e))?;
        let module = program
            .modules
            .root
            .clone()
            .unwrap_or_else(|| ModuleId(path.display().to_string()));
        units.push(crate::modules::ModuleUnit {
            id: ModuleId(path.display().to_string()),
            program,
            imports: resolved_imports,
            tokens,
            offset,
        });
        out.text.push_str(&source);
        if !source.ends_with('\n') {
            out.text.push('\n');
        }
        out.files.push(SourceFile {
            path: path.clone(),
            module,
            lines: source.lines().count().max(1),
            start: offset + 1,
            source,
            tokens: units.last().expect("source unit pushed").tokens.clone(),
        });
        stack.remove(&path);
        seen.insert(path);
        Ok(())
    }
    let mut out = Sources::default();
    let mut units = vec![];
    visit(
        &mut units,
        path,
        high,
        &mut out,
        &mut HashSet::new(),
        &mut HashSet::new(),
        overlays,
    )?;
    let root = units
        .last()
        .and_then(|unit| unit.program.modules.root.clone())
        .unwrap_or_else(|| units.last().expect("root source loaded").id.clone());
    out.program = crate::modules::resolve(units, root).map_err(|e| out.diagnostic(&e))?;
    out.collect_identifiers();
    Ok(out)
}

fn shift(program: &mut Program, offset: usize) {
    map_lines(program, |line| line + offset);
}

/// Change diagnostic lines without changing file-local token ranges.
pub fn map_lines(program: &mut Program, map: impl Fn(usize) -> usize) {
    fn expr(e: &mut Expr, map: &impl Fn(usize) -> usize) {
        e.line = map(e.line);
        match &mut e.kind {
            E::Call(_, _, args) | E::List(args) => {
                for arg in args {
                    expr(arg, map);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expr(a, map);
                expr(b, map);
            }
            E::Unary(_, a) | E::Try(a) | E::Await(a) | E::Field(a, _) => expr(a, map),
            E::Record(_, fields) => {
                for (_, value) in fields {
                    expr(value, map);
                }
            }
            _ => {}
        }
    }
    fn block(stmts: &mut [Stmt], map: &impl Fn(usize) -> usize) {
        for stmt in stmts {
            stmt.line = map(stmt.line);
            match &mut stmt.kind {
                S::Assign { value, .. }
                | S::Expr(value)
                | S::Spawn(value)
                | S::Return(Some(value)) => expr(value, map),
                S::If(condition, yes, no) => {
                    expr(condition, map);
                    block(yes, map);
                    block(no, map);
                }
                S::While(condition, body) | S::For(_, condition, body) => {
                    expr(condition, map);
                    block(body, map);
                }
                S::Scope(body) => block(body, map),
                S::Match(value, arms) => {
                    expr(value, map);
                    for arm in arms {
                        arm.line = map(arm.line);
                        block(&mut arm.body, map);
                    }
                }
                S::Return(None) => {}
            }
        }
    }
    for class in &mut program.classes {
        class.line = map(class.line);
        for line in &mut class.field_lines {
            *line = map(*line);
        }
    }
    for enumeration in &mut program.enums {
        enumeration.line = map(enumeration.line);
        for variant in &mut enumeration.variants {
            variant.line = map(variant.line);
            for line in &mut variant.field_lines {
                *line = map(*line);
            }
        }
    }
    for function in &mut program.functions {
        function.line = map(function.line);
        block(&mut function.body, &map);
    }
}

pub fn resolve_assets(program: &mut Program, source: &Path) -> Result<(), String> {
    fn expr(e: &mut Expr, source: &Path) -> Result<(), String> {
        match &mut e.kind {
            E::Call(name, _, args) => {
                if name == "include_text"
                    && args.len() == 1
                    && e.resolution != Some(NameResolution::Module)
                {
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
