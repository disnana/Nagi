//! Rust-only names. Keep the checked Nagi program and its Low output unchanged.
use crate::ast::*;
use std::collections::{BTreeSet, HashMap};

pub(crate) struct RustNames {
    escaped: HashMap<String, String>,
    symbols: BTreeSet<String>,
}

fn keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break"
            | "const"
            | "continue"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "static"
            | "struct"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
            | "union"
            | "gen"
    )
}

impl RustNames {
    pub(crate) fn new(program: &Program) -> Self {
        let symbols: BTreeSet<_> = program
            .modules
            .definitions
            .iter()
            .map(|definition| definition.symbol.clone())
            .collect();
        let mut names = BTreeSet::new();
        visit(&mut program.clone(), &mut |name| {
            names.insert(name.clone());
        });
        // Adapter exports use source spelling, even though the checked AST
        // stores a definition's unique symbol. Allocate escapes once so native
        // types, aliases, fields, and parameter names agree.
        let mut source_names = BTreeSet::new();
        for definition in &program.modules.definitions {
            source_names.insert(definition.id.name.clone());
            names.insert(definition.id.name.clone());
        }
        for binding in &program.modules.bindings {
            source_names.insert(binding.name.clone());
            names.insert(binding.name.clone());
        }
        let mut escaped = HashMap::new();
        let mut serial = 0;
        for name in names.clone() {
            // Source spelling can equal another definition's internal symbol.
            // Escape that public alias, while leaving the actual symbol intact.
            if symbols.contains(&name) && !source_names.contains(&name) {
                continue;
            }
            let replacement = if name.starts_with("__")
                || matches!(
                    name.as_str(),
                    "self" | "Self" | "crate" | "super" | "_" | "native"
                ) {
                loop {
                    let candidate = format!("__nagi_ident_{serial}");
                    serial += 1;
                    if names.insert(candidate.clone()) {
                        break candidate;
                    }
                }
            } else if keyword(&name) {
                format!("r#{name}")
            } else {
                continue;
            };
            escaped.insert(name, replacement);
        }
        Self { escaped, symbols }
    }

    pub(crate) fn program(&self, program: &Program) -> Program {
        let mut result = program.clone();
        visit(&mut result, &mut |name| {
            if self.symbols.contains(name) {
                return;
            }
            if let Some(escaped) = self.escaped.get(name) {
                *name = escaped.clone();
            }
        });
        result
    }

    pub(crate) fn source_name<'a>(&'a self, name: &'a str) -> &'a str {
        self.escaped.get(name).map(String::as_str).unwrap_or(name)
    }

    pub(crate) fn definition<'a>(&'a self, definition: &'a DefinitionInfo) -> &'a str {
        if definition.id.kind == DefKind::Function && definition.symbol == "main" {
            "__nagi_main"
        } else {
            &definition.symbol
        }
    }

    pub(crate) fn original<'a>(&'a self, escaped: &'a str) -> &'a str {
        self.escaped
            .iter()
            .find_map(|(original, value)| (value == escaped).then_some(original.as_str()))
            .unwrap_or(escaped)
    }
}

fn visit(program: &mut Program, name: &mut impl FnMut(&mut String)) {
    fn enum_path(path: &mut String, name: &mut impl FnMut(&mut String)) {
        let (enumeration, variant) = path.rsplit_once('.').expect("checked enum path");
        let mut enumeration = enumeration.to_owned();
        let mut variant = variant.to_owned();
        name(&mut enumeration);
        name(&mut variant);
        *path = format!("{enumeration}.{variant}");
    }
    fn ty(t: &mut Type, name: &mut impl FnMut(&mut String)) {
        if t.0 != "fn" || t.1.is_empty() {
            name(&mut t.0);
        }
        for arg in &mut t.1 {
            ty(arg, name);
        }
    }
    fn expr(e: &mut Expr, name: &mut impl FnMut(&mut String)) {
        if let Some(t) = &mut e.ty {
            ty(t, name);
        }
        match &mut e.kind {
            E::Name(n) => {
                if n == "main" && e.resolution == Some(NameResolution::Function) {
                    *n = "__nagi_main".into();
                } else {
                    name(n);
                }
            }
            E::Call(n, ts, args) => {
                if e.resolution == Some(NameResolution::Enum) {
                    enum_path(n, name);
                } else if e.resolution != Some(NameResolution::Builtin) {
                    if n == "main" && e.resolution == Some(NameResolution::Function) {
                        *n = "__nagi_main".into();
                    } else {
                        name(n);
                    }
                }
                for t in ts {
                    ty(t, name);
                }
                for arg in args {
                    expr(arg, name);
                }
            }
            E::Record(n, fields) => {
                if e.resolution == Some(NameResolution::Enum) {
                    enum_path(n, name);
                } else {
                    name(n);
                }
                for (n, value) in fields {
                    name(n);
                    expr(value, name);
                }
            }
            E::Field(value, n) => {
                name(n);
                expr(value, name);
            }
            E::Binary(a, _, b) | E::Index(a, b) => {
                expr(a, name);
                expr(b, name);
            }
            E::Unary(_, value) | E::Await(value) | E::Try(value) => expr(value, name),
            E::List(values) => {
                for value in values {
                    expr(value, name);
                }
            }
            _ => {}
        }
    }
    fn block(body: &mut [Stmt], name: &mut impl FnMut(&mut String)) {
        for stmt in body {
            if let Some(flow) = &mut stmt.flow {
                for binding in flow
                    .before
                    .iter_mut()
                    .chain(&mut flow.after)
                    .chain(flow.branch_entry.iter_mut().flatten())
                    .chain(
                        flow.loop_entry
                            .iter_mut()
                            .flat_map(|entry| entry.header.iter_mut().chain(&mut entry.body)),
                    )
                {
                    name(&mut binding.name);
                    ty(&mut binding.ty, name);
                    for origin in &mut binding.origins {
                        for field in &mut origin.fields {
                            name(field);
                        }
                    }
                }
                for mutation in &mut flow.content_mutations {
                    name(&mut mutation.name);
                    for origin in &mut mutation.added_origins {
                        for field in &mut origin.fields {
                            name(field);
                        }
                    }
                }
            }
            if let Some(t) = &mut stmt.binding_type {
                ty(t, name);
            }
            match &mut stmt.kind {
                S::Assign {
                    name: n,
                    annotation,
                    value,
                    ..
                } => {
                    name(n);
                    if let Some(t) = annotation {
                        ty(t, name);
                    }
                    expr(value, name);
                }
                S::Return(Some(value)) | S::Expr(value) | S::Spawn(value) => expr(value, name),
                S::If(condition, a, b) => {
                    expr(condition, name);
                    block(a, name);
                    block(b, name);
                }
                S::While(condition, body) => {
                    expr(condition, name);
                    block(body, name);
                }
                S::For(n, iterator, body) => {
                    name(n);
                    expr(iterator, name);
                    block(body, name);
                }
                S::Match(value, arms) => {
                    expr(value, name);
                    for arm in arms {
                        if let MatchPattern::Enum { name: path, .. } = &mut arm.pattern {
                            enum_path(path, name);
                        }
                        for binding in arm.pattern.bindings_mut() {
                            if let Some(n) = &mut binding.name {
                                name(n);
                            }
                            if let Some(t) = &mut binding.ty {
                                ty(t, name);
                            }
                        }
                        block(&mut arm.body, name);
                    }
                }
                S::Scope(body) => block(body, name),
                S::Return(None) => {}
            }
        }
    }
    for class in &mut program.classes {
        name(&mut class.name);
        for (n, t) in &mut class.fields {
            name(n);
            ty(t, name);
        }
    }
    for enumeration in &mut program.enums {
        name(&mut enumeration.name);
        for variant in &mut enumeration.variants {
            name(&mut variant.name);
            for (field, t) in &mut variant.fields {
                name(field);
                ty(t, name);
            }
        }
    }
    for function in &mut program.functions {
        name(&mut function.name);
        for (n, t) in &mut function.params {
            name(n);
            ty(t, name);
        }
        ty(&mut function.ret, name);
        block(&mut function.body, name);
    }
}
