//! Rust-only names. Keep the checked Nagi program and its Low output unchanged.
use crate::ast::*;
use std::collections::{BTreeSet, HashMap};

pub(crate) struct RustNames {
    escaped: HashMap<String, String>,
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
        let mut names = BTreeSet::new();
        visit(&mut program.clone(), &mut |name| {
            names.insert(name.clone());
        });
        let mut escaped = HashMap::new();
        let mut serial = 0;
        for name in names.clone() {
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
        Self { escaped }
    }

    pub(crate) fn program(&self, program: &Program) -> Program {
        let mut result = program.clone();
        visit(&mut result, &mut |name| {
            if let Some(escaped) = self.escaped.get(name) {
                *name = escaped.clone();
            }
        });
        result
    }

    pub(crate) fn original<'a>(&'a self, escaped: &'a str) -> &'a str {
        self.escaped
            .iter()
            .find_map(|(original, value)| (value == escaped).then_some(original.as_str()))
            .unwrap_or(escaped)
    }
}

fn visit(program: &mut Program, name: &mut impl FnMut(&mut String)) {
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
                if e.resolution != Some(NameResolution::Builtin) {
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
                name(n);
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
                        if let Some(n) = &mut arm.binding {
                            name(n);
                        }
                        if let Some(t) = &mut arm.binding_type {
                            ty(t, name);
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
