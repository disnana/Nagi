use crate::ast::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BindingId {
    line: usize,
    token: usize,
}
#[derive(Clone, PartialEq, Eq, Hash)]
struct BorrowedPlace {
    binding: BindingId,
    fields: Vec<String>,
    owner_loan: bool,
}
impl BorrowedPlace {
    fn overlaps(&self, other: &Self) -> bool {
        self.owner_loan
            && other.owner_loan
            && self.binding == other.binding
            && (self.fields.starts_with(&other.fields) || other.fields.starts_with(&self.fields))
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Var {
    binding: BindingId,
    ty: Type,
    moved: bool,
    moved_fields: HashSet<Vec<String>>,
    origins: HashSet<BorrowedPlace>,
    // Each entry records references kept after another array element is copied.
    content_origins: Vec<HashSet<BorrowedPlace>>,
    async_function: Option<String>,
}
struct Checker {
    classes: HashMap<String, Class>,
    functions: HashMap<String, Function>,
    vars: HashMap<String, Var>,
    ret: Type,
    asynchronous: bool,
    scope: usize,
    editor: bool,
    parameter_views: HashSet<BindingId>,
    iterators: Vec<HashSet<BorrowedPlace>>,
}
fn error(line: usize, s: impl AsRef<str>) -> String {
    format!("line {line}: {}", s.as_ref())
}
fn result(t: Type) -> Type {
    Type::generic("Result", vec![t, Type::named("Error")])
}
fn future(t: Type) -> Type {
    Type::generic("Future", vec![t])
}
fn matches_type(a: &Type, b: &Type) -> bool {
    a == b
}

// Generated records do not implement comparison or hashing. Borrowed slices
// inherit these operations from their elements, including nested containers.
fn comparable(t: &Type, ordered: bool) -> bool {
    match t.0.as_str() {
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" | "bool"
        | "str" | "bytes" | "unit" => true,
        "UUID" | "timestamp" => !ordered,
        "fn" if !t.1.is_empty() => true,
        "view" | "List" | "Option" | "owned" | "shared" => comparable(&t.inner(), ordered),
        "Result" => t.1.iter().all(|arg| comparable(arg, ordered)),
        "Map" => !ordered && hashable(&t.1[0]) && comparable(&t.1[1], false),
        _ => false,
    }
}

fn hashable(t: &Type) -> bool {
    match t.0.as_str() {
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "bool" | "str" | "bytes"
        | "unit" => true,
        "fn" if !t.1.is_empty() => true,
        "view" | "List" | "Option" | "owned" | "shared" => hashable(&t.inner()),
        "Result" => t.1.iter().all(hashable),
        _ => false,
    }
}

fn definitely_unhashable(t: &Type, classes: Option<&HashMap<String, Class>>) -> bool {
    match t.0.as_str() {
        // Primitive names can resolve to a local class in generated Rust.
        "f32" | "f64" => !classes.is_some_and(|classes| classes.contains_key(&t.0)),
        "UUID" | "timestamp" | "Map" => true,
        "view" | "List" | "Option" | "owned" | "shared" | "Result" => {
            t.1.iter()
                .any(|inner| definitely_unhashable(inner, classes))
        }
        // A Rust bridge can add Eq/Hash to a generated user record.
        _ => false,
    }
}

fn unowned(mut t: &Type) -> &Type {
    // `owned[T]` emits T itself, so it has exactly the same trait contracts.
    while t.0 == "owned" {
        t = &t.1[0];
    }
    t
}

fn json_type_supported(t: &Type, decoding: bool, classes: &HashMap<String, Class>) -> bool {
    if t.is_future() {
        return false;
    }
    if t.0 == "fn" && !t.1.is_empty() || matches!(t.0.as_str(), "Error" | "Db" | "Html") {
        return false;
    }
    if decoding && t.0 == "view" {
        // Serde deserializes borrowed strings/bytes, but no other slice types.
        return t.inner().0 == "str"
            || (t.inner().0 == "bytes" || unowned(&t.1[0]) == &Type::named("u8"))
                && !classes.contains_key("u8");
    }
    if decoding && t.0 == "Map" && definitely_unhashable(&t.1[0], Some(classes)) {
        return false;
    }
    t.1.iter()
        .all(|inner| json_type_supported(inner, decoding, classes))
}

fn json_encode_supported(expr: &Expr, classes: &HashMap<String, Class>) -> bool {
    if json_type_supported(expr.ty.as_ref().unwrap(), false, classes) {
        return true;
    }
    // A record constructor names the local struct directly. Container
    // constructors infer that struct too, while a typed parameter/return
    // uses rust_type and may instead name an unsupported runtime type.
    match &expr.kind {
        E::Record(_, _) => true,
        E::List(values) => values
            .iter()
            .all(|value| json_encode_supported(value, classes)),
        E::Call(name, _, args)
            if expr.resolution == Some(NameResolution::Builtin)
                && matches!(
                    name.as_str(),
                    "some" | "share" | "clone_shared" | "view" | "copy"
                ) =>
        {
            json_encode_supported(&args[0], classes)
        }
        E::Index(values, _) => json_encode_supported(values, classes),
        _ => false,
    }
}

fn class_field(t: &Type, line: usize) -> Result<(), String> {
    // Every generated class derives serialization. These foreign types cannot
    // acquire the missing implementations from a user's Rust bridge.
    if t.0 == "fn" && !t.1.is_empty() || matches!(t.0.as_str(), "Error" | "Db" | "Html") {
        return Err(error(line, format!("{t}はclassのフィールドに保存できません。関数の引数やローカル変数で使用してください")));
    }
    if t.0 == "Map" && definitely_unhashable(&t.1[0], None) {
        return Err(error(line, format!("classのMapフィールドのキーに{}は使えません。一致比較とハッシュに対応する型が必要です", t.1[0])));
    }
    for inner in &t.1 {
        class_field(inner, line)?;
    }
    Ok(())
}

fn recursive_layout(
    t: &Type,
    classes: &HashMap<String, Class>,
    visiting: &mut HashSet<String>,
    checked: &mut HashSet<String>,
) -> bool {
    if t.1.is_empty() {
        if let Some(class) = classes.get(&t.0) {
            if checked.contains(&t.0) {
                return false;
            }
            if !visiting.insert(t.0.clone()) {
                return true;
            }
            let cyclic = class
                .fields
                .iter()
                .any(|(_, field)| recursive_layout(field, classes, visiting, checked));
            visiting.remove(&t.0);
            if !cyclic {
                checked.insert(t.0.clone());
            }
            return cyclic;
        }
    }
    // These wrappers contain their values inline. Vec, Arc, HashMap and
    // function pointers have fixed layouts independent of their contents.
    matches!(t.0.as_str(), "Option" | "Result" | "owned")
        && t.1
            .iter()
            .any(|inner| recursive_layout(inner, classes, visiting, checked))
}

fn negative_boundary_type(expr: &Expr, expected: Option<&Type>) -> Option<Type> {
    let E::Int(value) = &expr.kind else {
        return None;
    };
    let ty = expected.cloned().unwrap_or_else(|| Type::named("i64"));
    let magnitude = match ty.0.as_str() {
        "i8" => 1_u128 << 7,
        "i16" => 1_u128 << 15,
        "i32" => 1_u128 << 31,
        "i64" => 1_u128 << 63,
        _ => return None,
    };
    (value.parse::<u128>().ok() == Some(magnitude)).then_some(ty)
}

pub fn check(p: &mut Program) -> Result<(), String> {
    check_mode(p, false)
}

/// Uses the same type and ownership rules as compilation. Failed statements
/// cannot introduce bindings or change the environment used by later statements.
pub(crate) fn editor_types(primary: &Program, native: &Program) -> Option<Program> {
    let mut p = primary.clone();
    p.classes.extend(native.classes.clone());
    p.functions.extend(
        native
            .functions
            .iter()
            .filter(|f| !f.attrs.iter().any(|(a, _)| a == "replace"))
            .cloned(),
    );
    check_mode(&mut p, true).ok()?;
    let replacement_lines: HashSet<_> = native
        .functions
        .iter()
        .filter(|f| f.attrs.iter().any(|(a, _)| a == "replace"))
        .map(|f| f.line)
        .collect();
    if !replacement_lines.is_empty() {
        let mut replaced = primary.clone();
        if integrate_mode(&mut replaced, native.clone(), true).is_ok() {
            p.functions.extend(
                replaced
                    .functions
                    .into_iter()
                    .filter(|f| replacement_lines.contains(&f.line)),
            );
        }
    }
    Some(p)
}

fn check_mode(p: &mut Program, editor: bool) -> Result<(), String> {
    if !p.imports.is_empty() {
        return Err("importはnagicのファイル読み込み経路で解決してください".into());
    }
    let mut c = Checker {
        classes: HashMap::new(),
        functions: HashMap::new(),
        vars: HashMap::new(),
        ret: Type::named("unit"),
        asynchronous: false,
        scope: 0,
        editor,
        parameter_views: HashSet::new(),
        iterators: vec![],
    };
    let mut symbols = HashSet::new();
    for class in &p.classes {
        if !symbols.insert(class.name.clone()) {
            return Err(error(class.line, "型の重複定義"));
        }
        c.classes.insert(class.name.clone(), class.clone());
    }
    let mut checked_layouts = HashSet::new();
    for class in &p.classes {
        let mut fields = HashSet::new();
        for (index, (n, t)) in class.fields.iter().enumerate() {
            let line = class.field_lines.get(index).copied().unwrap_or(class.line);
            c.valid(t, line)?;
            c.emittable(t, line, false)?;
            if !fields.insert(n) {
                return Err(error(line, "フィールドの重複"));
            }
            if t.contains_view() {
                return Err(error(
                    line,
                    "0.1のclassにviewは保存できません。所有型またはcopyを使用してください",
                ));
            }
        }
        if recursive_layout(
            &Type::named(&class.name),
            &c.classes,
            &mut HashSet::new(),
            &mut checked_layouts,
        ) {
            return Err(error(
                class.line,
                "再帰する値型レイアウト。List等の間接格納が必要です",
            ));
        }
        for (index, (_, t)) in class.fields.iter().enumerate() {
            class_field(
                t,
                class.field_lines.get(index).copied().unwrap_or(class.line),
            )?;
        }
    }
    for f in &p.functions {
        if !symbols.insert(f.name.clone()) {
            return Err(error(f.line, "関数の重複定義"));
        }
        c.functions.insert(f.name.clone(), f.clone());
    }
    // Do not infer calls from an invalid signature, including functions checked
    // later in source order. Declaration-only symbol information remains usable.
    if editor {
        for f in &p.functions {
            c.valid(&f.ret, f.line)?;
            c.emittable(&f.ret, f.line, false)?;
            let mut names = HashSet::new();
            for (n, t) in &f.params {
                c.valid(t, f.line)?;
                c.emittable(t, f.line, false)?;
                if !names.insert(n) {
                    return Err(error(f.line, "引数の重複"));
                }
            }
        }
    }
    for f in &mut p.functions {
        c.vars.clear();
        c.parameter_views.clear();
        c.iterators.clear();
        c.ret = f.ret.clone();
        c.asynchronous = f.asynchronous;
        c.scope = 0;
        c.valid(&f.ret, f.line)?;
        c.emittable(&f.ret, f.line, false)?;
        for (index, (n, t)) in f.params.iter().enumerate() {
            c.valid(t, f.line)?;
            c.emittable(t, f.line, false)?;
            let binding = BindingId {
                line: f.line,
                token: f.parameter_spans[index].start,
            };
            let origins = if t.contains_view() {
                c.parameter_views.insert(binding);
                HashSet::from([BorrowedPlace {
                    binding,
                    fields: vec![],
                    // Parameter contents borrow outside the function. They are
                    // valid return origins without borrowing the parameter Vec.
                    owner_loan: false,
                }])
            } else {
                HashSet::new()
            };
            if c.vars
                .insert(
                    n.clone(),
                    Var {
                        binding,
                        ty: t.clone(),
                        moved: false,
                        moved_fields: HashSet::new(),
                        content_origins: vec![origins.clone(); Checker::content_depth(t)],
                        origins,
                        async_function: None,
                    },
                )
                .is_some()
            {
                return Err(error(f.line, "引数の重複"));
            }
        }
        let rust_paths: Vec<_> = f.attrs.iter().filter(|(name, _)| name == "rust").collect();
        if f.external {
            if f.name == "main" || rust_paths.len() != 1 || f.attrs.len() != 1 {
                return Err(error(f.line, "extern関数には@rust(\"native::関数名\")を1つ指定してください。main・HTTP属性は使えません"));
            }
            let target = &rust_paths[0].1;
            let valid = target.starts_with("native::")
                && target.split("::").all(|part| {
                    !part.is_empty()
                        && part.chars().enumerate().all(|(i, ch)| {
                            ch == '_' || ch.is_ascii_alphabetic() || i > 0 && ch.is_ascii_digit()
                        })
                });
            if !valid || f.ret.contains_view() {
                return Err(error(
                    f.line,
                    "Rustの関数パスが不正、またはexternの戻り値にviewがあります",
                ));
            }
            continue;
        }
        if !rust_paths.is_empty() {
            return Err(error(f.line, "@rustはextern関数にのみ指定できます"));
        }
        c.block(&mut f.body)?;
        if !editor && f.ret.0 != "unit" && !returns(&f.body) {
            return Err(error(f.line, "すべての経路で戻り値を返してください"));
        }
    }
    if !editor {
        crate::routes::validate(p)?;
    }
    Ok(())
}
fn returns(ss: &[Stmt]) -> bool {
    ss.iter().any(|s| match &s.kind {
        S::Return(_) => true,
        S::If(_, a, b) => returns(a) && returns(b),
        S::Match(_, arms) => arms.len() == 2 && arms.iter().all(|arm| returns(&arm.body)),
        _ => false,
    })
}
impl Checker {
    fn emittable(&self, t: &Type, line: usize, local: bool) -> Result<(), String> {
        fn contains_future(t: &Type) -> bool {
            t.is_future() || t.1.iter().any(contains_future)
        }
        // Direct local async aliases use Rust inference rather than a named
        // Future type. Their nested signature types still need to be emitted.
        let supported_alias = local
            && t.is_async_function()
            && t.1[..t.1.len() - 1].iter().all(|arg| !contains_future(arg))
            && !contains_future(&t.1.last().unwrap().inner());
        if contains_future(t) && !supported_alias {
            return Err(error(
                line,
                if t.is_future() {
                    "非同期処理の戻り値は変数へ保存できません。呼び出し時にawaitしてください"
                } else {
                    "async関数を引数・戻り値・コンテナーの型として指定することは未対応です。ローカル変数への代入とawait呼び出しは使えます"
                },
            ));
        }
        Ok(())
    }
    fn copy_type(&self, t: &Type) -> bool {
        fn visit(t: &Type, classes: &HashMap<String, Class>, depth: usize) -> bool {
            if depth > 64 {
                return false;
            }
            if t.is_copy() {
                true
            } else if t.0 == "Option" {
                visit(&t.inner(), classes, depth + 1)
            } else if let Some(c) = classes.get(&t.0) {
                c.fields.iter().all(|(_, t)| visit(t, classes, depth + 1))
            } else {
                false
            }
        }
        visit(t, &self.classes, 0)
    }
    fn valid(&self, t: &Type, line: usize) -> Result<(), String> {
        if t.0 == "fn" && !t.1.is_empty() {
            for parameter in &t.1 {
                if parameter.is_future() {
                    self.valid(&parameter.inner(), line)?;
                } else {
                    self.valid(parameter, line)?;
                }
            }
            return Ok(());
        }
        let arity = match t.0.as_str() {
            "List" | "view" | "owned" | "shared" | "Option" => Some(1),
            "Map" | "Result" => Some(2),
            _ => None,
        };
        if let Some(n) = arity {
            if t.1.len() != n {
                return Err(error(line, format!("{t} の型引数は{n}個です")));
            }
            for a in &t.1 {
                self.valid(a, line)?;
            }
            return Ok(());
        }
        if !t.1.is_empty() {
            return Err(error(
                line,
                format!("generic class/functionは0.1では未実装です: {t}"),
            ));
        }
        if self.classes.contains_key(&t.0)
            || [
                "i8",
                "i16",
                "i32",
                "i64",
                "u8",
                "u16",
                "u32",
                "u64",
                "f32",
                "f64",
                "bool",
                "str",
                "bytes",
                "unit",
                "Error",
                "Db",
                "Html",
                "UUID",
                "timestamp",
            ]
            .contains(&t.0.as_str())
        {
            Ok(())
        } else {
            Err(error(line, format!("未定義の型: {t}")))
        }
    }
    fn demand(&self, a: &Type, b: &Type, line: usize) -> Result<(), String> {
        if matches_type(a, b) {
            Ok(())
        } else {
            Err(error(
                line,
                format!("型が一致しません: expected {b}, got {a}。変換は明示してください"),
            ))
        }
    }
    fn origin(&self, e: &Expr) -> HashSet<BorrowedPlace> {
        self.origin_at(e, 0)
    }
    fn content_depth(t: &Type) -> usize {
        if !t.contains_view() {
            return 0;
        }
        if matches!(t.0.as_str(), "List" | "view") && t.inner().contains_view() {
            1 + Self::content_depth(&t.inner())
        } else {
            t.1.iter().map(Self::content_depth).max().unwrap_or(0)
        }
    }
    fn content_origins(&self, e: &Expr, ty: &Type, offset: usize) -> Vec<HashSet<BorrowedPlace>> {
        (1..=Self::content_depth(ty))
            .map(|depth| self.origin_at(e, depth + offset))
            .collect()
    }
    fn origin_at(&self, e: &Expr, depth: usize) -> HashSet<BorrowedPlace> {
        match &e.kind {
            E::Name(n) => self
                .vars
                .get(n)
                .map(|v| {
                    if depth > 0 {
                        v.content_origins
                            .get(depth - 1)
                            .cloned()
                            .unwrap_or_default()
                    } else if v.ty.contains_view() {
                        v.origins.clone()
                    } else {
                        HashSet::from([BorrowedPlace {
                            binding: v.binding,
                            fields: vec![],
                            owner_loan: true,
                        }])
                    }
                })
                .unwrap_or_default(),
            E::Field(parent, field) => self
                .origin(parent)
                .into_iter()
                .map(|mut place| {
                    place.fields.push(field.clone());
                    place
                })
                .collect(),
            E::Call(n, _, args) if n == "view" && e.resolution == Some(NameResolution::Builtin) => {
                if depth > 0 {
                    // Indexing a view copies its element, which does not retain
                    // the outer container loan. Nested elements can still borrow.
                    return self.origin_at(&args[0], depth);
                }
                let mut origins = self.origin(&args[0]);
                if let Some((name, fields)) = Self::place(&args[0]) {
                    if let Some(var) = self.vars.get(name) {
                        origins.insert(BorrowedPlace {
                            binding: var.binding,
                            fields,
                            owner_loan: true,
                        });
                    }
                }
                origins
            }
            E::Call(n, _, args)
                if n == "json_decode"
                    && e.resolution == Some(NameResolution::Builtin)
                    && e.ty.as_ref().is_some_and(Type::contains_view) =>
            {
                // Deserialized views borrow the input, including owned str/bytes.
                // Direct string literals are emitted as static input and have no place.
                self.origin(&args[0])
            }
            E::Call(n, _, args)
                if e.resolution == Some(NameResolution::Builtin)
                    && e.ty.as_ref().is_some_and(Type::contains_view)
                    && matches!(
                        n.as_str(),
                        "copy" | "slice" | "ok" | "some" | "share" | "clone_shared"
                    ) =>
            {
                self.origin_at(&args[0], if n == "copy" { depth.max(1) } else { depth })
            }
            E::Call(_, _, args) if e.ty.as_ref().is_some_and(Type::contains_view) => args
                .iter()
                .filter(|arg| arg.ty.as_ref().is_some_and(Type::contains_view))
                .flat_map(|arg| self.origin(arg))
                .collect(),
            E::List(values) if e.ty.as_ref().is_some_and(Type::contains_view) => values
                .iter()
                .flat_map(|value| self.origin_at(value, depth.saturating_sub(1)))
                .collect(),
            E::Try(e) | E::Await(e) => self.origin_at(e, depth),
            E::Index(e, _) => self.origin_at(e, depth + 1),
            _ => HashSet::new(),
        }
    }
    fn borrows_temporary(e: &Expr) -> bool {
        Self::borrows_temporary_at(e, 0)
    }
    fn borrows_temporary_at(e: &Expr, depth: usize) -> bool {
        if !e.ty.as_ref().is_some_and(Type::contains_view) {
            return false;
        }
        match &e.kind {
            E::Call(n, _, args) if e.resolution == Some(NameResolution::Builtin) => {
                match n.as_str() {
                    "view" => {
                        if depth == 0 {
                            Self::place(&args[0]).is_none()
                        } else {
                            Self::borrows_temporary_at(&args[0], depth)
                        }
                    }
                    "json_decode" => {
                        let input = &args[0];
                        if input.ty.as_ref().is_some_and(Type::is_view) {
                            Self::borrows_temporary(input)
                        } else {
                            // string_arg emits literals directly, rather than a temporary String.
                            !matches!(input.kind, E::Str(_)) && Self::place(input).is_none()
                        }
                    }
                    "copy" => Self::borrows_temporary_at(&args[0], depth.max(1)),
                    "slice" | "ok" | "some" | "share" | "clone_shared" => {
                        Self::borrows_temporary_at(&args[0], depth)
                    }
                    _ => args.iter().any(Self::borrows_temporary),
                }
            }
            E::Call(_, _, args) => args.iter().any(Self::borrows_temporary),
            E::List(args) => args
                .iter()
                .any(|arg| Self::borrows_temporary_at(arg, depth.saturating_sub(1))),
            E::Try(e) | E::Await(e) | E::Field(e, _) => Self::borrows_temporary_at(e, depth),
            E::Index(e, _) => Self::borrows_temporary_at(e, depth + 1),
            _ => false,
        }
    }
    fn borrowed_place(&self, place: &BorrowedPlace) -> bool {
        self.vars.values().any(|v| {
            v.binding != place.binding
                && !v.moved
                && v.ty.contains_view()
                && v.origins.iter().any(|loan| loan.overlaps(place))
        }) || self
            .iterators
            .iter()
            .flatten()
            .any(|loan| loan.overlaps(place))
    }
    fn borrowed(&self, n: &str) -> bool {
        self.vars.get(n).is_some_and(|v| {
            self.borrowed_place(&BorrowedPlace {
                binding: v.binding,
                fields: vec![],
                owner_loan: true,
            })
        })
    }
    fn place(e: &Expr) -> Option<(&str, Vec<String>)> {
        match &e.kind {
            E::Name(n) => Some((n, vec![])),
            E::Field(parent, field) => {
                let (name, mut fields) = Self::place(parent)?;
                fields.push(field.clone());
                Some((name, fields))
            }
            _ => None,
        }
    }
    fn available(&self, e: &Expr, projection: bool) -> Result<(), String> {
        let Some((name, fields)) = Self::place(e) else {
            return Ok(());
        };
        let Some(v) = self.vars.get(name) else {
            return Ok(());
        };
        if v.moved {
            return Err(error(e.line, format!("{name} はmove後に使用されています")));
        }
        if let Some(moved) = v
            .moved_fields
            .iter()
            .filter(|moved| fields.starts_with(moved) || !projection && moved.starts_with(&fields))
            .min()
        {
            let place = if fields.is_empty() {
                name.to_owned()
            } else {
                format!("{name}.{}", fields.join("."))
            };
            let reason = if fields == *moved {
                format!("{place} はmove後に使用されています")
            } else {
                format!(
                    "{place} は {name}.{} のmove後に使用されています",
                    moved.join(".")
                )
            };
            return Err(error(e.line, format!("{reason}。再利用する値はmove前に複製してください（文字列などはcopy(view(...))）")));
        }
        Ok(())
    }
    fn consume(&mut self, e: &Expr) -> Result<(), String> {
        self.available(e, false)?;
        let Some((name, fields)) = Self::place(e) else {
            return Ok(());
        };
        let Some(v) = self.vars.get(name) else {
            return Ok(());
        };
        if self.copy_type(e.ty.as_ref().unwrap_or(&v.ty)) {
            return Ok(());
        }
        if self.borrowed_place(&BorrowedPlace {
            binding: v.binding,
            fields: fields.clone(),
            owner_loan: true,
        }) {
            return Err(error(
                e.line,
                format!(
                    "{name} はviewまたはループから参照されています。借用を終了するかcopyしてください"
                ),
            ));
        }
        let v = self.vars.get_mut(name).unwrap();
        if fields.is_empty() {
            v.moved = true;
        } else {
            v.moved_fields.insert(fields);
        }
        Ok(())
    }
    fn child(&mut self, ss: &mut [Stmt]) -> Result<HashMap<String, Var>, String> {
        let before = self.vars.clone();
        let checked = self.block(ss);
        let after = self.vars.clone();
        self.vars = before;
        checked?;
        Ok(after)
    }
    fn merge_moves(&mut self, after: HashMap<String, Var>) {
        for (n, v) in after {
            if let Some(x) = self.vars.get_mut(&n) {
                x.moved |= v.moved;
                x.moved_fields.extend(v.moved_fields);
                x.origins.extend(v.origins);
                for (contents, origins) in x.content_origins.iter_mut().zip(v.content_origins) {
                    contents.extend(origins);
                }
            }
        }
    }
    /// Only paths that reach the next statement contribute ownership state.
    /// Reassignment can restore a value when every continuing path restores it.
    fn join_moves(&mut self, paths: &[HashMap<String, Var>]) {
        if paths.is_empty() {
            return;
        }
        for (name, var) in &mut self.vars {
            var.moved = paths.iter().any(|path| path[name].moved);
            var.moved_fields = paths
                .iter()
                .flat_map(|path| path[name].moved_fields.iter().cloned())
                .collect();
            var.origins = paths
                .iter()
                .flat_map(|path| path[name].origins.iter().cloned())
                .collect();
            for (depth, contents) in var.content_origins.iter_mut().enumerate() {
                *contents = paths
                    .iter()
                    .flat_map(|path| path[name].content_origins[depth].iter().cloned())
                    .collect();
            }
        }
    }
    fn loop_body(
        &mut self,
        body: &mut [Stmt],
        mut condition: Option<&mut Expr>,
        binding: Option<(&str, Var)>,
    ) -> Result<(), String> {
        // Recheck the parsed body, not its annotated first pass: locals must be
        // declared anew on each iteration. Only outer moves cross the backedge.
        let template = body.to_vec();
        let condition_template = condition.as_deref().cloned();
        let mut header = self.vars.clone();
        let mut first = true;
        loop {
            self.vars = header.clone();
            let editor = self.editor;
            if !first {
                self.editor = false;
            }
            let checked = (|| {
                if let Some(c) = condition.as_deref_mut() {
                    let mut probe;
                    let c = if first {
                        c
                    } else {
                        probe = condition_template.clone().unwrap();
                        &mut probe
                    };
                    let t = self.expr(c, Some(&Type::named("bool")))?;
                    self.demand(&t, &Type::named("bool"), c.line)?;
                }
                // A while condition is also evaluated on its exit path. A for
                // iterator was evaluated once before entering this helper.
                let exit = self.vars.clone();
                if let Some((name, binding)) = &binding {
                    self.vars.insert((*name).to_owned(), binding.clone());
                }
                if first {
                    self.block(body)?;
                } else {
                    self.block(&mut template.clone())?;
                }
                Ok::<_, String>((exit, self.vars.clone()))
            })();
            self.editor = editor;
            let (exit, mut after) = checked.map_err(|message| {
                if !first && message.contains("move後") && !message.contains("次の周回") {
                    format!("{message}（ループの次の周回でも使用されるため）")
                } else {
                    message
                }
            })?;
            if returns(body) {
                self.vars = exit;
                return Ok(());
            }
            if let Some((name, _)) = &binding {
                after.remove(*name);
            }
            self.vars = header.clone();
            self.merge_moves(after);
            let next = self.vars.clone();
            self.vars = exit;
            if next == header {
                return Ok(());
            }
            // This union only adds moved places from a finite source program.
            // Stop when every possible iteration starts with the same state.
            header = next;
            first = false;
        }
    }
    fn block(&mut self, ss: &mut [Stmt]) -> Result<(), String> {
        // A path that returns will never advance any enclosing iterator again.
        // New loops inside this block still acquire their own loans.
        let ending = returns(ss).then(|| std::mem::take(&mut self.iterators));
        let checked = self.block_statements(ss);
        if let Some(iterators) = ending {
            self.iterators = iterators;
        }
        checked
    }
    fn block_statements(&mut self, ss: &mut [Stmt]) -> Result<(), String> {
        for s in ss {
            if self.editor {
                let before = self.vars.clone();
                let scope = self.scope;
                if self.statement(s).is_err() {
                    self.vars = before;
                    self.scope = scope;
                }
            } else {
                self.statement(s)?;
            }
        }
        Ok(())
    }
    fn statement(&mut self, s: &mut Stmt) -> Result<(), String> {
        match &mut s.kind {
            S::Assign {
                name,
                annotation,
                value,
                declare,
            } => {
                let old = self.vars.get(name).cloned();
                if old.is_some() && *declare {
                    return Err(error(s.line, "同じscope内でletを重複できません"));
                }
                let expected = annotation.as_ref().or_else(|| old.as_ref().map(|v| &v.ty));
                let ty = self.expr(value, expected)?;
                if let Some(t) = expected {
                    self.demand(&ty, t, s.line)?;
                }
                if let Some(a) = annotation.as_ref() {
                    self.valid(a, s.line)?;
                }
                self.emittable(&ty, s.line, true)?;
                let async_function = if ty.is_async_function() {
                    match (&value.kind, value.resolution) {
                        (E::Name(n), Some(NameResolution::Function)) => Some(n.clone()),
                        (E::Name(n), Some(NameResolution::Local)) => {
                            self.vars[n].async_function.clone()
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                if old
                    .as_ref()
                    .is_some_and(|v| v.ty.is_async_function() && v.async_function != async_function)
                {
                    return Err(error(s.line, "async関数を入れた変数には、別のasync関数を再代入できません。別の変数を使うか、呼び出しを分岐してください"));
                }
                if !old.as_ref().is_some_and(|v| v.ty.is_view()) && self.borrowed(name) {
                    return Err(error(
                        s.line,
                        "viewまたはループから参照中の所有値は再代入できません",
                    ));
                }
                let origin = if ty.contains_view() {
                    if Self::borrows_temporary(value) {
                        return Err(error(s.line, "一時的な所有値からのviewは保存できません。所有値を変数へ保存してからviewを作るか、同じ式でcopyしてください"));
                    }
                    self.origin(value)
                } else {
                    HashSet::new()
                };
                let content_origins = self.content_origins(value, &ty, 0);
                self.consume(value)?;
                *declare = old.is_none();
                *annotation = Some(ty.clone());
                s.binding_type = Some(ty.clone());
                self.vars.insert(
                    name.clone(),
                    Var {
                        binding: old.as_ref().map(|v| v.binding).unwrap_or(BindingId {
                            line: s.line,
                            token: s.binding_span.unwrap_or_default().start,
                        }),
                        ty,
                        moved: false,
                        moved_fields: HashSet::new(),
                        origins: origin,
                        content_origins,
                        async_function,
                    },
                );
            }
            S::Return(e) => {
                if self.scope > 0 {
                    return Err(error(
                        s.line,
                        "scope内のreturnは0.1では未対応です。scope終了後に返してください",
                    ));
                }
                let ret = self.ret.clone();
                let ty = if let Some(e) = e {
                    let ty = self.expr(e, Some(&ret))?;
                    if ty.contains_view() {
                        let origin = self.origin(e);
                        if Self::borrows_temporary(e)
                            || origin.is_empty()
                            || origin.iter().any(|place| {
                                place.owner_loan || !self.parameter_views.contains(&place.binding)
                            })
                        {
                            return Err(error(s.line,"request/local-scoped view escapes its lifetime。長生きさせる値にはcopy()を使用してください"));
                        }
                    }
                    self.consume(e)?;
                    ty
                } else {
                    Type::named("unit")
                };
                self.demand(&ty, &ret, s.line)?;
            }
            S::Expr(e) => {
                let t = self.expr(e, None)?;
                if t.is_future() {
                    return Err(error(
                        s.line,
                        "async呼び出しはawaitまたはscope内のspawnで実行してください",
                    ));
                }
                if t.0 == "Result" {
                    return Err(error(
                        s.line,
                        "Resultを無視できません。tryで伝播するか変数へ受けてください",
                    ));
                }
            }
            S::If(c, a, b) => {
                let t = self.expr(c, Some(&Type::named("bool")))?;
                self.demand(&t, &Type::named("bool"), s.line)?;
                let am = self.child(a)?;
                let bm = self.child(b)?;
                let mut paths = vec![];
                if !returns(a) {
                    paths.push(am);
                }
                if !returns(b) {
                    paths.push(bm);
                }
                self.join_moves(&paths);
            }
            S::While(c, b) => {
                self.loop_body(b, Some(c), None)?;
            }
            S::Match(value, arms) => {
                let ty = self.expr(value, None)?;
                if ty.0 != "Result" {
                    return Err(error(s.line, "matchの対象はResultです"));
                }
                let mut seen = HashSet::new();
                for arm in arms.iter() {
                    if !seen.insert(arm.ok) {
                        return Err(error(arm.line, "Ok / Errのcaseが重複しています"));
                    }
                }
                if seen.len() != 2 {
                    return Err(error(s.line, "matchにはOkとErrの両方のcaseが必要です"));
                }
                let origin = self.origin(value);
                let content_origins = self.content_origins(value, &ty, 0);
                self.consume(value)?;
                let before = self.vars.clone();
                let mut moves = vec![];
                for arm in arms {
                    self.vars = before.clone();
                    if let Some(name) = &arm.binding {
                        if self.vars.contains_key(name) {
                            return Err(error(
                                arm.line,
                                "caseの変数名は外側の変数と重複できません",
                            ));
                        }
                        let payload = ty.1[usize::from(!arm.ok)].clone();
                        arm.binding_type = Some(payload.clone());
                        self.vars.insert(
                            name.clone(),
                            Var {
                                binding: BindingId {
                                    line: arm.line,
                                    token: arm.binding_span.start,
                                },
                                origins: if payload.contains_view() {
                                    origin.clone()
                                } else {
                                    HashSet::new()
                                },
                                content_origins: content_origins
                                    .iter()
                                    .take(Self::content_depth(&payload))
                                    .cloned()
                                    .collect(),
                                ty: payload,
                                moved: false,
                                moved_fields: HashSet::new(),
                                async_function: None,
                            },
                        );
                    }
                    self.block(&mut arm.body)?;
                    let mut after = self.vars.clone();
                    if let Some(name) = &arm.binding {
                        after.remove(name);
                    }
                    if !returns(&arm.body) {
                        moves.push(after);
                    }
                }
                self.vars = before;
                self.join_moves(&moves);
            }
            S::For(n, e, b) => {
                let t = self.expr(e, None)?;
                let elem = match t.0.as_str() {
                    "Range" => Type::named("i64"),
                    "List" | "view" => t.inner(),
                    _ => return Err(error(s.line, "forにはrangeまたは連続配列が必要です")),
                };
                if !self.copy_type(&elem) {
                    return Err(error(s.line, "0.1のfor要素はprimitiveに限定されています"));
                }
                s.binding_type = Some(elem.clone());
                let origins = self.origin(e);
                let element_origins = self.origin_at(e, 1);
                let content_origins = self.content_origins(e, &elem, 1);
                let mut loans = origins.clone();
                if t.0 == "List" {
                    if let Some((name, fields)) = Self::place(e) {
                        if let Some(var) = self.vars.get(name) {
                            loans.insert(BorrowedPlace {
                                binding: var.binding,
                                fields,
                                owner_loan: true,
                            });
                        }
                    }
                }
                self.iterators.push(loans);
                let checked = self.loop_body(
                    b,
                    None,
                    Some((
                        n,
                        Var {
                            binding: BindingId {
                                line: s.line,
                                token: s.binding_span.unwrap_or_default().start,
                            },
                            origins: if elem.contains_view() {
                                element_origins
                            } else {
                                HashSet::new()
                            },
                            ty: elem,
                            moved: false,
                            moved_fields: HashSet::new(),
                            content_origins,
                            async_function: None,
                        },
                    )),
                );
                self.iterators.pop();
                checked?;
            }
            S::Scope(b) => {
                if !self.asynchronous || self.ret.0 != "Result" {
                    return Err(error(s.line, "scopeはasync Result関数内で使用してください"));
                }
                self.scope += 1;
                let m = self.child(b);
                self.scope -= 1;
                self.merge_moves(m?);
            }
            S::Spawn(e) => {
                if self.scope == 0 {
                    return Err(error(
                        s.line,
                        "spawnはasync with scopeの中で使用してください",
                    ));
                }
                let t = self.expr(e, None)?;
                if t != future(Type::named("unit")) && t != future(result(Type::named("unit"))) {
                    return Err(error(
                        s.line,
                        "0.1のspawnはasync unitまたはResult[unit,Error]を取ります",
                    ));
                }
                fn names(e: &Expr, out: &mut Vec<String>) {
                    match &e.kind {
                        E::Name(n) => out.push(n.clone()),
                        E::Call(_, _, a) | E::List(a) => {
                            for e in a {
                                names(e, out)
                            }
                        }
                        _ => {}
                    }
                }
                let mut ns = vec![];
                names(e, &mut ns);
                for n in ns {
                    if self.vars.get(&n).is_some_and(|v| v.ty.contains_view()) {
                        return Err(error(
                            s.line,
                            "viewを別taskへ渡せません。copyを使用してください",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    fn expr(&mut self, e: &mut Expr, expected: Option<&Type>) -> Result<Type, String> {
        self.expr_mode(e, expected, false)
    }
    fn expr_mode(
        &mut self,
        e: &mut Expr,
        expected: Option<&Type>,
        projection: bool,
    ) -> Result<Type, String> {
        let line = e.line;
        e.resolution = None;
        let t = match &mut e.kind {
            E::Int(s) => {
                let ty = expected
                    .filter(|t| {
                        ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"]
                            .contains(&t.0.as_str())
                    })
                    .cloned()
                    .unwrap_or_else(|| Type::named("i64"));
                let n = s
                    .parse::<u128>()
                    .map_err(|_| error(line, "整数リテラルが大きすぎます"))?;
                let max = match ty.0.as_str() {
                    "i8" => 127,
                    "i16" => 32767,
                    "i32" => i32::MAX as u128,
                    "i64" => i64::MAX as u128,
                    "u8" => 255,
                    "u16" => 65535,
                    "u32" => u32::MAX as u128,
                    _ => u64::MAX as u128,
                };
                if n > max {
                    return Err(error(line, format!("{ty} の範囲を超えています")));
                }
                ty
            }
            E::Float(s) => {
                let ty = expected
                    .filter(|t| t.0 == "f32" || t.0 == "f64")
                    .cloned()
                    .unwrap_or_else(|| Type::named("f64"));
                let finite = if ty.0 == "f32" {
                    s.parse::<f32>().is_ok_and(|x| x.is_finite())
                } else {
                    s.parse::<f64>().is_ok_and(|x| x.is_finite())
                };
                if !finite {
                    return Err(error(line, "浮動小数リテラルが範囲外です"));
                }
                ty
            }
            E::Str(_) => Type::named("str"),
            E::Bool(_) => Type::named("bool"),
            E::Null => expected
                .filter(|t| t.0 == "Option")
                .cloned()
                .ok_or_else(|| error(line, "Noneにはnullableの型注釈が必要です"))?,
            E::Name(n) => {
                if let Some(v) = self.vars.get(n) {
                    e.resolution = Some(NameResolution::Local);
                    v.ty.clone()
                } else if let Some(f) = self.functions.get(n) {
                    e.resolution = Some(NameResolution::Function);
                    let mut ts: Vec<Type> = f.params.iter().map(|p| p.1.clone()).collect();
                    ts.push(if f.asynchronous {
                        future(f.ret.clone())
                    } else {
                        f.ret.clone()
                    });
                    Type::generic("fn", ts)
                } else {
                    return Err(error(line, format!("未定義の変数: {n}")));
                }
            }
            E::Unary(op, x) => {
                let boundary = if op == "-" {
                    negative_boundary_type(x, expected)
                } else {
                    None
                };
                let t = if let Some(ty) = boundary {
                    x.ty = Some(ty.clone());
                    ty
                } else {
                    self.expr(x, expected)?
                };
                if op == "not" {
                    self.demand(&t, &Type::named("bool"), line)?;
                } else if !matches!(t.0.as_str(), "i8" | "i16" | "i32" | "i64" | "f32" | "f64") {
                    return Err(error(line, format!("{t}は符号反転できません。符号付き整数または浮動小数点数を指定してください")));
                }
                t
            }
            E::Binary(a, op, b) => {
                let left = self.expr(a, expected.filter(|t| t.0 != "bool"))?;
                let right = self.expr(b, Some(&left))?;
                self.demand(&right, &left, line)?;
                match op.as_str() {
                    "and" | "or" => {
                        self.demand(&left, &Type::named("bool"), line)?;
                        Type::named("bool")
                    }
                    "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                        let ordered = !matches!(op.as_str(), "==" | "!=");
                        if (!left.is_copy() && left.0 != "str") || !comparable(&left, ordered) {
                            return Err(error(
                                line,
                                format!("{left}は{op}による比較に対応していません"),
                            ));
                        }
                        Type::named("bool")
                    }
                    _ => {
                        if ![
                            "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
                        ]
                        .contains(&left.0.as_str())
                        {
                            return Err(error(line, "演算にはnative数値型が必要です"));
                        }
                        left
                    }
                }
            }
            E::Field(x, n) => {
                // The parent is only a field base. Check the complete path so
                // moving one field does not prevent access to its siblings.
                let t = self.expr_mode(x, None, true)?;
                self.classes
                    .get(&t.0)
                    .and_then(|c| c.fields.iter().find(|(k, _)| k == n))
                    .map(|f| f.1.clone())
                    .ok_or_else(|| error(line, format!("{t} にフィールド {n} はありません")))?
            }
            E::Index(x, i) => {
                let t = self.expr(x, None)?;
                let ix = self.expr(i, Some(&Type::named("i64")))?;
                self.demand(&ix, &Type::named("i64"), line)?;
                if t.0 == "List" || t.0 == "view" {
                    let a = t.inner();
                    if !self.copy_type(&a) {
                        return Err(error(line, "非Copy要素のindex取得は0.1では未対応です"));
                    }
                    a
                } else {
                    return Err(error(line, "indexには配列が必要です"));
                }
            }
            E::List(a) => {
                let exp = expected.filter(|t| t.0 == "List").map(Type::inner);
                let elem = if let Some(exp) = exp {
                    exp
                } else if let Some(x) = a.first_mut() {
                    self.expr(x, None)?
                } else {
                    return Err(error(line, "空配列には型注釈が必要です"));
                };
                for x in a {
                    let t = self.expr(x, Some(&elem))?;
                    self.demand(&t, &elem, line)?;
                    self.consume(x)?;
                }
                Type::generic("List", vec![elem])
            }
            E::Record(n, fields) => {
                let class = self
                    .classes
                    .get(n)
                    .cloned()
                    .ok_or_else(|| error(line, format!("未定義のclass: {n}")))?;
                let mut seen = HashSet::new();
                if class.fields.len() != fields.len() {
                    return Err(error(line, "classの全フィールドを指定してください"));
                }
                for (key, value) in fields {
                    if !seen.insert(key.clone()) {
                        return Err(error(line, "classフィールドの重複"));
                    }
                    let exp = class
                        .fields
                        .iter()
                        .find(|(k, _)| k == key)
                        .ok_or_else(|| error(line, "不明なフィールド"))?
                        .1
                        .clone();
                    let got = self.expr(value, Some(&exp))?;
                    self.demand(&got, &exp, line)?;
                    self.consume(value)?;
                }
                Type::named(n)
            }
            E::Await(x) => {
                if !self.asynchronous {
                    return Err(error(line, "awaitはasync関数内で使用してください"));
                }
                let t = self.expr(x, None)?;
                if !t.is_future() {
                    return Err(error(line, "await対象はasync呼び出しです"));
                }
                t.inner()
            }
            E::Try(x) => {
                if self.ret.0 != "Result" {
                    return Err(error(line, "tryで伝播する関数の戻り値はResultが必要です"));
                }
                let t = self.expr(x, None)?;
                if t.0 != "Result" {
                    return Err(error(line, "try対象はResultです"));
                }
                self.demand(&t.1[1], &self.ret.1[1], line)?;
                t.inner()
            }
            E::Call(n, ts, args) => {
                for t in ts.iter() {
                    self.valid(t, line)?;
                }
                let signature = if let Some(v) = self.vars.get(n) {
                    if v.ty.0 != "fn" {
                        return Err(error(line, format!("{n} は呼び出せる関数ではありません")));
                    }
                    e.resolution = Some(NameResolution::Local);
                    Some(v.ty.1.clone())
                } else if let Some(f) = self.functions.get(n) {
                    e.resolution = Some(NameResolution::Function);
                    let mut signature: Vec<_> = f.params.iter().map(|(_, t)| t.clone()).collect();
                    signature.push(if f.asynchronous {
                        future(f.ret.clone())
                    } else {
                        f.ret.clone()
                    });
                    Some(signature)
                } else {
                    None
                };
                if let Some(mut signature) = signature {
                    if !ts.is_empty() {
                        return Err(error(line, "generic関数は未実装です"));
                    }
                    let ret = signature.pop().unwrap();
                    if args.len() != signature.len() {
                        return Err(error(line, "引数の数が一致しません"));
                    }
                    for (arg, t) in args.iter_mut().zip(&signature) {
                        let got = self.expr(arg, Some(t))?;
                        self.demand(&got, t, line)?;
                        self.consume(arg)?;
                    }
                    ret
                } else {
                    e.resolution = Some(NameResolution::Builtin);
                    self.builtin(n, ts, args, expected, line)?
                }
            }
        };
        self.available(e, projection)?;
        e.ty = Some(t.clone());
        Ok(t)
    }
    fn builtin(
        &mut self,
        n: &str,
        ts: &[Type],
        args: &mut [Expr],
        expected: Option<&Type>,
        line: usize,
    ) -> Result<Type, String> {
        let arity = match n {
            "clock_ns" | "supervisor_demo" | "read_line" => 0,
            "size_of" => 0,
            "print" | "write" | "html" | "include_text" | "view" | "copy" | "share"
            | "clone_shared" | "len" | "range" | "sleep" | "db_open" | "json_decode"
            | "json_encode" | "ok" | "some" | "error" | "not_found" | "internal_error" | "fail"
            | "error_kind" | "error_message" | "assert_true" | "parse_i64" | "parse_f64"
            | "make_ints" | "actor_demo" | "actor_pair_demo" | "queue_demo" | "task_demo"
            | "cpu_sum" | "i64" | "i32" | "uuid_parse" | "uuid_format" => 1,
            "db_exec" | "db_all" | "append" | "serve" | "env" => 2,
            "db_query" | "db_write" | "slice" | "bench_i64" | "bench_f64" | "bench_scalar" => 3,
            "db_insert" => 4,
            "db_update" => 5,
            _ => return Err(error(line, format!("未定義の関数: {n}"))),
        };
        if arity != args.len() {
            return Err(error(line, format!("{n} は{arity}引数です")));
        }
        let generic = matches!(
            n,
            "json_decode" | "db_query" | "db_insert" | "db_update" | "db_all" | "size_of"
        );
        if generic && ts.len() != 1 || !generic && !ts.is_empty() {
            return Err(error(line, "型引数の数が一致しません"));
        }
        if matches!(n, "db_all" | "db_query" | "db_insert" | "db_update") {
            let row = unowned(&ts[0]);
            if !row.1.is_empty()
                || matches!(
                    row.0.as_str(),
                    "str" | "bytes" | "unit" | "Error" | "Db" | "Html" | "UUID" | "timestamp"
                )
                || !self.classes.contains_key(&row.0)
            {
                return Err(error(line, format!("{n}の型引数 {} は行に使用できません。classを指定してください（FromRowは生成コードまたはRust連携で実装します）", ts[0])));
            }
        }
        if n == "json_decode" && !json_type_supported(&ts[0], true, &self.classes) {
            return Err(error(line, format!("json_decodeの型引数 {} はJSONの読み取りに対応していません（Deserializeが必要です）", ts[0])));
        }
        let mut types = vec![];
        for (i, a) in args.iter_mut().enumerate() {
            let hint = match n {
                "ok" => expected.filter(|t| t.0 == "Result").map(Type::inner),
                "some" => expected.filter(|t| t.0 == "Option").map(Type::inner),
                "db_insert" if i == 3 => Some(Type::named("i32")),
                "db_update" if i == 4 => Some(Type::named("i32")),
                _ => None,
            };
            types.push(self.expr(a, hint.as_ref())?);
        }
        let require = |idx: usize, t: Type| {
            if types[idx] == t {
                Ok(())
            } else {
                Err(error(
                    line,
                    format!("型が一致しません: expected {t}, got {}", types[idx]),
                ))
            }
        };
        let is_string =
            |t: &Type| t.0 == "str" || t == &Type::generic("view", vec![Type::named("str")]);
        match n {
            "print" | "write" => {
                if !is_string(&types[0])
                    && !matches!(
                        types[0].0.as_str(),
                        "i8" | "i16"
                            | "i32"
                            | "i64"
                            | "u8"
                            | "u16"
                            | "u32"
                            | "u64"
                            | "f32"
                            | "f64"
                            | "bool"
                            | "UUID"
                    )
                {
                    return Err(error(
                        line,
                        format!(
                            "{n}に{}は渡せません。数値・bool・str・view[str]・UUIDを指定してください",
                            types[0]
                        ),
                    ));
                }
                Ok(Type::named("unit"))
            }
            "read_line" => Ok(result(Type::named("str"))),
            "html" => {
                require(0, Type::named("str"))?;
                self.consume(&args[0])?;
                Ok(Type::named("Html"))
            }
            "include_text" => {
                if !matches!(args[0].kind, E::Str(_)) {
                    return Err(error(
                        line,
                        "include_textにはファイルパスの文字列リテラルが必要です",
                    ));
                }
                Ok(Type::named("str"))
            }
            "view" => {
                let t = &types[0];
                if t.0 == "str" || t.0 == "bytes" {
                    Ok(Type::generic("view", vec![t.clone()]))
                } else if t.0 == "List" {
                    Ok(Type::generic("view", vec![t.inner()]))
                } else {
                    Err(error(line, "viewにはstr/bytes/Listの所有値が必要です"))
                }
            }
            "copy" => {
                if !types[0].is_view() {
                    return Err(error(line, "copyの対象はviewです"));
                }
                let t = types[0].inner();
                Ok(if t.0 == "str" || t.0 == "bytes" {
                    t
                } else {
                    Type::generic("List", vec![t])
                })
            }
            "share" => {
                self.consume(&args[0])?;
                Ok(Type::generic("shared", vec![types[0].clone()]))
            }
            "clone_shared" => {
                if types[0].0 != "shared" {
                    return Err(error(line, "clone_sharedにはsharedが必要です"));
                }
                Ok(types[0].clone())
            }
            "len" => {
                if !["str", "bytes", "List", "view"].contains(&types[0].0.as_str()) {
                    return Err(error(line, "len対象が不正です"));
                }
                Ok(Type::named("i64"))
            }
            "range" => {
                require(0, Type::named("i64"))?;
                Ok(Type::named("Range"))
            }
            "sleep" => {
                require(0, Type::named("i64"))?;
                Ok(future(Type::named("unit")))
            }
            "serve" => {
                require(0, Type::named("Db"))?;
                require(1, Type::named("i64"))?;
                self.consume(&args[0])?;
                Ok(future(result(Type::named("unit"))))
            }
            "env" => {
                require(0, Type::named("str"))?;
                require(1, Type::named("str"))?;
                Ok(Type::named("str"))
            }
            "db_open" => {
                if !is_string(&types[0]) {
                    return Err(error(line, "db_openはパス文字列を取ります"));
                }
                Ok(future(result(Type::named("Db"))))
            }
            "db_exec" | "db_all" | "db_query" | "db_insert" | "db_update" | "db_write" => {
                require(0, Type::named("Db"))?;
                if !is_string(&types[1]) {
                    return Err(error(line, "SQLは文字列です"));
                }
                if n == "db_query" || n == "db_write" || n == "db_update" {
                    require(2, Type::named("i64"))?;
                }
                if n == "db_insert" {
                    require(2, Type::named("str"))?;
                    require(3, Type::named("i32"))?;
                    self.consume(&args[2])?;
                }
                if n == "db_update" {
                    require(3, Type::named("str"))?;
                    require(4, Type::named("i32"))?;
                    self.consume(&args[3])?;
                }
                let ret = match n {
                    "db_exec" | "db_write" => Type::named("i64"),
                    "db_all" => Type::generic("List", vec![ts[0].clone()]),
                    "db_query" => Type::generic("Option", vec![ts[0].clone()]),
                    _ => ts[0].clone(),
                };
                Ok(future(result(ret)))
            }
            "json_decode" => {
                if !is_string(&types[0])
                    && types[0].0 != "bytes"
                    && types[0] != Type::generic("view", vec![Type::named("bytes")])
                {
                    return Err(error(line, "JSON入力はstr/bytes/viewです"));
                }
                Ok(result(ts[0].clone()))
            }
            "json_encode" => {
                if !json_encode_supported(&args[0], &self.classes) {
                    return Err(error(line, format!("json_encodeに{}は渡せません。JSONの書き出しに対応する型を指定してください（Serializeが必要です）", types[0])));
                }
                Ok(result(Type::named("str")))
            }
            "ok" => {
                let exp = expected
                    .filter(|t| t.0 == "Result")
                    .cloned()
                    .unwrap_or_else(|| result(types[0].clone()));
                self.demand(&types[0], &exp.inner(), line)?;
                self.consume(&args[0])?;
                Ok(exp)
            }
            "some" => {
                self.consume(&args[0])?;
                Ok(Type::generic("Option", vec![types[0].clone()]))
            }
            "error" | "not_found" | "internal_error" | "fail" => {
                require(0, Type::named(if n == "fail" { "Error" } else { "str" }))?;
                let ret = expected
                    .filter(|t| t.0 == "Result")
                    .cloned()
                    .unwrap_or_else(|| result(Type::named("unit")));
                self.demand(&ret.1[1], &Type::named("Error"), line)?;
                self.consume(&args[0])?;
                Ok(ret)
            }
            "error_kind" | "error_message" => {
                require(0, Type::named("Error"))?;
                Ok(Type::named("str"))
            }
            "assert_true" => {
                require(0, Type::named("bool"))?;
                Ok(Type::named("unit"))
            }
            "parse_i64" | "parse_f64" | "uuid_parse" => {
                if !is_string(&types[0]) {
                    return Err(error(line, "parseは文字列を取ります"));
                }
                Ok(result(Type::named(match n {
                    "parse_i64" => "i64",
                    "parse_f64" => "f64",
                    _ => "UUID",
                })))
            }
            "uuid_format" => {
                require(0, Type::named("UUID"))?;
                Ok(Type::named("str"))
            }
            "make_ints" => {
                require(0, Type::named("i64"))?;
                Ok(Type::generic("List", vec![Type::named("i64")]))
            }
            "i64" => {
                if !["i8", "i16", "i32", "u8", "u16", "u32"].contains(&types[0].0.as_str()) {
                    return Err(error(line, "i64()は損失のない整数拡張です"));
                }
                Ok(Type::named("i64"))
            }
            "i32" => {
                require(0, Type::named("i64"))?;
                Ok(result(Type::named("i32")))
            }
            "actor_demo" | "actor_pair_demo" | "queue_demo" | "task_demo" | "cpu_sum" => {
                require(0, Type::named("i64"))?;
                Ok(future(result(Type::named("i64"))))
            }
            "supervisor_demo" => Ok(future(result(Type::named("i64")))),
            "clock_ns" | "size_of" => Ok(Type::named("i64")),
            "slice" => {
                if !types[0].is_view() {
                    return Err(error(line, "sliceはviewを取ります"));
                }
                require(1, Type::named("i64"))?;
                require(2, Type::named("i64"))?;
                Ok(result(types[0].clone()))
            }
            "append" => {
                if types[0].0 != "List" {
                    return Err(error(line, "appendはListを取ります"));
                }
                require(1, types[0].inner())?;
                if let E::Name(n) = &args[0].kind {
                    if self.borrowed(n) {
                        return Err(error(
                            line,
                            "viewまたはループから参照中のListを変更できません",
                        ));
                    }
                } else {
                    return Err(error(line, "append対象は変数名です"));
                }
                self.consume(&args[1])?;
                Ok(Type::named("unit"))
            }
            "bench_i64" | "bench_f64" | "bench_scalar" => {
                require(0, Type::named("str"))?;
                require(1, Type::named("i64"))?;
                let elem = Type::named(if n == "bench_f64" { "f64" } else { "i64" });
                let arg = if n == "bench_scalar" {
                    elem.clone()
                } else {
                    Type::generic("view", vec![elem.clone()])
                };
                require(2, Type::generic("fn", vec![arg, elem]))?;
                Ok(Type::named("unit"))
            }
            _ => unreachable!(),
        }
    }
}

pub fn integrate(p: &mut Program, native: Program) -> Result<(), String> {
    integrate_mode(p, native, false)
}

fn integrate_mode(p: &mut Program, mut native: Program, editor: bool) -> Result<(), String> {
    for c in native.classes.drain(..) {
        if p.classes.iter().any(|x| x.name == c.name) {
            return Err(error(c.line, "nativeとgeneratedでclassが重複しています"));
        }
        p.classes.push(c);
    }
    let mut replaced = HashSet::new();
    for mut f in native.functions.drain(..) {
        if let Some((_, target)) = f.attrs.iter().find(|(a, _)| a == "replace") {
            let n = target
                .strip_prefix("generated::")
                .ok_or_else(|| error(f.line, "@replace generated::name が必要です"))?
                .to_string();
            if !replaced.insert(n.clone()) {
                return Err(error(f.line, "同じ関数を複数回replaceできません"));
            }
            let pos = p
                .functions
                .iter()
                .position(|x| x.name == n)
                .ok_or_else(|| error(f.line, "replace対象が存在しません"))?;
            let old = &p.functions[pos];
            if old.params.iter().map(|x| &x.1).collect::<Vec<_>>()
                != f.params.iter().map(|x| &x.1).collect::<Vec<_>>()
                || old.ret != f.ret
                || old.asynchronous != f.asynchronous
            {
                return Err(error(
                    f.line,
                    "replaceの引数・戻り値・async指定が一致しません",
                ));
            }
            f.name = n;
            f.attrs = old.attrs.clone();
            p.functions[pos] = f;
        } else {
            p.functions.push(f);
        }
    }
    check_mode(p, editor)
}
