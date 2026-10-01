use crate::ast::*;
use crate::diagnostics::Generated;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
pub fn low(p: &Program) -> String {
    low_with_lines(p).text
}

pub fn low_with_lines(p: &Program) -> Generated {
    fn expr(e: &Expr) -> String {
        match &e.kind {
            E::Int(s) | E::Float(s) | E::Name(s) => s.clone(),
            E::Str(s) => quote(s),
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
                        t.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
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
            E::Field(x, n) => format!("{}.{n}", expr(x)),
            E::Index(x, i) => format!("{}[{}]", expr(x), expr(i)),
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
                        format!(": {}", annotation.as_ref().unwrap())
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
                        out.push_str(&format!(
                            "{pad}    case {}({}) {{\n",
                            if arm.ok { "Ok" } else { "Err" },
                            arm.binding.as_deref().unwrap_or("_")
                        ));
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
    for (file, line) in &p.imports {
        out.origin(Some(*line));
        out.push_str(&format!("import {};\n", quote(file)));
    }
    for c in &p.classes {
        out.origin(Some(c.line));
        out.push_str(&format!("record {} {{\n", c.name));
        for (i, (n, t)) in c.fields.iter().enumerate() {
            out.origin(Some(c.field_lines.get(i).copied().unwrap_or(c.line)));
            out.push_str(&format!("    {n}: {t};\n"));
        }
        out.origin(Some(c.line));
        out.push_str("}\n\n");
    }
    for f in &p.functions {
        out.origin(Some(f.line));
        for (a, v) in &f.attrs {
            if a == "replace" {
                out.push_str(&format!("@replace {v}\n"));
            } else {
                out.push_str(&format!("@{a}({})\n", quote(v)));
            }
        }
        out.push_str(&format!(
            "{}{}fn {}({}) -> {}{}\n",
            if f.external { "extern " } else { "" },
            if f.asynchronous { "async " } else { "" },
            f.name,
            f.params
                .iter()
                .map(|(n, t)| format!("{n}: {t}"))
                .collect::<Vec<_>>()
                .join(", "),
            f.ret,
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
    match t.0.as_str() {
        "str" => "String".into(),
        "bytes" => "Vec<u8>".into(),
        "unit" => "()".into(),
        "Error" => "::nagi_runtime::Error".into(),
        "Html" => "::nagi_runtime::axum::response::Html<String>".into(),
        "Db" => "::nagi_runtime::Db".into(),
        "UUID" => "::nagi_runtime::Uuid".into(),
        "timestamp" => "::nagi_runtime::Timestamp".into(),
        "view" => {
            let a = t.inner();
            match a.0.as_str() {
                "str" => "&'a str".into(),
                "bytes" => "&'a [u8]".into(),
                _ => format!("&'a [{}]", rust_type(&a)),
            }
        }
        "owned" => rust_type(&t.inner()),
        "shared" => format!("std::sync::Arc<{}>", rust_type(&t.inner())),
        "List" => format!("Vec<{}>", rust_type(&t.inner())),
        "Map" => format!(
            "std::collections::HashMap<{}, {}>",
            rust_type(&t.1[0]),
            rust_type(&t.1[1])
        ),
        "Option" | "Result" => format!(
            "{}<{}>",
            t.0,
            t.1.iter().map(rust_type).collect::<Vec<_>>().join(", ")
        ),
        _ => t.0.clone(),
    }
}
fn local_type(t: &Type) -> String {
    rust_type(t).replace("&'a ", "&")
}
fn string_arg(e: &Expr) -> String {
    if let E::Str(s) = &e.kind {
        quote(s)
    } else {
        format!("&({})", re(e))
    }
}
fn re(e: &Expr) -> String {
    match &e.kind {
        E::Int(s) | E::Float(s) | E::Name(s) => s.clone(),
        E::Str(s) => format!("String::from({})", quote(s)),
        E::Bool(b) => b.to_string(),
        E::Null => "None".into(),
        E::Binary(a, o, b) => format!(
            "({} {} {})",
            re(a),
            match o.as_str() {
                "and" => "&&",
                "or" => "||",
                _ => o,
            },
            re(b)
        ),
        E::Unary(o, x) => format!("{}({})", if o == "not" { "!" } else { o }, re(x)),
        E::Field(x, n) => format!("({}).{n}", re(x)),
        E::Index(x, i) => format!(
            "({})[usize::try_from({}).expect(\"negative index\")]",
            re(x),
            re(i)
        ),
        E::List(a) => format!("vec![{}]", a.iter().map(re).collect::<Vec<_>>().join(", ")),
        E::Record(n, a) => format!(
            "{n} {{ {} }}",
            a.iter()
                .map(|(n, e)| format!("{n}: {}", re(e)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        E::Await(x) => format!("({}).await", re(x)),
        E::Try(x) => format!("({})?", re(x)),
        E::Call(n, ts, a) => {
            let args = a.iter().map(re).collect::<Vec<_>>();
            let join = args.join(", ");
            let g = if ts.is_empty() {
                String::new()
            } else {
                format!(
                    "::<{}>",
                    ts.iter().map(local_type).collect::<Vec<_>>().join(", ")
                )
            };
            match n.as_str() {
                "print" => format!("println!(\"{{}}\", {})", string_or_value(&a[0])),
                "write" => format!("print!(\"{{}}\", {})", string_or_value(&a[0])),
                "read_line" => "::nagi_runtime::read_line()".into(),
                "html" => format!("::nagi_runtime::axum::response::Html({})", args[0]),
                "include_text" => format!("include_str!({}).to_owned()", string_arg(&a[0])),
                "assert_true" => format!("assert!({})", args[0]),
                "view" => format!(
                    "({}).{}()",
                    args[0],
                    if a[0].ty.as_ref().is_some_and(|t| t.0 == "str") {
                        "as_str"
                    } else {
                        "as_slice"
                    }
                ),
                "copy" => format!("({}).to_owned()", args[0]),
                "share" => format!("std::sync::Arc::new({})", args[0]),
                "clone_shared" => format!("std::sync::Arc::clone(&{})", args[0]),
                "len" => format!("({}).len() as i64", string_or_value(&a[0])),
                "range" => format!("0i64..{}", args[0]),
                "append" => format!("{}.push({})", args[0], args[1]),
                "ok" => format!("Ok({})", args[0]),
                "some" => format!("Some({})", args[0]),
                "error" => format!("Err(::nagi_runtime::Error::invalid({}))", args[0]),
                "not_found" => format!(
                    "Err(::nagi_runtime::Error {{ kind: ::nagi_runtime::ErrorKind::NotFound, message: {} }})",
                    args[0]
                ),
                "internal_error" => format!("Err(::nagi_runtime::Error::internal({}))", args[0]),
                "fail" => format!("Err({})", args[0]),
                "error_kind" => format!("::nagi_runtime::error_kind(&({})).to_owned()", args[0]),
                "error_message" => format!("({}).message.clone()", args[0]),
                "serve" => format!("__nagi_serve({}, {})", args[0], args[1]),
                "env" => format!(
                    "std::env::var({}).unwrap_or_else(|_|({}).to_owned())",
                    string_arg(&a[0]),
                    string_arg(&a[1])
                ),
                "sleep" => format!("::nagi_runtime::sleep({})", args[0]),
                "db_open" => format!("::nagi_runtime::Db::open({})", string_arg(&a[0])),
                "db_exec" | "db_all" | "db_query" | "db_write" | "db_insert" | "db_update" => {
                    format!(
                        "{}.{}{}({})",
                        args[0],
                        n.trim_start_matches("db_"),
                        g,
                        std::iter::once(if let E::Str(s) = &a[1].kind {
                            format!("::nagi_runtime::Sql::Static({})", quote(s))
                        } else {
                            format!("::nagi_runtime::Sql::Owned(({}).to_owned())", string_arg(&a[1]))
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
                        format!("({}).as_bytes()", string_arg(&a[0]))
                    } else {
                        format!("&({})", args[0])
                    };
                    format!("::nagi_runtime::decode{g}({input})")
                }
                "json_encode" => format!("::nagi_runtime::encode(&{})", args[0]),
                "parse_i64" => format!("::nagi_runtime::parse_i64({})", string_arg(&a[0])),
                "parse_f64" => format!("::nagi_runtime::parse_f64({})", string_arg(&a[0])),
                "uuid_parse" => format!("::nagi_runtime::Uuid::parse({})", string_arg(&a[0])),
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
                "i64" => format!("i64::from({})", args[0]),
                "i32" => format!(
                    "i32::try_from({}).map_err(|e| ::nagi_runtime::Error::invalid(e.to_string()))",
                    args[0]
                ),
                "size_of" => format!("std::mem::size_of{}() as i64", g),
                "bench_i64" | "bench_f64" | "bench_scalar" => {
                    format!("::nagi_runtime::{n}({}, {}, {})", string_arg(&a[0]), args[1], args[2])
                }
                "clock_ns" | "make_ints" | "actor_demo" | "actor_pair_demo" | "supervisor_demo"
                | "queue_demo" | "task_demo" | "cpu_sum" => format!("::nagi_runtime::{n}({join})"),
                _ => format!("{n}({join})"),
            }
        }
    }
}
fn string_or_value(e: &Expr) -> String {
    if matches!(e.kind, E::Str(_)) {
        string_arg(e)
    } else {
        re(e)
    }
}
fn rb(ss: &[Stmt], out: &mut Generated, n: usize) {
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
                if *declare { "let mut " } else { "" },
                if *declare {
                    format!(": {}", local_type(annotation.as_ref().unwrap()))
                } else {
                    String::new()
                },
                re(value)
            )),
            S::Return(e) => out.push_str(&format!(
                "return {};\n",
                e.as_ref().map(re).unwrap_or_else(|| "()".into())
            )),
            S::Expr(e) => out.push_str(&format!("{};\n", re(e))),
            S::If(c, a, b) => {
                out.push_str(&format!("if {} {{\n", re(c)));
                rb(a, out, n + 1);
                out.origin(Some(s.line));
                out.push_str(&format!("{pad}}}"));
                if !b.is_empty() {
                    out.push_str(" else {\n");
                    rb(b, out, n + 1);
                    out.origin(Some(s.line));
                    out.push_str(&format!("{pad}}}"));
                }
                out.push('\n');
            }
            S::While(c, b) => {
                out.push_str(&format!("while {} {{\n", re(c)));
                rb(b, out, n + 1);
                out.origin(Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::Match(value, arms) => {
                out.push_str(&format!("match {} {{\n", re(value)));
                for arm in arms {
                    out.origin(Some(arm.line));
                    let binding = arm
                        .binding
                        .as_ref()
                        .map(|s| format!("mut {s}"))
                        .unwrap_or_else(|| "_".into());
                    out.push_str(&format!(
                        "{pad}    {}({binding}) => {{\n",
                        if arm.ok { "Ok" } else { "Err" }
                    ));
                    rb(&arm.body, out, n + 2);
                    out.origin(Some(arm.line));
                    out.push_str(&format!("{pad}    }},\n"));
                }
                out.origin(Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::For(v, e, b) => {
                let iterator = if e.ty.as_ref().is_some_and(|t| t.0 == "Range") {
                    re(e)
                } else {
                    format!("({}).iter().copied()", re(e))
                };
                out.push_str(&format!("for {v} in {iterator} {{\n"));
                rb(b, out, n + 1);
                out.origin(Some(s.line));
                out.push_str(&format!("{pad}}}\n"));
            }
            S::Spawn(e) => {
                let returns_result = e.ty.as_ref().is_some_and(|t| t.inner().0 == "Result");
                out.push_str(&format!(
                    "__scope.spawn(async move {{ {}.await{} }});\n",
                    re(e),
                    if returns_result { "" } else { "; Ok(())" }
                ));
            }
            S::Scope(b) => {
                out.push_str("{\n");
                out.push_str(&format!("{pad}    let mut __scope = ::nagi_runtime::Scope::new();\n{pad}    let __scope_result: Result<(), ::nagi_runtime::Error> = async {{\n"));
                rb(b, out, n + 2);
                out.origin(Some(s.line));
                out.push_str(&format!("{pad}        Ok(())\n{pad}    }}.await;\n{pad}    if let Err(e) = __scope_result {{ __scope.cancel().await; return Err(e); }}\n{pad}    __scope.join().await?;\n{pad}}}\n"));
            }
        }
    }
}
pub fn rust(p: &Program) -> Result<String, String> {
    Ok(rust_with_lines(p)?.text)
}

pub fn rust_with_lines(p: &Program) -> Result<Generated, String> {
    fn copy_type(t: &Type, p: &Program, depth: usize) -> bool {
        if depth > 64 {
            false
        } else if t.is_copy() {
            true
        } else if t.0 == "Option" {
            copy_type(&t.inner(), p, depth + 1)
        } else if let Some(c) = p.classes.iter().find(|c| c.name == t.0) {
            c.fields.iter().all(|(_, t)| copy_type(t, p, depth + 1))
        } else {
            false
        }
    }
    let mut out =
        Generated::new("#![allow(unused_mut, unused_parens, unused_variables, dead_code)]\n");
    for c in &p.classes {
        out.origin(Some(c.line));
        let copy = c.fields.iter().all(|(_, t)| copy_type(t, p, 0));
        out.push_str(&format!("#[derive(Debug, ::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize{} )]\n#[serde(crate = \"::nagi_runtime::serde\", deny_unknown_fields)]\npub struct {} {{\n",if copy{", Clone, Copy"}else{""},c.name));
        for (i, (n, t)) in c.fields.iter().enumerate() {
            out.origin(Some(c.field_lines.get(i).copied().unwrap_or(c.line)));
            out.push_str(&format!("    pub {n}: {},\n", rust_type(t)));
        }
        out.origin(Some(c.line));
        out.push_str("}\n");
        let db_compatible = c.fields.iter().all(|(_, t)| {
            [
                "i8", "i16", "i32", "i64", "u8", "u16", "u32", "f32", "f64", "bool", "str", "bytes",
            ]
            .contains(&t.0.as_str())
                || t.0 == "Option" && ["i64", "i32", "str"].contains(&t.inner().0.as_str())
        });
        if db_compatible {
            out.origin(None);
            out.push_str(&format!("impl ::nagi_runtime::FromRow for {} {{\n fn columns() -> &'static [&'static str] {{ &[{}] }}\n fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[usize]) -> ::nagi_runtime::rusqlite::Result<Self> {{ Ok(Self {{\n",c.name,c.fields.iter().map(|(n,_)|quote(n)).collect::<Vec<_>>().join(",")));
            for (i, (n, _)) in c.fields.iter().enumerate() {
                out.push_str(&format!("{n}: row.get(ix[{i}])?,\n"));
            }
            out.push_str("}) }\n}\n");
        }
    }
    for f in &p.functions {
        out.origin(Some(f.line));
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
            "pub {}fn {name}{lifetime}({}) -> {} {{\n",
            if f.asynchronous { "async " } else { "" },
            f.params
                .iter()
                .map(|(n, t)| format!("mut {n}: {}", rust_type(t)))
                .collect::<Vec<_>>()
                .join(", "),
            rust_type(&f.ret)
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
            rb(&f.body, &mut out, 1);
        }
        out.origin(Some(f.line));
        out.push_str("}\n");
    }
    out.origin(None);
    let routes: Vec<_> = p
        .functions
        .iter()
        .filter(|f| {
            f.attrs
                .iter()
                .any(|(a, _)| ["get", "post", "put", "delete"].contains(&a.as_str()))
        })
        .collect();
    if !routes.is_empty() {
        out.push_str("async fn __nagi_serve(db: ::nagi_runtime::Db,port:i64) -> Result<(),::nagi_runtime::Error> {\nlet router= ::nagi_runtime::axum::Router::new()\n");
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
            out.origin(Some(f.line));
            if !f.asynchronous || f.ret.0 != "Result" {
                return Err(format!("route {} must be async and return Result", f.name));
            }
            let mut extracts = vec![];
            let mut call = vec![];
            let mut pre = String::new();
            let mut query_fields = vec![];
            for (n, t) in &f.params {
                if t.0 == "Db" {
                    extracts.push(format!(
                        "::nagi_runtime::axum::extract::State({n}): ::nagi_runtime::axum::extract::State<::nagi_runtime::Db>"
                    ));
                    call.push(n.clone());
                } else if n == "id" && t.0 == "i64" {
                    extracts.push("::nagi_runtime::axum::extract::Path(id): ::nagi_runtime::axum::extract::Path<i64>".into());
                    call.push(n.clone());
                } else if t == &Type::generic("view", vec![Type::named("bytes")]) {
                    extracts.push(format!("{n}: ::nagi_runtime::axum::body::Bytes"));
                    call.push(format!("&{n}"));
                } else if p.classes.iter().any(|c| c.name == t.0) {
                    extracts.push(format!("__body_{n}: ::nagi_runtime::axum::body::Bytes"));
                    pre.push_str(&format!("let {n}: {} = match ::nagi_runtime::decode(&__body_{n}) {{Ok(x)=>x,Err(e)=>return ::nagi_runtime::error_response(e)}};\n",rust_type(t)));
                    call.push(n.clone());
                } else if ["str", "i64", "i32", "u64", "bool", "f64"].contains(&t.0.as_str()) {
                    query_fields.push((n.clone(), t.clone()));
                    pre.push_str(&format!("let {n}=__query.{n};\n"));
                    call.push(n.clone());
                } else {
                    return Err(format!("route parameter {n}: {t} は未対応です"));
                }
            }
            if !query_fields.is_empty() {
                out.push_str(&format!("#[derive(::nagi_runtime::serde::Deserialize)]\n#[serde(crate=\"::nagi_runtime::serde\")]\nstruct __Query_{i} {{ {} }}\n",query_fields.iter().map(|(n,t)|format!("{n}:{}",rust_type(t))).collect::<Vec<_>>().join(",")));
                extracts.push(format!(
                    "::nagi_runtime::axum::extract::Query(__query): ::nagi_runtime::axum::extract::Query<__Query_{i}>"
                ));
            }
            // Body extractorはAxumの規則に従い最後。引数の順序は元の関数を維持する。
            extracts.sort_by_key(|s| s.contains("body::Bytes"));
            let invocation = format!("{}({}).await", f.name, call.join(", "));
            let response = if f.ret.inner().0 == "Html" {
                format!("match {invocation} {{ Ok(v)=>::nagi_runtime::axum::response::IntoResponse::into_response(v),Err(e)=>::nagi_runtime::error_response(e) }}")
            } else if f.ret.inner().0 == "Option" {
                format!("match {invocation} {{ Ok(Some(v))=>::nagi_runtime::response(Ok(v)), Ok(None)=>::nagi_runtime::error_response(::nagi_runtime::Error::not_found()),Err(e)=>::nagi_runtime::error_response(e) }}")
            } else {
                format!("::nagi_runtime::response({invocation})")
            };
            out.push_str(&format!(
                "async fn __route_{i}({}) -> ::nagi_runtime::axum::response::Response {{\n{pre}{response}\n}}\n",
                extracts.join(", ")
            ));
        }
    }
    out.origin(None);
    if let Some(f) = p.functions.iter().find(|f| f.name == "main") {
        if !f.params.is_empty() {
            return Err("mainは引数を取りません".into());
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
            out.push_str(&format!(
                "if let Err(e) = {call} {{ eprintln!(\"{{}}\",e); std::process::exit(1); }}\n"
            ));
        } else {
            out.push_str(&format!("{call};\n"));
        }
        out.push_str(if f.asynchronous { "}); }\n" } else { "}\n" });
    } else {
        out.push_str("fn main() {}\n");
    }
    Ok(out)
}

pub fn cli(args: Vec<String>) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let options = crate::project::resolve(&args, &cwd)?;
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
        let np = sources.append(native_sources);
        all.classes.extend(np.classes);
        all.functions.extend(np.functions);
    }
    if cmd == "symbols" {
        println!("{}", crate::symbols::index(&sources, &[&p, &all])?);
        return Ok(());
    }
    // 手書きLowの通常関数はHighの名前解決にも使う。置換本体はLowの統合時に検査する。
    let mut resolution = p.clone();
    let nc = p.classes.len();
    let nf = p.functions.len();
    resolution.classes.extend(all.classes.clone());
    resolution.functions.extend(
        all.functions
            .iter()
            .filter(|f| !f.attrs.iter().any(|(a, _)| a == "replace"))
            .cloned(),
    );
    crate::check::check(&mut resolution).map_err(|e| sources.diagnostic(&e))?;
    p.classes = resolution.classes[..nc].to_vec();
    p.functions = resolution.functions[..nf].to_vec();
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    if high {
        let low_source = low_with_lines(&p);
        fs::write(out.join("generated.low"), &low_source.text).map_err(|e| e.to_string())?;
        // 生成Lowの文字列を独立parserに通す。High ASTをcodegenへ直接渡さない。
        p = crate::parser::parse(&low_source.text, false)?;
        low_source.restore_lines(&mut p)?;
    }
    crate::check::integrate(&mut p, all).map_err(|e| sources.diagnostic(&e))?;
    if cost {
        let report = cost_report(&p);
        fs::write(
            out.join("cost-report.json"),
            serde_json::to_string_pretty(&report).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }
    if cmd == "check" || cmd == "lower" {
        println!("checked {}", path.display());
        return Ok(());
    }
    if cmd != "build" && cmd != "run" {
        return Err(format!("unknown command: {cmd}"));
    }
    let root = std::env::var_os("NAGI_ROOT")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe().ok().and_then(|exe| {
                exe.ancestors()
                    .find(|p| {
                        p.join("runtime/Cargo.toml").is_file()
                            && p.join("compiler/Cargo.toml").is_file()
                    })
                    .map(Path::to_path_buf)
            })
        })
        .or_else(|| {
            std::env::current_dir().ok().and_then(|cwd| {
                cwd.ancestors()
                    .find(|p| p.join("runtime/Cargo.toml").is_file())
                    .map(Path::to_path_buf)
            })
        })
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .to_path_buf()
        });
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
                if ch.is_whitespace() || ch == '_' || ch == '.' {
                    '-'
                } else {
                    ch
                }
            })
            .collect::<String>()
    );
    let manifest=format!("[package]\nname={}\nversion=\"0.1.0\"\nedition=\"2021\"\n[workspace]\n[dependencies]\nnagi-runtime={{path={}}}\n[profile.release]\nopt-level=3\nlto=false\ncodegen-units=1\npanic=\"unwind\"\n",quote(&package),quote(&relative_path(&root.join("runtime"),&fs::canonicalize(&out).map_err(|e|e.to_string())?).display().to_string()));
    let dependencies = rust_deps
        .iter()
        .map(|(name, version)| format!("{} = {}\n", quote(name), quote(version)))
        .collect::<String>();
    let manifest = manifest.replacen(
        "[profile.release]",
        &format!("{dependencies}[profile.release]"),
        1,
    );
    fs::write(out.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
    let target = std::env::var_os("NAGI_NATIVE_TARGET_DIR")
        .map(|p| cwd.join(p))
        .unwrap_or_else(|| {
            options
                .project_root
                .as_ref()
                .map(|p| p.join("build/native-target"))
                .unwrap_or_else(|| root.join("native-target"))
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
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cargo/rustcがPATHに必要です: {e}"))?;
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
        return Err("Rust backend rejected program。詳細は上の診断を参照してください".into());
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
pub fn cost_report(p: &Program) -> serde_json::Value {
    fn walk(e: &Expr, a: &mut Vec<serde_json::Value>) {
        match &e.kind{
        E::Str(_)=>a.push(serde_json::json!({"line":e.line,"kind":"owned_string_literal","cost":"allocation/copy unless borrowed by intrinsic/codegen"})),
        E::List(xs)=>{a.push(serde_json::json!({"line":e.line,"kind":"contiguous_list","cost":"heap allocation; element boxing 0"}));for e in xs{walk(e,a);}},
        E::Call(n,_,xs)=>{if ["copy","share","clone_shared","json_decode","json_encode","db_query","db_all","db_insert","db_update"].contains(&n.as_str()){a.push(serde_json::json!({"line":e.line,"kind":n,"cost":"runtime/input dependent; measure allocation counters"}));}for e in xs{walk(e,a);}},E::Binary(ae,_,b)=>{walk(ae,a);walk(b,a)},E::Unary(_,e)|E::Try(e)|E::Await(e)|E::Field(e,_)=>walk(e,a),E::Record(_,fs)=>for(_,e)in fs{walk(e,a)},E::Index(e,i)=>{walk(e,a);walk(i,a)},_=>{}}
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
        fs.insert(f.name.clone(),serde_json::json!({"static_sites":a,"primitive_boxing":0,"warning":"sites are not dynamic allocation counts; runtime and loop multiplicity excluded"}));
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
