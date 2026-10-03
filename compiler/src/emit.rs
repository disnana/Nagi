use crate::ast::*;
use crate::diagnostics::Generated;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn quote(s: &str) -> String {
    format!("{s:?}")
}
fn low_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}
// Low carries resolved definition symbols. Type's user-facing Display may
// shorten those symbols for diagnostics, so emission must render the stored
// structural name directly.
fn low_type(t: &Type) -> String {
    if t.0 == "Option" {
        return format!("{}?", low_type(&t.inner()));
    }
    if t.1.is_empty() {
        t.0.clone()
    } else {
        format!(
            "{}[{}]",
            t.0,
            t.1.iter().map(low_type).collect::<Vec<_>>().join(", ")
        )
    }
}
pub fn low(p: &Program) -> String {
    low_with_lines(p).text
}

pub fn low_with_lines(p: &Program) -> Generated {
    let mut transport = crate::modules::prepare_low_types(p);
    crate::modules::synchronize(&mut transport);
    let p = &transport;
    fn receiver(e: &Expr) -> String {
        match &e.kind {
            E::Unary(_, _) | E::Await(_) | E::Try(_) => format!("({})", expr(e)),
            _ => expr(e),
        }
    }
    fn expr(e: &Expr) -> String {
        match &e.kind {
            E::Int(s) | E::Float(s) | E::Name(s) => s.clone(),
            E::Str(s) => low_quote(s),
            E::Bool(b) => b.to_string(),
            E::Null => "None".into(),
            E::Binary(a, o, b) => format!("({} {o} {})", expr(a), expr(b)),
            E::Unary(o, x) => format!("{o} {}", expr(x)),
            E::Call(n, t, a) => format!(
                "{n}{}({})",
                if t.is_empty() {
                    String::new()
                } else {
                    format!(
                        "[{}]",
                        t.iter().map(low_type).collect::<Vec<_>>().join(", ")
                    )
                },
                a.iter().map(expr).collect::<Vec<_>>().join(", ")
            ),
            E::Record(n, a) => format!(
                "{n}({})",
                a.iter()
                    .map(|(n, e)| format!("{n} = {}", expr(e)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            E::Field(x, n) => format!("{}.{n}", receiver(x)),
            E::Index(x, i) => format!("{}[{}]", receiver(x), expr(i)),
            E::List(a) => format!("[{}]", a.iter().map(expr).collect::<Vec<_>>().join(", ")),
            E::Await(x) => format!("await {}", expr(x)),
            E::Try(x) => format!("try {}", expr(x)),
        }
    }
    fn block(ss: &[Stmt], n: usize, out: &mut Generated) {
        let pad = "    ".repeat(n);
        for s in ss {
            out.origin(Some(s.line));
            out.push_str(&pad);
            match &s.kind {
                S::Assign {
                    name,
                    annotation,
                    value,
                    declare,
                } => out.push_str(&format!(
                    "{}{name}{} = {};\n",
                    if *declare { "let " } else { "" },
                    if *declare {
                        format!(": {}", low_type(annotation.as_ref().unwrap()))
                    } else {
                        String::new()
                    },
                    expr(value)
                )),
                S::Return(e) => out.push_str(&format!(
                    "return{};\n",
                    e.as_ref()
                        .map(|e| format!(" {}", expr(e)))
                        .unwrap_or_default()
                )),
                S::Expr(e) => out.push_str(&format!("{};\n", expr(e))),
                S::Spawn(e) => out.push_str(&format!("spawn {};\n", expr(e))),
                S::If(c, a, b) => {
                    out.push_str(&format!("if {} {{\n", expr(c)));
                    block(a, n + 1, out);
                    out.origin(Some(s.line));
                    out.push_str(&format!("{pad}}}"));
                    if !b.is_empty() {
                        out.push_str(" else {\n");
                        block(b, n + 1, out);
                        out.origin(Some(s.line));
                        out.push_str(&format!("{pad}}}"));
                    }
                    out.push('\n');
                }
                S::While(c, b) => {
                    out.push_str(&format!("while {} {{\n", expr(c)));
                    block(b, n + 1, out);
                    out.origin(Some(s.line));
                    out.push_str(&format!("{pad}}}\n"));
                }
                S::Match(value, arms) => {
                    out.push_str(&format!("match {} {{\n", expr(value)));
                    for arm in arms {
                        out.origin(Some(arm.line));
                        let pattern = match &arm.pattern {
                            MatchPattern::Result { ok, binding } => format!(
                                "{}({})",
                                if *ok { "Ok" } else { "Err" },
                                binding.name.as_deref().unwrap_or("_")
                            ),
                            MatchPattern::Option { binding } => match binding {
                                Some(binding) => {
                                    format!("Some({})", binding.name.as_deref().unwrap_or("_"))
                                }
                                None => "None".into(),
                            },
                            MatchPattern::Enum { name, bindings, .. } => {
                                if bindings.is_empty() {
                                    name.clone()
                                } else {
                                    format!(
                                        "{name}({})",
                                        bindings
                                            .iter()
                                            .map(|binding| binding.name.as_deref().unwrap_or("_"))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    )
                                }
                            }
                        };
                        out.push_str(&format!("{pad}    case {pattern} {{\n"));
                        block(&arm.body, n + 2, out);
                        out.origin(Some(arm.line));
                        out.push_str(&format!("{pad}    }}\n"));
                    }
                    out.origin(Some(s.line));
                    out.push_str(&format!("{pad}}}\n"));
                }
                S::For(v, e, b) => {
                    out.push_str(&format!("for {v} in {} {{\n", expr(e)));
                    block(b, n + 1, out);
                    out.origin(Some(s.line));
                    out.push_str(&format!("{pad}}}\n"));
                }
                S::Scope(b) => {
                    out.push_str("scope {\n");
                    block(b, n + 1, out);
                    out.origin(Some(s.line));
                    out.push_str(&format!("{pad}}}\n"));
                }
            }
        }
    }
    let mut out =
        Generated::new("# Nagi Low 0.1 / generated. 手書き変更はnative/で@replaceしてください。\n");
    if p.modules.root.is_some() {
        out.push_str("# nagi-modules-v1 ");
        out.push_str(&serde_json::to_string(&p.modules).expect("module metadata is serializable"));
        out.push('\n');
    }
    if p.module_imports.is_empty() {
        for (file, line) in &p.imports {
            out.origin(Some(*line));
            out.push_str(&format!("import {};\n", low_quote(file)));
        }
    } else {
        for import in &p.module_imports {
            out.origin(Some(import.line));
            let path = match import.source {
                ImportSource::File => low_quote(&import.path),
                ImportSource::Standard => import.path.clone(),
            };
            match &import.kind {
                ImportKind::Flat => {
                    out.push_str(&format!("import {path};\n"));
                }
                ImportKind::Module { alias, .. } => {
                    out.push_str(&format!("import {path} as {alias};\n"));
                }
                ImportKind::Names(names) => {
                    for name in names {
                        out.push_str(&format!(
                            "from {} import {}{};\n",
                            path,
                            name.name,
                            if name.name == name.alias {
                                String::new()
                            } else {
                                format!(" as {}", name.alias)
                            }
                        ));
                    }
                }
            }
        }
    }
    for c in &p.classes {
        out.origin(Some(c.line));
        out.push_str(&format!("record {} {{\n", c.name));
        for (i, (n, t)) in c.fields.iter().enumerate() {
            out.origin(Some(c.field_lines.get(i).copied().unwrap_or(c.line)));
            out.push_str(&format!("    {n}: {};\n", low_type(t)));
        }
        out.origin(Some(c.line));
        out.push_str("}\n\n");
    }
    for enumeration in &p.enums {
        out.origin(Some(enumeration.line));
        out.push_str(&format!("enum {} {{\n", enumeration.name));
        for variant in &enumeration.variants {
            out.origin(Some(variant.line));
            out.push_str(&format!(
                "    {}{};\n",
                variant.name,
                if variant.fields.is_empty() {
                    String::new()
                } else {
                    format!(
                        "({})",
                        variant
                            .fields
                            .iter()
                            .map(|(name, ty)| format!("{name}: {}", low_type(ty)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            ));
        }
        out.origin(Some(enumeration.line));
        out.push_str("}\n\n");
    }
    for f in &p.functions {
        out.origin(Some(f.line));
        for (a, v) in &f.attrs {
            if a == "replace" {
                out.push_str(&format!("@replace {v}\n"));
            } else {
                out.push_str(&format!("@{a}({})\n", low_quote(v)));
            }
        }
        out.push_str(&format!(
            "{}{}fn {}({}) -> {}{}\n",
            if f.external { "extern " } else { "" },
            if f.asynchronous { "async " } else { "" },
            f.name,
            f.params
                .iter()
                .map(|(n, t)| format!("{n}: {}", low_type(t)))
                .collect::<Vec<_>>()
                .join(", "),
            low_type(&f.ret),
            if f.external { ";" } else { " {" }
        ));
        if !f.external {
            block(&f.body, 1, &mut out);
            out.origin(Some(f.line));
            out.push_str("}\n\n");
        }
    }
    out
}
pub fn rust_type(t: &Type) -> String {
    rust_type_at(
        t,
        0,
        &RustTypes {
            raw_classes: &[],
            enums: &[],
            modules: &ModuleMetadata::default(),
        },
    )
}
struct RustTypes<'a> {
    raw_classes: &'a [Class],
    enums: &'a [Enum],
    modules: &'a ModuleMetadata,
}

impl RustTypes<'_> {
    fn ty(&self, t: &Type) -> String {
        rust_type_at(t, 0, self)
    }

    fn variant(&self, path: &str) -> &EnumVariant {
        let (enumeration, variant) = path.rsplit_once('.').expect("checked enum path");
        self.enums
            .iter()
            .find(|definition| definition.name == enumeration)
            .and_then(|definition| {
                definition
                    .variants
                    .iter()
                    .find(|field| field.name == variant)
            })
            .expect("checked enum variant")
    }
}

fn rust_enum_path(path: &str) -> String {
    let (enumeration, variant) = path.rsplit_once('.').expect("checked enum path");
    format!("{enumeration}::{variant}")
}

fn registered_resource(symbol: &str, modules: &ModuleMetadata) -> Option<crate::stdlib::Resource> {
    let resource = crate::stdlib::resource(symbol)?;
    let definition = modules.definition(symbol)?;
    (definition.id == crate::stdlib::resource_id(resource)).then_some(resource)
}

fn registered_rust_path(symbol: &str, modules: &ModuleMetadata) -> Option<&'static str> {
    if let Some(resource) = registered_resource(symbol, modules) {
        Some(crate::stdlib::resource_info(resource).rust_path)
    } else {
        let operation = crate::stdlib::operation(symbol)?;
        let definition = modules.definition(symbol)?;
        (definition.id == crate::stdlib::function_id(operation))
            .then_some(crate::stdlib::operation_info(operation).rust_path)
    }
}

fn resource_type(mut ty: &Type, types: &RustTypes<'_>) -> Option<crate::stdlib::Resource> {
    while matches!(ty.0.as_str(), "owned" | "shared" | "view") {
        ty = &ty.1[0];
    }
    registered_resource(&ty.0, types.modules)
}

fn native_resource_view(ty: &Type, types: &RustTypes<'_>) -> Option<crate::stdlib::Resource> {
    let element = crate::stdlib::native_view_element(ty)?;
    registered_resource(&element.0, types.modules)
}

fn rust_type_at(t: &Type, depth: usize, types: &RustTypes<'_>) -> String {
    if let Some(resource) = registered_resource(&t.0, types.modules) {
        let path = crate::stdlib::resource_info(resource).rust_path;
        return if t.1.is_empty() {
            path.into()
        } else {
            format!(
                "{path}<{}>",
                t.1.iter()
                    .map(|arg| rust_type_at(arg, depth + 1, types))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
    }
    match t.0.as_str() {
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" | "bool" => {
            if types.raw_classes.iter().any(|class| class.name == t.0) {
                t.0.clone()
            } else {
                format!("::std::primitive::{}", t.0)
            }
        }
        "str" => "::std::string::String".into(),
        "bytes" => "::std::vec::Vec<::std::primitive::u8>".into(),
        "unit" => "()".into(),
        "Error" => "::nagi_runtime::Error".into(),
        "Html" => "::nagi_runtime::axum::response::Html<::std::string::String>".into(),
        "Db" => "::nagi_runtime::Db".into(),
        "UUID" => "::nagi_runtime::Uuid".into(),
        "timestamp" => "::nagi_runtime::Timestamp".into(),
        "fn" if !t.1.is_empty() => {
            let signature = format!(
                "fn({}) -> {}",
                t.1[..t.1.len() - 1]
                    .iter()
                    .map(|arg| rust_type_at(arg, depth + 1, types))
                    .collect::<Vec<_>>()
                    .join(", "),
                rust_type_at(t.1.last().unwrap(), depth + 1, types)
            );
            if t.1.iter().any(Type::contains_view) {
                let lifetime = format!("'nagi_fn_{depth}");
                format!(
                    "for<{lifetime}> {}",
                    signature.replace("&'a ", &format!("&{lifetime} "))
                )
            } else {
                signature
            }
        }
        "view" => {
            let a = t.inner();
            if native_resource_view(t, types).is_some() {
                return format!("&'a {}", rust_type_at(&a, depth + 1, types));
            }
            match a.0.as_str() {
                "str" => "&'a ::std::primitive::str".into(),
                "bytes" => "&'a [::std::primitive::u8]".into(),
                _ => format!("&'a [{}]", rust_type_at(&a, depth + 1, types)),
            }
        }
        "owned" => rust_type_at(&t.inner(), depth + 1, types),
        "shared" => format!(
            "::std::sync::Arc<{}>",
            rust_type_at(&t.inner(), depth + 1, types)
        ),
        "List" => format!(
            "::std::vec::Vec<{}>",
            rust_type_at(&t.inner(), depth + 1, types)
        ),
        "Map" => format!(
            "::std::collections::HashMap<{}, {}>",
            rust_type_at(&t.1[0], depth + 1, types),
            rust_type_at(&t.1[1], depth + 1, types)
        ),
        "Option" | "Result" => format!(
            "::std::{}::{}<{}>",
            if t.0 == "Option" { "option" } else { "result" },
            t.0,
            t.1.iter()
                .map(|arg| rust_type_at(arg, depth + 1, types))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => t.0.clone(),
    }
}
fn local_type(t: &Type, types: &RustTypes<'_>) -> String {
    types.ty(t).replace("&'a ", "&")
}
fn string_arg(e: &Expr, types: &RustTypes<'_>) -> String {
    if let E::Str(s) = &e.kind {
        quote(s)
    } else {
        format!("&({})", re(e, types))
    }
}

fn reference_arg(e: &Expr, types: &RustTypes<'_>) -> String {
    if let E::Str(value) = &e.kind {
        quote(value)
    } else if e.ty.as_ref().is_some_and(Type::is_view) {
        re(e, types)
    } else {
        format!("&({})", re(e, types))
    }
}

fn re(e: &Expr, types: &RustTypes<'_>) -> String {
    match &e.kind {
        E::Int(s) | E::Float(s) => {
            // Formatting and container operations do not always give Rust a
            // numeric type context. Preserve the type selected by the checker
            // instead of allowing Rust's default i32/f64 inference to replace it.
            match e.ty.as_ref() {
                Some(t)
                    if matches!(
                        t.0.as_str(),
                        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64"
                    ) =>
                {
                    format!("{s}{}", t.0)
                }
                _ => s.clone(),
            }
        }
        E::Name(s) => {
            if e.resolution == Some(NameResolution::Standard) {
                registered_rust_path(s, types.modules)
                    .expect("checked standard definition")
                    .into()
            } else if e.resolution == ::std::option::Option::Some(NameResolution::Function) {
                format!("crate::{s}")
            } else {
                s.clone()
            }
        }
        E::Str(s) => format!("::std::string::String::from({})", quote(s)),
        E::Bool(b) => b.to_string(),
        E::Null => "::std::option::Option::None".into(),
        E::Binary(a, o, b) => {
            let comparison = matches!(o.as_str(), "==" | "!=" | "<" | ">" | "<=" | ">=");
            let borrowed_content = |value: &Expr| {
                let mut ty = value.ty.as_ref()?;
                while ty.0 == "owned" {
                    ty = ty.1.first()?;
                }
                let borrowed = ty.is_view();
                if borrowed {
                    ty = ty.1.first()?;
                }
                match ty.0.as_str() {
                    "str" => Some(("str", borrowed)),
                    "bytes" => Some(("bytes", borrowed)),
                    _ => None,
                }
            };
            let content = if comparison {
                borrowed_content(a)
                    .zip(borrowed_content(b))
                    .filter(|((left, _), (right, _))| left == right)
                    .map(|((content, _), _)| content)
            } else {
                None
            };
            let operand = |value: &Expr| {
                if let Some(content) = content {
                    if let E::Str(literal) = &value.kind {
                        return quote(literal);
                    }
                    if borrowed_content(value).is_some_and(|(_, borrowed)| !borrowed) {
                        return format!(
                            "({}).{}()",
                            re(value, types),
                            if content == "str" {
                                "as_str"
                            } else {
                                "as_slice"
                            }
                        );
                    }
                }
                if matches!(o.as_str(), "==" | "!=") {
                    if let E::Str(literal) = &value.kind {
                        return quote(literal);
                    }
                }
                re(value, types)
            };
            format!(
                "({} {} {})",
                operand(a),
                match o.as_str() {
                    "and" => "&&",
                    "or" => "||",
                    _ => o,
                },
                operand(b)
            )
        }
        E::Unary(o, x) => format!("{}({})", if o == "not" { "!" } else { o }, re(x, types)),
        E::Field(x, n) if e.resolution == Some(NameResolution::Enum) => {
            format!("{}::{n}", re(x, types))
        }
        E::Field(x, n) if e.resolution == Some(NameResolution::ResourceConstant) => {
            let E::Name(owner) = &x.kind else {
                unreachable!("checked resource constant owner")
            };
            let resource =
                registered_resource(owner, types.modules).expect("checked resource constant type");
            let constant = crate::stdlib::constant(resource, n).expect("checked resource constant");
            format!(
                "{}::{}",
                crate::stdlib::resource_info(resource).rust_path,
                constant.native_name
            )
        }
        E::Field(x, n) if e.resolution == Some(NameResolution::ResourceField) => {
            let resource = resource_type(
                x.ty.as_ref().expect("checked resource receiver type"),
                types,
            )
            .expect("checked resource field owner");
            let field = crate::stdlib::field(resource, n).expect("checked resource field");
            format!(
                "({}).{}{}",
                re(x, types),
                field.accessor,
                if field.owned { "" } else { "()" }
            )
        }
        E::Field(x, n) => format!("({}).{n}", re(x, types)),
        E::Index(x, i) => format!(
            "({})[::std::primitive::usize::try_from({}).expect(\"negative index\")]",
            re(x, types),
            re(i, types)
        ),
        E::List(a) => format!(
            "vec![{}]",
            a.iter()
                .map(|e| re(e, types))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        E::Record(n, a) => {
            let path = if e.resolution == Some(NameResolution::Enum) {
                rust_enum_path(n)
            } else {
                n.clone()
            };
            if e.resolution == Some(NameResolution::Enum) && a.is_empty() {
                path
            } else {
                format!(
                    "{path} {{ {} }}",
                    a.iter()
                        .map(|(n, e)| format!("{n}: {}", re(e, types)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        E::Await(x) => format!("({}).await", re(x, types)),
        E::Try(x) => format!("({})?", re(x, types)),
        E::Call(n, ts, a) => {
            if e.resolution == Some(NameResolution::Standard) {
                let operation = crate::stdlib::operation(n).expect("checked standard operation");
                let info = crate::stdlib::operation_info(operation);
                let arguments = a
                    .iter()
                    .zip(info.parameters)
                    .map(|(argument, passing)| match passing {
                        crate::stdlib::Passing::Reference => reference_arg(argument, types),
                        crate::stdlib::Passing::Borrow => format!("&({})", re(argument, types)),
                        crate::stdlib::Passing::Move
                        | crate::stdlib::Passing::Handler
                        | crate::stdlib::Passing::Mapper => re(argument, types),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let generic = if ts.is_empty() || !info.emit_type_arguments {
                    String::new()
                } else {
                    format!(
                        "::<{}>",
                        ts.iter()
                            .map(|ty| local_type(ty, types))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                return format!("{}{generic}({arguments})", info.rust_path);
            }
            let args = a.iter().map(|e| re(e, types)).collect::<Vec<_>>();
            let join = args.join(", ");
            match e.resolution {
                Some(NameResolution::Enum) => {
                    let path = rust_enum_path(n);
                    let variant = types.variant(n);
                    return if variant.fields.is_empty() {
                        path
                    } else {
                        format!(
                            "{path} {{ {} }}",
                            variant
                                .fields
                                .iter()
                                .zip(&args)
                                .map(|((field, _), arg)| format!("{field}: {arg}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                }
                ::std::option::Option::Some(NameResolution::Function) => {
                    return format!("crate::{n}({join})")
                }
                ::std::option::Option::Some(NameResolution::Local) => {
                    return format!("{n}({join})")
                }
                _ => {}
            }
            let g = if ts.is_empty() {
                String::new()
            } else {
                format!(
                    "::<{}>",
                    ts.iter()
                        .map(|t| local_type(t, types))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            match n.as_str() {
                "print" => format!("println!(\"{{}}\", {})", string_or_value(&a[0], types)),
                "write" => format!("print!(\"{{}}\", {})", string_or_value(&a[0], types)),
                "read_line" => "::nagi_runtime::read_line()".into(),
                "html" => format!("::nagi_runtime::axum::response::Html({})", args[0]),
                "include_text" => format!("include_str!({}).to_owned()", string_arg(&a[0], types)),
                "assert_true" => format!("assert!({})", args[0]),
                "view" => {
                    if e.resolution == Some(NameResolution::Builtin) {
                        if let E::Str(value) = &a[0].kind {
                            return quote(value);
                        }
                    }
                    if e.ty.as_ref().and_then(|ty| native_resource_view(ty, types)).is_some() {
                        format!("&({})", args[0])
                    } else {
                        format!("({}).{}()", args[0], if a[0].ty.as_ref().is_some_and(|t| t.0 == "str") { "as_str" } else { "as_slice" })
                    }
                }
                "copy" => {
                    if a[0].ty.as_ref().and_then(|ty| native_resource_view(ty, types))
                        .is_some_and(|resource| crate::stdlib::resource_info(resource).copy) {
                        format!("*({})", args[0])
                    } else {
                        format!("({}).to_owned()", args[0])
                    }
                }
                "share" => format!("::std::sync::Arc::new({})", args[0]),
                "clone_shared" => format!("::std::sync::Arc::clone(&{})", args[0]),
                "len" => format!("(({}).len() as ::std::primitive::i64)", string_or_value(&a[0], types)),
                "range" => format!("0i64..{}", args[0]),
                "append" => format!("{}.push({})", args[0], args[1]),
                "ok" => format!("::std::result::Result::Ok({})", args[0]),
                "some" => format!("::std::option::Option::Some({})", args[0]),
                "error" => format!("::std::result::Result::Err(::nagi_runtime::Error::invalid({}))", args[0]),
                "not_found" => format!(
                    "::std::result::Result::Err(::nagi_runtime::Error {{ kind: ::nagi_runtime::ErrorKind::NotFound, message: {} }})",
                    args[0]
                ),
                "internal_error" => format!("::std::result::Result::Err(::nagi_runtime::Error::internal({}))", args[0]),
                "fail" => format!("::std::result::Result::Err({})", args[0]),
                "error_kind" => format!("::nagi_runtime::error_kind(&({})).to_owned()", args[0]),
                "error_message" => format!("({}).message.clone()", args[0]),
                "serve" => format!("__nagi_serve({}, {})", args[0], args[1]),
                "env" => format!(
                    "::std::env::var({}).unwrap_or_else(|_|({}).to_owned())",
                    string_arg(&a[0], types),
                    string_arg(&a[1], types)
                ),
                "sleep" => format!("::nagi_runtime::sleep({})", args[0]),
                "db_open" => format!("::nagi_runtime::Db::open({})", string_arg(&a[0], types)),
                "db_exec" | "db_all" | "db_query" | "db_write" | "db_insert" | "db_update" => {
                    format!(
                        "{}.{}{}({})",
                        args[0],
                        n.trim_start_matches("db_"),
                        g,
                        std::iter::once(if let E::Str(s) = &a[1].kind {
                            format!("::nagi_runtime::Sql::Static({})", quote(s))
                        } else {
                            format!("::nagi_runtime::Sql::Owned(({}).to_owned())", string_arg(&a[1], types))
                        })
                        .chain(args.iter().skip(2).cloned())
                        .collect::<Vec<_>>()
                        .join(", ")
                    )
                }
                "json_decode" => {
                    let input = if a[0].ty.as_ref().is_some_and(|t| {
                        t.0 == "str" || t == &Type::generic("view", vec![Type::named("str")])
                    }) {
                        format!("({}).as_bytes()", string_arg(&a[0], types))
                    } else {
                        format!("&({})", args[0])
                    };
                    format!("::nagi_runtime::decode{g}({input})")
                }
                "json_encode" => format!("::nagi_runtime::encode(&{})", args[0]),
                "parse_i64" => format!("::nagi_runtime::parse_i64({})", string_arg(&a[0], types)),
                "parse_f64" => format!("::nagi_runtime::parse_f64({})", string_arg(&a[0], types)),
                "uuid_parse" => format!("::nagi_runtime::Uuid::parse({})", string_arg(&a[0], types)),
                "uuid_format" => format!("{}.to_string()", args[0]),
                "slice" => format!(
                    "::nagi_runtime::{}({}, {}, {})",
                    if a[0].ty.as_ref().is_some_and(|t| t.inner().0 == "str") {
                        "slice_str"
                    } else {
                        "slice"
                    },
                    args[0],
                    args[1],
                    args[2]
                ),
                "i64" => format!("::std::primitive::i64::from({})", args[0]),
                "i32" => format!(
                    "::std::primitive::i32::try_from({}).map_err(|e| ::nagi_runtime::Error::invalid(e.to_string()))",
                    args[0]
                ),
                "size_of" => format!("(::std::mem::size_of{}() as ::std::primitive::i64)", g),
                "bench_i64" | "bench_f64" | "bench_scalar" => {
                    format!("::nagi_runtime::{n}({}, {}, {})", string_arg(&a[0], types), args[1], args[2])
                }
                "clock_ns" | "make_ints" | "actor_demo" | "actor_pair_demo" | "supervisor_demo"
                | "queue_demo" | "task_demo" | "cpu_sum" => format!("::nagi_runtime::{n}({join})"),
                _ => format!("{n}({join})"),
            }
        }
    }
}
fn string_or_value(e: &Expr, types: &RustTypes<'_>) -> String {
    if matches!(e.kind, E::Str(_)) {
        string_arg(e, types)
    } else {
        re(e, types)
    }
}
fn rb(ss: &[Stmt], out: &mut Generated, n: usize, types: &RustTypes<'_>) {
    let pad = "    ".repeat(n);
    for s in ss {
        out.origin(::std::option::Option::Some(s.line));
        out.push_str(&pad);
        match &s.kind {
            S::Assign {
                name,
                annotation,
                value,
                declare,
            } => out.push_str(&format!(
                "{}{name}{} = {};\n",
                if *declare { "let mut " } else { "" },
                if *declare && !annotation.as_ref().is_some_and(Type::is_async_function) {
                    format!(": {}", local_type(annotation.as_ref().unwrap(), types))
                } else {
                    String::new()
                },
                re(value, types)
            )),
            S::Return(e) => out.push_str(&format!(
                "return {};\n",
                e.as_ref()
                    .map(|e| re(e, types))
                    .unwrap_or_else(|| "()".into())
            )),
            S::Expr(e) => out.push_str(&format!("{};\n", re(e, types))),
            S::If(c, a, b) => {
                out.push_str(&format!("if {} {{\n", re(c, types)));
                rb(a, out, n + 1, types);
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}}}"));
                if !b.is_empty() {
                    out.push_str(" else {\n");
                    rb(b, out, n + 1, types);
                    out.origin(::std::option::Option::Some(s.line));
                    out.push_str(&format!("{pad}}}"));
                }
                out.push('\n');
            }
            S::While(c, b) => {
                out.push_str(&format!("while {} {{\n", re(c, types)));
                rb(b, out, n + 1, types);
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::Match(value, arms) => {
                out.push_str(&format!("match {} {{\n", re(value, types)));
                for arm in arms {
                    out.origin(::std::option::Option::Some(arm.line));
                    let binding = |binding: &PatternBinding| {
                        binding
                            .name
                            .as_ref()
                            .map(|name| format!("mut {name}"))
                            .unwrap_or_else(|| "_".into())
                    };
                    let pattern = match &arm.pattern {
                        MatchPattern::Result { ok, binding: value } => format!(
                            "{}({})",
                            if *ok {
                                "::std::result::Result::Ok"
                            } else {
                                "::std::result::Result::Err"
                            },
                            binding(value)
                        ),
                        MatchPattern::Option { binding: value } => match value {
                            Some(value) => {
                                format!("::std::option::Option::Some({})", binding(value))
                            }
                            None => "::std::option::Option::None".into(),
                        },
                        MatchPattern::Enum { name, bindings, .. } => {
                            let path = rust_enum_path(name);
                            if bindings.is_empty() {
                                path
                            } else {
                                format!(
                                    "{path} {{ {} }}",
                                    types
                                        .variant(name)
                                        .fields
                                        .iter()
                                        .zip(bindings)
                                        .map(|((field, _), value)| format!(
                                            "{field}: {}",
                                            binding(value)
                                        ))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            }
                        }
                    };
                    out.push_str(&format!("{pad}    {pattern} => {{\n"));
                    rb(&arm.body, out, n + 2, types);
                    out.origin(::std::option::Option::Some(arm.line));
                    out.push_str(&format!("{pad}    }},\n"));
                }
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::For(v, e, b) => {
                let iterator = if e.ty.as_ref().is_some_and(|t| t.0 == "Range") {
                    re(e, types)
                } else {
                    format!("({}).iter().copied()", re(e, types))
                };
                out.push_str(&format!("for mut {v} in {iterator} {{\n"));
                rb(b, out, n + 1, types);
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::Spawn(e) => {
                let returns_result = e.ty.as_ref().is_some_and(|t| t.inner().0 == "Result");
                out.push_str(&format!(
                    "{{ let __nagi_spawn_future = {}; __scope.spawn(async move {{ __nagi_spawn_future.await{} }}); }}\n",
                    re(e, types),
                    if returns_result {
                        ""
                    } else {
                        "; ::std::result::Result::Ok(())"
                    }
                ));
            }
            S::Scope(b) => {
                out.push_str("{\n");
                out.push_str(&format!("{pad}    let mut __scope = ::nagi_runtime::Scope::new();\n{pad}    let __scope_result: ::std::result::Result<(), _> = async {{\n"));
                rb(b, out, n + 2, types);
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}        ::std::result::Result::Ok(())\n{pad}    }}.await;\n{pad}    if let ::std::result::Result::Err(e) = __scope_result {{ __scope.cancel().await; return ::std::result::Result::Err(e); }}\n{pad}    __scope.join().await?;\n{pad}}}\n"));
            }
        }
    }
}
pub fn rust(p: &Program) -> Result<String, String> {
    ::std::result::Result::Ok(rust_with_lines(p)?.text)
}

fn calls_builtin(statements: &[Stmt], builtin: &str) -> bool {
    fn expr(e: &Expr, builtin: &str) -> bool {
        match &e.kind {
            E::Call(name, _, args) => {
                (name == builtin && e.resolution == Some(NameResolution::Builtin))
                    || args.iter().any(|e| expr(e, builtin))
            }
            E::Record(_, fields) => fields.iter().any(|(_, e)| expr(e, builtin)),
            E::List(values) => values.iter().any(|e| expr(e, builtin)),
            E::Binary(a, _, b) | E::Index(a, b) => expr(a, builtin) || expr(b, builtin),
            E::Unary(_, e) | E::Field(e, _) | E::Await(e) | E::Try(e) => expr(e, builtin),
            E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null | E::Name(_) => false,
        }
    }
    statements.iter().any(|s| match &s.kind {
        S::Assign { value, .. } | S::Expr(value) | S::Spawn(value) => expr(value, builtin),
        S::Return(value) => value.as_ref().is_some_and(|e| expr(e, builtin)),
        S::If(condition, a, b) => {
            expr(condition, builtin) || calls_builtin(a, builtin) || calls_builtin(b, builtin)
        }
        S::Match(value, arms) => {
            expr(value, builtin) || arms.iter().any(|arm| calls_builtin(&arm.body, builtin))
        }
        S::While(condition, body) | S::For(_, condition, body) => {
            expr(condition, builtin) || calls_builtin(body, builtin)
        }
        S::Scope(body) => calls_builtin(body, builtin),
    })
}

fn charge_impl_start(out: &mut Generated, name: &str, inline_only: bool) {
    out.push_str(&format!(
        "impl ::nagi_runtime::actor::ChargeOwned for {name} {{\n    const INLINE_ONLY: ::std::primitive::bool = {inline_only};\n    fn owned_heap_bytes(&self, walk: &mut ::nagi_runtime::actor::ChargeWalk) -> ::std::result::Result<::std::primitive::usize, ::nagi_runtime::actor::ChargeError> {{\n"
    ));
}

fn charge_fields(out: &mut Generated, fields: impl Iterator<Item = String>, indent: &str) {
    out.push_str(&format!(
        "{indent}let mut __nagi_charge_heap: ::std::primitive::usize = 0;\n"
    ));
    for (index, field) in fields.enumerate() {
        out.push_str(&format!(
            "{indent}let __nagi_charge_field_{index} = walk.visit({field})?;\n{indent}__nagi_charge_heap = walk.add(__nagi_charge_heap, __nagi_charge_field_{index})?;\n"
        ));
    }
    out.push_str(&format!(
        "{indent}::std::result::Result::Ok(__nagi_charge_heap)\n"
    ));
}

pub fn rust_with_lines(p: &Program) -> Result<Generated, String> {
    crate::routes::validate(p)?;
    let names = crate::rust_names::RustNames::new(p);
    let mapped = names.program(p);
    let p = &mapped;
    // Direct parser/check callers have no module identities. Preserve their
    // legacy primitive-named records; resolved files use canonical class
    // symbols and keep intrinsic types independent of public adapter aliases.
    let types = RustTypes {
        raw_classes: if p.modules.root.is_none() {
            &p.classes
        } else {
            &[]
        },
        enums: &p.enums,
        modules: &p.modules,
    };
    fn copy_type(t: &Type, p: &Program, depth: usize) -> bool {
        if let Some(resource) = registered_resource(&t.0, &p.modules) {
            return crate::stdlib::resource_info(resource).copy;
        }
        if depth > 64 || matches!(t.0.as_str(), "str" | "bytes" | "Error" | "Db" | "Html") {
            false
        } else if let Some(enumeration) = p.enums.iter().find(|definition| definition.name == t.0) {
            enumeration.variants.iter().all(|variant| {
                variant
                    .fields
                    .iter()
                    .all(|(_, ty)| copy_type(ty, p, depth + 1))
            })
        } else if t.is_copy() {
            true
        } else if matches!(t.0.as_str(), "Option" | "owned") {
            copy_type(&t.inner(), p, depth + 1)
        } else if let ::std::option::Option::Some(c) = p.classes.iter().find(|c| c.name == t.0) {
            c.fields.iter().all(|(_, t)| copy_type(t, p, depth + 1))
        } else {
            false
        }
    }
    let mut out =
        Generated::new("#![allow(unused_mut, unused_parens, unused_variables, dead_code)]\n");
    let classes = p
        .classes
        .iter()
        .map(|class| (class.name.clone(), class.clone()))
        .collect();
    let enums = p
        .enums
        .iter()
        .map(|enumeration| (enumeration.name.clone(), enumeration.clone()))
        .collect();
    let actor_support = p.modules.definitions.iter().any(|definition| {
        definition.id.module.0
            == crate::stdlib::module_info(crate::stdlib::StandardModule::Actor).id
            && crate::stdlib::definition(&definition.id)
    });
    for c in &p.classes {
        out.origin(::std::option::Option::Some(c.line));
        let copy = c.fields.iter().all(|(_, t)| copy_type(t, p, 0));
        let serde = c
            .fields
            .iter()
            .all(|(_, ty)| crate::capabilities::serde_type(ty, &classes, &enums));
        let readable_debug = p.modules.definition(&c.name).is_some()
            || names.original(&c.name) != c.name
            || c.fields
                .iter()
                .any(|(field, _)| names.original(field) != field);
        let mut derives = Vec::new();
        if !readable_debug {
            derives.push("Debug");
        }
        if serde {
            derives.extend([
                "::nagi_runtime::serde::Serialize",
                "::nagi_runtime::serde::Deserialize",
            ]);
        }
        if copy {
            derives.extend(["Clone", "Copy"]);
        }
        out.push_str("#[allow(non_camel_case_types, non_snake_case)]\n");
        if !derives.is_empty() {
            out.push_str(&format!("#[derive({})]\n", derives.join(", ")));
        }
        if serde {
            out.push_str("#[serde(crate = \"::nagi_runtime::serde\", deny_unknown_fields)]\n");
        }
        out.push_str(&format!("pub struct {} {{\n", c.name));
        for (i, (n, t)) in c.fields.iter().enumerate() {
            out.origin(::std::option::Option::Some(
                c.field_lines.get(i).copied().unwrap_or(c.line),
            ));
            if serde && names.original(n) != n {
                out.push_str(&format!(
                    "    #[serde(rename = {})]\n",
                    quote(names.original(n))
                ));
            }
            out.push_str(&format!("    pub {n}: {},\n", types.ty(t)));
        }
        out.origin(::std::option::Option::Some(c.line));
        out.push_str("}\n");
        if readable_debug {
            out.origin(None);
            let label = p
                .modules
                .definition(&c.name)
                .map(|definition| definition.id.name.as_str())
                .unwrap_or_else(|| names.original(&c.name));
            out.push_str(&format!(
                "impl ::std::fmt::Debug for {} {{\n    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {{\n        formatter.debug_struct({})",
                c.name,
                quote(label)
            ));
            for (field, _) in &c.fields {
                out.push_str(&format!(
                    ".field({}, &self.{field})",
                    quote(names.original(field))
                ));
            }
            out.push_str(".finish()\n    }\n}\n");
        }
        let db_compatible = c.fields.iter().all(|(_, t)| {
            [
                "i8", "i16", "i32", "i64", "u8", "u16", "u32", "f32", "f64", "bool", "str", "bytes",
            ]
            .contains(&t.0.as_str())
                || t.0 == "Option" && ["i64", "i32", "str"].contains(&t.inner().0.as_str())
        });
        if db_compatible {
            out.origin(::std::option::Option::None);
            out.push_str(&format!("impl ::nagi_runtime::FromRow for {} {{\n fn columns() -> &'static [&'static ::std::primitive::str] {{ &[{}] }}\n fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> {{ ::std::result::Result::Ok(Self {{\n",c.name,c.fields.iter().map(|(n,_)|quote(names.original(n))).collect::<Vec<_>>().join(",")));
            for (i, (n, _)) in c.fields.iter().enumerate() {
                out.push_str(&format!("{n}: row.get(ix[{i}])?,\n"));
            }
            out.push_str("}) }\n}\n");
        }
        if actor_support
            && crate::capabilities::charge_type_supported(&Type::named(&c.name), &classes, &enums)
                .is_ok()
        {
            out.origin(None);
            charge_impl_start(
                &mut out,
                &c.name,
                crate::capabilities::charge_inline_only(&Type::named(&c.name), &classes, &enums),
            );
            charge_fields(
                &mut out,
                c.fields.iter().map(|(field, _)| format!("&self.{field}")),
                "        ",
            );
            out.push_str("    }\n}\n");
        }
    }
    for enumeration in &p.enums {
        out.origin(Some(enumeration.line));
        let copy = enumeration
            .variants
            .iter()
            .all(|variant| variant.fields.iter().all(|(_, ty)| copy_type(ty, p, 0)));
        let readable_debug = enumeration.variants.iter().any(|variant| {
            names.original(&variant.name) != variant.name
                || variant
                    .fields
                    .iter()
                    .any(|(field, _)| names.original(field) != field)
        });
        let mut derives = Vec::new();
        if !readable_debug {
            derives.push("Debug");
        }
        if copy {
            derives.extend(["Clone", "Copy"]);
        }
        out.push_str("#[allow(non_camel_case_types, non_snake_case)]\n");
        if !derives.is_empty() {
            out.push_str(&format!("#[derive({})]\n", derives.join(", ")));
        }
        out.push_str(&format!("pub enum {} {{\n", enumeration.name));
        for variant in &enumeration.variants {
            out.origin(Some(variant.line));
            out.push_str(&format!("    {}", variant.name));
            if !variant.fields.is_empty() {
                out.push_str(" {\n");
                for (index, (field, ty)) in variant.fields.iter().enumerate() {
                    out.origin(Some(
                        variant
                            .field_lines
                            .get(index)
                            .copied()
                            .unwrap_or(variant.line),
                    ));
                    out.push_str(&format!("        {field}: {},\n", types.ty(ty)));
                }
                out.origin(Some(variant.line));
                out.push_str("    }");
            }
            out.push_str(",\n");
        }
        out.origin(Some(enumeration.line));
        out.push_str("}\n");
        if readable_debug {
            out.origin(None);
            out.push_str(&format!(
                "impl ::std::fmt::Debug for {} {{\n    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {{\n        match self {{\n",
                enumeration.name
            ));
            for variant in &enumeration.variants {
                let label = quote(names.original(&variant.name));
                if variant.fields.is_empty() {
                    out.push_str(&format!(
                        "            Self::{} => formatter.write_str({label}),\n",
                        variant.name
                    ));
                } else {
                    out.push_str(&format!(
                        "            Self::{} {{ {} }} => formatter.debug_struct({label})",
                        variant.name,
                        variant
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(index, (field, _))| format!("{field}: __nagi_debug_{index}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    for (index, (field, _)) in variant.fields.iter().enumerate() {
                        out.push_str(&format!(
                            ".field({}, __nagi_debug_{index})",
                            quote(names.original(field))
                        ));
                    }
                    out.push_str(".finish(),\n");
                }
            }
            out.push_str("        }\n    }\n}\n");
        }
        if actor_support
            && crate::capabilities::charge_type_supported(
                &Type::named(&enumeration.name),
                &classes,
                &enums,
            )
            .is_ok()
        {
            out.origin(None);
            charge_impl_start(
                &mut out,
                &enumeration.name,
                crate::capabilities::charge_inline_only(
                    &Type::named(&enumeration.name),
                    &classes,
                    &enums,
                ),
            );
            out.push_str("        match self {\n");
            for variant in &enumeration.variants {
                if variant.fields.is_empty() {
                    out.push_str(&format!(
                        "            Self::{} => ::std::result::Result::Ok(0),\n",
                        variant.name
                    ));
                } else {
                    out.push_str(&format!(
                        "            Self::{} {{ {} }} => {{\n",
                        variant.name,
                        variant
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(index, (field, _))| format!(
                                "{field}: __nagi_charge_value_{index}"
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    charge_fields(
                        &mut out,
                        (0..variant.fields.len())
                            .map(|index| format!("__nagi_charge_value_{index}")),
                        "                ",
                    );
                    out.push_str("            },\n");
                }
            }
            out.push_str("        }\n    }\n}\n");
        }
    }
    for f in &p.functions {
        out.origin(::std::option::Option::Some(f.line));
        let name = if f.name == "main" {
            "__nagi_main"
        } else {
            &f.name
        };
        let lifetime = if f.params.iter().any(|(_, t)| t.contains_view()) || f.ret.contains_view() {
            "<'a>"
        } else {
            ""
        };
        out.push_str(&format!(
            "#[allow(non_snake_case)]\npub {}fn {name}{lifetime}({}) -> {} {{\n",
            if f.asynchronous { "async " } else { "" },
            f.params
                .iter()
                .map(|(n, t)| format!("mut {n}: {}", types.ty(t)))
                .collect::<Vec<_>>()
                .join(", "),
            types.ty(&f.ret)
        ));
        if f.external {
            let target = &f.attrs.iter().find(|(a, _)| a == "rust").unwrap().1;
            out.push_str(&format!(
                "    {target}({}){}\n",
                f.params
                    .iter()
                    .map(|(n, _)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                if f.asynchronous { ".await" } else { "" }
            ));
        } else {
            rb(&f.body, &mut out, 1, &types);
        }
        out.origin(::std::option::Option::Some(f.line));
        out.push_str("}\n");
    }
    out.origin(::std::option::Option::None);
    // Public adapter names are aliases of the unique generated item, not new
    // wrapper types. Only a module's own definitions are exposed through an
    // `as` import; its imported names are not implicitly reexported.
    for binding in p.modules.root_bindings() {
        match &binding.target {
            BindingTarget::Definition(id) => {
                let Some(definition) = p.modules.definition_id(id) else {
                    continue;
                };
                let alias = names.source_name(&binding.name);
                let symbol = names.definition(definition);
                let path = registered_rust_path(&definition.symbol, &p.modules)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("crate::{symbol}"));
                // The executable entry point owns Rust's root `main` name.
                if alias != symbol && !(alias == "main" && id.kind == DefKind::Function) {
                    out.push_str(&format!(
                        "#[allow(unused_imports)]\npub use {path} as {alias};\n"
                    ));
                }
            }
            BindingTarget::Module(module) => {
                let alias = names.source_name(&binding.name);
                out.push_str(&format!("#[allow(non_snake_case)]\npub mod {alias} {{\n"));
                for definition in p.modules.exports(module) {
                    let symbol = names.definition(definition);
                    let export = names.source_name(&definition.id.name);
                    let path = registered_rust_path(&definition.symbol, &p.modules)
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("crate::{symbol}"));
                    out.push_str(&format!(
                        "    #[allow(unused_imports)]\n    pub use {path} as {export};\n"
                    ));
                }
                out.push_str("}\n");
            }
        }
    }
    let routes: Vec<_> = p
        .functions
        .iter()
        .filter(|f| {
            f.attrs
                .iter()
                .any(|(a, _)| ["get", "post", "put", "delete"].contains(&a.as_str()))
        })
        .collect();
    if !routes.is_empty() || p.functions.iter().any(|f| calls_builtin(&f.body, "serve")) {
        out.push_str("async fn __nagi_serve(db: ::nagi_runtime::Db,port: ::std::primitive::i64) -> ::std::result::Result<(),::nagi_runtime::Error> {\nlet router= ::nagi_runtime::axum::Router::new()\n");
        let mut paths = std::collections::BTreeMap::<String, Vec<(String, usize)>>::new();
        for (i, f) in routes.iter().enumerate() {
            let (a, path) = f
                .attrs
                .iter()
                .find(|(a, _)| ["get", "post", "put", "delete"].contains(&a.as_str()))
                .unwrap();
            paths.entry(path.clone()).or_default().push((a.clone(), i));
        }
        for (path, methods) in paths {
            let mut it = methods.iter();
            let (m, i) = it.next().unwrap();
            out.push_str(&format!(
                ".route({}, ::nagi_runtime::axum::routing::{m}(__route_{i})",
                quote(&path)
            ));
            for (m, i) in it {
                out.push_str(&format!(".{m}(__route_{i})"));
            }
            out.push_str(")\n");
        }
        out.push_str(".with_state(db); ::nagi_runtime::serve(router,port).await\n}\n");
        for (i, f) in routes.iter().enumerate() {
            out.origin(::std::option::Option::Some(f.line));
            let (_, path) = crate::routes::attribute(f).unwrap();
            if !f.asynchronous || f.ret.0 != "Result" {
                return ::std::result::Result::Err(format!(
                    "route {} must be async and return Result",
                    f.name
                ));
            }
            let mut extracts = vec![];
            let mut call = vec![];
            let mut pre = String::new();
            let mut query_fields = vec![];
            for (parameter_index, (n, t)) in f.params.iter().enumerate() {
                if t.0 == "Db" {
                    extracts.push(format!(
                        "::nagi_runtime::axum::extract::State({n}): ::nagi_runtime::axum::extract::State<::nagi_runtime::Db>"
                    ));
                    call.push(n.clone());
                } else if n == "id" && t.0 == "i64" && crate::routes::has_capture(path) {
                    extracts.push("::nagi_runtime::axum::extract::Path(id): ::nagi_runtime::axum::extract::Path<::std::primitive::i64>".into());
                    call.push(n.clone());
                } else if t == &Type::generic("view", vec![Type::named("bytes")]) {
                    extracts.push(format!("{n}: ::nagi_runtime::axum::body::Bytes"));
                    call.push(format!("&{n}"));
                } else if p.classes.iter().any(|c| c.name == t.0) {
                    extracts.push(format!(
                        "__body_{parameter_index}: ::nagi_runtime::axum::body::Bytes"
                    ));
                    pre.push_str(&format!("let {n}: {} = match ::nagi_runtime::decode(&__body_{parameter_index}) {{::std::result::Result::Ok(x)=>x,::std::result::Result::Err(e)=>return ::nagi_runtime::error_response(e)}};\n",types.ty(t)));
                    call.push(n.clone());
                } else if ["str", "i64", "i32", "u64", "bool", "f64"].contains(&t.0.as_str()) {
                    query_fields.push((n.clone(), t.clone()));
                    pre.push_str(&format!("let {n}=__query.{n};\n"));
                    call.push(n.clone());
                } else {
                    return ::std::result::Result::Err(format!(
                        "route parameter {n}: {t} は未対応です"
                    ));
                }
            }
            if !query_fields.is_empty() {
                out.push_str(&format!("#[derive(::nagi_runtime::serde::Deserialize)]\n#[serde(crate=\"::nagi_runtime::serde\")]\nstruct __NagiQuery{i} {{ {} }}\n",query_fields.iter().map(|(n,t)|format!("#[serde(rename = {})] {n}: {}",quote(names.original(n)),types.ty(t))).collect::<Vec<_>>().join(",")));
                extracts.push(format!(
                    "::nagi_runtime::axum::extract::Query(__query): ::nagi_runtime::axum::extract::Query<__NagiQuery{i}>"
                ));
            }
            // Body extractorはAxumの規則に従い最後。引数の順序は元の関数を維持する。
            extracts.sort_by_key(|s| s.contains("body::Bytes"));
            let invocation = format!("crate::{}({}).await", f.name, call.join(", "));
            let response = if f.ret.inner().0 == "Html" {
                format!("match {invocation} {{ ::std::result::Result::Ok(v)=>::nagi_runtime::axum::response::IntoResponse::into_response(v),::std::result::Result::Err(e)=>::nagi_runtime::error_response(e) }}")
            } else if f.ret.inner().0 == "Option" {
                format!("match {invocation} {{ ::std::result::Result::Ok(::std::option::Option::Some(v))=>::nagi_runtime::response(::std::result::Result::Ok(v)), ::std::result::Result::Ok(::std::option::Option::None)=>::nagi_runtime::error_response(::nagi_runtime::Error::not_found()),::std::result::Result::Err(e)=>::nagi_runtime::error_response(e) }}")
            } else {
                format!("::nagi_runtime::response({invocation})")
            };
            out.push_str(&format!(
                "async fn __route_{i}({}) -> ::nagi_runtime::axum::response::Response {{\n{pre}{response}\n}}\n",
                extracts.join(", ")
            ));
        }
    }
    out.origin(::std::option::Option::None);
    if let ::std::option::Option::Some(f) = p.functions.iter().find(|f| f.name == "main") {
        if !f.params.is_empty() {
            return ::std::result::Result::Err("mainは引数を取りません".into());
        }
        if f.asynchronous {
            out.push_str("fn main() { ::nagi_runtime::block_on(async {\n");
        } else {
            out.push_str("fn main() {\n");
        }
        let call = if f.asynchronous {
            "__nagi_main().await"
        } else {
            "__nagi_main()"
        };
        if f.ret.0 == "Result" {
            let mut error_type = &f.ret.1[1];
            while error_type.0 == "owned" {
                error_type = &error_type.1[0];
            }
            let display = if error_type.0 == "Error" {
                "{}"
            } else {
                "{:?}"
            };
            out.push_str(&format!(
                "if let ::std::result::Result::Err(e) = {call} {{ eprintln!({},e); ::std::process::exit(1); }}\n",
                quote(display)
            ));
        } else {
            out.push_str(&format!("{call};\n"));
        }
        out.push_str(if f.asynchronous { "}); }\n" } else { "}\n" });
    } else {
        out.push_str("fn main() {}\n");
    }
    ::std::result::Result::Ok(out)
}

fn cargo_manifest(
    name: &str,
    runtime: PathBuf,
    mut dependencies: std::collections::BTreeMap<String, crate::project::RustDependency>,
) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Package<'a> {
        name: &'a str,
        version: &'static str,
        edition: &'static str,
    }
    #[derive(serde::Serialize)]
    struct Release {
        #[serde(rename = "opt-level")]
        opt_level: u8,
        lto: bool,
        #[serde(rename = "codegen-units")]
        codegen_units: u8,
        panic: &'static str,
    }
    #[derive(serde::Serialize)]
    struct Profile {
        release: Release,
    }
    #[derive(serde::Serialize)]
    struct Manifest<'a> {
        package: Package<'a>,
        workspace: toml::Table,
        dependencies: std::collections::BTreeMap<String, crate::project::RustDependency>,
        profile: Profile,
    }
    dependencies.insert(
        "nagi-runtime".into(),
        crate::project::RustDependency::Detailed(crate::project::RustDependencyTable {
            path: Some(runtime),
            ..Default::default()
        }),
    );
    toml::to_string(&Manifest {
        package: Package {
            name,
            version: "0.1.0",
            edition: "2021",
        },
        workspace: toml::Table::new(),
        dependencies,
        profile: Profile {
            release: Release {
                opt_level: 3,
                lto: false,
                codegen_units: 1,
                panic: "unwind",
            },
        },
    })
    .map_err(|error| {
        format!("Cargo.tomlの生成に失敗しました。パスにはUTF-8文字列が必要です: {error}")
    })
}

pub fn cli(args: Vec<String>) -> Result<(), String> {
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "version" | "--version" | "-V"))
    {
        if args.len() != 1 {
            return Err("version takes no additional arguments".into());
        }
        println!("nagic {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let help = matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h")
    ) || args.first().is_some_and(|command| {
        ["check", "lower", "build", "run", "symbols", "map"].contains(&command.as_str())
            && args[1..].iter().any(|arg| arg == "--help" || arg == "-h")
    });
    if help {
        println!(
            "Nagi compiler {}\n\n{}",
            env!("CARGO_PKG_VERSION"),
            crate::project::USAGE
        );
        return Ok(());
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let (options, map_options) = if args.first().map(String::as_str) == Some("map") {
        let (options, map) = crate::project::resolve_map(&args, &cwd)?;
        (options, Some(map))
    } else {
        (crate::project::resolve(&args, &cwd)?, None)
    };
    let map_manifest = if map_options.is_some() {
        options.project_root.as_ref().map(|root| {
            args.iter()
                .position(|arg| arg == "--project")
                .map(|index| cwd.join(&args[index + 1]))
                .map(|selected| {
                    if selected.is_dir() {
                        selected.join("nagi.toml")
                    } else {
                        selected
                    }
                })
                .unwrap_or_else(|| root.join("nagi.toml"))
        })
    } else {
        None
    };
    let cmd = options.command.as_str();
    let path = options.source;
    let high = path.extension().is_none_or(|x| x != "low");
    let overlays = if options.editor_input {
        crate::symbols::read_overlays(std::io::stdin().lock(), &cwd)?
    } else {
        std::collections::HashMap::new()
    };
    let mut sources = crate::source::load_with_overlays(&path, high, &overlays)?;
    let mut p = std::mem::take(&mut sources.program);
    let native = options.native;
    let out = options.out;
    let cost = options.cost;
    let rust_file = options.rust_file;
    let rust_deps = options.rust_dependencies;
    let mut all = Program::default();
    for n in native {
        let native_sources = crate::source::load_with_overlays(&n, false, &overlays)?;
        let mut np = sources.append(native_sources);
        crate::modules::prepare_native_fragment(&mut np, &p);
        for class in np.classes {
            if !all.classes.iter().any(|old| old.name == class.name) {
                all.classes.push(class);
            }
        }
        for enumeration in np.enums {
            if !all.enums.iter().any(|old| old.name == enumeration.name) {
                all.enums.push(enumeration);
            }
        }
        for function in np.functions {
            if !all.functions.iter().any(|old| old.name == function.name) {
                all.functions.push(function);
            }
        }
        all.modules
            .merge_native(np.modules)
            .map_err(|e| sources.diagnostic(&e))?;
    }
    crate::modules::rebind_native(&mut p, &mut all).map_err(|e| sources.diagnostic(&e))?;
    if cmd == "symbols" {
        println!("{}", crate::symbols::index(&sources, &[&p, &all])?);
        return Ok(());
    }
    // 手書きLowの通常関数はHighの名前解決にも使う。置換本体はLowの統合時に検査する。
    let mut resolution = p.clone();
    let nc = p.classes.len();
    let ne = p.enums.len();
    let nf = p.functions.len();
    let mut native_resolution = all.clone();
    native_resolution
        .functions
        .retain(|f| !f.attrs.iter().any(|(a, _)| a == "replace"));
    crate::modules::synchronize(&mut native_resolution);
    for class in &native_resolution.classes {
        if !resolution.classes.iter().any(|old| old.name == class.name) {
            resolution.classes.push(class.clone());
        }
    }
    for enumeration in &native_resolution.enums {
        if !resolution
            .enums
            .iter()
            .any(|old| old.name == enumeration.name)
        {
            resolution.enums.push(enumeration.clone());
        }
    }
    for function in &native_resolution.functions {
        if !resolution
            .functions
            .iter()
            .any(|old| old.name == function.name)
        {
            resolution.functions.push(function.clone());
        }
    }
    resolution
        .modules
        .merge(native_resolution.modules)
        .map_err(|e| sources.diagnostic(&e))?;
    crate::modules::synchronize(&mut resolution);
    crate::check::check(&mut resolution).map_err(|e| sources.diagnostic(&e))?;
    p.classes = resolution.classes[..nc].to_vec();
    p.enums = resolution.enums[..ne].to_vec();
    p.functions = resolution.functions[..nf].to_vec();
    let write_output = map_options.is_none() && !(options.editor_input && cmd == "check");
    if write_output {
        fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    }
    if high {
        let low_source = low_with_lines(&p);
        if write_output {
            fs::write(out.join("generated.low"), &low_source.text).map_err(|e| e.to_string())?;
        }
        // 生成Lowの文字列を独立parserに通す。High ASTをcodegenへ直接渡さない。
        p = crate::parser::parse(&low_source.text, false)?;
        low_source.restore_lines(&mut p)?;
    }
    crate::check::integrate(&mut p, all).map_err(|e| sources.diagnostic(&e))?;
    if let Some(map) = map_options {
        let graph = match map.view {
            crate::project::MapView::Types => crate::graph::types(&p, &sources),
            crate::project::MapView::Modules => crate::graph::modules(&p, &sources),
            crate::project::MapView::Calls => crate::graph::calls(&p, &sources),
        };
        let module = map
            .module
            .as_deref()
            .map(|name| crate::graph::resolve_module_filter(&p, &sources, name))
            .transpose()?;
        let graph = graph.filtered(&crate::graph::Filter {
            module,
            focus: map.focus.clone(),
            depth: map.depth,
        })?;
        if let Some(output) = &map.output {
            protect_map_output(
                output,
                &sources,
                rust_file.as_deref(),
                map_manifest.as_deref(),
            )?;
        }
        use crate::project::MapFormat;
        let mut text = match map.format {
            MapFormat::Mermaid => crate::graph::render::mermaid(&graph),
            MapFormat::D2 => crate::graph::render::d2(&graph),
            MapFormat::Json => serde_json::to_string_pretty(&graph)
                .map_err(|error| format!("Cannot serialize map: {error}"))?,
            MapFormat::Html => crate::graph::html::html(&graph),
            MapFormat::Svg | MapFormat::Png => {
                return crate::graph::render::image(
                    &graph,
                    map.output.as_deref().expect("validated image output"),
                    map.format.as_str(),
                    map.layout.as_str(),
                );
            }
        };
        if !text.ends_with('\n') {
            text.push('\n');
        }
        if let Some(output) = &map.output {
            fs::write(output, text)
                .map_err(|error| format!("Cannot write map {}: {error}", output.display()))?;
        } else {
            std::io::stdout()
                .lock()
                .write_all(text.as_bytes())
                .map_err(|error| format!("Cannot write map to stdout: {error}"))?;
        }
        return Ok(());
    }
    if cost {
        let report = cost_report(&p);
        if write_output {
            fs::write(
                out.join("cost-report.json"),
                serde_json::to_string_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
        }
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }
    if cmd == "check" || cmd == "lower" {
        println!("checked {}", path.display());
        return Ok(());
    }
    if cmd != "build" && cmd != "run" {
        return Err(format!("unknown command: {cmd}"));
    }
    let root = crate::installation::root()?;
    fs::create_dir_all(out.join("src")).map_err(|e| e.to_string())?;
    if p.functions.iter().any(|f| f.external) && rust_file.is_none() {
        return Err("extern関数のビルドには--rust FILE.rsが必要です".into());
    }
    let mut generated_rust = rust_with_lines(&p)?;
    if let Some(file) = &rust_file {
        generated_rust.origin(None);
        generated_rust.push_str(&format!(
            "\n#[path = {}]\nmod native;\n",
            quote(&file.display().to_string())
        ));
    }
    fs::write(out.join("src/main.rs"), &generated_rust.text).map_err(|e| e.to_string())?;
    let generated_file = fs::canonicalize(out.join("src/main.rs")).map_err(|e| e.to_string())?;
    let package = format!(
        "nagi-{}",
        path.file_stem()
            .unwrap()
            .to_string_lossy()
            .chars()
            .map(|ch| {
                if ch == '_'
                    || (ch != '-' && (!ch.is_alphanumeric() || !unicode_ident::is_xid_continue(ch)))
                {
                    '-'
                } else {
                    ch
                }
            })
            .collect::<String>()
    );
    let manifest = cargo_manifest(
        &package,
        relative_path(
            &root.join("runtime"),
            &fs::canonicalize(&out).map_err(|e| e.to_string())?,
        ),
        rust_deps,
    )?;
    fs::write(out.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(|p| cwd.join(p))
        .unwrap_or_else(|| {
            options
                .project_root
                .as_ref()
                .map(|p| p.join("build/native-target"))
                .unwrap_or_else(|| cwd.join("native-target"))
        });
    let mut child = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(out.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        // In-place progress can overwrite mapped diagnostics on the same terminal.
        .env("CARGO_TERM_PROGRESS_WHEN", "never")
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("Cargoが見つかりません。Rust/Cargoを導入するか、導入済みならPATHを確認してください。\n確認: cargo --version\nPATHを変更した場合はターミナルとVS Codeを開き直してください。\n{e}")
            } else {
                format!("Cargoを起動できません: {e}\nCargoの実行権限やファイルの状態を確認してください。")
            }
        })?;
    let stdout = child.stdout.take().unwrap();
    for line in BufReader::new(stdout).lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Cargoの診断を読み取れません: {e}"));
            }
        };
        if let Some(message) =
            crate::diagnostics::cargo_message(&line, &generated_rust, &generated_file, &sources)
        {
            eprint!("{message}");
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Build failed. Check the diagnostics above for code, dependency, or build environment errors.".into());
    }
    let binary = target
        .join("release")
        .join(format!("{package}{}", std::env::consts::EXE_SUFFIX));
    println!("native: {}", binary.display());
    if cmd == "run" {
        let mut process = Command::new(binary);
        if let Some(root) = options.project_root {
            process.current_dir(root);
        }
        let status = process.status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("program exited: {status}"));
        }
    }
    Ok(())
}

fn protect_map_output(
    output: &Path,
    sources: &crate::source::Sources,
    rust_file: Option<&Path>,
    manifest: Option<&Path>,
) -> Result<(), String> {
    let existing = match fs::canonicalize(output) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Cannot access map output {}: {error}",
                output.display()
            ));
        }
    };
    for input in sources
        .files()
        .map(|(path, _, _)| path)
        .chain(rust_file)
        .chain(manifest)
    {
        if fs::canonicalize(input).is_ok_and(|path| path == existing) {
            return Err(format!(
                "Map output cannot overwrite input file: {}",
                input.display()
            ));
        }
    }
    Ok(())
}
pub fn cost_report(p: &Program) -> serde_json::Value {
    fn walk(e: &Expr, a: &mut Vec<serde_json::Value>) {
        match &e.kind{
        E::Str(_)=>a.push(serde_json::json!({"line":e.line,"kind":"owned_string_literal","cost":"allocation/copy unless borrowed by intrinsic/codegen"})),
        E::List(xs)=>{a.push(serde_json::json!({"line":e.line,"kind":"contiguous_list","cost":"heap allocation; element boxing 0"}));for e in xs{walk(e,a);}},
        E::Call(n,_,xs)=>{if e.resolution == Some(NameResolution::Builtin) && ["copy","share","clone_shared","json_decode","json_encode","db_query","db_all","db_insert","db_update"].contains(&n.as_str()){a.push(serde_json::json!({"line":e.line,"kind":n,"cost":"runtime/input dependent; measure allocation counters"}));}for e in xs{walk(e,a);}},E::Binary(ae,_,b)=>{walk(ae,a);walk(b,a)},E::Unary(_,e)|E::Try(e)|E::Await(e)|E::Field(e,_)=>walk(e,a),E::Record(_,fs)=>for(_,e)in fs{walk(e,a)},E::Index(e,i)=>{walk(e,a);walk(i,a)},_=>{}}
    }
    fn stmts(ss: &[Stmt], a: &mut Vec<serde_json::Value>) {
        for s in ss {
            match &s.kind {
                S::Assign { value, .. } | S::Expr(value) | S::Spawn(value) => walk(value, a),
                S::Return(Some(e)) => walk(e, a),
                S::If(c, x, y) => {
                    walk(c, a);
                    stmts(x, a);
                    stmts(y, a)
                }
                S::For(_, e, b) | S::While(e, b) => {
                    walk(e, a);
                    stmts(b, a)
                }
                S::Scope(b) => stmts(b, a),
                S::Match(value, arms) => {
                    walk(value, a);
                    for arm in arms {
                        stmts(&arm.body, a);
                    }
                }
                _ => {}
            }
        }
    }
    let mut fs = serde_json::Map::new();
    for f in &p.functions {
        let mut a = vec![];
        stmts(&f.body, &mut a);
        let name = if let Some(definition) = p.modules.definition(&f.name) {
            let direct = p
                .modules
                .root_bindings()
                .filter(|binding| {
                    binding.target == BindingTarget::Definition(definition.id.clone())
                })
                .min_by_key(|binding| binding.name != definition.id.name);
            if let Some(binding) = direct {
                binding.name.clone()
            } else if let Some(binding) = p.modules.root_bindings().find(|binding| {
                binding.target == BindingTarget::Module(definition.id.module.clone())
            }) {
                format!("{}.{}", binding.name, definition.id.name)
            } else {
                format!("{}::{}", definition.id.module.0, definition.id.name)
            }
        } else {
            f.name.clone()
        };
        fs.insert(name,serde_json::json!({"static_sites":a,"primitive_boxing":0,"warning":"sites are not dynamic allocation counts; runtime and loop multiplicity excluded"}));
    }
    serde_json::json!({"format":"nagi-cost-sites-v1","functions":fs})
}

fn relative_path(target: &Path, base: &Path) -> PathBuf {
    let a: Vec<_> = target.components().collect();
    let b: Vec<_> = base.components().collect();
    let same = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if same == 0 {
        return target.to_path_buf();
    }
    let mut out = PathBuf::new();
    for _ in same..b.len() {
        out.push("..");
    }
    for c in &a[same..] {
        out.push(c.as_os_str());
    }
    out
}
