pub(crate) mod checked;

use crate::ast::{block_returns as returns, *};
use std::collections::{HashMap, HashSet};
#[derive(Clone, PartialEq, Eq, Hash)]
struct BorrowedPlace {
    binding: BindingId,
    fields: Vec<String>,
    owner_loan: bool,
    static_origin: bool,
}
impl BorrowedPlace {
    fn overlaps(&self, other: &Self) -> bool {
        !self.static_origin
            && !other.static_origin
            && self.owner_loan
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
    borrowed_element: bool,
}
struct Checker {
    classes: HashMap<String, Class>,
    enums: HashMap<String, Enum>,
    functions: HashMap<String, Function>,
    registered: HashSet<String>,
    vars: HashMap<String, Var>,
    ret: Type,
    asynchronous: bool,
    scope: usize,
    editor: bool,
    collect_view_flow: bool,
    view_flow_names: HashSet<String>,
    view_content_mutations: Vec<FlowContentMutation>,
    view_expression_uses: HashMap<ExprUseId, FlowExprUse>,
    parameter_views: HashSet<BindingId>,
    iterators: Vec<HashSet<BorrowedPlace>>,
    expression_loans: Vec<HashSet<BorrowedPlace>>,
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

fn reset_flow(statements: &mut [Stmt]) {
    for statement in statements {
        statement.flow = None;
        match &mut statement.kind {
            S::If(_, then_body, else_body) => {
                reset_flow(then_body);
                reset_flow(else_body);
            }
            S::Match(_, arms) => {
                for arm in arms {
                    reset_flow(&mut arm.body);
                }
            }
            S::While(_, body) | S::For(_, _, body) | S::Scope(body) => reset_flow(body),
            S::Assign { .. } | S::Return(_) | S::Expr(_) | S::Spawn(_) => {}
        }
    }
}

fn should_collect_view_flow(editor: bool, _asynchronous: bool, ret: &Type, body: &[Stmt]) -> bool {
    !editor && ret.contains_view() && supports_view_flow(body)
}

// Collect the names that can feed a return expression. This is only a
// conservative collection filter; checked BindingIds drive the actual plan.
// Conservative expression groups expose alias and content-mutation inputs.
// Each group is expanded once. Actual candidates are later narrowed with the
// checker's typed value dependencies and return observers, using BindingIds.
fn view_return_dependency_names(body: &[Stmt]) -> HashSet<String> {
    fn statements(
        body: &[Stmt],
        observers: &mut HashSet<String>,
        groups: &mut Vec<HashSet<String>>,
    ) {
        for statement in body {
            // This precheck graph is deliberately conservative. Expression
            // co-occurrence also includes potential content-mutation inputs.
            // The postcheck BindingId graph narrows actual escaping candidates.
            let mut names = statement_view_names(statement);
            match &statement.kind {
                S::Return(Some(expr)) => expression_names(expr, observers),
                S::If(_, a, b) => {
                    statements(a, observers, groups);
                    statements(b, observers, groups);
                }
                S::Match(_, arms) => {
                    for arm in arms {
                        for binding in arm.pattern.bindings() {
                            if let Some(name) = &binding.name {
                                names.insert(name.clone());
                            }
                        }
                        statements(&arm.body, observers, groups);
                    }
                }
                S::While(_, body) | S::For(_, _, body) | S::Scope(body) => {
                    statements(body, observers, groups)
                }
                S::Assign { .. } | S::Return(None) | S::Expr(_) | S::Spawn(_) => {}
            }
            if !names.is_empty() {
                groups.push(names);
            }
        }
    }
    let mut names = HashSet::new();
    let mut groups = Vec::new();
    statements(body, &mut names, &mut groups);
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    for (group, members) in groups.iter().enumerate() {
        for name in members {
            index.entry(name.clone()).or_default().push(group);
        }
    }
    let mut pending: Vec<_> = names.iter().cloned().collect();
    while let Some(name) = pending.pop() {
        for group in index.remove(&name).unwrap_or_default() {
            // A hyperedge is expanded once, avoiding a quadratic name clique.
            for input in std::mem::take(&mut groups[group]) {
                if names.insert(input.clone()) {
                    pending.push(input);
                }
            }
        }
    }
    names
}

fn statement_view_names(statement: &Stmt) -> HashSet<String> {
    let mut names = HashSet::new();
    match &statement.kind {
        S::Assign { name, value, .. } => {
            names.insert(name.clone());
            expression_names(value, &mut names);
        }
        S::Return(Some(expr))
        | S::Expr(expr)
        | S::Spawn(expr)
        | S::If(expr, _, _)
        | S::Match(expr, _)
        | S::While(expr, _)
        | S::For(_, expr, _) => expression_names(expr, &mut names),
        S::Return(None) | S::Scope(_) => {}
    }
    names
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

fn sequence_element(t: &Type) -> Option<Type> {
    match t.0.as_str() {
        "List" => Some(t.inner()),
        "view" => match t.inner().0.as_str() {
            // Match rust_type's exact view special cases. For example,
            // view[owned[str]] is a slice of Strings, rather than &str.
            "str" => None,
            "bytes" => Some(Type::named("u8")),
            _ => Some(t.inner()),
        },
        _ => None,
    }
}

fn json_type_supported(
    t: &Type,
    decoding: bool,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
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
    crate::capabilities::serde_type(t, classes, enums)
        && t.1
            .iter()
            .all(|inner| json_type_supported(inner, decoding, classes, enums))
}

fn json_encode_supported(
    expr: &Expr,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
) -> bool {
    if json_type_supported(expr.ty.as_ref().unwrap(), false, classes, enums) {
        return true;
    }
    // A record constructor names the local struct directly. Container
    // constructors infer that struct too, while a typed parameter/return
    // uses rust_type and may instead name an unsupported runtime type.
    match &expr.kind {
        E::Record(name, _) => classes.get(name).is_some_and(|class| {
            class
                .fields
                .iter()
                .all(|(_, ty)| crate::capabilities::serde_type(ty, classes, enums))
        }),
        E::List(values) => values
            .iter()
            .all(|value| json_encode_supported(value, classes, enums)),
        E::Call(name, _, args)
            if expr.resolution == Some(NameResolution::Builtin)
                && matches!(
                    name.as_str(),
                    "some" | "share" | "clone_shared" | "view" | "copy"
                ) =>
        {
            json_encode_supported(&args[0], classes, enums)
        }
        E::Index(values, _) => json_encode_supported(values, classes, enums),
        _ => false,
    }
}

fn class_field(t: &Type, line: usize) -> Result<(), String> {
    // Resource and callback storage needs a separate ownership design. Runtime
    // Error causes are owned values; their records omit serialization derives.
    if t.0 == "fn" && !t.1.is_empty() || matches!(t.0.as_str(), "Html") {
        return Err(error(line, format!("{t}はclassのフィールドに保存できません。関数の引数やローカル変数で使用してください")));
    }
    if t.0 == "Map" && definitely_unhashable(&t.1[0], None) {
        return Err(error(line, format!("classのMapフィールドのキーに{}は使えません。一致比較とハッシュに対応する型が必要です", t.1[0])));
    }
    if let Some(resource) = crate::stdlib::resource(&t.0) {
        let info = crate::stdlib::resource_info(resource);
        if !info.storage {
            return Err(error(line, format!("{t}は所有fieldへ保存できません")));
        }
        // Native signatures and phantom markers are not stored values. Follow
        // the same physical payload contract as layout/shared validation;
        // each resource still decides whether its own value can be stored.
        for index in info
            .inline_type_arguments
            .iter()
            .chain(crate::stdlib::shared_type_arguments(resource))
        {
            if let Some(inner) = t.1.get(*index) {
                class_field(inner, line)?;
            }
        }
        return Ok(());
    }
    for inner in &t.1 {
        class_field(inner, line)?;
    }
    Ok(())
}

fn recursive_layout(
    t: &Type,
    classes: &HashMap<String, Class>,
    enums: &HashMap<String, Enum>,
    visiting: &mut HashSet<String>,
    checked: &mut HashSet<String>,
) -> bool {
    if t.1.is_empty() && (classes.contains_key(&t.0) || enums.contains_key(&t.0)) {
        if checked.contains(&t.0) {
            return false;
        }
        if !visiting.insert(t.0.clone()) {
            return true;
        }
        let fields: Vec<_> = if let Some(class) = classes.get(&t.0) {
            class.fields.iter().map(|(_, ty)| ty).collect()
        } else {
            enums[&t.0]
                .variants
                .iter()
                .flat_map(|variant| variant.fields.iter().map(|(_, ty)| ty))
                .collect()
        };
        let cyclic = fields
            .into_iter()
            .any(|field| recursive_layout(field, classes, enums, visiting, checked));
        visiting.remove(&t.0);
        if !cyclic {
            checked.insert(t.0.clone());
        }
        return cyclic;
    }
    // These wrappers contain their values inline. Vec, Arc, HashMap and
    // function pointers have fixed layouts independent of their contents.
    if let Some(resource) = crate::stdlib::resource(&t.0) {
        return crate::stdlib::resource_info(resource)
            .inline_type_arguments
            .iter()
            .filter_map(|index| t.1.get(*index))
            .any(|inner| recursive_layout(inner, classes, enums, visiting, checked));
    }
    matches!(t.0.as_str(), "Option" | "Result" | "owned")
        && t.1
            .iter()
            .any(|inner| recursive_layout(inner, classes, enums, visiting, checked))
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
    let mut native = native.clone();
    crate::modules::rebind_native(&mut p, &mut native).ok()?;
    p.classes.extend(native.classes.clone());
    p.enums.extend(native.enums.clone());
    p.functions.extend(
        native
            .functions
            .iter()
            .filter(|f| !f.attrs.iter().any(|(a, _)| a == "replace"))
            .cloned(),
    );
    let mut visible_native = native.clone();
    visible_native
        .functions
        .retain(|f| !f.attrs.iter().any(|(a, _)| a == "replace"));
    crate::modules::synchronize(&mut visible_native);
    p.modules.merge_native(visible_native.modules).ok()?;
    crate::modules::synchronize(&mut p);
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
    for function in &mut p.functions {
        reset_flow(&mut function.body);
    }
    if !p.imports.is_empty() || !p.module_imports.is_empty() {
        return Err("importはnagicのファイル読み込み経路で解決してください".into());
    }
    crate::modules::decode_low_types(p)?;
    if p.modules
        .modules
        .iter()
        .any(|module| crate::stdlib::is_registered_module(&module.id))
    {
        crate::modules::validate(p)?;
    }
    let mut c = Checker {
        classes: HashMap::new(),
        enums: HashMap::new(),
        functions: HashMap::new(),
        registered: p
            .modules
            .definitions
            .iter()
            .filter(|definition| crate::stdlib::definition(&definition.id))
            .map(|definition| definition.symbol.clone())
            .collect(),
        vars: HashMap::new(),
        ret: Type::named("unit"),
        asynchronous: false,
        scope: 0,
        editor,
        collect_view_flow: false,
        view_flow_names: HashSet::new(),
        view_content_mutations: Vec::new(),
        view_expression_uses: HashMap::new(),
        parameter_views: HashSet::new(),
        iterators: vec![],
        expression_loans: vec![],
    };
    let mut symbols = HashSet::new();
    for class in &p.classes {
        if !symbols.insert(class.name.clone()) {
            return Err(error(class.line, "型の重複定義"));
        }
        c.classes.insert(class.name.clone(), class.clone());
    }
    for enumeration in &p.enums {
        if p.modules.root.is_none()
            && matches!(
                enumeration.name.as_str(),
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
                    | "str"
                    | "bytes"
                    | "unit"
                    | "Error"
                    | "Db"
                    | "Html"
                    | "UUID"
                    | "timestamp"
                    | "List"
                    | "view"
                    | "owned"
                    | "shared"
                    | "Option"
                    | "Map"
                    | "Result"
                    | "Future"
                    | "fn"
                    | "Range"
            )
        {
            return Err(error(enumeration.line, format!("enum {} は組み込み型と同名のため、ファイルのmodule名前解決が必要です。source::loadまたはnagicのファイル読み込み経路を使用してください", enumeration.name)));
        }
        if !symbols.insert(enumeration.name.clone()) {
            return Err(error(enumeration.line, "型の重複定義"));
        }
        c.enums
            .insert(enumeration.name.clone(), enumeration.clone());
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
            &c.enums,
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
    for enumeration in &p.enums {
        if enumeration.variants.is_empty() {
            return Err(error(
                enumeration.line,
                "enumには1つ以上のvariantが必要です",
            ));
        }
        let mut variants = HashSet::new();
        for variant in &enumeration.variants {
            if !variants.insert(&variant.name) {
                return Err(error(variant.line, "enumのvariantが重複しています"));
            }
            let mut fields = HashSet::new();
            for (index, (name, ty)) in variant.fields.iter().enumerate() {
                let line = variant
                    .field_lines
                    .get(index)
                    .copied()
                    .unwrap_or(variant.line);
                c.valid(ty, line)?;
                c.emittable(ty, line, false)?;
                if !fields.insert(name) {
                    return Err(error(line, "enumのpayloadフィールドが重複しています"));
                }
                if ty.contains_view() {
                    return Err(error(
                        line,
                        "enumのpayloadにviewは保存できません。copyして所有値を渡してください",
                    ));
                }
                class_field(ty, line)?;
            }
        }
        if recursive_layout(
            &Type::named(&enumeration.name),
            &c.classes,
            &c.enums,
            &mut HashSet::new(),
            &mut checked_layouts,
        ) {
            return Err(error(
                enumeration.line,
                "再帰する値型レイアウト。List等の間接格納が必要です",
            ));
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
        c.expression_loans.clear();
        c.ret = f.ret.clone();
        c.asynchronous = f.asynchronous;
        c.scope = 0;
        c.collect_view_flow = should_collect_view_flow(editor, f.asynchronous, &f.ret, &f.body);
        c.view_flow_names.clear();
        if c.collect_view_flow {
            c.view_flow_names = view_return_dependency_names(&f.body);
            c.collect_view_flow = !c.view_flow_names.is_empty();
        }
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
                    static_origin: false,
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
                        borrowed_element: false,
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
        for function in &p.functions {
            if !function.external {
                crate::constant_eval::validate(function)?;
            }
        }
        crate::routes::validate(p)?;
    }
    Ok(())
}
impl Checker {
    fn record_view_use(&mut self, expression: &Expr, mode: ExprUseMode) {
        if !self.collect_view_flow {
            return;
        }
        let E::Name(name) = &expression.kind else {
            return;
        };
        if !self.view_flow_names.contains(name) {
            return;
        }
        let Some(var) = self
            .vars
            .get(name)
            .filter(|var| var.ty.contains_view() && !var.ty.is_view() && !var.borrowed_element)
        else {
            return;
        };
        let id = ExprUseId::of(expression);
        self.view_expression_uses.insert(
            id,
            FlowExprUse {
                expression: id,
                binding: var.binding,
                mode,
            },
        );
    }
    fn view_value_bindings(&self, value: &Expr) -> Vec<BindingId> {
        if !self.collect_view_flow {
            return Vec::new();
        }
        let mut names = HashSet::new();
        view_value_names(value, &mut names);
        let mut inputs: Vec<_> = names
            .iter()
            .filter_map(|name| self.vars.get(name))
            .filter(|var| var.ty.contains_view())
            .map(|var| var.binding)
            .collect();
        inputs.sort();
        inputs.dedup();
        inputs
    }

    fn extend_content_origins(
        &mut self,
        name: &str,
        origin: HashSet<BorrowedPlace>,
        contents: Vec<HashSet<BorrowedPlace>>,
        inputs: Vec<BindingId>,
    ) {
        let target = self.vars.get_mut(name).expect("checked list binding");
        if self.collect_view_flow
            && self.view_flow_names.contains(name)
            && target.ty.is_view_containing_list()
        {
            // Compare every content depth: a nested append can add a loan
            // below an unchanged outer origin. Keep the event even when a
            // later expression replaces the binding's content snapshots.
            let mut added_origins = Vec::new();
            for (depth, origins) in std::iter::once(&origin).chain(contents.iter()).enumerate() {
                for origin in origins {
                    if !target
                        .content_origins
                        .get(depth)
                        .is_some_and(|previous| previous.contains(origin))
                    {
                        added_origins.push(FlowOrigin {
                            binding: origin.binding,
                            fields: origin.fields.clone(),
                            owner_loan: origin.owner_loan,
                            static_origin: origin.static_origin,
                        });
                    }
                }
            }
            added_origins.sort_by(|a, b| {
                (a.binding, &a.fields, a.owner_loan, a.static_origin).cmp(&(
                    b.binding,
                    &b.fields,
                    b.owner_loan,
                    b.static_origin,
                ))
            });
            added_origins.dedup();
            if !added_origins.is_empty() || !inputs.is_empty() {
                self.view_content_mutations.push(FlowContentMutation {
                    binding: target.binding,
                    name: name.to_owned(),
                    added_origins,
                    inputs,
                });
            }
        }
        target.origins.extend(origin.iter().cloned());
        if let Some(element_origins) = target.content_origins.first_mut() {
            element_origins.extend(origin);
        }
        for (target, source) in target.content_origins.iter_mut().skip(1).zip(contents) {
            target.extend(source);
        }
    }

    fn view_list_snapshot(&self, names: &HashSet<String>) -> Vec<ViewListBindingSnapshot> {
        let mut snapshot: Vec<_> = self
            .view_flow_names
            .intersection(names)
            .filter_map(|name| {
                self.vars
                    .get(name)
                    .filter(|var| {
                        var.ty.contains_view() && !var.ty.is_view() && !var.borrowed_element
                    })
                    .map(|var| (name, var))
            })
            .map(|(name, var)| {
                let mut origins: Vec<_> = std::iter::once(&var.origins)
                    .chain(var.content_origins.iter())
                    .flatten()
                    .map(|origin| FlowOrigin {
                        binding: origin.binding,
                        fields: origin.fields.clone(),
                        owner_loan: origin.owner_loan,
                        static_origin: origin.static_origin,
                    })
                    .collect();
                origins.sort_by(|a, b| {
                    (a.binding, &a.fields, a.owner_loan, a.static_origin).cmp(&(
                        b.binding,
                        &b.fields,
                        b.owner_loan,
                        b.static_origin,
                    ))
                });
                origins.dedup();
                ViewListBindingSnapshot {
                    ty: var.ty.clone(),
                    name: name.clone(),
                    binding: var.binding,
                    moved: var.moved,
                    origins,
                }
            })
            .collect();
        snapshot.sort_by_key(|binding| binding.binding);
        snapshot
    }

    fn resource(&self, name: &str) -> Option<crate::stdlib::Resource> {
        self.registered
            .contains(name)
            .then(|| crate::stdlib::resource(name))
            .flatten()
    }
    fn native_resource_view(&self, ty: &Type) -> Option<crate::stdlib::Resource> {
        crate::stdlib::native_view_element(ty).and_then(|element| self.resource(&element.0))
    }
    fn field_owner<'a>(&self, mut ty: &'a Type) -> &'a Type {
        while ty.0 == "owned" || ty.0 == "shared" || self.native_resource_view(ty).is_some() {
            ty = &ty.1[0];
        }
        ty
    }
    fn reference(&self, got: &Type, wanted: &Type, line: usize) -> Result<(), String> {
        if got == wanted || wanted.0 == "view" && got == &wanted.inner() {
            Ok(())
        } else {
            self.demand(got, wanted, line)
        }
    }
    fn mapper(&self, ty: &Type, error_type: &Type, line: usize) -> Result<(), String> {
        let response = crate::stdlib::resource_type(crate::stdlib::Resource::Response, vec![]);
        let expected = Type::generic("fn", vec![error_type.clone(), response]);
        self.demand(ty, &expected, line)?;
        if error_type.contains_view() {
            return Err(error(line, "HTTP mapperのエラー値にviewは保持できません"));
        }
        Ok(())
    }
    fn handler_error(&self, handler: &Expr, state: &Type, line: usize) -> Result<Type, String> {
        let named = match (&handler.kind, handler.resolution) {
            (E::Name(name), Some(NameResolution::Function)) => {
                self.functions.get(name).is_some_and(|f| f.asynchronous)
            }
            (E::Name(name), Some(NameResolution::Local)) => self
                .vars
                .get(name)
                .is_some_and(|v| v.async_function.is_some()),
            _ => false,
        };
        if !named {
            return Err(error(
                line,
                "HTTP handlerは名前付きasync関数またはそのローカルaliasを指定してください",
            ));
        }
        let ty = handler.ty.as_ref().expect("checked callback");
        if ty.0 != "fn" || ty.1.len() != 3 || !ty.1[2].is_future() {
            return Err(error(
                line,
                "HTTP handlerはasync (Request, shared[State])->Result[Response,E]が必要です",
            ));
        }
        self.demand(
            &ty.1[0],
            &crate::stdlib::resource_type(crate::stdlib::Resource::Request, vec![]),
            line,
        )?;
        self.demand(
            &ty.1[1],
            &Type::generic("shared", vec![state.clone()]),
            line,
        )?;
        let output = ty.1[2].inner();
        if output.0 != "Result" || output.1.len() != 2 {
            return Err(error(
                line,
                "HTTP handlerはResult[Response,E]を返してください",
            ));
        }
        self.demand(
            &output.1[0],
            &crate::stdlib::resource_type(crate::stdlib::Resource::Response, vec![]),
            line,
        )?;
        if ty.1[0].contains_view() || ty.1[1].contains_view() || output.contains_view() {
            return Err(error(
                line,
                "HTTP handlerの引数・戻り値にviewを保持できません",
            ));
        }
        Ok(output.1[1].clone())
    }
    fn standard(
        &mut self,
        operation: crate::stdlib::Operation,
        types: &[Type],
        args: &mut [Expr],
        line: usize,
    ) -> Result<Type, String> {
        use crate::stdlib::{Operation as O, Passing, Resource as R};
        let info = crate::stdlib::operation_info(operation);
        if info.module == crate::stdlib::StandardModule::Actor {
            return self.actor_standard(operation, types, args, line);
        }
        if info.module == crate::stdlib::StandardModule::Result {
            return self.result_standard(types, args, line);
        }
        if args.len() != info.arity {
            return Err(error(
                line,
                format!("{}の引数は{}個です", info.name, info.arity),
            ));
        }
        if !types.is_empty() && types.len() != info.generic_arity
            || info.generic_arity == 0 && !types.is_empty()
        {
            return Err(error(
                line,
                format!("{}の型引数は{}個です", info.name, info.generic_arity),
            ));
        }
        for ty in types {
            self.valid(ty, line)?;
            self.emittable(ty, line, false)?;
        }
        let resource = |kind| crate::stdlib::resource_type(kind, vec![]);
        let view = |ty| Type::generic("view", vec![ty]);
        let mut hints: Vec<Option<Type>> = vec![None; args.len()];
        match operation {
            O::Status => hints[0] = Some(Type::named("i64")),
            O::Method => hints[0] = Some(view(Type::named("str"))),
            O::MethodName => hints[0] = Some(view(resource(R::Method))),
            O::Empty => hints[0] = Some(resource(R::Status)),
            O::Text | O::Html | O::Bytes => {
                hints[0] = Some(resource(R::Status));
                hints[1] = Some(view(Type::named(if operation == O::Bytes {
                    "bytes"
                } else {
                    "str"
                })));
            }
            O::Json => {
                hints[0] = Some(resource(R::Status));
                hints[1] = types.first().cloned();
            }
            O::AppendHeader | O::AppendHeaderText => {
                hints[0] = Some(resource(R::Response));
                hints[1] = Some(view(Type::named("str")));
                hints[2] = Some(view(Type::named(if operation == O::AppendHeaderText {
                    "str"
                } else {
                    "bytes"
                })));
            }
            O::Header | O::HeaderText | O::Headers => {
                hints[0] = Some(view(resource(R::Request)));
                hints[1] = Some(view(Type::named("str")));
            }
            O::IsJsonContentType => hints[0] = Some(view(resource(R::Request))),
            O::DefaultOptions => {}
            O::Options => {
                hints.fill(Some(Type::named("i64")));
            }
            O::Capacity | O::HeaderTimeout | O::HeaderLimits | O::SendTimeout => {
                hints.fill(Some(Type::named("i64")));
                hints[0] = Some(resource(R::Options));
            }
            O::App => {
                if types.len() == 2 {
                    hints[0] = Some(types[0].clone());
                    hints[1] = Some(Type::generic(
                        "fn",
                        vec![types[1].clone(), resource(R::Response)],
                    ));
                }
            }
            O::AppDefault => hints[0] = types.first().cloned(),
            O::Route | O::RouteMapped => {
                hints[1] = Some(resource(R::Method));
                hints[2] = Some(view(Type::named("str")));
            }
            O::Serve => {
                hints[1] = Some(Type::named("i64"));
                hints[2] = Some(resource(R::Options));
            }
            _ => unreachable!("actor operation is checked separately"),
        }
        let mut arguments = vec![];
        for (index, arg) in args.iter_mut().enumerate() {
            let ty = self.expr(arg, hints[index].as_ref())?;
            if let Some(wanted) = &hints[index] {
                if info.parameters[index] == Passing::Reference {
                    self.reference(&ty, wanted, arg.line)?;
                } else {
                    self.demand(&ty, wanted, arg.line)?;
                }
            }
            if info.parameters[index] == Passing::Reference {
                self.available(arg, false)?;
            }
            if info.parameters[index] == Passing::Move {
                self.consume(arg)?;
            }
            self.hold_value(
                arg,
                matches!(info.parameters[index], Passing::Reference | Passing::Borrow),
            );
            arguments.push(ty);
        }
        let output = match operation {
            O::Status => result(resource(R::Status)),
            O::Method => result(resource(R::Method)),
            O::MethodName => view(Type::named("str")),
            O::Empty | O::Text | O::Html | O::Bytes => resource(R::Response),
            O::Json => {
                if !json_encode_supported(&args[1], &self.classes, &self.enums) {
                    return Err(error(
                        line,
                        "HTTP JSON builderの値はSerializeに対応していません",
                    ));
                }
                result(resource(R::Response))
            }
            O::AppendHeader | O::AppendHeaderText => result(resource(R::Response)),
            O::Header | O::HeaderText => result(Type::generic(
                "Option",
                vec![view(Type::named(if operation == O::HeaderText {
                    "str"
                } else {
                    "bytes"
                }))],
            )),
            O::Headers => result(Type::generic("List", vec![view(Type::named("bytes"))])),
            O::IsJsonContentType => result(Type::named("bool")),
            O::DefaultOptions => resource(R::Options),
            O::Options | O::Capacity | O::HeaderTimeout | O::HeaderLimits | O::SendTimeout => {
                result(resource(R::Options))
            }
            O::App | O::AppDefault => {
                let state = &arguments[0];
                if state.contains_view() {
                    return Err(error(line, "Appのstateにはviewを保存できません"));
                }
                let failure = if operation == O::AppDefault {
                    Type::named("Error")
                } else {
                    let mapper = &arguments[1];
                    if mapper.0 != "fn" || mapper.1.len() != 2 {
                        return Err(error(line, "Appのmapperは同期fn[E,Response]が必要です"));
                    }
                    let failure = mapper.1[0].clone();
                    self.mapper(mapper, &failure, line)?;
                    failure
                };
                if let Some(expected) = types.get(1) {
                    self.demand(&failure, expected, line)?;
                }
                crate::stdlib::resource_type(R::App, vec![state.clone(), failure])
            }
            O::Route | O::RouteMapped | O::Serve => {
                let app = &arguments[0];
                if self.resource(&app.0) != Some(R::App) || app.1.len() != 2 {
                    return Err(error(line, "HTTP登録・起動にはApp[State,E]が必要です"));
                }
                if operation == O::Serve {
                    future(result(Type::named("unit")))
                } else {
                    let failure = self.handler_error(&args[3], &app.1[0], line)?;
                    if operation == O::Route {
                        self.demand(&failure, &app.1[1], line)?;
                    } else {
                        self.mapper(&arguments[4], &failure, line)?;
                    }
                    result(app.clone())
                }
            }
            _ => unreachable!("actor operation is checked separately"),
        };
        if self.resource(&output.0).is_some() {
            self.valid(&output, line)?;
        }
        Ok(output)
    }

    fn result_standard(
        &mut self,
        types: &[Type],
        args: &mut [Expr],
        line: usize,
    ) -> Result<Type, String> {
        if args.len() != 2 {
            return Err(error(line, "map_errorの引数はResultと同期mapperの2個です"));
        }
        if !types.is_empty() {
            return Err(error(
                line,
                "map_errorの型はResultとmapperから推論します。型引数は指定できません",
            ));
        }
        let value = self.expr(&mut args[0], None)?;
        if value.0 != "Result" || value.1.len() != 2 {
            return Err(error(args[0].line, "map_errorの第1引数はResult[T,E]です"));
        }
        self.consume(&args[0])?;
        self.hold_value(&args[0], false);
        let mapper = self.expr(&mut args[1], None)?;
        if !matches!(args[1].kind, E::Name(_))
            || !matches!(
                args[1].resolution,
                Some(NameResolution::Function | NameResolution::Local)
            )
            || mapper.0 != "fn"
            || mapper.1.len() != 2
            || mapper.1[1].is_future()
        {
            return Err(error(
                args[1].line,
                "map_errorのmapperは名前付き同期fn[E,F]またはそのローカルaliasが必要です",
            ));
        }
        self.demand(&mapper.1[0], &value.1[1], args[1].line)?;
        self.emittable(&mapper, args[1].line, false)?;
        if mapper.1[1].contains_view() {
            return Err(error(
                args[1].line,
                "map_errorの変換後のエラーにviewは保持できません",
            ));
        }
        self.consume(&args[1])?;
        let output = Type::generic("Result", vec![value.1[0].clone(), mapper.1[1].clone()]);
        self.valid(&output, line)?;
        Ok(output)
    }

    fn actor_resource_argument(
        &self,
        got: &Type,
        wanted: crate::stdlib::Resource,
        line: usize,
    ) -> Result<Type, String> {
        let inner = if got.is_view() {
            if self.native_resource_view(got) != Some(wanted) {
                return Err(error(
                    line,
                    format!(
                        "{}のresource参照が必要です",
                        crate::stdlib::resource_info(wanted).name
                    ),
                ));
            }
            got.inner()
        } else {
            got.clone()
        };
        let resource = unowned(&inner);
        if self.resource(&resource.0) != Some(wanted) {
            return Err(error(
                line,
                format!(
                    "{}が必要です: got {got}",
                    crate::stdlib::resource_info(wanted).name
                ),
            ));
        }
        self.valid(resource, line)?;
        Ok(resource.clone())
    }
    fn named_async_signature(
        &self,
        callback: &Expr,
        line: usize,
        role: &str,
    ) -> Result<Vec<Type>, String> {
        let named = match (&callback.kind, callback.resolution) {
            (E::Name(name), Some(NameResolution::Function)) => self
                .functions
                .get(name)
                .is_some_and(|function| function.asynchronous),
            (E::Name(name), Some(NameResolution::Local)) => self
                .vars
                .get(name)
                .is_some_and(|var| var.async_function.is_some()),
            _ => false,
        };
        if !named {
            return Err(error(
                line,
                format!("{role}は名前付きasync関数またはそのローカルaliasが必要です"),
            ));
        }
        let signature = callback.ty.as_ref().expect("checked callback");
        if signature.0 != "fn" || signature.1.is_empty() || !signature.1.last().unwrap().is_future()
        {
            return Err(error(line, format!("{role}のasync署名が不正です")));
        }
        Ok(signature.1.clone())
    }
    fn actor_outer_result(
        &self,
        signature: &[Type],
        line: usize,
        role: &str,
    ) -> Result<Type, String> {
        let output = signature.last().expect("async callback return").inner();
        if output.0 != "Result" || output.1.len() != 2 {
            return Err(error(
                line,
                format!("{role}はResult[T,Error]を返してください"),
            ));
        }
        self.demand(&output.1[1], &Type::named("Error"), line)?;
        Ok(output.1[0].clone())
    }
    fn actor_payload(&self, ty: &Type, line: usize, role: &str) -> Result<(), String> {
        crate::capabilities::charge_type_supported(ty, &self.classes, &self.enums).map_err(
            |reason| {
                error(
                    line,
                    format!("{role}はChargeOwnedに対応していません: {reason}"),
                )
            },
        )
    }
    fn actor_standard(
        &mut self,
        operation: crate::stdlib::Operation,
        types: &[Type],
        args: &mut [Expr],
        line: usize,
    ) -> Result<Type, String> {
        use crate::stdlib::{Operation as O, Passing, Resource as R};
        let info = crate::stdlib::operation_info(operation);
        if args.len() != info.arity {
            return Err(error(
                line,
                format!("{}の引数は{}個です", info.name, info.arity),
            ));
        }
        if !types.is_empty() && types.len() != info.generic_arity
            || info.generic_arity == 0 && !types.is_empty()
        {
            return Err(error(
                line,
                format!("{}の型引数は{}個です", info.name, info.generic_arity),
            ));
        }
        for ty in types {
            self.valid(ty, line)?;
            self.emittable(ty, line, false)?;
        }
        let resource = |kind| crate::stdlib::resource_type(kind, vec![]);
        let view = |ty| Type::generic("view", vec![ty]);
        let mut arguments = Vec::new();
        for (index, arg) in args.iter_mut().enumerate() {
            let hint = match operation {
                O::ActorOptions => Some(Type::named("i64")),
                O::ActorActorOptions => Some(if index == 4 {
                    resource(R::RestartPolicy)
                } else {
                    Type::named("i64")
                }),
                O::ActorRestartDelay => Some(if index == 0 {
                    resource(R::SupervisorOptions)
                } else {
                    Type::named("i64")
                }),
                O::ActorSupervisor => {
                    if index == 0 {
                        types.first().cloned()
                    } else {
                        Some(resource(R::SupervisorOptions))
                    }
                }
                O::ActorRegister => match index {
                    1 => Some(view(Type::named("str"))),
                    4 => Some(resource(R::ActorOptions)),
                    _ => None,
                },
                O::ActorTask | O::ActorTaskWithReady => match index {
                    1 => Some(view(Type::named("str"))),
                    3 => Some(resource(R::RestartPolicy)),
                    _ => None,
                },
                O::ActorTurn => {
                    if types.len() == 3 {
                        Some(if index == 0 {
                            types[0].clone()
                        } else {
                            Type::generic("Result", vec![types[1].clone(), types[2].clone()])
                        })
                    } else {
                        None
                    }
                }
                O::ActorReady | O::ActorNextEventTimeout => {
                    (index == 1).then(|| Type::named("i64"))
                }
                O::ActorCall => match index {
                    1 => Some(
                        self.actor_resource_argument(&arguments[0], R::Actor, line)?
                            .1[0]
                            .clone(),
                    ),
                    2 | 3 => Some(Type::named("i64")),
                    _ => None,
                },
                _ => None,
            };
            let got = self.expr(arg, hint.as_ref())?;
            if let Some(wanted) = &hint {
                if info.parameters[index] == Passing::Reference {
                    self.reference(&got, wanted, arg.line)?;
                } else {
                    self.demand(&got, wanted, arg.line)?;
                }
            }
            if info.parameters[index] == Passing::Reference {
                self.available(arg, false)?;
            }
            if info.parameters[index] == Passing::Move {
                self.consume(arg)?;
            }
            self.hold_value(
                arg,
                matches!(info.parameters[index], Passing::Reference | Passing::Borrow),
            );
            arguments.push(got);
        }
        let output = match operation {
            O::ActorDefaultOptions => resource(R::SupervisorOptions),
            O::ActorOptions | O::ActorRestartDelay => result(resource(R::SupervisorOptions)),
            O::ActorDefaultActorOptions => resource(R::ActorOptions),
            O::ActorActorOptions => result(resource(R::ActorOptions)),
            O::ActorSupervisor => {
                let context = arguments[0].clone();
                if context.contains_view() {
                    return Err(error(line, "Supervisorのcontextにはviewを保存できません"));
                }
                crate::stdlib::resource_type(R::Supervisor, vec![context])
            }
            O::ActorControl => {
                self.actor_resource_argument(&arguments[0], R::Supervisor, line)?;
                resource(R::Control)
            }
            O::ActorCloneControl
            | O::ActorShutdown
            | O::ActorNextEvent
            | O::ActorNextEventTimeout => {
                self.actor_resource_argument(&arguments[0], R::Control, line)?;
                match operation {
                    O::ActorCloneControl => resource(R::Control),
                    O::ActorShutdown => future(result(Type::named("unit"))),
                    O::ActorNextEventTimeout => future(Type::generic(
                        "Result",
                        vec![
                            Type::generic("Option", vec![resource(R::Event)]),
                            resource(R::WaitError),
                        ],
                    )),
                    _ => future(result(Type::generic("Option", vec![resource(R::Event)]))),
                }
            }
            O::ActorMarkReady => {
                self.actor_resource_argument(&arguments[0], R::TaskReady, line)?;
                result(Type::named("unit"))
            }
            O::ActorRegister | O::ActorTask | O::ActorTaskWithReady => {
                let group = self.actor_resource_argument(&arguments[0], R::Supervisor, line)?;
                let factory = self.named_async_signature(&args[2], line, "actor factory")?;
                let with_ready = operation == O::ActorTaskWithReady;
                if factory.len() != if with_ready { 3 } else { 2 } {
                    return Err(error(
                        line,
                        if with_ready {
                            "task_with_readyのfactory引数はshared[Context]とTaskReadyです"
                        } else {
                            "actor factoryの引数はshared[Context]1個です"
                        },
                    ));
                }
                if with_ready {
                    self.demand(&factory[1], &resource(R::TaskReady), line)?;
                }
                self.demand(
                    &factory[0],
                    &Type::generic("shared", vec![group.1[0].clone()]),
                    line,
                )?;
                let state = self.actor_outer_result(&factory, line, "actor factory")?;
                if matches!(operation, O::ActorTask | O::ActorTaskWithReady) {
                    self.demand(&state, &Type::named("unit"), line)?;
                    result(Type::named("unit"))
                } else {
                    let handler = self.named_async_signature(&args[3], line, "actor handler")?;
                    if handler.len() != 3 {
                        return Err(error(line, "actor handlerの引数はStateとMessageの2個です"));
                    }
                    self.demand(&handler[0], &state, line)?;
                    let turn = self.actor_outer_result(&handler, line, "actor handler")?;
                    if self.resource(&turn.0) != Some(R::Turn) || turn.1.len() != 3 {
                        return Err(error(
                            line,
                            "actor handlerはResult[Turn[State,Reply,E],Error]を返してください",
                        ));
                    }
                    self.demand(&turn.1[0], &state, line)?;
                    let message = handler[1].clone();
                    let reply = turn.1[1].clone();
                    let failure = turn.1[2].clone();
                    for (role, ty) in [
                        ("actor Message", &message),
                        ("actor Reply", &reply),
                        ("actor E", &failure),
                    ] {
                        self.actor_payload(ty, line, role)?;
                    }
                    if state.contains_view() {
                        return Err(error(line, "actor Stateにはviewを保持できません"));
                    }
                    if !types.is_empty() {
                        for (got, wanted) in
                            [&state, &message, &reply, &failure].into_iter().zip(types)
                        {
                            self.demand(got, wanted, line)?;
                        }
                    }
                    result(crate::stdlib::resource_type(
                        R::Actor,
                        vec![message, reply, failure],
                    ))
                }
            }
            O::ActorTurn => {
                let reply = &arguments[1];
                if reply.0 != "Result" || reply.1.len() != 2 {
                    return Err(error(line, "turnのreplyにはResult[R,E]が必要です"));
                }
                if arguments[0].contains_view() {
                    return Err(error(line, "TurnのStateにはviewを保持できません"));
                }
                self.actor_payload(&reply.1[0], line, "Turn Reply")?;
                self.actor_payload(&reply.1[1], line, "Turn E")?;
                crate::stdlib::resource_type(
                    R::Turn,
                    vec![arguments[0].clone(), reply.1[0].clone(), reply.1[1].clone()],
                )
            }
            O::ActorCloneActor | O::ActorReady | O::ActorCall => {
                let actor = self.actor_resource_argument(&arguments[0], R::Actor, line)?;
                if !types.is_empty() {
                    for (got, wanted) in actor.1.iter().zip(types) {
                        self.demand(got, wanted, line)?;
                    }
                }
                match operation {
                    O::ActorCloneActor => actor,
                    O::ActorReady => future(Type::generic(
                        "Result",
                        vec![Type::named("unit"), resource(R::CallError)],
                    )),
                    _ => {
                        self.demand(&arguments[1], &actor.1[0], line)?;
                        future(Type::generic(
                            "Result",
                            vec![
                                Type::generic(
                                    "Result",
                                    vec![actor.1[1].clone(), actor.1[2].clone()],
                                ),
                                resource(R::CallError),
                            ],
                        ))
                    }
                }
            }
            O::ActorRun => {
                if arguments[0].is_view() {
                    return Err(error(line, "runはSupervisorの所有値をmoveで取ります"));
                }
                let group = self.actor_resource_argument(&arguments[0], R::Supervisor, line)?;
                if let Some(wanted) = types.first() {
                    self.demand(&group.1[0], wanted, line)?;
                }
                future(result(Type::named("unit")))
            }
            O::ActorYieldNow => future(Type::named("unit")),
            _ => unreachable!("HTTP operation is checked separately"),
        };
        if self.resource(&output.0).is_some() {
            self.valid(&output, line)?;
        }
        Ok(output)
    }
    fn enum_variant(
        &self,
        name: &str,
        line: usize,
    ) -> Result<Option<(String, EnumVariant)>, String> {
        let Some((owner, variant)) = name.rsplit_once('.') else {
            return Ok(None);
        };
        let Some(enumeration) = self.enums.get(owner) else {
            return Ok(None);
        };
        let variant = enumeration
            .variants
            .iter()
            .find(|item| item.name == variant)
            .cloned()
            .ok_or_else(|| {
                error(
                    line,
                    format!("{}にvariant {variant} はありません", Type::named(owner)),
                )
            })?;
        Ok(Some((owner.to_owned(), variant)))
    }
    fn enum_call(
        &mut self,
        name: &str,
        types: &[Type],
        args: &mut [Expr],
        line: usize,
    ) -> Result<Option<Type>, String> {
        if name
            .rsplit_once('.')
            .is_some_and(|(owner, _)| self.vars.contains_key(owner))
        {
            return Ok(None);
        }
        let Some((owner, variant)) = self.enum_variant(name, line)? else {
            return Ok(None);
        };
        if !types.is_empty() {
            return Err(error(line, "enumのvariantには型引数を指定できません"));
        }
        if args.len() != variant.fields.len() {
            return Err(error(line, "enumのvariantのpayload数が一致しません"));
        }
        for (arg, (_, expected)) in args.iter_mut().zip(&variant.fields) {
            let got = self.expr(arg, Some(expected))?;
            self.demand(&got, expected, arg.line)?;
            self.consume(arg)?;
        }
        Ok(Some(Type::named(&owner)))
    }
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
        fn visit(
            t: &Type,
            classes: &HashMap<String, Class>,
            enums: &HashMap<String, Enum>,
            registered: &HashSet<String>,
            depth: usize,
        ) -> bool {
            if depth > 64 {
                return false;
            }
            // These raw heads emit their intrinsic owned representation even
            // when a direct-AST caller also declares a same-spelled record.
            if matches!(t.0.as_str(), "str" | "bytes" | "Error" | "Db" | "Html") {
                return false;
            }
            if registered.contains(&t.0) {
                if let Some(resource) = crate::stdlib::resource(&t.0) {
                    return crate::stdlib::resource_info(resource).copy;
                }
            }
            if t.1.is_empty() && enums.contains_key(&t.0) {
                enums[&t.0].variants.iter().all(|variant| {
                    variant
                        .fields
                        .iter()
                        .all(|(_, ty)| visit(ty, classes, enums, registered, depth + 1))
                })
            } else if t.is_copy() {
                true
            } else if matches!(t.0.as_str(), "Option" | "owned") {
                visit(&t.inner(), classes, enums, registered, depth + 1)
            } else if let Some(c) = classes.get(&t.0) {
                c.fields
                    .iter()
                    .all(|(_, t)| visit(t, classes, enums, registered, depth + 1))
            } else {
                false
            }
        }
        visit(t, &self.classes, &self.enums, &self.registered, 0)
    }
    fn valid(&self, t: &Type, line: usize) -> Result<(), String> {
        if let Some(alias) = t.0.strip_prefix(crate::modules::NAMESPACE_PREFIX) {
            return Err(error(
                line,
                format!("module名 {alias} は型として使えません。module内のclassを指定してください"),
            ));
        }
        if let Some(resource) = self.resource(&t.0) {
            let arity = crate::stdlib::resource_info(resource).arity;
            if t.1.len() != arity {
                return Err(error(line, format!("{t}の型引数は{arity}個です")));
            }
            if resource == crate::stdlib::Resource::App && t.1.iter().any(Type::contains_view) {
                return Err(error(line, "Appのstate/error型引数にviewを保持できません"));
            }
            if matches!(
                resource,
                crate::stdlib::Resource::Supervisor
                    | crate::stdlib::Resource::Actor
                    | crate::stdlib::Resource::Turn
            ) && t.1.iter().any(Type::contains_view)
            {
                return Err(error(
                    line,
                    format!(
                        "{}の型引数にviewを保持できません",
                        crate::stdlib::resource_info(resource).name
                    ),
                ));
            }
            if resource == crate::stdlib::Resource::Grant {
                let marker = &t.1[0];
                if !marker.1.is_empty()
                    || !(self.classes.contains_key(&marker.0) || self.enums.contains_key(&marker.0))
                {
                    return Err(error(
                        line,
                        "Grantのpermission markerには型引数のないclass/enumを指定してください",
                    ));
                }
            }
            for index in crate::stdlib::shared_type_arguments(resource) {
                if crate::capabilities::contains_auth_proof(
                    &t.1[*index],
                    &self.classes,
                    &self.enums,
                ) {
                    return Err(error(
                        line,
                        "auth proofを内部のshared state/contextへ格納できません",
                    ));
                }
            }
            for argument in &t.1 {
                self.valid(argument, line)?;
            }
            if resource == crate::stdlib::Resource::Actor {
                for (role, argument) in ["Actor Message", "Actor Reply", "Actor E"]
                    .into_iter()
                    .zip(&t.1)
                {
                    self.actor_payload(argument, line, role)?;
                }
            } else if resource == crate::stdlib::Resource::Turn {
                self.actor_payload(&t.1[1], line, "Turn Reply")?;
                self.actor_payload(&t.1[2], line, "Turn E")?;
            }
            return Ok(());
        }
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
            if t.0 == "shared"
                && crate::capabilities::contains_auth_proof(t, &self.classes, &self.enums)
            {
                return Err(error(
                    line,
                    "auth proofをsharedへ格納できません（nested wrapperを含みます）",
                ));
            }
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
            || self.enums.contains_key(&t.0)
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
    fn views_absent(&self, e: &Expr) -> bool {
        if !e.ty.as_ref().is_some_and(Type::contains_view) {
            return true;
        }
        match &e.kind {
            E::Null => true,
            E::List(values) => values.iter().all(|value| self.views_absent(value)),
            E::Call(name, _, args) if e.resolution == Some(NameResolution::Builtin) => {
                match name.as_str() {
                    "ok" | "some" | "fail" | "error" | "not_found" | "internal_error" | "share"
                    | "clone_shared" => self.views_absent(&args[0]),
                    "copy" => {
                        // Copying a slice drops its container loan. Prove its
                        // contents empty only for a view of a proven value.
                        matches!(&args[0].kind, E::Call(name, _, values)
                            if name == "view"
                                && args[0].resolution == Some(NameResolution::Builtin)
                                && self.views_absent(&values[0]))
                    }
                    _ => false,
                }
            }
            E::Try(value) => self.views_absent(value),
            E::Call(name, _, args)
                if e.resolution == Some(NameResolution::Standard)
                    && crate::stdlib::operation(name)
                        == Some(crate::stdlib::Operation::ResultMapError) =>
            {
                self.views_absent(&args[0])
            }
            // Do not infer absence from view-containing locals: Rust unifies
            // a binding's lifetime type across aliases and all assignments,
            // including terminating branches. Calls are likewise opaque.
            _ => false,
        }
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
                    } else if v.ty.contains_view() || v.borrowed_element {
                        v.origins.clone()
                    } else {
                        HashSet::from([BorrowedPlace {
                            binding: v.binding,
                            fields: vec![],
                            owner_loan: true,
                            static_origin: false,
                        }])
                    }
                })
                .unwrap_or_default(),
            E::Field(_, _) if e.resolution == Some(NameResolution::ResourceConstant) => {
                // Registered Method/Status constants are native promotable
                // values; borrowing them does not borrow a local owner.
                HashSet::from([BorrowedPlace {
                    binding: BindingId { line: 0, token: 0 },
                    fields: vec![],
                    owner_loan: false,
                    static_origin: true,
                }])
            }
            E::Field(parent, field) if e.resolution == Some(NameResolution::ResourceField) => {
                let Some(ty) = parent.ty.as_ref() else {
                    return HashSet::new();
                };
                let Some(resource) = self.resource(&self.field_owner(ty).0) else {
                    return HashSet::new();
                };
                let Some(info) = crate::stdlib::field(resource, field) else {
                    return HashSet::new();
                };
                if info.static_borrow {
                    HashSet::from([BorrowedPlace {
                        binding: BindingId { line: 0, token: 0 },
                        fields: vec![],
                        owner_loan: false,
                        static_origin: true,
                    }])
                } else if info.whole_owner {
                    self.origin(parent)
                } else {
                    self.origin(parent)
                        .into_iter()
                        .map(|mut place| {
                            place.fields.push(field.clone());
                            place
                        })
                        .collect()
                }
            }
            E::Call(name, _, args) if e.resolution == Some(NameResolution::Standard) => {
                if crate::stdlib::operation(name) == Some(crate::stdlib::Operation::ResultMapError)
                {
                    // Mapping the failure leaves the success payload and all
                    // nested view origins unchanged, without borrowing Result.
                    return self.origin_at(&args[0], depth);
                }
                crate::stdlib::operation(name)
                    .and_then(|op| crate::stdlib::operation_info(op).borrow_owner)
                    .map(|owner| self.origin(&args[owner]))
                    .unwrap_or_default()
            }
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
                        // A field of a borrowed resource lives with the caller's
                        // resource, rather than the local reference binding.
                        if self.native_resource_view(&var.ty).is_none() {
                            origins.insert(BorrowedPlace {
                                binding: var.binding,
                                fields,
                                owner_loan: true,
                                static_origin: false,
                            });
                        }
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
            E::Call(name, _, args) if e.ty.as_ref().is_some_and(Type::contains_view) => {
                let static_return = match e.resolution {
                    Some(NameResolution::Local) => self
                        .vars
                        .get(name)
                        .is_some_and(|var| var.ty.function_view_return_is_static()),
                    Some(NameResolution::Function) => self.functions.get(name).is_some_and(|f| {
                        f.ret.contains_view() && !f.params.iter().any(|(_, ty)| ty.contains_view())
                    }),
                    _ => false,
                };
                if static_return {
                    HashSet::from([BorrowedPlace {
                        binding: BindingId { line: 0, token: 0 },
                        fields: vec![],
                        owner_loan: false,
                        static_origin: true,
                    }])
                } else {
                    args.iter()
                        .filter(|arg| arg.ty.as_ref().is_some_and(Type::contains_view))
                        .flat_map(|arg| self.origin(arg))
                        .collect()
                }
            }
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
        Self::borrows_temporary_in(e, depth, false)
    }
    fn borrows_temporary_in(e: &Expr, depth: usize, literal_views_static: bool) -> bool {
        if !e.ty.as_ref().is_some_and(Type::contains_view) {
            return false;
        }
        let visit =
            |value: &Expr, depth| Self::borrows_temporary_in(value, depth, literal_views_static);
        match &e.kind {
            E::Call(n, _, args) if e.resolution == Some(NameResolution::Builtin) => {
                match n.as_str() {
                    "view" => {
                        if depth == 0 {
                            Self::place(&args[0]).is_none()
                                && !(literal_views_static && matches!(args[0].kind, E::Str(_)))
                        } else {
                            visit(&args[0], depth)
                        }
                    }
                    "json_decode" => {
                        let input = &args[0];
                        if input.ty.as_ref().is_some_and(Type::is_view) {
                            visit(input, 0)
                        } else {
                            // string_arg emits literals directly, rather than a temporary String.
                            !matches!(input.kind, E::Str(_)) && Self::place(input).is_none()
                        }
                    }
                    "copy" => visit(&args[0], depth.max(1)),
                    "slice" | "ok" | "some" | "share" | "clone_shared" => visit(&args[0], depth),
                    _ => args.iter().any(|arg| visit(arg, 0)),
                }
            }
            E::Call(name, _, args) if e.resolution == Some(NameResolution::Standard) => {
                if crate::stdlib::operation(name) == Some(crate::stdlib::Operation::ResultMapError)
                {
                    return visit(&args[0], depth);
                }
                crate::stdlib::operation(name)
                    .and_then(|op| crate::stdlib::operation_info(op).borrow_owner)
                    .is_some_and(|owner| {
                        let arg = &args[owner];
                        if arg.ty.as_ref().is_some_and(Type::is_view) {
                            visit(arg, 0)
                        } else {
                            Self::place(arg).is_none()
                        }
                    })
            }
            E::Call(_, _, args) => args.iter().any(|arg| visit(arg, 0)),
            E::List(args) => args.iter().any(|arg| visit(arg, depth.saturating_sub(1))),
            E::Field(parent, field) if e.resolution == Some(NameResolution::ResourceField) => {
                let resource = parent
                    .ty
                    .as_ref()
                    .map(|ty| {
                        let mut ty = ty;
                        while matches!(ty.0.as_str(), "owned" | "shared" | "view")
                            && !ty.1.is_empty()
                        {
                            ty = &ty.1[0];
                        }
                        ty
                    })
                    .and_then(|ty| crate::stdlib::resource(&ty.0));
                resource
                    .and_then(|resource| crate::stdlib::field(resource, field))
                    .is_some_and(|info| {
                        !info.static_borrow
                            && (visit(parent, 0)
                                || !parent.ty.as_ref().is_some_and(Type::is_view)
                                    && Self::place(parent).is_none())
                    })
            }
            E::Try(e) | E::Await(e) | E::Field(e, _) => visit(e, depth),
            E::Index(e, _) => visit(e, depth + 1),
            _ => false,
        }
    }
    fn borrowed_place(&self, place: &BorrowedPlace) -> bool {
        self.borrowed_place_at(place, self.expression_loans.len())
    }
    fn borrowed_place_at(&self, place: &BorrowedPlace, expression_count: usize) -> bool {
        self.vars.values().any(|v| {
            v.binding != place.binding
                && !v.moved
                && (v.ty.contains_view() || v.borrowed_element)
                && v.origins.iter().any(|loan| loan.overlaps(place))
        }) || self
            .iterators
            .iter()
            .flatten()
            .any(|loan| loan.overlaps(place))
            || self.expression_loans[..expression_count]
                .iter()
                .flatten()
                .any(|loan| loan.overlaps(place))
    }
    fn hold_value(&mut self, e: &Expr, implicit_borrow: bool) {
        if !implicit_borrow && !e.ty.as_ref().is_some_and(Type::contains_view) {
            return;
        }
        let mut origins = self.origin(e);
        if implicit_borrow && !e.ty.as_ref().is_some_and(Type::is_view) {
            if let Some((name, fields)) = Self::place(e) {
                if let Some(var) = self.vars.get(name) {
                    origins.insert(BorrowedPlace {
                        binding: var.binding,
                        fields,
                        owner_loan: true,
                        static_origin: false,
                    });
                }
            }
        }
        self.expression_loans
            .last_mut()
            .expect("borrowed value belongs to an expression")
            .extend(origins);
    }
    fn comparison_origins(&self, e: &Expr) -> HashSet<BorrowedPlace> {
        match &e.kind {
            E::Name(name) => self
                .vars
                .get(name)
                .map(|var| {
                    HashSet::from([BorrowedPlace {
                        binding: var.binding,
                        fields: vec![],
                        owner_loan: true,
                        static_origin: false,
                    }])
                })
                .unwrap_or_default(),
            E::Field(parent, field) => {
                if e.resolution == Some(NameResolution::ResourceField) {
                    let resource = parent
                        .ty
                        .as_ref()
                        .and_then(|ty| self.resource(&self.field_owner(ty).0));
                    if resource
                        .and_then(|resource| crate::stdlib::field(resource, field))
                        .is_some_and(|info| !info.owned)
                    {
                        // Native getters return a value or reference rather
                        // than exposing a place in the resource.
                        return HashSet::new();
                    }
                }
                self.comparison_origins(parent)
                    .into_iter()
                    .map(|mut place| {
                        place.fields.push(field.clone());
                        place
                    })
                    .collect()
            }
            E::Index(parent, _) => {
                let mut origins = self.comparison_origins(parent);
                // A trait comparison borrows the indexed container even when
                // its Copy element contains no views. A slice-producing call
                // can also retain a loan on the original container.
                origins.extend(self.origin(parent));
                origins
            }
            _ => HashSet::new(),
        }
    }
    fn hold_comparison(&mut self, e: &Expr, ty: &Type) {
        self.hold_value(e, false);
        let ty = unowned(ty);
        let scalar = matches!(
            ty.0.as_str(),
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" | "bool"
        ) && !self.classes.contains_key(&ty.0);
        if scalar || ty.0 == "fn" && !ty.1.is_empty() {
            return;
        }
        // Rust materializes scalar operands, but PartialEq/PartialOrd borrow
        // aggregate places even when their values implement Copy.
        let origins = self.comparison_origins(e);
        self.expression_loans
            .last_mut()
            .expect("comparison belongs to an expression")
            .extend(origins);
    }
    fn borrowed(&self, n: &str) -> bool {
        self.vars.get(n).is_some_and(|v| {
            self.borrowed_place(&BorrowedPlace {
                binding: v.binding,
                fields: vec![],
                owner_loan: true,
                static_origin: false,
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
        fn borrowed_base(checker: &Checker, expr: &Expr) -> bool {
            if let E::Field(parent, _) = &expr.kind {
                let borrowed = parent.ty.as_ref().is_some_and(|ty| {
                    let ty = unowned(ty);
                    ty.0 == "shared" || checker.native_resource_view(ty).is_some()
                });
                borrowed || borrowed_base(checker, parent)
            } else {
                false
            }
        }
        // Field access can borrow through an owned wrapper or a temporary
        // returned by a call. Neither creates permission to move out of Arc
        // or a native resource reference.
        if borrowed_base(self, e) && !e.ty.as_ref().is_some_and(|ty| self.copy_type(ty)) {
            return Err(error(
                e.line,
                "sharedまたは借用resourceの非Copy fieldはmoveできません。viewで借用してください",
            ));
        }
        let Some((name, fields)) = Self::place(e) else {
            return Ok(());
        };
        let Some(v) = self.vars.get(name) else {
            return Ok(());
        };
        if self.copy_type(e.ty.as_ref().unwrap_or(&v.ty)) {
            self.record_view_use(e, ExprUseMode::Copy);
            return Ok(());
        }
        if v.borrowed_element {
            return Err(error(e.line, format!("{name} はList要素の読み取り専用借用です。所有値として渡す文字列・配列はcopy(view(...))で明示的に複製してください")));
        }
        if self.borrowed_place(&BorrowedPlace {
            binding: v.binding,
            fields: fields.clone(),
            owner_loan: true,
            static_origin: false,
        }) {
            return Err(error(
                e.line,
                if self.expression_loans.iter().flatten().any(|loan| {
                    loan.overlaps(&BorrowedPlace {
                        binding: v.binding,
                        fields: fields.clone(),
                        owner_loan: true,
                        static_origin: false,
                    })
                }) {
                    format!("{name} は同じ式で先に参照されています。copyを使うか、別の所有値を渡してください")
                } else {
                    format!("{name} はviewまたはループから参照されています。借用を終了するかcopyしてください")
                },
            ));
        }
        let v = self.vars.get_mut(name).unwrap();
        if fields.is_empty() {
            v.moved = true;
        } else {
            v.moved_fields.insert(fields);
        }
        self.record_view_use(e, ExprUseMode::Move);
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
    fn scope_origins(
        before: &HashMap<String, Var>,
        after: &HashMap<String, Var>,
        line: usize,
    ) -> Result<(), String> {
        // An existing alias can keep an outer binding alive even when a loop
        // counter shadows its name. Only newly introduced origins may escape.
        let owners: HashSet<_> = before
            .values()
            .flat_map(|var| {
                std::iter::once(var.binding).chain(
                    var.origins
                        .iter()
                        .chain(var.content_origins.iter().flatten())
                        .map(|origin| origin.binding),
                )
            })
            .collect();
        for (name, var) in after {
            if !before.contains_key(name) || var.moved {
                continue;
            }
            if var
                .origins
                .iter()
                .chain(var.content_origins.iter().flatten())
                .any(|origin| !origin.static_origin && !owners.contains(&origin.binding))
            {
                return Err(error(
                    line,
                    format!("{name} のviewは内側のscopeの所有値を参照しています。scopeの外へ保存する値にはcopyを使用してください"),
                ));
            }
        }
        Ok(())
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
        line: usize,
        flow: &mut Option<FlowLoopEntry>,
    ) -> Result<(), String> {
        // Recheck the parsed body, not its annotated first pass: locals must be
        // declared anew on each iteration. Only outer moves cross the backedge.
        let template = body.to_vec();
        let condition_template = condition.as_deref().cloned();
        let mut header = self.vars.clone();
        let mut first = true;
        let mutation_start = self.view_content_mutations.len();
        loop {
            self.vars = header.clone();
            self.view_content_mutations.truncate(mutation_start);
            let header_facts = self
                .collect_view_flow
                .then(|| self.view_list_snapshot(&self.view_flow_names));
            let editor = self.editor;
            if !first {
                self.editor = false;
            }
            let checked = (|| {
                if let Some(c) = condition.as_deref_mut() {
                    if !first {
                        *c = condition_template.clone().unwrap();
                    }
                    let t = self.expr(c, Some(&Type::named("bool")))?;
                    self.demand(&t, &Type::named("bool"), c.line)?;
                }
                // A while condition is also evaluated on its exit path. A for
                // iterator was evaluated once before entering this helper.
                let exit = self.vars.clone();
                if let Some((name, binding)) = &binding {
                    self.vars.insert((*name).to_owned(), binding.clone());
                }
                *flow = header_facts.clone().map(|header| FlowLoopEntry {
                    header,
                    body: self.view_list_snapshot(&self.view_flow_names),
                });
                if first {
                    self.block(body)?;
                } else {
                    let mut probe = template.clone();
                    self.block(&mut probe)?;
                    body.clone_from_slice(&probe);
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
            Self::scope_origins(&header, &after, line)?;
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
        let relevant = self.collect_view_flow.then(|| statement_view_names(s));
        let before = relevant.as_ref().map(|names| {
            self.view_list_snapshot(
                if matches!(s.kind, S::While(..) | S::For(..) | S::Scope(..)) {
                    &self.view_flow_names
                } else {
                    names
                },
            )
        });
        // A child statement has its own expression effects. Preserve the
        // parent's condition effects while checking either branch.
        let parent_mutations = std::mem::take(&mut self.view_content_mutations);
        let parent_uses = std::mem::take(&mut self.view_expression_uses);
        let mut assignment = None;
        let mut branch_entry = None;
        let mut loop_entry = None;
        let checked = self.statement_inner(s, &mut assignment, &mut branch_entry, &mut loop_entry);
        let content_mutations =
            std::mem::replace(&mut self.view_content_mutations, parent_mutations);
        let mut expression_uses: Vec<_> =
            std::mem::replace(&mut self.view_expression_uses, parent_uses)
                .into_values()
                .collect();
        expression_uses.sort_by_key(|use_| use_.expression);
        if let (Ok(()), Some(before)) = (&checked, before) {
            let mut value_dependencies = Vec::new();
            let mut return_observers = Vec::new();
            match &s.kind {
                S::Assign { value, .. } => value_dependencies.push(FlowValueDependency {
                    target: assignment.unwrap().target,
                    inputs: self.view_value_bindings(value),
                }),
                S::Match(value, arms) => {
                    let inputs = self.view_value_bindings(value);
                    for arm in arms {
                        for binding in arm.pattern.bindings() {
                            if binding.ty.as_ref().is_some_and(Type::contains_view) {
                                value_dependencies.push(FlowValueDependency {
                                    target: BindingId {
                                        line: arm.line,
                                        token: binding.span.start,
                                    },
                                    inputs: inputs.clone(),
                                });
                            }
                        }
                    }
                }
                S::Return(Some(value)) => return_observers = self.view_value_bindings(value),
                _ => {}
            }
            s.flow = Some(StmtFlowFacts {
                before,
                after: self.view_list_snapshot(
                    if matches!(
                        s.kind,
                        S::If(..) | S::Match(..) | S::While(..) | S::For(..) | S::Scope(..)
                    ) {
                        &self.view_flow_names
                    } else {
                        relevant.as_ref().unwrap()
                    },
                ),
                branch_entry,
                loop_entry,
                content_mutations,
                assignment,
                value_dependencies,
                return_observers,
                expression_uses,
            });
        }
        checked
    }
    fn statement_inner(
        &mut self,
        s: &mut Stmt,
        assignment: &mut Option<FlowAssignment>,
        branch_entry: &mut Option<Vec<ViewListBindingSnapshot>>,
        loop_entry: &mut Option<FlowLoopEntry>,
    ) -> Result<(), String> {
        match &mut s.kind {
            S::Assign {
                name,
                annotation,
                value,
                declare,
            } => {
                let old = self.vars.get(name).cloned();
                if old.as_ref().is_some_and(|v| v.borrowed_element) {
                    return Err(error(
                        s.line,
                        "List要素の読み取り専用借用には再代入できません",
                    ));
                }
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
                let target_binding = old.as_ref().map(|v| v.binding).unwrap_or(BindingId {
                    line: s.line,
                    token: s.binding_span.unwrap_or_default().start,
                });
                let rhs_consumed_target = old.as_ref().is_some_and(|previous| {
                    !previous.moved
                        && self.vars.get(name).is_some_and(|current| {
                            current.binding == previous.binding && current.moved
                        })
                });
                *assignment = Some(FlowAssignment {
                    target: target_binding,
                    rhs_consumed_target,
                });
                *declare = old.is_none();
                *annotation = Some(ty.clone());
                s.binding_type = Some(ty.clone());
                self.vars.insert(
                    name.clone(),
                    Var {
                        binding: target_binding,
                        ty,
                        moved: false,
                        moved_fields: HashSet::new(),
                        origins: origin,
                        content_origins,
                        async_function,
                        borrowed_element: false,
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
                            || origin.is_empty() && !self.views_absent(e)
                            || origin.iter().any(|place| {
                                !place.static_origin
                                    && (place.owner_loan
                                        || !self.parameter_views.contains(&place.binding))
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
                if unowned(&t).0 == "Result" {
                    return Err(error(
                        s.line,
                        if t.0 == "owned" {
                            "Resultを無視できません。ownedで包んだ値は変数へ受けてください"
                        } else {
                            "Resultを無視できません。tryで伝播するか変数へ受けてください"
                        },
                    ));
                }
                self.consume(e)?;
            }
            S::If(c, a, b) => {
                let t = self.expr(c, Some(&Type::named("bool")))?;
                self.demand(&t, &Type::named("bool"), s.line)?;
                *branch_entry = self.collect_view_flow.then(|| {
                    let mut names = HashSet::new();
                    expression_names(c, &mut names);
                    self.view_list_snapshot(&names)
                });
                let am = self.child(a)?;
                let bm = self.child(b)?;
                let mut paths = vec![];
                if !returns(a) {
                    Self::scope_origins(&self.vars, &am, s.line)?;
                    paths.push(am);
                }
                if !returns(b) {
                    Self::scope_origins(&self.vars, &bm, s.line)?;
                    paths.push(bm);
                }
                self.join_moves(&paths);
            }
            S::While(c, b) => {
                self.loop_body(b, Some(c), None, s.line, loop_entry)?;
            }
            S::Match(value, arms) => {
                let ty = self.expr(value, None)?;
                let enumeration = self.enums.get(&ty.0).cloned();
                let is_result = ty.0 == "Result" && ty.1.len() == 2;
                let is_option = ty.0 == "Option" && ty.1.len() == 1;
                if !is_result && !is_option && enumeration.is_none() {
                    return Err(error(s.line, "matchの対象はResult、Optionまたはenumです"));
                }
                let mut seen = HashSet::new();
                for arm in arms.iter_mut() {
                    let (key, payloads) = match &arm.pattern {
                        MatchPattern::Result { ok, .. } if is_result => (
                            if *ok { "Ok" } else { "Err" }.to_owned(),
                            vec![ty.1[usize::from(!*ok)].clone()],
                        ),
                        MatchPattern::Option { binding } if is_option => (
                            if binding.is_some() { "Some" } else { "None" }.to_owned(),
                            if binding.is_some() {
                                vec![ty.inner()]
                            } else {
                                vec![]
                            },
                        ),
                        MatchPattern::Enum { name, bindings, .. } if enumeration.is_some() => {
                            let Some((owner, variant)) = self.enum_variant(name, arm.line)? else {
                                return Err(error(
                                    arm.line,
                                    format!("未定義のenum variant: {name}"),
                                ));
                            };
                            self.demand(&Type::named(&owner), &ty, arm.line)?;
                            if variant.fields.len() != bindings.len() {
                                return Err(error(
                                    arm.line,
                                    "enum variantのpayload数とcaseの変数数が一致しません",
                                ));
                            }
                            (
                                variant.name,
                                variant.fields.into_iter().map(|(_, ty)| ty).collect(),
                            )
                        }
                        _ => {
                            return Err(error(
                                arm.line,
                                if is_result {
                                    "ResultのcaseにはOk、Errを指定してください"
                                } else if is_option {
                                    "OptionのcaseにはSome、Noneを指定してください"
                                } else {
                                    "matchの対象とcaseの型が一致しません"
                                },
                            ))
                        }
                    };
                    if !seen.insert(key) {
                        return Err(error(arm.line, "matchのcaseが重複しています"));
                    }
                    for (binding, payload) in arm.pattern.bindings_mut().iter_mut().zip(payloads) {
                        binding.ty = Some(payload);
                    }
                }
                if let Some(enumeration) = &enumeration {
                    let missing: Vec<_> = enumeration
                        .variants
                        .iter()
                        .filter(|variant| !seen.contains(&variant.name))
                        .map(|variant| variant.name.clone())
                        .collect();
                    if !missing.is_empty() {
                        return Err(error(
                            s.line,
                            format!(
                                "enumのmatchが網羅されていません。caseが必要です: {}",
                                missing.join(", ")
                            ),
                        ));
                    }
                } else if seen.len() != 2 {
                    return Err(error(
                        s.line,
                        if is_option {
                            "matchにはSomeとNoneの両方のcaseが必要です"
                        } else {
                            "matchにはOkとErrの両方のcaseが必要です"
                        },
                    ));
                }
                let temporary_owner =
                    (ty.contains_view() && Self::borrows_temporary(value)).then(|| BorrowedPlace {
                        binding: BindingId {
                            line: s.line,
                            token: value.span.start,
                        },
                        fields: vec![],
                        owner_loan: true,
                        static_origin: false,
                    });
                let mut origin = self.origin(value);
                let mut content_origins = self.content_origins(value, &ty, 0);
                if let Some(owner) = &temporary_owner {
                    origin.insert(owner.clone());
                    for contents in &mut content_origins {
                        contents.insert(owner.clone());
                    }
                }
                self.consume(value)?;
                *branch_entry = self.collect_view_flow.then(|| {
                    let mut names = HashSet::new();
                    expression_names(value, &mut names);
                    self.view_list_snapshot(&names)
                });
                let before = self.vars.clone();
                let mut moves = vec![];
                for arm in arms {
                    self.vars = before.clone();
                    for binding in arm.pattern.bindings() {
                        let Some(name) = &binding.name else {
                            continue;
                        };
                        if self.vars.contains_key(name) {
                            return Err(error(
                                arm.line,
                                "caseの変数名は外側の変数と重複できません",
                            ));
                        }
                        let payload = binding
                            .ty
                            .as_ref()
                            .expect("validated pattern payload")
                            .clone();
                        self.vars.insert(
                            name.clone(),
                            Var {
                                binding: BindingId {
                                    line: arm.line,
                                    token: binding.span.start,
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
                                borrowed_element: false,
                            },
                        );
                    }
                    self.block(&mut arm.body)?;
                    let mut after = self.vars.clone();
                    for binding in arm.pattern.bindings() {
                        if let Some(name) = &binding.name {
                            after.remove(name);
                        }
                    }
                    if !returns(&arm.body) {
                        if let Some(owner) = &temporary_owner {
                            let escaped = before.keys().any(|name| {
                                after.get(name).is_some_and(|var| {
                                    !var.moved
                                        && (var.origins.contains(owner)
                                            || var
                                                .content_origins
                                                .iter()
                                                .any(|contents| contents.contains(owner)))
                                })
                            });
                            if escaped {
                                return Err(error(arm.line, "一時的な所有値のviewをmatchの外へ持ち出せません。case内でcopyしてください"));
                            }
                        }
                        Self::scope_origins(&before, &after, arm.line)?;
                        moves.push(after);
                    }
                }
                self.vars = before;
                self.join_moves(&moves);
            }
            S::For(n, e, b) => {
                let t = self.expr(e, None)?;
                if self.native_resource_view(&t).is_some() {
                    return Err(error(
                        s.line,
                        "resourceのviewは配列ではないためforで反復できません",
                    ));
                }
                let elem = match t.0.as_str() {
                    "Range" => Type::named("i64"),
                    "List" | "view" => sequence_element(&t).ok_or_else(|| {
                        error(s.line, "view[str]の文字列反復は未対応です。forにはrangeまたは連続配列が必要です")
                    })?,
                    _ => return Err(error(s.line, "forにはrangeまたは連続配列が必要です")),
                };
                let borrowed_element = !self.copy_type(&elem);
                s.binding_type = Some(elem.clone());
                s.binding_borrowed = borrowed_element;
                let origins = self.origin(e);
                let mut element_origins = self.origin_at(e, 1);
                let mut content_origins = self.content_origins(e, &elem, 1);
                if elem.contains_view() {
                    for (depth, origins) in std::iter::once(&mut element_origins)
                        .chain(content_origins.iter_mut())
                        .enumerate()
                    {
                        let temporary = Self::borrows_temporary_in(e, depth + 1, true);
                        let static_literal = origins.is_empty()
                            && !temporary
                            && Self::borrows_temporary_at(e, depth + 1);
                        if !temporary && !static_literal {
                            continue;
                        }
                        // Iterator temporaries live through the body, but not
                        // through aliases retained after the loop. Copied
                        // elements do not retain an outer container's loan.
                        // Literal string views emit static references instead.
                        let owner = BorrowedPlace {
                            binding: if static_literal {
                                BindingId { line: 0, token: 0 }
                            } else {
                                BindingId {
                                    line: s.line,
                                    token: e.span.start,
                                }
                            },
                            fields: vec![],
                            owner_loan: !static_literal,
                            static_origin: static_literal,
                        };
                        origins.insert(owner);
                    }
                }
                let mut loans = origins.clone();
                if t.0 == "List" {
                    if let Some((name, fields)) = Self::place(e) {
                        if let Some(var) = self.vars.get(name) {
                            loans.insert(BorrowedPlace {
                                binding: var.binding,
                                fields,
                                owner_loan: true,
                                static_origin: false,
                            });
                        }
                    }
                }
                if borrowed_element {
                    // A borrowed element always pins its container, even when
                    // an ending body releases the iterator's next-step loan.
                    element_origins.extend(loans.iter().cloned());
                    element_origins.insert(BorrowedPlace {
                        binding: BindingId {
                            line: s.line,
                            token: s.binding_span.unwrap_or_default().start,
                        },
                        fields: vec![],
                        owner_loan: true,
                        static_origin: false,
                    });
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
                            origins: if elem.contains_view() || borrowed_element {
                                element_origins
                            } else {
                                HashSet::new()
                            },
                            ty: elem,
                            moved: false,
                            moved_fields: HashSet::new(),
                            content_origins,
                            async_function: None,
                            borrowed_element,
                        },
                    )),
                    s.line,
                    loop_entry,
                );
                self.iterators.pop();
                checked?;
            }
            S::Scope(b) => {
                if !self.asynchronous || self.ret.0 != "Result" {
                    return Err(error(s.line, "scopeはasync Result関数内で使用してください"));
                }
                let mut failure = &self.ret.1[1];
                while failure.0 == "owned" {
                    failure = &failure.1[0];
                }
                // Scope joins return a runtime Error. Local types may supply
                // From<Error> in their Rust adapter; foreign types cannot.
                if failure.0 != "Error"
                    && (!failure.1.is_empty()
                        || matches!(
                            failure.0.as_str(),
                            "str" | "bytes" | "unit" | "Db" | "Html" | "UUID" | "timestamp"
                        )
                        || !self.classes.contains_key(&failure.0)
                            && !self.enums.contains_key(&failure.0))
                {
                    return Err(error(s.line, format!("scopeの失敗はErrorです。戻り値のエラー型 {failure} へ変換できません。ErrorまたはRust連携でFrom<Error>を実装したclassまたはenumを使用してください")));
                }
                self.scope += 1;
                let m = self.child(b);
                self.scope -= 1;
                let m = m?;
                Self::scope_origins(&self.vars, &m, s.line)?;
                self.merge_moves(m);
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
                if let E::Call(name, _, args) = &e.kind {
                    let passing = (e.resolution == Some(NameResolution::Standard))
                        .then(|| crate::stdlib::operation(name))
                        .flatten()
                        .map(|operation| crate::stdlib::operation_info(operation).parameters);
                    for (index, arg) in args.iter().enumerate() {
                        let native_borrow = passing.is_some_and(|parameters| {
                            matches!(
                                parameters[index],
                                crate::stdlib::Passing::Reference | crate::stdlib::Passing::Borrow
                            )
                        });
                        if !arg.ty.as_ref().is_some_and(Type::contains_view) && !native_borrow {
                            continue;
                        }
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
        let retains_values = matches!(
            e.kind,
            E::Call(_, _, _) | E::List(_) | E::Binary(_, _, _) | E::Index(_, _)
        );
        if retains_values {
            self.expression_loans.push(HashSet::new());
        }
        let checked = self.expr_scoped(e, expected, projection);
        if retains_values {
            self.expression_loans.pop();
        }
        checked
    }
    fn expr_scoped(
        &mut self,
        e: &mut Expr,
        expected: Option<&Type>,
        projection: bool,
    ) -> Result<Type, String> {
        let line = e.line;
        if e.resolution == Some(NameResolution::Module) {
            let alias = match &e.kind {
                E::Name(name) | E::Call(name, _, _) | E::Record(name, _) => name.as_str(),
                _ => "",
            };
            return Err(error(
                line,
                format!(
                    "module名 {alias} は値や関数として使えません。module内の定義を指定してください"
                ),
            ));
        }
        e.resolution = None;
        if let E::Field(base, member) = &e.kind {
            if let E::Name(owner) = &base.kind {
                if !self.vars.contains_key(owner) {
                    if let Some(resource) = self.resource(owner) {
                        if crate::stdlib::constant(resource, member).is_none() {
                            return Err(error(line, format!("{owner}に定数{member}はありません")));
                        }
                        let ty = crate::stdlib::resource_type(resource, vec![]);
                        e.resolution = Some(NameResolution::ResourceConstant);
                        e.ty = Some(ty.clone());
                        return Ok(ty);
                    }
                }
            }
        }
        if let E::Field(base, variant) = &e.kind {
            if let E::Name(owner) = &base.kind {
                if !self.vars.contains_key(owner) && self.enums.contains_key(owner) {
                    let (_, definition) = self
                        .enum_variant(&format!("{owner}.{variant}"), line)?
                        .expect("known enum owner");
                    if !definition.fields.is_empty() {
                        return Err(error(line, "enumのvariantのpayloadを指定してください"));
                    }
                    let ty = Type::named(owner);
                    e.resolution = Some(NameResolution::Enum);
                    e.ty = Some(ty.clone());
                    return Ok(ty);
                }
            }
        }
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
                    e.resolution = Some(if v.borrowed_element {
                        NameResolution::BorrowedLocal
                    } else {
                        NameResolution::Local
                    });
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
                let comparison = matches!(op.as_str(), "==" | "!=" | "<" | ">" | "<=" | ">=");
                if comparison {
                    self.hold_comparison(a, &left);
                }
                let right = self.expr(b, Some(&left))?;
                let borrowed_pair = comparison
                    && ((left.0 == "view"
                        && matches!(left.inner().0.as_str(), "str" | "bytes")
                        && left.inner() == right)
                        || (right.0 == "view"
                            && matches!(right.inner().0.as_str(), "str" | "bytes")
                            && right.inner() == left));
                if !borrowed_pair {
                    self.demand(&right, &left, line)?;
                }
                match op.as_str() {
                    "and" | "or" => {
                        self.demand(&left, &Type::named("bool"), line)?;
                        Type::named("bool")
                    }
                    "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                        let ordered = !matches!(op.as_str(), "==" | "!=");
                        if (!self.copy_type(&left)
                            && left.0 != "str"
                            && !borrowed_pair
                            && !self.resource(&left.0).is_some_and(|resource| {
                                crate::stdlib::resource_info(resource).equality
                            }))
                            || !(comparable(&left, ordered)
                                || !ordered
                                    && self.resource(&left.0).is_some_and(|resource| {
                                        crate::stdlib::resource_info(resource).equality
                                    }))
                        {
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
                        if matches!(op.as_str(), "/" | "%")
                            && matches!(
                                left.0.as_str(),
                                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                            )
                        {
                            crate::constant_eval::validate_literal_divisor(b, op)?;
                        }
                        left
                    }
                }
            }
            E::Field(x, n) => {
                // The parent is only a field base. Check the complete path so
                // moving one field does not prevent access to its siblings.
                let t = self.expr_mode(x, None, true)?;
                let owner = self.field_owner(&t);
                if let Some(resource) = self.resource(&owner.0) {
                    let field = crate::stdlib::field(resource, n)
                        .ok_or_else(|| error(line, format!("{t}にfield {n}はありません")))?;
                    if field.whole_owner {
                        self.available(x, false)?;
                    }
                    e.resolution = Some(NameResolution::ResourceField);
                    field.ty
                } else {
                    self.classes
                        .get(&owner.0)
                        .and_then(|c| c.fields.iter().find(|(k, _)| k == n))
                        .map(|f| f.1.clone())
                        .ok_or_else(|| error(line, format!("{t} にフィールド {n} はありません")))?
                }
            }
            E::Index(x, i) => {
                let t = self.expr(x, None)?;
                if self.native_resource_view(&t).is_some() {
                    return Err(error(
                        line,
                        "resourceのviewは配列ではないためindex取得できません",
                    ));
                }
                self.hold_value(x, true);
                let ix = self.expr(i, Some(&Type::named("i64")))?;
                self.demand(&ix, &Type::named("i64"), line)?;
                if let Some(a) = sequence_element(&t) {
                    // A metadata-free record named Error/Db/Html can be built
                    // directly even though its typed head emits the intrinsic
                    // runtime type. Preserve known literal-record provenance,
                    // without making runtime values or cause fields Copy.
                    let literal_records_copy = self.classes.get(&a.0).is_some_and(|class| {
                        class.fields.iter().all(|(_, ty)| self.copy_type(ty))
                            && matches!(&x.kind, E::List(values) if !values.is_empty()
                                && values.iter().all(|value| matches!(&value.kind, E::Record(name, _) if name == &a.0)))
                    });
                    if !self.copy_type(&a) && !literal_records_copy {
                        return Err(error(line, "非Copy要素のindex取得は0.1では未対応です"));
                    }
                    a
                } else {
                    return Err(error(line, "indexには配列が必要です"));
                }
            }
            E::List(a) => {
                let exp = expected.filter(|t| t.0 == "List").map(Type::inner);
                let mut values = a.iter_mut();
                let elem = if let Some(exp) = exp {
                    exp
                } else if let Some(first) = values.next() {
                    // Inferring from the first element must not check its
                    // moves twice when checking the remaining elements.
                    let elem = self.expr(first, None)?;
                    self.consume(first)?;
                    self.hold_value(first, false);
                    elem
                } else {
                    return Err(error(line, "空配列には型注釈が必要です"));
                };
                for x in values {
                    let t = self.expr(x, Some(&elem))?;
                    self.demand(&t, &elem, line)?;
                    self.consume(x)?;
                    self.hold_value(x, false);
                }
                Type::generic("List", vec![elem])
            }
            E::Record(n, fields) => {
                if matches!(
                    self.resource(n),
                    Some(crate::stdlib::Resource::Principal | crate::stdlib::Resource::Grant)
                ) {
                    return Err(error(line, "auth proof resourceは構築できません。trusted Rust issuerを使用してください"));
                }
                if let Some((owner, variant)) = self.enum_variant(n, line)? {
                    if variant.fields.len() != fields.len() {
                        return Err(error(
                            line,
                            "enumのvariantの全payloadフィールドを指定してください",
                        ));
                    }
                    let mut seen = HashSet::new();
                    for (name, value) in fields {
                        if !seen.insert(name.clone()) {
                            return Err(error(line, "enumのpayloadフィールドが重複しています"));
                        }
                        let expected = variant
                            .fields
                            .iter()
                            .find(|(field, _)| field == name)
                            .map(|(_, ty)| ty)
                            .ok_or_else(|| {
                                error(line, "enumのvariantに不明なpayloadフィールドがあります")
                            })?;
                        let got = self.expr(value, Some(expected))?;
                        self.demand(&got, expected, value.line)?;
                        self.consume(value)?;
                    }
                    e.resolution = Some(NameResolution::Enum);
                    let ty = Type::named(&owner);
                    e.ty = Some(ty.clone());
                    return Ok(ty);
                }
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
                let hint = expected.map(|success| {
                    Type::generic("Result", vec![success.clone(), self.ret.1[1].clone()])
                });
                let t = self.expr(x, hint.as_ref())?;
                if t.0 != "Result" {
                    return Err(error(line, "try対象はResultです"));
                }
                self.demand(&t.1[1], &self.ret.1[1], line)?;
                self.consume(x)?;
                t.inner()
            }
            E::Call(n, ts, args) => {
                if !self.vars.contains_key(n)
                    && !self.functions.contains_key(n)
                    && self.registered.contains(n)
                {
                    if let Some(operation) = crate::stdlib::operation(n) {
                        let ty = self.standard(operation, ts, args, line)?;
                        e.resolution = Some(NameResolution::Standard);
                        e.ty = Some(ty.clone());
                        return Ok(ty);
                    }
                }
                if matches!(
                    self.resource(n),
                    Some(crate::stdlib::Resource::Principal | crate::stdlib::Resource::Grant)
                ) {
                    return Err(error(line, "auth proof resourceは構築できません。trusted Rust issuerを使用してください"));
                }
                for t in ts.iter() {
                    self.valid(t, line)?;
                }
                if let Some(ty) = self.enum_call(n, ts, args, line)? {
                    e.resolution = Some(NameResolution::Enum);
                    e.ty = Some(ty.clone());
                    return Ok(ty);
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
                        self.hold_value(arg, false);
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
        if matches!(e.kind, E::Name(..)) {
            self.record_view_use(e, ExprUseMode::Borrow);
        }
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
        if n == "json_decode" && !json_type_supported(&ts[0], true, &self.classes, &self.enums) {
            return Err(error(line, format!("json_decodeの型引数 {} はJSONの読み取りに対応していません（Deserializeが必要です）", ts[0])));
        }
        let mut types = vec![];
        for (i, a) in args.iter_mut().enumerate() {
            let hint = match n {
                "ok" => expected.filter(|t| t.0 == "Result").map(Type::inner),
                "some" => expected.filter(|t| t.0 == "Option").map(Type::inner),
                "fail" => expected
                    .filter(|t| t.0 == "Result" && t.1.len() == 2)
                    .map(|t| t.1[1].clone()),
                "db_insert" if i == 3 => Some(Type::named("i32")),
                "db_update" if i == 4 => Some(Type::named("i32")),
                _ => None,
            };
            let ty = self.expr(a, hint.as_ref())?;
            // DB SQL is copied into Sql::Owned before later arguments are
            // evaluated; env finishes its key lookup before the fallback.
            // Their input views do not remain borrowed for the enclosing call.
            let materialized = n == "env"
                || i == 1
                    && matches!(
                        n,
                        "db_exec" | "db_all" | "db_query" | "db_insert" | "db_update" | "db_write"
                    );
            if !materialized {
                let implicit_borrow = i == 0
                    && matches!(
                        n,
                        "append"
                            | "db_exec"
                            | "db_all"
                            | "db_query"
                            | "db_insert"
                            | "db_update"
                            | "db_write"
                            | "bench_i64"
                            | "bench_f64"
                            | "bench_scalar"
                    );
                self.hold_value(a, implicit_borrow);
            }
            types.push(ty);
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
                    let element = t.inner();
                    if self.resource(&unowned(&element).0).is_some() {
                        return Err(error(line, "List[resource]のviewは未対応です。Listを直接index取得または反復してください"));
                    }
                    Ok(Type::generic("view", vec![element]))
                } else if self.resource(&unowned(t).0).is_some() {
                    self.available(&args[0], false)?;
                    Ok(Type::generic("view", vec![t.clone()]))
                } else {
                    Err(error(line, "viewにはstr/bytes/Listの所有値が必要です"))
                }
            }
            "copy" => {
                if crate::capabilities::contains_auth_proof(&types[0], &self.classes, &self.enums) {
                    return Err(error(
                        line,
                        "非Copy auth proofはcopyできません（nested wrapperを含みます）",
                    ));
                }
                if !types[0].is_view() {
                    return Err(error(line, "copyの対象はviewです"));
                }
                let t = types[0].inner();
                if let Some(resource) = self.native_resource_view(&types[0]) {
                    if !crate::stdlib::resource_info(resource).copy {
                        return Err(error(
                            line,
                            "非Copy resourceのcopyは未対応です。viewで借用してください",
                        ));
                    }
                    return Ok(t);
                }
                Ok(if t.0 == "str" || t.0 == "bytes" {
                    t
                } else {
                    Type::generic("List", vec![t])
                })
            }
            "share" => {
                if crate::capabilities::contains_auth_proof(&types[0], &self.classes, &self.enums) {
                    return Err(error(
                        line,
                        "auth proofをsharedへ変換できません（nested wrapperを含みます）",
                    ));
                }
                if self
                    .resource(&types[0].0)
                    .is_some_and(|resource| !crate::stdlib::resource_info(resource).shared)
                {
                    return Err(error(line, "このresourceはsharedへ変換できません"));
                }
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
                if self.native_resource_view(&types[0]).is_some() {
                    return Err(error(
                        line,
                        "resourceのviewは配列ではないためlenで長さを取得できません",
                    ));
                }
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
                if !json_encode_supported(&args[0], &self.classes, &self.enums) {
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
            "fail" => {
                let ret = expected
                    .filter(|ty| ty.0 == "Result" && ty.1.len() == 2)
                    .cloned()
                    .unwrap_or_else(|| {
                        Type::generic("Result", vec![Type::named("unit"), types[0].clone()])
                    });
                self.demand(&types[0], &ret.1[1], line)?;
                self.consume(&args[0])?;
                Ok(ret)
            }
            "error" | "not_found" | "internal_error" => {
                require(0, Type::named("str"))?;
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
                if self.native_resource_view(&types[0]).is_some() {
                    return Err(error(
                        line,
                        "resourceのviewは配列ではないためsliceを取得できません",
                    ));
                }
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
                    if self.vars[n].borrowed_element {
                        return Err(error(
                            line,
                            "List要素の読み取り専用借用はappendで変更できません",
                        ));
                    }
                    self.available(&args[0], false)?;
                    let receiver = BorrowedPlace {
                        binding: self.vars[n].binding,
                        fields: vec![],
                        owner_loan: true,
                        static_origin: false,
                    };
                    // Vec::push reserves its mutable receiver before evaluating
                    // the element. Shared reads used to compute that element
                    // may finish first; outer calls and stored views still loan it.
                    if self.borrowed_place_at(&receiver, self.expression_loans.len() - 1) {
                        return Err(error(
                            line,
                            "viewまたはループから参照中のListを変更できません",
                        ));
                    }
                } else {
                    return Err(error(line, "append対象は変数名です"));
                }
                self.record_view_use(&args[0], ExprUseMode::BorrowMut);
                self.consume(&args[1])?;
                if types[1].contains_view() {
                    let origin = self.origin(&args[1]);
                    let contents = self.content_origins(&args[1], &types[1], 0);
                    let E::Name(name) = &args[0].kind else {
                        unreachable!("checked append target")
                    };
                    let inputs = self.view_value_bindings(&args[1]);
                    self.extend_content_origins(name, origin, contents, inputs);
                }
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

/// Consume the final Low AST and native fragment, perform final integration,
/// and seal all decisions required by Rust generation.
pub fn finalize(
    mut primary: Program,
    native: Program,
    mut provenance: crate::source::SourceProvenance,
) -> Result<crate::checked::CheckedProgram, crate::checked::FinalizeError> {
    let mixed =
        !native.functions.is_empty() || !native.classes.is_empty() || !native.enums.is_empty();
    provenance.mark_replacements(&primary, &native);
    integrate_mode(&mut primary, native, false)
        .map_err(|error| checked::FinalizeError::checked(error, &provenance, mixed))?;
    checked::CheckedProgram::seal(primary, provenance)
}

pub fn integrate(p: &mut Program, native: Program) -> Result<(), String> {
    integrate_mode(p, native, false)
}

fn integrate_mode(p: &mut Program, mut native: Program, editor: bool) -> Result<(), String> {
    crate::modules::rebind_native(p, &mut native)?;
    let mut replaced = HashSet::new();
    // Resolve all replacement targets before changing the definitions. Aliases
    // of one function must not permit it to be replaced twice.
    let mut replacements = Vec::new();
    for f in &native.functions {
        let Some((_, target)) = f.attrs.iter().find(|(a, _)| a == "replace") else {
            continue;
        };
        let path = target
            .strip_prefix("generated::")
            .ok_or_else(|| error(f.line, "@replace generated::name が必要です"))?;
        let target_def = p.modules.resolve_root_path(path);
        let name = target_def
            .map(|d| d.symbol.clone())
            .unwrap_or_else(|| path.to_owned());
        if !replaced.insert(name.clone()) {
            return Err(error(f.line, "同じ関数を複数回replaceできません"));
        }
        let old = p
            .functions
            .iter()
            .find(|x| x.name == name)
            .ok_or_else(|| error(f.line, "replace対象が存在しません"))?;
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
        replacements.push((
            f.name.clone(),
            name,
            old.attrs.clone(),
            native.modules.definition(&f.name).map(|d| d.id.clone()),
            target_def.map(|d| d.id.clone()),
        ));
    }
    for (old, new, _, old_id, new_id) in &replacements {
        crate::modules::remap_definition(p, old, new);
        crate::modules::remap_definition(&mut native, old, new);
        if let (Some(old_id), Some(new_id)) = (old_id, new_id) {
            for metadata in [&mut p.modules, &mut native.modules] {
                metadata.definitions.retain(|d| &d.id != old_id);
                // Replacement declarations are private to their Low fragment.
                // Their calls still refer to the shared generated definition.
                metadata
                    .bindings
                    .retain(|b| b.target != BindingTarget::Definition(old_id.clone()));
                for reference in &mut metadata.references {
                    if &reference.target == old_id {
                        reference.target = new_id.clone();
                    }
                }
            }
        }
    }
    for c in native.classes.drain(..) {
        if p.classes.iter().any(|x| x.name == c.name) {
            return Err(error(c.line, "nativeとgeneratedでclassが重複しています"));
        }
        p.classes.push(c);
    }
    for enumeration in native.enums.drain(..) {
        if p.enums.iter().any(|old| old.name == enumeration.name) {
            return Err(error(
                enumeration.line,
                "nativeとgeneratedでenumが重複しています",
            ));
        }
        p.enums.push(enumeration);
    }
    for mut f in native.functions.drain(..) {
        if f.attrs.iter().any(|(a, _)| a == "replace") {
            let (_, _, attrs, _, _) = replacements
                .iter()
                .find(|(_, new, _, _, _)| new == &f.name)
                .ok_or_else(|| error(f.line, "replace対象が存在しません"))?;
            let pos = p
                .functions
                .iter()
                .position(|x| x.name == f.name)
                .ok_or_else(|| error(f.line, "replace対象が存在しません"))?;
            f.attrs = attrs.clone();
            p.functions[pos] = f;
        } else {
            p.functions.push(f);
        }
    }
    p.modules.merge_native(native.modules)?;
    crate::modules::synchronize(p);
    check_mode(p, editor)
}

#[cfg(test)]
mod flow_metadata_tests {
    use super::*;

    const RESTORE: &str = r#"def restore(flag: bool, parts: List[view[str]]) -> List[view[str]]:
    alias = [parts[0]]
    alias = alias
    if flag:
        local = "temporary"
        alias = [view(local)]
        alias = [parts[0]]
        return alias
    return alias
"#;

    fn snapshot<'a>(
        facts: &'a StmtFlowFacts,
        edge: bool,
        name: &str,
    ) -> &'a ViewListBindingSnapshot {
        let bindings = if edge { &facts.after } else { &facts.before };
        bindings
            .iter()
            .find(|binding| binding.name == name)
            .unwrap_or_else(|| panic!("missing {name} in {bindings:?}"))
    }

    fn assert_restore_facts(program: &Program) -> BindingId {
        let function = program
            .functions
            .iter()
            .find(|function| function.name == "restore")
            .unwrap();
        let first = function.body[0].flow.as_ref().unwrap();
        let alias_binding = first.assignment.unwrap().target;
        assert_eq!(snapshot(first, true, "alias").binding, alias_binding);

        let self_assignment = function.body[1].flow.as_ref().unwrap();
        let self_assignment_facts = self_assignment.assignment.unwrap();
        assert_eq!(self_assignment_facts.target, alias_binding);
        assert!(self_assignment_facts.rhs_consumed_target);
        assert!(!snapshot(self_assignment, false, "alias").moved);
        assert!(!snapshot(self_assignment, true, "alias").moved);

        let S::If(_, then_body, _) = &function.body[2].kind else {
            panic!("expected the restoration branch")
        };
        let local_assignment = then_body[1].flow.as_ref().unwrap();
        assert_eq!(local_assignment.assignment.unwrap().target, alias_binding);
        assert!(snapshot(local_assignment, true, "alias")
            .origins
            .iter()
            .any(|origin| origin.owner_loan && !origin.static_origin));

        let restore_assignment = then_body[2].flow.as_ref().unwrap();
        assert_eq!(restore_assignment.assignment.unwrap().target, alias_binding);
        let restored = snapshot(restore_assignment, true, "alias");
        assert!(restored
            .origins
            .iter()
            .any(|origin| !origin.owner_loan && !origin.static_origin));
        assert_eq!(restored.binding, alias_binding);

        let returned = then_body[3].flow.as_ref().unwrap();
        assert_eq!(snapshot(returned, false, "alias").binding, alias_binding);
        alias_binding
    }

    fn all_flow_none(statements: &[Stmt]) -> bool {
        statements.iter().all(|statement| {
            statement.flow.is_none()
                && match &statement.kind {
                    S::If(_, then_body, else_body) => {
                        all_flow_none(then_body) && all_flow_none(else_body)
                    }
                    S::Match(_, arms) => arms.iter().all(|arm| all_flow_none(&arm.body)),
                    S::While(_, body) | S::For(_, _, body) | S::Scope(body) => all_flow_none(body),
                    S::Assign { .. } | S::Return(_) | S::Expr(_) | S::Spawn(_) => true,
                }
        })
    }

    #[test]
    fn flow_facts_recompute_for_high_saved_low_and_self_consumption() {
        let mut high = crate::parser::parse(RESTORE, true).unwrap();
        check(&mut high).unwrap();
        let high_binding = assert_restore_facts(&high);

        // Checked facts are compiler-only and do not survive Low serialization.
        let low_text = crate::emit::low(&high);
        let mut low = crate::parser::parse(&low_text, false).unwrap();
        assert!(low
            .functions
            .iter()
            .all(|function| all_flow_none(&function.body)));
        check(&mut low).unwrap();
        assert_restore_facts(&low);

        // Rechecking replaces stale snapshots while retaining source binding IDs.
        high.functions[0].body[1]
            .flow
            .as_mut()
            .unwrap()
            .before
            .clear();
        check(&mut high).unwrap();
        assert_eq!(assert_restore_facts(&high), high_binding);
    }

    #[test]
    fn checked_operand_roles_reach_codegen_and_missing_roles_fail() {
        let source = "def restore(part: view[str]) -> List[view[str]]:\n    local = \"inner\"\n    parts = [view(local)]\n    length = len(parts)\n    append(parts, part)\n    parts = [part]\n    return parts\n";
        let mut program = crate::parser::parse(source, true).unwrap();
        check(&mut program).unwrap();
        let body = &program.functions[0].body;
        assert_eq!(
            body[2].flow.as_ref().unwrap().expression_uses[0].mode,
            ExprUseMode::Borrow
        );
        assert_eq!(
            body[3].flow.as_ref().unwrap().expression_uses[0].mode,
            ExprUseMode::BorrowMut
        );
        assert_eq!(
            body[5].flow.as_ref().unwrap().expression_uses[0].mode,
            ExprUseMode::Move
        );
        let checked = checked::CheckedProgram::seal(
            program.clone(),
            crate::source::SourceProvenance::user_low_unmapped(),
        )
        .unwrap();
        crate::emit::rust(&checked).unwrap();
        let mut wrong_binding = program.functions[0].body[5]
            .flow
            .as_ref()
            .unwrap()
            .expression_uses[0]
            .clone();
        program.functions[0].body[5]
            .flow
            .as_mut()
            .unwrap()
            .expression_uses
            .clear();
        assert!(checked::CheckedProgram::seal(
            program.clone(),
            crate::source::SourceProvenance::user_low_unmapped()
        )
        .err()
        .unwrap()
        .to_string()
        .contains("internal owning-view lowering"));
        wrong_binding.binding.token += 1;
        program.functions[0].body[5]
            .flow
            .as_mut()
            .unwrap()
            .expression_uses
            .push(wrong_binding);
        assert!(checked::CheckedProgram::seal(
            program.clone(),
            crate::source::SourceProvenance::user_low_unmapped()
        )
        .err()
        .unwrap()
        .to_string()
        .contains("internal owning-view lowering"));
    }

    #[test]
    fn flow_collection_is_gated_to_view_return_dependencies() {
        let source = r#"def ordinary() -> i64:
    return 1
async def asynchronous(parts: List[view[str]]) -> List[view[str]]:
    alias = parts
    return alias
def looped(flag: bool, part: view[str]) -> List[view[str]]:
    alias = [part]
    while flag:
        alias = [part]
    return alias
def matched(value: Option[view[str]]) -> List[view[str]]:
    match value:
        case Some(part):
            return [part]
        case None:
            return []
async def scoped() -> Result[unit, Error]:
    async with scope:
        print(1)
    return ok(print(0))
"#;
        let mut program = crate::parser::parse(source, true).unwrap();
        check(&mut program).unwrap();

        for function in program.functions.iter().filter(|function| {
            !matches!(
                function.name.as_str(),
                "matched" | "looped" | "asynchronous"
            )
        }) {
            assert!(
                all_flow_none(&function.body),
                "{} had flow facts",
                function.name
            );
        }

        let view_list = Type::generic(
            "List",
            vec![Type::generic("view", vec![Type::named("str")])],
        );
        let scalar = Type::named("i64");
        assert!(!should_collect_view_flow(false, false, &scalar, &[]));
        assert!(should_collect_view_flow(false, true, &view_list, &[]));
        let function = program
            .functions
            .iter()
            .find(|function| function.name == "scoped")
            .unwrap();
        assert!(supports_view_flow(&function.body));
        assert!(!should_collect_view_flow(
            false,
            function.asynchronous,
            &function.ret,
            &function.body
        ));
    }

    #[test]
    fn flow_snapshots_only_keep_return_dependency_bindings() {
        let mut body = String::new();
        for index in 0..128 {
            body.push_str(&format!("    value{index}: List[view[str]] = []\n"));
        }
        let source = format!(
            "def literal() -> List[view[str]]:\n{body}    return []\n\n\
             def named(part: view[str]) -> List[view[str]]:\n{body}    value127 = [part]\n    return value127\n"
        );
        let mut program = crate::parser::parse(&source, true).unwrap();
        check(&mut program).unwrap();
        assert!(all_flow_none(&program.functions[0].body));
        let named = &program.functions[1];
        assert!(crate::view_flow::plan(named).unwrap().is_none());
        for statement in &named.body {
            let flow = statement.flow.as_ref().unwrap();
            assert!(flow.before.len() <= 1);
            assert!(flow.after.len() <= 1);
            assert!(flow
                .before
                .iter()
                .chain(&flow.after)
                .all(|binding| binding.name == "value127"));
        }
    }

    #[test]
    fn return_alias_dependencies_keep_straight_line_snapshots_sparse() {
        for count in [100, 500, 1000] {
            let mut source = String::from(
                "def ordinary(part: view[str]) -> List[view[str]]:\n    value0 = [part]\n",
            );
            for index in 1..count {
                source.push_str(&format!("    value{index} = value{}\n", index - 1));
            }
            source.push_str(&format!("    return value{}\n", count - 1));
            let mut program = crate::parser::parse(&source, true).unwrap();
            check(&mut program).unwrap();
            let function = &program.functions[0];
            let snapshots: usize = function
                .body
                .iter()
                .map(|statement| {
                    let flow = statement.flow.as_ref().unwrap();
                    assert!(flow.before.len() <= 1);
                    assert!(flow.after.len() <= 2);
                    flow.before.len() + flow.after.len()
                })
                .sum();
            assert!(snapshots <= 3 * count + 2);
            assert!(crate::view_flow::plan(function).unwrap().is_none());
        }
    }

    #[test]
    fn loop_regions_preserve_candidate_reads_and_assignments() {
        for use_ in [
            "parts = [part]",
            "length = len(parts)",
            "length = len(view(parts))",
            "selected = parts[0]",
        ] {
            let source = format!("def restoring(part: view[str]) -> List[view[str]]:\n    local = \"inner\"\n    parts = [view(local)]\n    parts = [part]\n    for number in range(2):\n        {use_}\n    return parts\n");
            let mut program = crate::parser::parse(&source, true).unwrap();
            check(&mut program).unwrap();
            assert!(
                crate::view_flow::plan(&program.functions[0])
                    .unwrap()
                    .is_some(),
                "{use_}"
            );
        }
    }

    #[test]
    fn semantic_value_edges_keep_mutation_inputs_without_scalar_overselection() {
        let source = r#"def mutation_input(part: view[str]) -> List[List[view[str]]]:
    local = "inner"
    item = [view(local)]
    item = [part]
    parts = [[part]]
    append(parts, item)
    return parts
def inspect(left: i64, right: i64):
    return
def scalar_observation(part: view[str]) -> List[view[str]]:
    local = "inner"
    parts = [view(local)]
    parts = [part]
    item = [view(local)]
    inspect(len(parts), len(item))
    for number in range(2):
        length = len(item)
    return parts
"#;
        fn assert_edges(program: &Program) {
            let function = &program.functions[0];
            let item = function.body[1]
                .flow
                .as_ref()
                .unwrap()
                .assignment
                .unwrap()
                .target;
            let parts = function.body[3]
                .flow
                .as_ref()
                .unwrap()
                .assignment
                .unwrap()
                .target;
            let mutations = &function.body[4].flow.as_ref().unwrap().content_mutations;
            assert_eq!(mutations.len(), 1);
            assert_eq!(mutations[0].binding, parts);
            assert!(mutations[0].added_origins.is_empty());
            assert_eq!(mutations[0].inputs, vec![item]);
            assert_eq!(
                function.body[5].flow.as_ref().unwrap().return_observers,
                vec![parts]
            );
            assert!(crate::view_flow::plan(function).unwrap().is_some());

            let observation = &program.functions[2];
            assert!(observation.body[4]
                .flow
                .as_ref()
                .unwrap()
                .value_dependencies
                .is_empty());
            let plan = crate::view_flow::plan(observation)
                .unwrap()
                .expect("scalar inspection must not make item an escaping candidate");
            let S::Assign { name, .. } = &plan.body.statements[3].stmt.kind else {
                panic!("expected the item declaration");
            };
            assert_eq!(name, "item");
        }
        let mut high = crate::parser::parse(source, true).unwrap();
        check(&mut high).unwrap();
        assert_edges(&high);
        let mut saved = crate::parser::parse(&crate::emit::low(&high), false).unwrap();
        check(&mut saved).unwrap();
        assert_edges(&saved);
        check(&mut high).unwrap();
        assert_edges(&high);
    }

    #[test]
    fn content_mutation_events_survive_rhs_replacement_and_stay_in_their_statement() {
        let source = r#"def make(ignored: unit, part: view[str]) -> List[view[str]]:
    return [part]
def consume(ignored: unit, parts: List[view[str]]) -> bool:
    return len(parts) > 0
def rhs(flag: bool, part: view[str]) -> List[view[str]]:
    parts = [part]
    if flag:
        local = "RHS temporary"
        parts = make(append(parts, view(local)), part)
    return parts
def condition(flag: bool, part: view[str]) -> List[view[str]]:
    local = "condition temporary"
    parts = [part]
    if flag and consume(append(parts, view(local)), parts):
        selected = True
    parts = [part]
    return parts
"#;
        fn assert_events(program: &Program) {
            let rhs = program
                .functions
                .iter()
                .find(|function| function.name == "rhs")
                .unwrap();
            let branch = &rhs.body[1];
            assert!(branch.flow.as_ref().unwrap().content_mutations.is_empty());
            let S::If(_, then_body, _) = &branch.kind else {
                panic!("expected branch")
            };
            let replacement = then_body[1].flow.as_ref().unwrap();
            assert_eq!(replacement.content_mutations.len(), 1);
            let mutation = &replacement.content_mutations[0];
            assert_eq!(mutation.binding, replacement.assignment.unwrap().target);
            assert!(mutation
                .added_origins
                .iter()
                .any(|origin| origin.owner_loan));
            assert!(snapshot(replacement, true, "parts")
                .origins
                .iter()
                .all(|origin| !origin.owner_loan));
            assert!(crate::view_flow::plan(rhs).unwrap().is_some());

            let condition = program
                .functions
                .iter()
                .find(|function| function.name == "condition")
                .unwrap();
            let branch = &condition.body[2];
            let facts = branch.flow.as_ref().unwrap();
            assert_eq!(facts.content_mutations.len(), 1);
            assert!(
                facts
                    .branch_entry
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|binding| binding.name == "parts")
                    .unwrap()
                    .moved
            );
            let S::If(_, then_body, _) = &branch.kind else {
                panic!("expected branch")
            };
            assert!(then_body[0]
                .flow
                .as_ref()
                .unwrap()
                .content_mutations
                .is_empty());
        }
        let mut high = crate::parser::parse(source, true).unwrap();
        check(&mut high).unwrap();
        assert_events(&high);
        check(&mut high).unwrap();
        assert_events(&high);
        let mut low = crate::parser::parse(&crate::emit::low(&high), false).unwrap();
        check(&mut low).unwrap();
        assert_events(&low);
    }
}
