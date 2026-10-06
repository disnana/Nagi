use crate::ast::*;
use crate::checked::{
    BlockPlan, CheckedProgram, CompareRead, CopyRead, EmissionPlan, ExpressionKey, ExpressionPlan,
    FunctionPlan, IteratorRead, MainError, RouteInput, RouteOutput, StaticRead, ViewRead,
};
use crate::diagnostics::Generated;
use std::{
    collections::{BTreeMap, BTreeSet},
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
    if t.0 == "Option" && t.inner().0 != "Option" {
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
                }
                | S::SpawnBind {
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
                    if matches!(s.kind, S::SpawnBind { .. }) {
                        format!("spawn {}", expr(value))
                    } else {
                        expr(value)
                    }
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
    crate::check::checked::unmapped_rust_type(t)
}
struct RustTypes<'a> {
    enums: &'a [Enum],
    plan: Option<&'a EmissionPlan>,
    function: Option<&'a FunctionPlan>,
    storage_slots: &'a BTreeSet<String>,
    expression_uses: &'a BTreeMap<ExprUseId, ExprUseMode>,
    function_error: Option<&'a Type>,
    error_exit: ErrorExit,
}

#[derive(Clone, Copy)]
enum ErrorExit {
    Function,
    Scope(usize),
}

fn error_exit(error: &str, types: &RustTypes<'_>) -> String {
    let error = format!("::std::result::Result::Err(::std::convert::From::from({error}))");
    match types.error_exit {
        ErrorExit::Function => format!("return {error}"),
        ErrorExit::Scope(depth) => format!("break '__nagi_scope_body_{depth} {error}"),
    }
}

fn try_result(value: String, types: &RustTypes<'_>) -> String {
    match types.error_exit {
        ErrorExit::Function => format!("({value})?"),
        ErrorExit::Scope(_) => format!(
            "match ({value}) {{ ::std::result::Result::Ok(__nagi_try_value) => __nagi_try_value, ::std::result::Result::Err(__nagi_try_error) => {} }}",
            error_exit("__nagi_try_error", types)
        ),
    }
}

impl RustTypes<'_> {
    fn expression(&self, e: &Expr) -> &ExpressionPlan {
        &self.function.expect("sealed function plan").expressions[&ExpressionKey::of(e)]
    }

    fn ty(&self, t: &Type) -> String {
        self.plan
            .expect("sealed type plan")
            .rust_types
            .get(t)
            .expect("sealed Rust type")
            .clone()
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

fn local_type(t: &Type, types: &RustTypes<'_>) -> String {
    types.ty(t).replace("&'a ", "&")
}
// These borrowed reads need no temporary String. error_kind returns a
// static label, so later arguments may still move its Error without extending
// a borrow. Owned assignments/returns continue to use re() and make a String.
fn static_string(e: &Expr, types: &RustTypes<'_>) -> Option<String> {
    match types.expression(e).static_read {
        StaticRead::Literal => {
            let E::Str(value) = &e.kind else {
                unreachable!("sealed literal")
            };
            Some(quote(value))
        }
        StaticRead::ErrorKind => {
            let E::Call(_, _, args) = &e.kind else {
                unreachable!("sealed call")
            };
            Some(format!(
                "::nagi_runtime::error_kind(&({}))",
                re(&args[0], types)
            ))
        }
        StaticRead::None => None,
    }
}

fn string_arg(e: &Expr, types: &RustTypes<'_>) -> String {
    static_string(e, types).unwrap_or_else(|| format!("&({})", re(e, types)))
}

fn reference_arg(e: &Expr, types: &RustTypes<'_>) -> String {
    if let Some(borrowed) = static_string(e, types) {
        borrowed
    } else if types.expression(e).reference_is_view {
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
            types
                .expression(e)
                .numeric_suffix
                .as_ref()
                .map_or_else(|| s.clone(), |suffix| format!("{s}{suffix}"))
        }
        E::Name(s) => {
            if types.storage_slots.contains(s) {
                match types.expression_uses[&ExprUseId::of(e)] {
                    ExprUseMode::Move => format!("{s}.expect(\"checked view binding\")"),
                    ExprUseMode::Copy | ExprUseMode::Borrow => {
                        format!("(*{s}.as_ref().expect(\"checked view binding\"))")
                    }
                    ExprUseMode::BorrowMut => {
                        format!("(*{s}.as_mut().expect(\"checked view binding\"))")
                    }
                }
            } else if types.expression(e).argument_resolution == Some(NameResolution::BorrowedLocal)
            {
                format!("(*{s})")
            } else if types.expression(e).argument_resolution == Some(NameResolution::Standard) {
                types
                    .expression(e)
                    .symbol_path
                    .clone()
                    .expect("sealed standard path")
            } else if types.expression(e).argument_resolution
                == ::std::option::Option::Some(NameResolution::Function)
            {
                format!("crate::{s}")
            } else {
                s.clone()
            }
        }
        E::Str(s) => format!("::std::string::String::from({})", quote(s)),
        E::Bool(b) => b.to_string(),
        E::Null => "::std::option::Option::None".into(),
        E::Binary(a, o, b) => {
            let (left, right) = types
                .expression(e)
                .comparison
                .expect("sealed comparison plan");
            let operand = |value: &Expr, read: CompareRead| match read {
                CompareRead::Static => static_string(value, types).expect("sealed static operand"),
                CompareRead::Str => format!("({}).as_str()", re(value, types)),
                CompareRead::Slice => format!("({}).as_slice()", re(value, types)),
                CompareRead::Value => re(value, types),
            };
            format!(
                "({} {} {})",
                operand(a, left),
                match o.as_str() {
                    "and" => "&&",
                    "or" => "||",
                    _ => o,
                },
                operand(b, right)
            )
        }
        E::Unary(o, x) => {
            if let Some(ty) = &types.expression(e).minimum {
                format!("(::std::primitive::{ty}::MIN)")
            } else {
                format!("{}({})", if o == "not" { "!" } else { o }, re(x, types))
            }
        }
        E::Field(x, n) if types.expression(e).argument_resolution == Some(NameResolution::Enum) => {
            types
                .expression(e)
                .symbol_path
                .clone()
                .expect("sealed enum path")
        }
        E::Field(x, n)
            if types.expression(e).argument_resolution
                == Some(NameResolution::ResourceConstant) =>
        {
            types
                .expression(e)
                .symbol_path
                .clone()
                .expect("sealed constant path")
        }
        E::Field(x, n)
            if types.expression(e).argument_resolution == Some(NameResolution::ResourceField) =>
        {
            let (accessor, owned) = types
                .expression(e)
                .field
                .as_ref()
                .expect("sealed field accessor");
            format!(
                "({}).{}{}",
                re(x, types),
                accessor,
                if *owned { "" } else { "()" }
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
            let path = if types.expression(e).argument_resolution == Some(NameResolution::Enum) {
                rust_enum_path(n)
            } else {
                n.clone()
            };
            if types.expression(e).argument_resolution == Some(NameResolution::Enum) && a.is_empty()
            {
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
        E::Await(x) => {
            if let Some(use_) = types.expression(e).task {
                assert_eq!(use_.action, TaskAction::Receive);
                format!(
                    "__nagi_task_scope_{}.receive({}).await",
                    use_.scope.0,
                    re(x, types)
                )
            } else {
                format!("({}).await", re(x, types))
            }
        }
        E::Try(x) => try_result(re(x, types), types),
        E::Call(n, ts, a) => {
            if let Some(use_) = types.expression(e).task {
                assert_eq!(use_.action, TaskAction::Discard);
                return format!(
                    "__nagi_task_scope_{}.discard({})",
                    use_.scope.0,
                    re(&a[0], types)
                );
            }
            if types.expression(e).argument_resolution == Some(NameResolution::Standard) {
                let operation = types
                    .expression(e)
                    .operation
                    .as_ref()
                    .expect("sealed operation");
                let (path, parameters, emit_type_arguments) = match operation {
                    crate::check::checked::OperationPlan::IdentityTransfer { argument } => {
                        // Consume or copy into a value temporary even when the caller
                        // only borrows the result. Parentheses would preserve a place;
                        // a block would shorten lifetimes of borrowed temporaries.
                        return format!("::std::convert::identity({})", re(&a[*argument], types));
                    }
                    crate::check::checked::OperationPlan::NativeCall {
                        path,
                        parameters,
                        emit_type_arguments,
                    } => (path, parameters, emit_type_arguments),
                };
                let arguments = a
                    .iter()
                    .zip(parameters)
                    .map(|(argument, passing)| match passing {
                        crate::stdlib::Passing::Reference => reference_arg(argument, types),
                        crate::stdlib::Passing::Borrow => {
                            format!("&({})", re(argument, types))
                        }
                        crate::stdlib::Passing::Move
                        | crate::stdlib::Passing::Handler
                        | crate::stdlib::Passing::Mapper => re(argument, types),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let generic = if ts.is_empty() || !*emit_type_arguments {
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
                return format!("{}{generic}({arguments})", path);
            }
            let args = a.iter().map(|e| re(e, types)).collect::<Vec<_>>();
            let join = args.join(", ");
            match types.expression(e).argument_resolution {
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
                    if types.expression(e).argument_resolution == Some(NameResolution::Builtin) {
                        if let Some(borrowed) = static_string(&a[0], types) {
                            return borrowed;
                        }
                    }
                    match types.expression(e).view {
                        ViewRead::Native => format!("&({})", args[0]),
                        ViewRead::Str => format!("({}).as_str()", args[0]),
                        ViewRead::Slice => format!("({}).as_slice()", args[0]),
                    }
                }
                "copy" => match types.expression(e).copy {
                    CopyRead::Native => format!("*({})", args[0]),
                    CopyRead::List => format!("({}).to_vec()", args[0]),
                    CopyRead::Owned => format!("({}).to_owned()", args[0]),
                },
                "share" => format!("::std::sync::Arc::new({})", args[0]),
                "clone_shared" => format!("::std::sync::Arc::clone(&{})", args[0]),
                "len" => format!("(({}).len() as ::std::primitive::i64)", string_or_value(&a[0], types)),
                "range" => format!("0i64..{}", args[0]),
                "append" => {
                    if matches!(&a[0].kind, E::Name(name) if types.storage_slots.contains(name)) {
                        // The checked receiver is a pure local place. Evaluate
                        // its item before projecting optional storage, so a
                        // shared read of the same container retains the source
                        // Vec::push two-phase borrow behavior.
                        debug_assert_eq!(types.expression_uses[&ExprUseId::of(&a[0])], ExprUseMode::BorrowMut);
                        format!("{{ let __nagi_view_flow_item = {}; {}.push(__nagi_view_flow_item) }}", args[1], args[0])
                    } else {
                        format!("{}.push({})", args[0], args[1])
                    }
                }
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
                    // Source try/await must stay in its lexical error/async
                    // context. Match also keeps the fallback lazy and retains
                    // the existing string copy only on the missing-key path.
                    "match ::std::env::var({}) {{ ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ({}).to_owned() }}",
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
                        std::iter::once(if types.expression(e).sql_static {
                            let E::Str(s) = &a[1].kind else { unreachable!("sealed static SQL") };
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
                    let input = if types.expression(e).json_string {
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
                    if types.expression(e).slice_string {
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
    static_string(e, types).unwrap_or_else(|| re(e, types))
}
fn flow_actions(
    actions: &[crate::view_flow::Action],
    out: &mut Generated,
    n: usize,
    types: &RustTypes<'_>,
) {
    use crate::view_flow::Action;
    let pad = "    ".repeat(n);
    out.origin(None);
    for action in actions {
        match action {
            Action::DeclareSlot { name, ty } => out.push_str(&format!(
                "{pad}let mut {name}: ::std::option::Option<{}>;\n",
                local_type(ty, types)
            )),
            Action::Retire { slot } => out.push_str(&format!(
                "{pad}{slot} = ::std::option::Option::None;\n{pad}::std::mem::drop({slot});\n"
            )),
            Action::InitEmpty { slot } => {
                out.push_str(&format!("{pad}{slot} = ::std::option::Option::None;\n"));
            }
            Action::Transfer { from, to } => out.push_str(&format!("{pad}{to} = {from};\n")),
            Action::Install { from, to } => out.push_str(&format!(
                "{pad}{to} = ::std::option::Option::Some({from});\n"
            )),
        }
    }
}

fn rb_child(
    ss: &[Stmt],
    out: &mut Generated,
    n: usize,
    types: &RustTypes<'_>,
    plan: &BlockPlan,
    flow: Option<&crate::view_flow::Block>,
) {
    for (name, ty) in &plan.shadow {
        out.origin(None);
        out.push_str(&format!(
            "{}let mut {name}: {} = {name};\n",
            "    ".repeat(n),
            local_type(ty, types)
        ));
    }
    rb(ss, out, n, types, plan, flow);
}

fn rb(
    ss: &[Stmt],
    out: &mut Generated,
    n: usize,
    types: &RustTypes<'_>,
    plan: &BlockPlan,
    flow: Option<&crate::view_flow::Block>,
) {
    if let Some(flow) = flow {
        flow_actions(&flow.entry, out, n, types);
    }
    let pad = "    ".repeat(n);
    for (index, original) in ss.iter().enumerate() {
        let decision = &plan.statements[index];
        let node = flow.map(|block| &block.statements[index]);
        let s = node.map_or(original, |node| &node.stmt);
        if let Some(node) = node {
            flow_actions(&node.before, out, n, types);
        }
        out.origin(::std::option::Option::Some(s.line));
        out.push_str(&pad);
        match &s.kind {
            S::Assign {
                name,
                annotation,
                value,
                declare,
            }
            | S::SpawnBind {
                name,
                annotation,
                value,
                declare,
            } => {
                // A block that returns has no outgoing binding updates.
                // Give each top-level immutable view assignment its own
                // inferred lifetime, rather than unifying an earlier local
                // borrow with a later returned input view. Direct views are
                // Copy references and have no destructor. Continuing child
                // blocks still mutate their parent's binding, so branch and
                // loop updates remain visible. Owning containers must retain
                // assignment and destruction semantics.
                if let Some(assignment) = node.and_then(|node| node.assignment.as_ref()) {
                    // Evaluate the RHS before retiring the old allocation.
                    // The independent slot retains the source declaration's
                    // cleanup position and has its own inferred lifetime.
                    out.push_str(&format!(
                        "let {} = {};\n",
                        assignment.rhs_temp,
                        re(value, types)
                    ));
                    // Install at the original cleanup anchor before dropping
                    // the old value. Rust assignment also retains the new RHS
                    // there when an old element's destructor unwinds.
                    out.origin(None);
                    out.push_str(&format!(
                        "{pad}{} = ::std::option::Option::Some({});\n",
                        assignment.new_slot, assignment.rhs_temp
                    ));
                    for old in &assignment.retire_slots {
                        flow_actions(
                            &[crate::view_flow::Action::Retire { slot: old.clone() }],
                            out,
                            n,
                            types,
                        );
                    }
                } else {
                    let rebind = decision.rebind;
                    let storage = types.storage_slots.contains(name);
                    let value = if matches!(s.kind, S::SpawnBind { .. }) {
                        let id = decision.scope.expect("sealed SpawnBind scope");
                        format!("{{ let __nagi_spawn_future = {}; __nagi_task_scope_{}.spawn_value(async move {{ __nagi_spawn_future.await }}) }}",re(value,types),id.0)
                    } else {
                        re(value, types)
                    };
                    out.push_str(&format!(
                        "{}{name}{} = {};\n",
                        if *declare || rebind { "let mut " } else { "" },
                        if decision.annotate {
                            let ty = local_type(annotation.as_ref().unwrap(), types);
                            format!(
                                ": {}",
                                if storage {
                                    format!("::std::option::Option<{ty}>")
                                } else {
                                    ty
                                }
                            )
                        } else {
                            String::new()
                        },
                        if storage {
                            format!("::std::option::Option::Some({value})")
                        } else {
                            value
                        }
                    ));
                }
            }
            S::Return(e) => out.push_str(&format!(
                "return {};\n",
                e.as_ref()
                    .map(|e| re(e, types))
                    .unwrap_or_else(|| "()".into())
            )),
            S::Expr(e) => out.push_str(&format!("{};\n", re(e, types))),
            S::If(c, a, b) => {
                out.push_str(&format!("if {} {{\n", re(c, types)));
                rb_child(
                    a,
                    out,
                    n + 1,
                    types,
                    &decision.children[0],
                    node.map(|node| &node.children[0]),
                );
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}}}"));
                if !b.is_empty() || node.is_some_and(|node| !node.children[1].tail.is_empty()) {
                    out.push_str(" else {\n");
                    rb_child(
                        b,
                        out,
                        n + 1,
                        types,
                        &decision.children[1],
                        node.map(|node| &node.children[1]),
                    );
                    out.origin(::std::option::Option::Some(s.line));
                    out.push_str(&format!("{pad}}}"));
                }
                out.push('\n');
            }
            S::While(c, b) => {
                if let Some(node) = node {
                    out.push_str("loop {\n");
                    flow_actions(&node.condition_before, out, n + 1, types);
                    out.push_str(&format!("{pad}    if !({}) {{\n", re(c, types)));
                    flow_actions(&node.condition_exit, out, n + 2, types);
                    out.push_str(&format!("{pad}        break;\n{pad}    }}\n"));
                } else {
                    out.push_str(&format!("while {} {{\n", re(c, types)));
                }
                rb_child(
                    b,
                    out,
                    n + 1,
                    types,
                    &decision.children[0],
                    node.map(|node| &node.children[0]),
                );
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::Match(value, arms) => {
                out.push_str(&format!("match {} {{\n", re(value, types)));
                for (index, arm) in arms.iter().enumerate() {
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
                                        .map(|((field, _), value)| {
                                            if value.name.as_deref() == Some(field.as_str()) {
                                                binding(value)
                                            } else {
                                                format!("{field}: {}", binding(value))
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                            }
                        }
                    };
                    out.push_str(&format!("{pad}    {pattern} => {{\n"));
                    rb_child(
                        &arm.body,
                        out,
                        n + 2,
                        types,
                        &decision.children[index],
                        node.map(|node| &node.children[index]),
                    );
                    out.origin(::std::option::Option::Some(arm.line));
                    out.push_str(&format!("{pad}    }},\n"));
                }
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::For(v, e, b) => {
                let iterator = match decision.iterator.expect("sealed iterator") {
                    IteratorRead::Range => re(e, types),
                    IteratorRead::Borrowed => format!("({}).iter()", re(e, types)),
                    IteratorRead::Copied => format!("({}).iter().copied()", re(e, types)),
                };
                out.push_str(&format!("for mut {v} in {iterator} {{\n"));
                rb_child(
                    b,
                    out,
                    n + 1,
                    types,
                    &decision.children[0],
                    node.map(|node| &node.children[0]),
                );
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::Spawn(e) => {
                let returns_result = decision.spawn_result;
                let scope = if decision.task_bridge {
                    format!(
                        "__nagi_task_scope_{}",
                        decision.scope.expect("sealed spawn scope").0
                    )
                } else {
                    "__scope".into()
                };
                out.push_str(&format!(
                    "{{ let __nagi_spawn_future = {}; {scope}.spawn(async move {{ __nagi_spawn_future.await{} }}); }}\n",
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
                // Scope is a lexical region of this coroutine. Introducing a
                // nested async boundary would capture outer places before their
                // source uses and prevent them from carrying body-local views.
                // The label routes errors through cancel/join after body locals
                // drop, while every outer value keeps its source cleanup anchor.
                let depth = match types.error_exit {
                    ErrorExit::Function => 1,
                    ErrorExit::Scope(depth) => depth + 1,
                };
                let body_types = RustTypes {
                    error_exit: ErrorExit::Scope(depth),
                    ..*types
                };
                let failure = local_type(
                    types.function_error.expect("checked scope Result function"),
                    types,
                );
                let (scope, scope_type, cancel) = if decision.task_bridge {
                    let scope = format!(
                        "__nagi_task_scope_{}",
                        decision.scope.expect("sealed scope identity").0
                    );
                    let cancel = format!("let _ = {scope}.cancel().await;");
                    (scope, "TaskScope", cancel)
                } else {
                    ("__scope".into(), "Scope", "__scope.cancel().await;".into())
                };
                out.push_str(&format!("{pad}    let mut {scope} = ::nagi_runtime::{scope_type}::new();\n{pad}    let __scope_result: ::std::result::Result<(), {failure}> = '__nagi_scope_body_{depth}: {{\n"));
                rb(
                    b,
                    out,
                    n + 2,
                    &body_types,
                    &decision.children[0],
                    node.map(|node| &node.children[0]),
                );
                out.origin(::std::option::Option::Some(s.line));
                out.push_str(&format!("{pad}        ::std::result::Result::Ok(())\n{pad}    }};\n{pad}    if let ::std::result::Result::Err(e) = __scope_result {{ {cancel} {}; }}\n{pad}    {};\n{pad}}}\n", error_exit("e", types), try_result(format!("{scope}.join().await"), types)));
            }
        }
        if let Some(node) = node {
            flow_actions(&node.after, out, n, types);
        }
    }
    if let Some(block) = flow {
        flow_actions(&block.tail, out, n, types);
    }
}
/// Rust generation accepts only a sealed, finally checked program.
///
/// ```compile_fail,E0308
/// let unchecked = nagic::ast::Program::default();
/// let _ = nagic::emit::rust(&unchecked);
/// ```
pub fn rust(p: &CheckedProgram) -> Result<String, String> {
    ::std::result::Result::Ok(rust_with_lines(p)?.text)
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

pub fn rust_with_lines(checked: &CheckedProgram) -> Result<Generated, String> {
    checked.validate_for_emission()?;
    let plan = &checked.emission;
    let names = &plan.names;
    let p = &plan.program;
    // Direct parser/check callers have no module identities. Preserve their
    // legacy primitive-named records; resolved files use canonical class
    // symbols and keep intrinsic types independent of public adapter aliases.
    let types = RustTypes {
        enums: &p.enums,
        plan: Some(plan),
        function: None,
        storage_slots: &BTreeSet::new(),
        expression_uses: &BTreeMap::new(),
        function_error: None,
        error_exit: ErrorExit::Function,
    };
    let mut out =
        Generated::new("#![allow(unused_mut, unused_parens, unused_variables, dead_code)]\n");
    out.attach_provenance(checked.provenance().clone());
    for (index, c) in p.classes.iter().enumerate() {
        out.origin(Some(c.line));
        let item = &plan.classes[index];
        let (copy, serde, readable_debug) = (item.copy, item.serde, item.readable_debug);
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
        if item.from_row {
            out.origin(::std::option::Option::None);
            out.push_str(&format!("impl ::nagi_runtime::FromRow for {} {{\n fn columns() -> &'static [&'static ::std::primitive::str] {{ &[{}] }}\n fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> {{ ::std::result::Result::Ok(Self {{\n",c.name,c.fields.iter().map(|(n,_)|quote(names.original(n))).collect::<Vec<_>>().join(",")));
            for (i, (n, _)) in c.fields.iter().enumerate() {
                out.push_str(&format!("{n}: row.get(ix[{i}])?,\n"));
            }
            out.push_str("}) }\n}\n");
        }
        if let Some(inline) = item.charge {
            out.origin(None);
            charge_impl_start(&mut out, &c.name, inline);
            charge_fields(
                &mut out,
                c.fields.iter().map(|(field, _)| format!("&self.{field}")),
                "        ",
            );
            out.push_str("    }\n}\n");
        }
    }
    for (index, enumeration) in p.enums.iter().enumerate() {
        out.origin(Some(enumeration.line));
        let item = &plan.enums[index];
        let (copy, readable_debug) = (item.copy, item.readable_debug);
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
        if let Some(inline) = item.charge {
            out.origin(None);
            charge_impl_start(&mut out, &enumeration.name, inline);
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
    for (index, f) in p.functions.iter().enumerate() {
        let function = &plan.functions[index];
        out.function_lineage(Some(function.origin.clone()));
        out.origin(::std::option::Option::Some(f.line));
        let name = if f.name == "main" {
            "__nagi_main"
        } else {
            &f.name
        };
        let lifetime = if function.lifetime { "<'a>" } else { "" };
        out.push_str(&format!(
            "#[allow(non_snake_case, arithmetic_overflow)]\npub {}fn {name}{lifetime}({}) -> {} {{\n",
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
            let flow = &function.flow;
            let types = RustTypes {
                function: Some(function),
                function_error: function.error_type.as_ref(),
                storage_slots: flow
                    .as_ref()
                    .map_or(types.storage_slots, |flow| &flow.storage_slots),
                expression_uses: flow
                    .as_ref()
                    .map_or(types.expression_uses, |flow| &flow.expression_uses),
                ..types
            };
            // The public signature retains the caller's borrow lifetime, but
            // parameter bindings can be reassigned just like other locals.
            // Elide their local lifetimes so shorter, non-escaping views do
            // not have to live for the signature's entire 'a.
            // When this moves an owning view-containing parameter, move the
            // other non-Copy parameters in declaration order too. Their local
            // drop order then remains the same as the original parameter
            // drop order. These are ownership moves, with no payload clones
            // or heap allocations.
            out.origin(None);
            for (name, ty) in &f.params {
                if let Some(actions) = flow
                    .as_ref()
                    .and_then(|flow| flow.parameter_actions.get(name))
                {
                    flow_actions(actions, &mut out, 1, &types);
                } else if function.rebind_parameters.contains(name) {
                    out.push_str(&format!(
                        "    let mut {name}: {} = {name};\n",
                        local_type(ty, &types)
                    ));
                }
            }
            rb(
                &f.body,
                &mut out,
                1,
                &types,
                &function.body,
                flow.as_ref().map(|flow| &flow.body),
            );
        }
        out.origin(::std::option::Option::Some(f.line));
        out.push_str("}\n");
    }
    out.origin(::std::option::Option::None);
    out.function_lineage(None);
    // Public adapter names are aliases of the unique generated item, not new
    // wrapper types. Only a module's own definitions are exposed through an
    // `as` import; its imported names are not implicitly reexported.
    for binding in p.modules.root_bindings() {
        match &binding.target {
            BindingTarget::Definition(id) => {
                let Some(definition) = p.modules.definition_id(id) else {
                    continue;
                };
                if plan.native_paths.contains_key(&definition.symbol)
                    && !plan.native_reexports.contains(&definition.symbol)
                {
                    continue;
                }
                let alias = names.source_name(&binding.name);
                let symbol = names.definition(definition);
                let path = plan
                    .native_paths
                    .get(&definition.symbol)
                    .cloned()
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
                    if plan.native_paths.contains_key(&definition.symbol)
                        && !plan.native_reexports.contains(&definition.symbol)
                    {
                        continue;
                    }
                    let symbol = names.definition(definition);
                    let export = names.source_name(&definition.id.name);
                    let path = plan
                        .native_paths
                        .get(&definition.symbol)
                        .cloned()
                        .unwrap_or_else(|| format!("crate::{symbol}"));
                    out.push_str(&format!(
                        "    #[allow(unused_imports)]\n    pub use {path} as {export};\n"
                    ));
                }
                out.push_str("}\n");
            }
        }
    }
    let routes = &plan.routes;
    if plan.needs_server {
        out.push_str("async fn __nagi_serve(db: ::nagi_runtime::Db,port: ::std::primitive::i64) -> ::std::result::Result<(),::nagi_runtime::Error> {\nlet router= ::nagi_runtime::axum::Router::new()\n");
        let mut paths = std::collections::BTreeMap::<String, Vec<(String, usize)>>::new();
        for (i, route) in routes.iter().enumerate() {
            paths
                .entry(route.path.clone())
                .or_default()
                .push((route.method.clone(), i));
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
        for (i, route) in routes.iter().enumerate() {
            let f = &p.functions[route.function];
            out.function_lineage(Some(plan.functions[route.function].origin.clone()));
            out.origin(::std::option::Option::Some(f.line));
            let mut extracts = vec![];
            let mut call = vec![];
            let mut pre = String::new();
            let mut query_fields = vec![];
            for (parameter_index, (n, t)) in f.params.iter().enumerate() {
                if route.inputs[parameter_index] == RouteInput::State {
                    extracts.push(format!(
                        "::nagi_runtime::axum::extract::State({n}): ::nagi_runtime::axum::extract::State<::nagi_runtime::Db>"
                    ));
                    call.push(n.clone());
                } else if route.inputs[parameter_index] == RouteInput::Capture {
                    extracts.push("::nagi_runtime::axum::extract::Path(id): ::nagi_runtime::axum::extract::Path<::std::primitive::i64>".into());
                    call.push(n.clone());
                } else if route.inputs[parameter_index] == RouteInput::Bytes {
                    extracts.push(format!("{n}: ::nagi_runtime::axum::body::Bytes"));
                    call.push(format!("&{n}"));
                } else if route.inputs[parameter_index] == RouteInput::Body {
                    extracts.push(format!(
                        "__body_{parameter_index}: ::nagi_runtime::axum::body::Bytes"
                    ));
                    pre.push_str(&format!("let {n}: {} = match ::nagi_runtime::decode(&__body_{parameter_index}) {{::std::result::Result::Ok(x)=>x,::std::result::Result::Err(e)=>return ::nagi_runtime::error_response(e)}};\n",types.ty(t)));
                    call.push(n.clone());
                } else if route.inputs[parameter_index] == RouteInput::Query {
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
            let response = if route.output == RouteOutput::Html {
                format!("match {invocation} {{ ::std::result::Result::Ok(v)=>::nagi_runtime::axum::response::IntoResponse::into_response(v),::std::result::Result::Err(e)=>::nagi_runtime::error_response(e) }}")
            } else if route.output == RouteOutput::Optional {
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
    out.function_lineage(None);
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
        if plan.main_error != MainError::None {
            if plan.main_error != MainError::Opaque {
                let display = if plan.main_error == MainError::Display {
                    "{}"
                } else {
                    "{:?}"
                };
                out.push_str(&format!("if let ::std::result::Result::Err(e) = {call} {{ eprintln!({},e); ::std::process::exit(1); }}\n", quote(display)));
            } else {
                out.push_str(&format!("if let ::std::result::Result::Err(_e) = {call} {{ eprintln!({}); ::std::process::exit(1); }}\n", quote("NagiのmainがErrを返しました")));
            }
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
    bin_name: &str,
    runtime: PathBuf,
    mut dependencies: std::collections::BTreeMap<String, crate::project::RustDependency>,
) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Package<'a> {
        name: &'a str,
        version: &'static str,
        edition: &'static str,
        autobins: bool,
    }
    #[derive(serde::Serialize)]
    struct Bin<'a> {
        name: &'a str,
        path: &'static str,
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
        bin: Vec<Bin<'a>>,
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
            autobins: false,
        },
        bin: vec![Bin {
            name: bin_name,
            path: "src/main.rs",
        }],
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
    if crate::sql_check::worker(&args)? {
        return Ok(());
    }
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
    let project_manifest = options.manifest_path;
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
    let sql_schema = options.sql_schema;
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
    let mut inputs: Vec<_> = sources
        .files()
        .map(|(path, _, _)| path.to_path_buf())
        .chain(rust_file.iter().cloned())
        .chain(project_manifest.iter().cloned())
        .chain(sql_schema.iter().cloned())
        .chain(crate::output::assets(&resolution))
        .chain(crate::output::assets(&all))
        .collect();
    inputs.sort();
    inputs.dedup();
    let mut outputs = Vec::new();
    let output_lock = if write_output {
        if high {
            outputs.push(out.join("generated.low"));
        }
        if cost {
            outputs.push(out.join("cost-report.json"));
        }
        if matches!(cmd, "build" | "run") {
            outputs.extend([
                out.join("src/main.rs"),
                out.join("Cargo.toml"),
                out.join("Cargo.lock"),
            ]);
        }
        if !outputs.is_empty() {
            if let Ok(directory) = fs::canonicalize(&out) {
                outputs.push(crate::generation::latest_path(&path, &directory)?);
            }
        }
        crate::output::protect(&outputs, &inputs, "Generated")?;
        if outputs.is_empty() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            None
        } else {
            let lock = crate::generation::OutputLock::acquire(&out)?;
            if !outputs.is_empty() {
                let latest = crate::generation::latest_path(&path, &lock.directory)?;
                if !outputs.contains(&latest) {
                    outputs.push(latest);
                }
            }
            crate::output::protect(&outputs, &inputs, "Generated")?;
            Some(lock)
        }
    } else {
        None
    };
    let partial_projection = |error: String| {
        if write_output && !outputs.is_empty() && !error.contains("projection may be partial") {
            format!("{error}\nCompatibility projection may be partial; prior latest unchanged.")
        } else {
            error
        }
    };
    let mut low_snapshot = None;
    if high {
        let low_source = low_with_lines(&p);
        if write_output {
            fs::write(out.join("generated.low"), &low_source.text)
                .map_err(|e| partial_projection(e.to_string()))?;
        }
        // 生成Lowの文字列を独立parserに通す。High ASTをcodegenへ直接渡さない。
        p = crate::parser::parse(&low_source.text, false).map_err(partial_projection)?;
        low_source
            .restore_lines(&mut p)
            .map_err(partial_projection)?;
        if matches!(cmd, "build" | "run") {
            low_snapshot = Some(low_source.text);
        }
    }
    let checked = crate::check::finalize(p, all, sources.provenance())
        .map_err(|e| partial_projection(crate::diagnostics::finalize_message(&e, &sources)))?;
    let p = checked.program();
    if let Some(schema) = sql_schema {
        crate::sql_check::check(p, &sources, &schema).map_err(partial_projection)?;
    }
    if let Some(map) = map_options {
        let mut graph = match map.view {
            crate::project::MapView::Types => crate::graph::types(p, &sources),
            crate::project::MapView::Modules => crate::graph::modules(p, &sources),
            crate::project::MapView::Calls => crate::graph::calls(p, &sources),
        };
        let mut module = map
            .module
            .as_deref()
            .map(|name| crate::graph::resolve_module_filter(p, &sources, name))
            .transpose()?;
        // A loaded module can have no definitions in the selected view.
        if module.as_ref().is_some_and(|id| {
            !graph
                .nodes
                .iter()
                .any(|node| node.module.as_ref() == Some(id))
        }) {
            graph.nodes.clear();
            graph.edges.clear();
            graph.groups.clear();
            module = None;
        }
        let graph = graph.filtered(&crate::graph::Filter {
            module,
            focus: map.focus.clone(),
            depth: map.depth,
        })?;
        if let Some(output) = &map.output {
            crate::output::protect(std::slice::from_ref(output), &inputs, "Map")?;
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
        let report = cost_report(p);
        if write_output {
            fs::write(
                out.join("cost-report.json"),
                serde_json::to_string_pretty(&report).unwrap(),
            )
            .map_err(|e| e.to_string())?;
        }
        let report = serde_json::to_string_pretty(&report).unwrap();
        if cmd == "run" {
            // Application output must stay pipeable, including when a cost
            // report is requested. The report file is available separately.
            eprintln!("{report}");
        } else {
            println!("{report}");
        }
    }
    if cmd == "check" || cmd == "lower" {
        eprintln!("checked {}", path.display());
        return Ok(());
    }
    if cmd != "build" && cmd != "run" {
        return Err(format!("unknown command: {cmd}"));
    }
    let binary = (|| -> Result<PathBuf, String> {
    let root = crate::installation::root()?;
    fs::create_dir_all(out.join("src")).map_err(|e| e.to_string())?;
    if p.functions.iter().any(|f| f.external) && rust_file.is_none() {
        return Err("extern関数のビルドには--rust FILE.rsが必要です".into());
    }
    let mut generated_rust = rust_with_lines(&checked)?;
    if let Some(file) = &rust_file {
        generated_rust.origin(None);
        generated_rust.push_str(&format!(
            "\n#[path = {}]\nmod native;\n",
            quote(&file.display().to_string())
        ));
    }
    fs::write(out.join("src/main.rs"), &generated_rust.text).map_err(|e| e.to_string())?;
    let generated_directory = &output_lock
        .as_ref()
        .expect("build owns output lock")
        .directory;
    let package = crate::generation::application_name(&path, generated_directory)?;
    let generation = crate::generation::BuildGeneration::create(generated_directory, &package)?;
    let compatibility_manifest = cargo_manifest(
        &package,
        &generation.bin,
        relative_path(&root.join("runtime"), generated_directory),
        rust_deps.clone(),
    )?;
    fs::write(out.join("Cargo.toml"), &compatibility_manifest).map_err(|e| e.to_string())?;
    generation.write("src/main.rs", generated_rust.text.as_bytes())?;
    let manifest = cargo_manifest(
        &package,
        &generation.bin,
        relative_path(&root.join("runtime"), &generation.staging),
        rust_deps,
    )?;
    generation.write("Cargo.toml", manifest.as_bytes())?;
    let canonical_source = fs::canonicalize(&path).map_err(|e| e.to_string())?;
    let low_snapshot = low_snapshot
        .as_deref()
        .or_else(|| {
            sources
                .files()
                .find(|(file, _, _)| *file == canonical_source)
                .map(|(_, text, _)| text)
        })
        .ok_or("Cannot find read source for generation snapshot")?;
    generation.snapshot(&sources, &generated_rust, low_snapshot)?;
    let generated_file =
        fs::canonicalize(generation.staging.join("src/main.rs")).map_err(|e| e.to_string())?;
    let native_target = std::env::var_os("NAGI_NATIVE_TARGET_DIR");
    let target = native_target.map(|p| cwd.join(p)).unwrap_or_else(|| {
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
        .arg(generation.staging.join("Cargo.toml"))
        .arg("--bin").arg(&generation.bin)
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
        if let Some(message) = crate::diagnostics::cargo_message_with_details(
            &line,
            &generated_rust,
            &generated_file,
            &sources,
            options.rust_diagnostics,
        ) {
            eprint!("{message}");
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Build failed. Check the diagnostics above for code, dependency, or build environment errors.".into());
    }
    let cached_binary = target.join("release").join(format!(
        "{}{}",
        generation.bin,
        std::env::consts::EXE_SUFFIX
    ));
    generation.finish(&cached_binary, generated_directory, high, &inputs, &compatibility_manifest)
    })().map_err(partial_projection)?;
    // Retain this chosen success path; never re-read latest after unlocking.
    drop(output_lock);
    eprintln!("native: {}", binary.display());
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
                S::Assign { value, .. }
                | S::SpawnBind { value, .. }
                | S::Expr(value)
                | S::Spawn(value) => walk(value, a),
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
