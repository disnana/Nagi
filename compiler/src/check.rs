use crate::ast::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct Var {
    ty: Type,
    moved: bool,
    moved_fields: HashSet<Vec<String>>,
    origin: Option<String>,
    param: bool,
}
struct Checker {
    classes: HashMap<String, Class>,
    functions: HashMap<String, Function>,
    vars: HashMap<String, Var>,
    ret: Type,
    asynchronous: bool,
    scope: usize,
    editor: bool,
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
    };
    let mut symbols = HashSet::new();
    for class in &p.classes {
        if !symbols.insert(class.name.clone()) {
            return Err(error(class.line, "型の重複定義"));
        }
        c.classes.insert(class.name.clone(), class.clone());
    }
    for class in &p.classes {
        let mut fields = HashSet::new();
        for (n, t) in &class.fields {
            c.valid(t, class.line)?;
            if !fields.insert(n) {
                return Err(error(class.line, "フィールドの重複"));
            }
            if t.contains_view() {
                return Err(error(
                    class.line,
                    "0.1のclassにviewは保存できません。所有型またはcopyを使用してください",
                ));
            }
        }
        fn cycle(name: &str, classes: &HashMap<String, Class>, seen: &mut HashSet<String>) -> bool {
            if !seen.insert(name.into()) {
                return true;
            }
            let result = classes[name]
                .fields
                .iter()
                .any(|(_, t)| classes.contains_key(&t.0) && cycle(&t.0, classes, seen));
            seen.remove(name);
            result
        }
        if cycle(&class.name, &c.classes, &mut HashSet::new()) {
            return Err(error(
                class.line,
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
            let mut names = HashSet::new();
            for (n, t) in &f.params {
                c.valid(t, f.line)?;
                if !names.insert(n) {
                    return Err(error(f.line, "引数の重複"));
                }
            }
        }
    }
    for f in &mut p.functions {
        c.vars.clear();
        c.ret = f.ret.clone();
        c.asynchronous = f.asynchronous;
        c.scope = 0;
        c.valid(&f.ret, f.line)?;
        for (n, t) in &f.params {
            c.valid(t, f.line)?;
            if c.vars
                .insert(
                    n.clone(),
                    Var {
                        ty: t.clone(),
                        moved: false,
                        moved_fields: HashSet::new(),
                        origin: if t.contains_view() {
                            Some(n.clone())
                        } else {
                            None
                        },
                        param: true,
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
    Ok(())
}
fn returns(ss: &[Stmt]) -> bool {
    ss.last().is_some_and(|s| match &s.kind {
        S::Return(_) => true,
        S::If(_, a, b) => returns(a) && returns(b),
        S::Match(_, arms) => arms.len() == 2 && arms.iter().all(|arm| returns(&arm.body)),
        _ => false,
    })
}
impl Checker {
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
    fn origin(&self, e: &Expr) -> Option<String> {
        match &e.kind {
            E::Name(n) => self
                .vars
                .get(n)
                .and_then(|v| v.origin.clone().or_else(|| Some(n.clone()))),
            E::Call(_, _, args) => args.iter().find_map(|x| self.origin(x)),
            E::Field(e, _) | E::Try(e) | E::Await(e) => self.origin(e),
            _ => None,
        }
    }
    fn borrowed(&self, n: &str) -> bool {
        self.vars.iter().any(|(name, v)| {
            name != n && !v.moved && v.ty.contains_view() && v.origin.as_deref() == Some(n)
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
        if self.borrowed(name) {
            return Err(error(
                e.line,
                format!(
                    "{name} はviewから参照されています。viewのscopeを終了するかcopyしてください"
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
        self.block(ss)?;
        let after = self.vars.clone();
        self.vars = before;
        Ok(after)
    }
    fn merge_moves(&mut self, after: HashMap<String, Var>) {
        for (n, v) in after {
            if let Some(x) = self.vars.get_mut(&n) {
                x.moved |= v.moved;
                x.moved_fields.extend(v.moved_fields);
            }
        }
    }
    fn block(&mut self, ss: &mut [Stmt]) -> Result<(), String> {
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
                if self.borrowed(name) {
                    return Err(error(s.line, "viewが生きている所有値は再代入できません"));
                }
                let origin = if ty.contains_view() {
                    self.origin(value)
                } else {
                    None
                };
                self.consume(value)?;
                *declare = old.is_none();
                *annotation = Some(ty.clone());
                s.binding_type = Some(ty.clone());
                self.vars.insert(
                    name.clone(),
                    Var {
                        ty,
                        moved: false,
                        moved_fields: HashSet::new(),
                        origin,
                        param: false,
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
                        if !origin
                            .as_ref()
                            .and_then(|n| self.vars.get(n))
                            .is_some_and(|v| v.param && v.ty.contains_view())
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
                if t.0 == "Future" {
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
                self.merge_moves(am);
                self.merge_moves(bm);
            }
            S::While(c, b) => {
                let t = self.expr(c, Some(&Type::named("bool")))?;
                self.demand(&t, &Type::named("bool"), s.line)?;
                let m = self.child(b)?;
                self.merge_moves(m);
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
                                origin: if payload.contains_view() {
                                    origin.clone()
                                } else {
                                    None
                                },
                                ty: payload,
                                moved: false,
                                moved_fields: HashSet::new(),
                                param: false,
                            },
                        );
                    }
                    self.block(&mut arm.body)?;
                    let mut after = self.vars.clone();
                    if let Some(name) = &arm.binding {
                        after.remove(name);
                    }
                    moves.push(after);
                }
                self.vars = before;
                for after in moves {
                    self.merge_moves(after);
                }
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
                let before = self.vars.clone();
                self.vars.insert(
                    n.clone(),
                    Var {
                        ty: elem,
                        moved: false,
                        moved_fields: HashSet::new(),
                        origin: None,
                        param: false,
                    },
                );
                self.block(b)?;
                let after = self.vars.clone();
                self.vars = before;
                self.merge_moves(after);
            }
            S::Scope(b) => {
                if !self.asynchronous || self.ret.0 != "Result" {
                    return Err(error(s.line, "scopeはasync Result関数内で使用してください"));
                }
                self.scope += 1;
                let m = self.child(b)?;
                self.scope -= 1;
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
                if !s.parse::<f64>().is_ok_and(|x| x.is_finite()) {
                    return Err(error(line, "浮動小数リテラルが範囲外です"));
                }
                expected
                    .filter(|t| t.0 == "f32" || t.0 == "f64")
                    .cloned()
                    .unwrap_or_else(|| Type::named("f64"))
            }
            E::Str(_) => Type::named("str"),
            E::Bool(_) => Type::named("bool"),
            E::Null => expected
                .filter(|t| t.0 == "Option")
                .cloned()
                .ok_or_else(|| error(line, "Noneにはnullableの型注釈が必要です"))?,
            E::Name(n) => {
                if let Some(v) = self.vars.get(n) {
                    v.ty.clone()
                } else if let Some(f) = self.functions.get(n) {
                    let mut ts: Vec<Type> = f.params.iter().map(|p| p.1.clone()).collect();
                    ts.push(f.ret.clone());
                    Type::generic("fn", ts)
                } else {
                    return Err(error(line, format!("未定義の変数: {n}")));
                }
            }
            E::Unary(op, x) => {
                let t = self.expr(x, expected)?;
                if op == "not" {
                    self.demand(&t, &Type::named("bool"), line)?;
                } else if !t.0.starts_with('i') && !t.0.starts_with('f') {
                    return Err(error(line, "符号反転には符号付き数値が必要です"));
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
                        if !left.is_copy() && left.0 != "str" {
                            return Err(error(line, "比較に対応していない型です"));
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
                if t.0 != "Future" {
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
                if let Some(f) = self.functions.get(n).cloned() {
                    if !ts.is_empty() {
                        return Err(error(line, "generic関数は未実装です"));
                    }
                    if args.len() != f.params.len() {
                        return Err(error(line, "引数の数が一致しません"));
                    }
                    for (arg, (_, t)) in args.iter_mut().zip(&f.params) {
                        let got = self.expr(arg, Some(t))?;
                        self.demand(&got, t, line)?;
                        self.consume(arg)?;
                    }
                    if f.asynchronous {
                        future(f.ret)
                    } else {
                        f.ret
                    }
                } else {
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
                if !types[0].is_copy() && types[0].0 != "str" {
                    return Err(error(line, "print/writeはprimitiveまたはstrを取ります"));
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
            "json_encode" => Ok(result(Type::named("str"))),
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
                        return Err(error(line, "viewが生きているListを変更できません"));
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
