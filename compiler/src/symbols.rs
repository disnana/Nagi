//! Definition locations for the files and AST actually loaded by the compiler.
use crate::{
    ast::*,
    lexer::{lex, Token, K},
    source::Sources,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
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
#[derive(Serialize)]
struct Definition {
    name: String,
    kind: &'static str,
    location: Location,
    signature: String,
    parameters: Vec<Member>,
    fields: Vec<Member>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_type: Option<String>,
    asynchronous: bool,
}
#[derive(Serialize)]
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
    fn follows_call(&self, index: usize) -> bool {
        let Some(token) = self.tokens.get(index + 1) else {
            return false;
        };
        if matches!(&token.kind, K::Sym(s) if s == "(") {
            return true;
        }
        if !matches!(&token.kind, K::Sym(s) if s == "[") {
            return false;
        }
        let mut depth = 0usize;
        for (i, token) in self.tokens.iter().enumerate().skip(index + 1) {
            match &token.kind {
                K::Sym(s) if s == "[" => depth += 1,
                K::Sym(s) if s == "]" => {
                    depth -= 1;
                    if depth == 0 {
                        return self
                            .tokens
                            .get(i + 1)
                            .is_some_and(|t| matches!(&t.kind, K::Sym(s) if s == "("));
                    }
                }
                _ => {}
            }
        }
        false
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

// Resolve lexical bindings independently of type/ownership checking. Navigation
// remains useful on a moved value or a binding whose initializer has a type error.
struct Bindings<'a, 'b> {
    files: &'a [File<'b>],
    vars: HashMap<String, Location>,
    references: Vec<Reference>,
}
impl Bindings<'_, '_> {
    fn binding(&mut self, line: usize, span: Span, name: &str) {
        if let Some(location) = name_location(self.files, line, span, name) {
            self.vars.insert(name.into(), location.clone());
            self.references.push(Reference {
                target: location.clone(),
                location,
            });
        }
    }
    fn reference(&mut self, line: usize, span: Span, name: &str) {
        if let (Some(location), Some(target)) = (
            name_location(self.files, line, span, name),
            self.vars.get(name),
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
                        if let Some(name) = &arm.binding {
                            if self.vars.contains_key(name) {
                                // Nagi forbids a case binding with an outer name.
                                // Do not guess which declaration an invalid arm means.
                                self.vars.remove(name);
                            } else {
                                self.binding(arm.line, arm.binding_span, name);
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
    locals: Vec<Local>,
    expressions: Vec<TypedExpression>,
}
impl Types<'_, '_> {
    fn file(&self, line: usize) -> Option<&File<'_>> {
        self.files
            .iter()
            .find(|f| line >= f.start && line < f.start + f.lines.len().max(1))
    }
    fn binding(&mut self, line: usize, span: Span, name: &str, ty: &Type) {
        let Some(location) = name_location(self.files, line, span, name) else {
            return;
        };
        self.locals.push(Local {
            name: name.into(),
            ty: ty.to_string(),
            location,
        });
    }
    fn expr(&mut self, e: &Expr) {
        if let Some(ty) = &e.ty {
            if let E::Name(name) = &e.kind {
                if ty.0 != "fn" {
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
                                    ty: ty.to_string(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    self.expressions.push(TypedExpression {
                        location,
                        end_line: end.line,
                        end_column: end.column + end.length,
                        ty: ty.to_string(),
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
                        if let (Some(name), Some(ty)) = (&arm.binding, &arm.binding_type) {
                            self.binding(arm.line, arm.binding_span, name, ty);
                        }
                        self.block(&arm.body);
                    }
                }
                S::Return(None) => {}
            }
        }
    }
}

pub fn index(sources: &Sources, programs: &[&Program]) -> Result<serde_json::Value, String> {
    let files = sources
        .files()
        .map(|(path, text, start)| {
            Ok(File {
                path,
                lines: text.lines().collect(),
                start,
                tokens: lex(text, path.extension().is_none_or(|x| x != "low"))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut definitions = vec![];
    let mut targets = BTreeMap::new();
    for p in programs {
        for (name, kind, line, keyword) in p
            .classes
            .iter()
            .map(|c| (&c.name, "class", c.line, "class"))
            .chain(
                p.functions
                    .iter()
                    .map(|f| (&f.name, "function", f.line, "def")),
            )
        {
            let Some(file) = files
                .iter()
                .find(|f| line >= f.start && line < f.start + f.lines.len().max(1))
            else {
                continue;
            };
            let local = line - file.start + 1;
            let low_keyword = if keyword == "def" { "fn" } else { "record" };
            let Some(pair) = file.tokens.windows(2).find(|pair| {
                pair[0].line == local
                    && matches!(&pair[0].kind, K::Id(n) if n == keyword || n == low_keyword)
                    && matches!(&pair[1].kind, K::Id(n) if n == name)
            }) else {
                continue;
            };
            let location = file.location(&pair[1], name.len());
            // A Low replacement navigates to the High declaration when both are present.
            targets
                .entry(name.clone())
                .or_insert_with(|| location.clone());
            definitions.push(Definition {
                name: name.clone(),
                kind,
                location,
                signature: String::new(),
                parameters: vec![],
                fields: vec![],
                return_type: None,
                asynchronous: false,
            });
            let definition = definitions.last_mut().unwrap();
            let members = |items: &[(String, Type)]| {
                items
                    .iter()
                    .map(|(name, ty)| Member {
                        name: name.clone(),
                        ty: ty.to_string(),
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
                definition.return_type = Some(function.ret.to_string());
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
                    name,
                    function
                        .params
                        .iter()
                        .map(|(n, t)| format!("{n}: {t}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    function.ret
                );
            } else {
                let class = p
                    .classes
                    .iter()
                    .find(|c| c.name == *name && c.line == line)
                    .unwrap();
                definition.fields = members(&class.fields);
                definition.signature = format!(
                    "class {name}\n{}",
                    class
                        .fields
                        .iter()
                        .map(|(n, t)| format!("    {n}: {t}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
    }
    let mut calls = BTreeSet::new();
    fn expr(e: &Expr, calls: &mut BTreeSet<(usize, String)>) {
        match &e.kind {
            E::Call(name, _, args) => {
                calls.insert((e.line, name.clone()));
                for a in args {
                    expr(a, calls);
                }
            }
            E::Record(name, fields) => {
                calls.insert((e.line, name.clone()));
                for (_, a) in fields {
                    expr(a, calls);
                }
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expr(a, calls);
                expr(b, calls);
            }
            E::Unary(_, a) | E::Try(a) | E::Await(a) | E::Field(a, _) => expr(a, calls),
            E::List(args) => {
                for a in args {
                    expr(a, calls);
                }
            }
            _ => {}
        }
    }
    fn block(ss: &[Stmt], calls: &mut BTreeSet<(usize, String)>) {
        for s in ss {
            match &s.kind {
                S::Assign { value, .. }
                | S::Return(Some(value))
                | S::Expr(value)
                | S::Spawn(value) => expr(value, calls),
                S::If(value, a, b) => {
                    expr(value, calls);
                    block(a, calls);
                    block(b, calls);
                }
                S::While(value, body) | S::For(_, value, body) => {
                    expr(value, calls);
                    block(body, calls);
                }
                S::Match(value, arms) => {
                    expr(value, calls);
                    for arm in arms {
                        block(&arm.body, calls);
                    }
                }
                S::Scope(body) => block(body, calls),
                S::Return(None) => {}
            }
        }
    }
    for p in programs {
        for f in &p.functions {
            block(&f.body, &mut calls);
        }
    }
    let mut references = vec![];
    let classes: BTreeSet<_> = definitions
        .iter()
        .filter(|d| d.kind == "class")
        .map(|d| d.name.as_str())
        .collect();
    for file in &files {
        let mut reading_type = false;
        let mut type_depth = 0usize;
        for (i, token) in file.tokens.iter().enumerate() {
            match &token.kind {
                K::Sym(s) if s == ":" || s == "->" => reading_type = true,
                K::Sym(s) if s == "[" && reading_type => type_depth += 1,
                K::Sym(s) if s == "]" && type_depth > 0 => type_depth -= 1,
                K::Sym(s) if s == "," && type_depth > 0 => {}
                K::Sym(_) | K::Newline | K::Indent | K::Dedent if type_depth == 0 => {
                    reading_type = false
                }
                _ => {}
            }
            if let K::Id(name) = &token.kind {
                let call = file.follows_call(i);
                if (call && calls.contains(&(file.start + token.line - 1, name.clone())))
                    || (reading_type && classes.contains(name.as_str()))
                {
                    if let Some(target) = targets.get(name) {
                        references.push(Reference {
                            location: file.location(token, name.len()),
                            target: target.clone(),
                        });
                    }
                }
            }
            if let K::Str(import) = &token.kind {
                if i > 0 && matches!(&file.tokens[i - 1].kind, K::Id(n) if n == "import") {
                    let target = file.path.parent().unwrap().join(import);
                    let target = std::fs::canonicalize(target).map_err(|e| e.to_string())?;
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
    for def in &definitions {
        references.push(Reference {
            location: def.location.clone(),
            target: def.location.clone(),
        });
    }
    let mut bindings = Bindings {
        files: &files,
        vars: HashMap::new(),
        references: vec![],
    };
    for p in programs {
        for f in &p.functions {
            bindings.function(f);
        }
    }
    references.extend(bindings.references);
    // Compound assignments contain a synthetic read of their left-hand name.
    let mut seen = HashSet::new();
    references.retain(|r| seen.insert((r.location.clone(), r.target.clone())));
    let mut primary = Program::default();
    let mut native = Program::default();
    for (i, p) in programs.iter().enumerate() {
        let dest = if i == 0 { &mut primary } else { &mut native };
        dest.classes.extend(p.classes.clone());
        dest.functions.extend(p.functions.clone());
    }
    let typed = crate::check::editor_types(&primary, &native);
    let mut types = Types {
        files: &files,
        classes: typed
            .as_ref()
            .map(|p| p.classes.as_slice())
            .unwrap_or_default(),
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
        serde_json::json!({ "format": "nagi-symbols-v1", "definitions": definitions, "references": references, "locals": types.locals, "expressions": types.expressions, "files": files.iter().map(|f| f.path.display().to_string()).collect::<Vec<_>>() }),
    )
}
