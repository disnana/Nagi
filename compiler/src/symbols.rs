//! Definition locations for the files and AST actually loaded by the compiler.
use crate::{
    ast::*,
    lexer::{lex, Token, K},
    source::Sources,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
struct Location {
    file: String,
    line: usize,
    column: usize,
    length: usize,
}
#[derive(Clone, Serialize)]
struct Definition {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<DefId>,
    name: String,
    kind: &'static str,
    location: Location,
    signature: String,
    parameters: Vec<Member>,
    fields: Vec<Member>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    variants: Vec<Variant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_type: Option<String>,
    asynchronous: bool,
}
#[derive(Clone, Serialize)]
struct Variant {
    name: String,
    kind: &'static str,
    location: Location,
    signature: String,
    parameters: Vec<Member>,
    return_type: String,
}
#[derive(Clone, Serialize)]
struct Member {
    name: String,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Serialize)]
struct Local {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    location: Location,
}

#[derive(Serialize)]
struct TypedExpression {
    location: Location,
    end_line: usize,
    end_column: usize,
    #[serde(rename = "type")]
    ty: String,
    fields: Vec<Member>,
}

pub fn read_overlays(input: impl Read, cwd: &Path) -> Result<HashMap<PathBuf, String>, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        files: Vec<Buffer>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Buffer {
        file: PathBuf,
        text: String,
    }
    let mut data = vec![];
    input
        .take(16_000_001)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > 16_000_000 {
        return Err("editor inputは16 MBまでです".into());
    }
    let input: Input = serde_json::from_slice(&data).map_err(|e| format!("editor input: {e}"))?;
    if input.files.len() > 128
        || input.files.iter().map(|f| f.text.len()).sum::<usize>() > 8_000_000
    {
        return Err("editor buffersは128ファイル、合計8 MBまでです".into());
    }
    let mut overlays = HashMap::new();
    for buffer in input.files {
        if !matches!(
            buffer.file.extension().and_then(|x| x.to_str()),
            Some("nagi" | "low")
        ) {
            return Err("editor bufferには.nagi / .lowを指定してください".into());
        }
        let path = std::fs::canonicalize(cwd.join(buffer.file)).map_err(|e| e.to_string())?;
        if overlays.insert(path, buffer.text).is_some() {
            return Err("editor bufferのパスが重複しています".into());
        }
    }
    Ok(overlays)
}
#[derive(Serialize)]
struct Reference {
    location: Location,
    target: Location,
}

struct File<'a> {
    path: &'a Path,
    module: ModuleId,
    lines: Vec<&'a str>,
    start: usize,
    tokens: Vec<Token>,
}
impl File<'_> {
    fn token_length(&self, token: &Token) -> usize {
        match &token.kind {
            K::Id(n) | K::Sym(n) => n.encode_utf16().count(),
            K::Str(_) => self.string_length(token),
            K::Num(_) => self.lines[token.line - 1]
                .chars()
                .skip(token.col - 1)
                .take_while(|c| c.is_ascii_digit() || *c == '_' || *c == '.')
                .count(),
            _ => 0,
        }
    }
    fn string_length(&self, token: &Token) -> usize {
        let mut chars = self.lines[token.line - 1].chars().skip(token.col - 1);
        let quote = chars.next().unwrap();
        let mut length = 1;
        let mut escaped = false;
        for c in chars {
            length += c.len_utf16();
            if c == quote && !escaped {
                break;
            }
            escaped = c == '\\' && !escaped;
        }
        length
    }
    fn location(&self, token: &Token, length: usize) -> Location {
        // VS Code uses UTF-16 columns; the lexer counts Unicode characters.
        let prefix = self.lines.get(token.line - 1).copied().unwrap_or("");
        let column = prefix
            .chars()
            .take(token.col - 1)
            .map(char::len_utf16)
            .sum::<usize>()
            + 1;
        Location {
            file: self.path.display().to_string(),
            line: token.line,
            column,
            length,
        }
    }
}

fn name_location(files: &[File<'_>], line: usize, span: Span, name: &str) -> Option<Location> {
    let file = files
        .iter()
        .find(|f| line >= f.start && line < f.start + f.lines.len().max(1))?;
    let token = file
        .tokens
        .get(span.start..span.end)?
        .iter()
        .find(|t| matches!(&t.kind, K::Id(n) if n == name))?;
    Some(file.location(token, name.encode_utf16().count()))
}

// Local identifiers may be renamed to avoid generated global symbols. Their
// parser span still identifies exactly one original source token.
fn local_location(files: &[File<'_>], line: usize, span: Span) -> Option<(String, Location)> {
    if span.end != span.start + 1 {
        return None;
    }
    let file = files
        .iter()
        .find(|f| line >= f.start && line < f.start + f.lines.len().max(1))?;
    let token = file.tokens.get(span.start)?;
    let K::Id(name) = &token.kind else {
        return None;
    };
    Some((
        name.clone(),
        file.location(token, name.encode_utf16().count()),
    ))
}

// Resolve lexical bindings independently of type/ownership checking. Navigation
// remains useful on a moved value or a binding whose initializer has a type error.
struct Bindings<'a, 'b> {
    files: &'a [File<'b>],
    vars: HashMap<String, Location>,
    functions: HashMap<String, Location>,
    references: Vec<Reference>,
}
impl Bindings<'_, '_> {
    fn binding(&mut self, line: usize, span: Span, name: &str) {
        if let Some((_, location)) = local_location(self.files, line, span) {
            self.vars.insert(name.into(), location.clone());
            self.references.push(Reference {
                target: location.clone(),
                location,
            });
        }
    }
    fn reference(&mut self, line: usize, span: Span, name: &str) {
        if let (Some(location), Some(target)) = (
            if self.vars.contains_key(name) {
                // A call's span includes its arguments; the callee remains the
                // first token, even when its resolved local name was renamed.
                local_location(
                    self.files,
                    line,
                    Span {
                        start: span.start,
                        end: span.start + 1,
                    },
                )
                .map(|(_, l)| l)
            } else {
                name_location(self.files, line, span, name)
            },
            self.vars.get(name).or_else(|| self.functions.get(name)),
        ) {
            self.references.push(Reference {
                location,
                target: target.clone(),
            });
        }
    }
    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            E::Name(name) => self.reference(e.line, e.span, name),
            E::Call(name, _, args) => {
                self.reference(e.line, e.span, name);
                for arg in args {
                    self.expr(arg);
                }
            }
            E::List(args) => {
                for arg in args {
                    self.expr(arg);
                }
            }
            E::Record(_, fields) => {
                for (_, value) in fields {
                    self.expr(value);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                self.expr(a);
                self.expr(b);
            }
            E::Unary(_, e) | E::Field(e, _) | E::Try(e) | E::Await(e) => self.expr(e),
            _ => {}
        }
    }
    fn child(&mut self, body: &[Stmt]) {
        let before = self.vars.clone();
        self.block(body);
        self.vars = before;
    }
    fn block(&mut self, ss: &[Stmt]) {
        for s in ss {
            match &s.kind {
                S::Assign {
                    name,
                    value,
                    declare,
                    ..
                } => {
                    // A use in the initializer resolves before the new binding.
                    self.expr(value);
                    if let Some(span) = s.binding_span {
                        if !self.vars.contains_key(name) {
                            self.binding(s.line, span, name);
                        } else if !declare {
                            self.reference(s.line, span, name);
                        }
                        // A duplicate `let` is invalid and cannot replace a binding.
                    }
                }
                S::Return(Some(value)) | S::Expr(value) | S::Spawn(value) => self.expr(value),
                S::If(value, a, b) => {
                    self.expr(value);
                    self.child(a);
                    self.child(b);
                }
                S::While(value, body) => {
                    self.expr(value);
                    self.child(body);
                }
                S::For(name, value, body) => {
                    self.expr(value);
                    let before = self.vars.clone();
                    if let Some(span) = s.binding_span {
                        self.binding(s.line, span, name);
                    }
                    self.block(body);
                    self.vars = before;
                }
                S::Match(value, arms) => {
                    self.expr(value);
                    let before = self.vars.clone();
                    for arm in arms {
                        self.vars = before.clone();
                        for binding in arm.pattern.bindings() {
                            if let Some(name) = &binding.name {
                                if self.vars.contains_key(name) {
                                    // Invalid shadowing does not imply a declaration.
                                    self.vars.remove(name);
                                } else {
                                    self.binding(arm.line, binding.span, name);
                                }
                            }
                        }
                        self.block(&arm.body);
                    }
                    self.vars = before;
                }
                S::Scope(body) => self.child(body),
                S::Return(None) => {}
            }
        }
    }
    fn function(&mut self, f: &Function) {
        self.vars.clear();
        for ((name, _), span) in f.params.iter().zip(&f.parameter_spans) {
            self.binding(f.line, *span, name);
        }
        self.block(&f.body);
    }
}

struct Types<'a, 'b> {
    files: &'a [File<'b>],
    classes: &'a [Class],
    metadata: &'a ModuleMetadata,
    locals: Vec<Local>,
    expressions: Vec<TypedExpression>,
}
impl Types<'_, '_> {
    fn file(&self, line: usize) -> Option<&File<'_>> {
        self.files
            .iter()
            .find(|f| line >= f.start && line < f.start + f.lines.len().max(1))
    }
    fn binding(&mut self, line: usize, span: Span, _name: &str, ty: &Type) {
        let Some((original, location)) = local_location(self.files, line, span) else {
            return;
        };
        let namespace = self
            .file(line)
            .map(|f| f.module.0.as_str())
            .unwrap_or(&location.file);
        let display = display_type(ty, namespace, self.metadata);
        self.locals.push(Local {
            name: original,
            ty: display,
            location,
        });
    }
    fn expr(&mut self, e: &Expr) {
        if let Some(ty) = &e.ty {
            if let E::Name(name) = &e.kind {
                if ty.0 != "fn" || e.resolution == Some(NameResolution::Local) {
                    self.binding(e.line, e.span, name, ty);
                }
            }
            if let Some(file) = self.file(e.line) {
                if let Some(tokens) = file
                    .tokens
                    .get(e.span.start..e.span.end)
                    .filter(|t| !t.is_empty())
                {
                    let first = &tokens[0];
                    let last = tokens.last().unwrap();
                    let location = file.location(first, file.token_length(first));
                    let end = file.location(last, file.token_length(last));
                    let fields = self
                        .classes
                        .iter()
                        .find(|c| c.name == ty.0 && ty.1.is_empty())
                        .map(|c| {
                            c.fields
                                .iter()
                                .map(|(name, ty)| Member {
                                    name: name.clone(),
                                    ty: display_type(ty, &file.module.0, self.metadata),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let display = display_type(ty, &file.module.0, self.metadata);
                    self.expressions.push(TypedExpression {
                        location,
                        end_line: end.line,
                        end_column: end.column + end.length,
                        ty: display,
                        fields,
                    });
                }
            }
        }
        match &e.kind {
            E::Call(_, _, args) | E::List(args) => {
                for arg in args {
                    self.expr(arg);
                }
            }
            E::Record(_, fields) => {
                for (_, value) in fields {
                    self.expr(value);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                self.expr(a);
                self.expr(b);
            }
            E::Unary(_, e) | E::Field(e, _) | E::Try(e) | E::Await(e) => self.expr(e),
            _ => {}
        }
    }
    fn block(&mut self, ss: &[Stmt]) {
        for s in ss {
            if let (Some(span), Some(ty)) = (s.binding_span, &s.binding_type) {
                if let S::Assign { name, .. } | S::For(name, _, _) = &s.kind {
                    self.binding(s.line, span, name, ty);
                }
            }
            match &s.kind {
                S::Assign { value, .. }
                | S::Return(Some(value))
                | S::Expr(value)
                | S::Spawn(value) => self.expr(value),
                S::If(value, a, b) => {
                    self.expr(value);
                    self.block(a);
                    self.block(b);
                }
                S::While(value, body) | S::For(_, value, body) => {
                    self.expr(value);
                    self.block(body);
                }
                S::Scope(body) => self.block(body),
                S::Match(value, arms) => {
                    self.expr(value);
                    for arm in arms {
                        for binding in arm.pattern.bindings() {
                            if let (Some(name), Some(ty)) = (&binding.name, &binding.ty) {
                                self.binding(arm.line, binding.span, name, ty);
                            }
                        }
                        self.block(&arm.body);
                    }
                }
                S::Return(None) => {}
            }
        }
    }
}

// Render types through the source file's namespace while retaining canonical
// class identity for inference and field lookup.
fn display_type(ty: &Type, file: &str, metadata: &ModuleMetadata) -> String {
    let definition = metadata.definitions.iter().find(|d| d.symbol == ty.0);
    let name = definition
        .map(|d| {
            let direct = metadata.bindings.iter().find(|b| {
                b.module.0 == file
                    && matches!(&b.target, BindingTarget::Definition(id) if id == &d.id)
            });
            if let Some(binding) = direct {
                return binding.name.clone();
            }
            let module = metadata.bindings.iter().find(|b| {
                b.module.0 == file
                    && matches!(&b.target, BindingTarget::Module(id) if id == &d.id.module)
            });
            module
                .map(|b| format!("{}.{}", b.name, d.id.name))
                .unwrap_or_else(|| d.id.name.clone())
        })
        .unwrap_or_else(|| ty.0.clone());
    if ty.0 == "Option" && ty.1.len() == 1 {
        return format!("{}?", display_type(&ty.inner(), file, metadata));
    }
    if ty.1.is_empty() {
        return name;
    }
    format!(
        "{}[{}]",
        name,
        ty.1.iter()
            .map(|t| display_type(t, file, metadata))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn variant_signature(parent: &str, name: &str, parameters: &[Member]) -> String {
    if parameters.is_empty() {
        return format!("{parent}.{name}");
    }
    format!(
        "{parent}.{name}({})",
        parameters
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn enum_signature(name: &str, variants: &[Variant]) -> String {
    format!(
        "enum {name}\n{}",
        variants
            .iter()
            .map(|v| format!("    {}", v.signature))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn reference_locations(
    files: &[File<'_>],
    line: usize,
    span: Span,
    spelling: &str,
) -> Vec<Location> {
    let Some(file) = files
        .iter()
        .find(|f| line >= f.start && line < f.start + f.lines.len().max(1))
    else {
        return vec![];
    };
    let Some(tokens) = file.tokens.get(span.start..span.end) else {
        return vec![];
    };
    let parts = spelling.split('.').collect::<Vec<_>>();
    let width = parts.len() * 2 - 1;
    if parts.is_empty() || width > tokens.len() {
        return vec![];
    }
    let Some(path) = tokens.windows(width).find(|tokens| {
        parts.iter().enumerate().all(|(i, part)| {
            matches!(&tokens[i * 2].kind, K::Id(name) if name == part)
                && (i == 0 || matches!(&tokens[i * 2 - 1].kind, K::Sym(dot) if dot == "."))
        })
    }) else {
        return vec![];
    };
    parts
        .iter()
        .enumerate()
        .map(|(i, part)| file.location(&path[i * 2], part.encode_utf16().count()))
        .collect()
}

pub fn index(sources: &Sources, programs: &[&Program]) -> Result<serde_json::Value, String> {
    let module_files = sources.module_files().collect::<HashMap<_, _>>();
    let files = sources
        .files()
        .map(|(path, text, start)| {
            Ok(File {
                path,
                module: module_files
                    .iter()
                    .find(|(_, file)| **file == path)
                    .map(|(module, _)| (*module).clone())
                    .unwrap_or_else(|| ModuleId(path.display().to_string())),
                lines: text.lines().collect(),
                start,
                tokens: lex(text, path.extension().is_none_or(|x| x != "low"))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut metadata = ModuleMetadata::default();
    for p in programs {
        metadata.modules.extend(p.modules.modules.clone());
        metadata.definitions.extend(p.modules.definitions.clone());
        metadata.bindings.extend(p.modules.bindings.clone());
        metadata.references.extend(p.modules.references.clone());
    }
    let mut definitions = vec![];
    let mut targets = HashMap::new();
    let mut symbols = HashMap::new();
    let mut variant_targets = HashMap::new();
    for p in programs {
        for (name, kind, line, keyword) in p
            .classes
            .iter()
            .map(|c| (&c.name, "class", c.line, "class"))
            .chain(p.enums.iter().map(|e| (&e.name, "enum", e.line, "enum")))
            .chain(
                p.functions
                    .iter()
                    .map(|f| (&f.name, "function", f.line, "def")),
            )
        {
            let info = metadata
                .definitions
                .iter()
                .find(|d| d.symbol == *name && d.line == line)
                .or_else(|| metadata.definitions.iter().find(|d| d.symbol == *name));
            let display_name = info.map(|d| d.id.name.as_str()).unwrap_or(name);
            let canonical_name = info.map(|d| crate::modules::symbol(&d.id));
            let Some(file) = files
                .iter()
                .find(|f| line >= f.start && line < f.start + f.lines.len().max(1))
            else {
                continue;
            };
            let local = line - file.start + 1;
            let low_keyword = match keyword {
                "def" => "fn",
                "class" => "record",
                _ => keyword,
            };
            let Some(pair) = file.tokens.windows(2).find(|pair| {
                pair[0].line == local
                    && matches!(&pair[0].kind, K::Id(n) if n == keyword || n == low_keyword)
                    && matches!(&pair[1].kind, K::Id(n) if n == display_name || n == name || canonical_name.as_ref() == Some(n))
            }) else {
                continue;
            };
            let location = file.location(&pair[1], file.token_length(&pair[1]));
            symbols
                .entry(name.clone())
                .or_insert_with(|| location.clone());
            if let Some(info) = info {
                targets
                    .entry(info.id.clone())
                    .or_insert_with(|| location.clone());
            }
            let mut definition = Definition {
                id: info.map(|d| d.id.clone()),
                name: display_name.into(),
                kind,
                location,
                signature: String::new(),
                parameters: vec![],
                fields: vec![],
                variants: vec![],
                return_type: None,
                asynchronous: false,
            };
            let members = |items: &[(String, Type)]| {
                items
                    .iter()
                    .map(|(name, ty)| Member {
                        name: name.clone(),
                        ty: display_type(ty, &file.module.0, &metadata),
                    })
                    .collect::<Vec<_>>()
            };
            if kind == "function" {
                let function = p
                    .functions
                    .iter()
                    .find(|f| f.name == *name && f.line == line)
                    .unwrap();
                definition.parameters = members(&function.params);
                definition.return_type =
                    Some(display_type(&function.ret, &file.module.0, &metadata));
                definition.asynchronous = function.asynchronous;
                definition.signature = format!(
                    "{}{}{} {}({}) -> {}",
                    if function.external { "extern " } else { "" },
                    if function.asynchronous { "async " } else { "" },
                    if file.path.extension().is_some_and(|x| x == "low") {
                        "fn"
                    } else {
                        "def"
                    },
                    display_name,
                    definition
                        .parameters
                        .iter()
                        .map(|m| format!("{}: {}", m.name, m.ty))
                        .collect::<Vec<_>>()
                        .join(", "),
                    definition.return_type.as_deref().unwrap()
                );
            } else if kind == "enum" {
                let enumeration = p
                    .enums
                    .iter()
                    .find(|e| e.name == *name && e.line == line)
                    .unwrap();
                for variant in &enumeration.variants {
                    let Some(location) =
                        name_location(&files, variant.line, variant.name_span, &variant.name)
                    else {
                        continue;
                    };
                    if let Some(info) = info {
                        variant_targets
                            .insert((info.id.clone(), variant.name.clone()), location.clone());
                    }
                    let parameters = members(&variant.fields);
                    let signature = variant_signature(display_name, &variant.name, &parameters);
                    definition.variants.push(Variant {
                        name: variant.name.clone(),
                        kind: "enum_member",
                        location,
                        signature,
                        parameters,
                        return_type: display_name.into(),
                    });
                }
                definition.signature = enum_signature(display_name, &definition.variants);
            } else {
                let class = p
                    .classes
                    .iter()
                    .find(|c| c.name == *name && c.line == line)
                    .unwrap();
                definition.fields = members(&class.fields);
                definition.signature = format!(
                    "class {display_name}\n{}",
                    definition
                        .fields
                        .iter()
                        .map(|m| format!("    {}: {}", m.name, m.ty))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            definitions.push(definition);
        }
    }
    let render_alias = |definition: &Definition, name: &str, file: &str| {
        let mut alias = definition.clone();
        alias.name = name.into();
        let symbol = definition
            .id
            .as_ref()
            .and_then(|id| metadata.definitions.iter().find(|d| &d.id == id))
            .map(|d| &d.symbol);
        if let Some(symbol) = symbol {
            for program in programs {
                if let Some(function) = program.functions.iter().find(|f| &f.name == symbol) {
                    alias.parameters = function
                        .params
                        .iter()
                        .map(|(name, ty)| Member {
                            name: name.clone(),
                            ty: display_type(ty, file, &metadata),
                        })
                        .collect();
                    alias.return_type = Some(display_type(&function.ret, file, &metadata));
                    break;
                }
                if let Some(enumeration) = program.enums.iter().find(|e| &e.name == symbol) {
                    for variant in &mut alias.variants {
                        if let Some(source) =
                            enumeration.variants.iter().find(|v| v.name == variant.name)
                        {
                            variant.parameters = source
                                .fields
                                .iter()
                                .map(|(name, ty)| Member {
                                    name: name.clone(),
                                    ty: display_type(ty, file, &metadata),
                                })
                                .collect();
                        }
                        variant.signature =
                            variant_signature(name, &variant.name, &variant.parameters);
                        variant.return_type = name.into();
                    }
                    break;
                }
                if let Some(class) = program.classes.iter().find(|c| &c.name == symbol) {
                    alias.fields = class
                        .fields
                        .iter()
                        .map(|(name, ty)| Member {
                            name: name.clone(),
                            ty: display_type(ty, file, &metadata),
                        })
                        .collect();
                    break;
                }
            }
        }
        alias.signature = if alias.kind == "enum" {
            enum_signature(&alias.name, &alias.variants)
        } else if alias.kind == "class" {
            format!(
                "class {}\n{}",
                alias.name,
                alias
                    .fields
                    .iter()
                    .map(|m| format!("    {}: {}", m.name, m.ty))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            format!(
                "{}def {}({}) -> {}",
                if alias.asynchronous { "async " } else { "" },
                alias.name,
                alias
                    .parameters
                    .iter()
                    .map(|m| format!("{}: {}", m.name, m.ty))
                    .collect::<Vec<_>>()
                    .join(", "),
                alias.return_type.as_deref().unwrap_or("unit")
            )
        };
        alias
    };
    let mut references = vec![];
    let mut namespace_bindings = vec![];
    for binding in &metadata.bindings {
        let location = name_location(&files, binding.line, binding.span, &binding.name);
        let scope_file = module_files
            .get(&binding.module)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| binding.module.0.clone());
        match &binding.target {
            BindingTarget::Definition(id) => {
                let Some(definition) = definitions.iter().find(|d| d.id.as_ref() == Some(id))
                else {
                    continue;
                };
                let Some(target) = targets.get(id) else {
                    continue;
                };
                if let Some(location) = &location {
                    references.push(Reference {
                        location: location.clone(),
                        target: target.clone(),
                    });
                }
                let alias = render_alias(definition, &binding.name, &binding.module.0);
                namespace_bindings.push(
                    serde_json::json!({"file":scope_file,"name":binding.name,"kind":alias.kind,
                    "location":location,"target":target,"definition_id":id,"definition":alias}),
                );
            }
            BindingTarget::Module(id) => {
                let path = metadata
                    .modules
                    .iter()
                    .find(|m| &m.id == id)
                    .map(|m| m.path.clone())
                    .unwrap_or_else(|| id.0.clone());
                let target = Location {
                    file: path.clone(),
                    line: 1,
                    column: 1,
                    length: 0,
                };
                if let Some(location) = &location {
                    references.push(Reference {
                        location: location.clone(),
                        target: target.clone(),
                    });
                }
                let members = definitions
                    .iter()
                    .filter(|d| d.id.as_ref().is_some_and(|d| &d.module == id))
                    .map(|d| {
                        let mut member = render_alias(
                            d,
                            &format!("{}.{}", binding.name, d.name),
                            &binding.module.0,
                        );
                        member.name = d.name.clone();
                        member
                    })
                    .collect::<Vec<_>>();
                namespace_bindings.push(serde_json::json!({"file":scope_file,"name":binding.name,"kind":"module",
                    "location":location,"target":target,"members":members,"signature":format!("module {}",binding.name)}));
            }
        }
    }
    for reference in &metadata.references {
        let Some(target) = targets.get(&reference.target) else {
            continue;
        };
        let locations =
            reference_locations(&files, reference.line, reference.span, &reference.spelling);
        let variant = reference.spelling.rsplit_once('.').and_then(|(parent, name)| {
            if reference.target.kind != DefKind::Enum {
                return None;
            }
            let canonical = metadata.definition_id(&reference.target).is_some_and(|d| d.symbol == parent);
            let parts = parent.split('.').collect::<Vec<_>>();
            let scoped = match parts.as_slice() {
                [name] => metadata.binding(&reference.module, name).is_some_and(|b| {
                    matches!(&b.target, BindingTarget::Definition(id) if id == &reference.target)
                }),
                [alias, name] => metadata.binding(&reference.module, alias).is_some_and(|b| {
                    matches!(&b.target, BindingTarget::Module(id) if id == &reference.target.module && *name == reference.target.name)
                }),
                _ => false,
            };
            if canonical || scoped {
                variant_targets.get(&(reference.target.clone(), name.to_owned()))
            } else {
                None
            }
        });
        if let Some(location) = locations.last() {
            references.push(Reference {
                location: location.clone(),
                target: variant.unwrap_or(target).clone(),
            });
        }
        if variant.is_some() && locations.len() >= 2 {
            references.push(Reference {
                location: locations[locations.len() - 2].clone(),
                target: target.clone(),
            });
        }
        if locations.len() > 1 {
            let root = reference.spelling.split('.').next().unwrap();
            if let Some(binding) = metadata.bindings.iter().find(|b| {
                b.module == reference.module
                    && b.name == root
                    && matches!(b.target, BindingTarget::Module(_))
            }) {
                if let (Some(location), Some(target)) = (
                    locations.first(),
                    name_location(&files, binding.line, binding.span, &binding.name),
                ) {
                    references.push(Reference {
                        location: location.clone(),
                        target,
                    });
                }
            }
        }
    }
    // Import strings navigate to the loaded canonical file, including `from`.
    for file in &files {
        for (i, token) in file.tokens.iter().enumerate() {
            if let K::Str(import) = &token.kind {
                if i > 0
                    && matches!(&file.tokens[i-1].kind, K::Id(n) if n == "import" || n == "from")
                {
                    let target = std::fs::canonicalize(file.path.parent().unwrap().join(import))
                        .map_err(|e| e.to_string())?;
                    references.push(Reference {
                        location: file.location(token, file.string_length(token)),
                        target: Location {
                            file: target.display().to_string(),
                            line: 1,
                            column: 1,
                            length: 0,
                        },
                    });
                }
            }
        }
    }
    for definition in &definitions {
        references.push(Reference {
            location: definition.location.clone(),
            target: definition.location.clone(),
        });
        for variant in &definition.variants {
            references.push(Reference {
                location: variant.location.clone(),
                target: variant.location.clone(),
            });
        }
    }
    let mut bindings = Bindings {
        files: &files,
        vars: HashMap::new(),
        functions: symbols,
        references: vec![],
    };
    for p in programs {
        for f in &p.functions {
            bindings.function(f);
        }
    }
    let resolved = bindings
        .references
        .iter()
        .map(|r| &r.location)
        .collect::<HashSet<_>>();
    references.retain(|r| !resolved.contains(&r.location));
    references.extend(bindings.references);
    let mut seen = HashSet::new();
    references.retain(|r| seen.insert((r.location.clone(), r.target.clone())));
    let mut primary = Program::default();
    let mut native = Program::default();
    for (i, p) in programs.iter().enumerate() {
        let dest = if i == 0 { &mut primary } else { &mut native };
        dest.classes.extend(p.classes.clone());
        dest.enums.extend(p.enums.clone());
        dest.functions.extend(p.functions.clone());
        dest.modules.merge_native(p.modules.clone())?;
    }
    let typed = crate::check::editor_types(&primary, &native);
    let mut types = Types {
        files: &files,
        classes: typed
            .as_ref()
            .map(|p| p.classes.as_slice())
            .unwrap_or_default(),
        metadata: &metadata,
        locals: vec![],
        expressions: vec![],
    };
    if let Some(p) = &typed {
        for f in &p.functions {
            for ((name, ty), span) in f.params.iter().zip(&f.parameter_spans) {
                types.binding(f.line, *span, name, ty);
            }
            types.block(&f.body);
        }
    }
    Ok(
        serde_json::json!({"format":"nagi-symbols-v1","definitions":definitions,"bindings":namespace_bindings,
        "references":references,"locals":types.locals,"expressions":types.expressions,
        "files":files.iter().map(|f|f.path.display().to_string()).collect::<Vec<_>>()}),
    )
}
