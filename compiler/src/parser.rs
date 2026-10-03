use crate::ast::*;
use crate::lexer::{lex, Token, K};

pub fn parse(src: &str, high: bool) -> Result<Program, String> {
    if src.len() > 2_000_000 {
        return Err("source limit: 2 MB".into());
    }
    let mut modules = ModuleMetadata::default();
    if !high {
        let mut has_header = false;
        for (line, text) in src.lines().enumerate() {
            let text = text.trim_start();
            if text == "# nagi-modules-v1" {
                return Err(format!(
                    "line {}: invalid module metadata: missing JSON",
                    line + 1
                ));
            }
            if let Some(json) = text.strip_prefix("# nagi-modules-v1 ") {
                if has_header {
                    return Err(format!(
                        "line {}: module metadata header is duplicated",
                        line + 1
                    ));
                }
                modules = serde_json::from_str(json).map_err(|error| {
                    format!("line {}: invalid module metadata: {error}", line + 1)
                })?;
                if modules.is_empty() {
                    return Err(format!(
                        "line {}: invalid module metadata: empty header",
                        line + 1
                    ));
                }
                has_header = true;
            }
        }
    }
    let mut program = Parser {
        ts: lex(src, high)?,
        pos: 0,
        high,
        depth: 0,
    }
    .program()?;
    program.modules = modules;
    Ok(program)
}
struct Parser {
    ts: Vec<Token>,
    pos: usize,
    high: bool,
    depth: usize,
}
impl Parser {
    fn t(&self) -> &Token {
        &self.ts[self.pos.min(self.ts.len() - 1)]
    }
    fn span(&self, start: usize) -> Span {
        Span {
            start,
            end: self.pos,
        }
    }
    fn err(&self, msg: &str) -> String {
        format!(
            "line {}:{}: {msg} (found {:?})",
            self.t().line,
            self.t().col,
            self.t().kind
        )
    }
    fn eat(&mut self, s: &str) -> bool {
        if matches!(&self.t().kind,K::Sym(x)|K::Id(x) if x==s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Result<(), String> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(self.err(&format!("{s} が必要です")))
        }
    }
    fn name(&mut self) -> Result<String, String> {
        if let K::Id(s) = self.t().kind.clone() {
            self.pos += 1;
            Ok(s)
        } else {
            Err(self.err("識別子が必要です"))
        }
    }
    fn qualified_name(&mut self) -> Result<String, String> {
        let mut name = self.name()?;
        while self.eat(".") {
            name.push('.');
            name.push_str(&self.name()?);
        }
        Ok(name)
    }
    fn callee_name(expr: &Expr) -> Option<String> {
        let mut receiver = expr;
        let mut members = vec![];
        loop {
            match &receiver.kind {
                E::Name(name) => {
                    members.push(name.as_str());
                    members.reverse();
                    return Some(members.join("."));
                }
                E::Field(base, member) => {
                    members.push(member.as_str());
                    receiver = base;
                }
                _ => return None,
            }
        }
    }
    fn skip(&mut self) {
        while matches!(self.t().kind, K::Newline) || self.eat(";") {
            if matches!(self.t().kind, K::Newline) {
                self.pos += 1;
            }
        }
    }
    fn end_stmt(&mut self) -> Result<(), String> {
        if self.eat(";") {
            self.skip();
            return Ok(());
        }
        if matches!(self.t().kind, K::Newline) {
            self.pos += 1;
            self.skip();
            Ok(())
        } else if matches!(self.t().kind, K::Dedent | K::Eof)
            || matches!(&self.t().kind,K::Sym(s)if s=="}")
        {
            Ok(())
        } else {
            Err(self.err("改行または ; が必要です"))
        }
    }
    fn ty(&mut self) -> Result<Type, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err(self.err("型の入れ子は64段までです"));
        }
        let t = if self.eat("[") {
            let a = self.ty()?;
            self.expect("]")?;
            Type::generic("List", vec![a])
        } else {
            let n = self.qualified_name()?;
            let mut a = vec![];
            if self.eat("[") {
                loop {
                    a.push(self.ty()?);
                    if self.eat("]") {
                        break;
                    }
                    self.expect(",")?;
                }
            }
            Type(n, a)
        };
        let t = if self.eat("?") {
            Type::generic("Option", vec![t])
        } else {
            t
        };
        self.depth -= 1;
        Ok(t)
    }
    fn begin(&mut self) -> Result<(), String> {
        if self.high {
            self.expect(":")?;
            self.end_stmt()?;
            if matches!(self.t().kind, K::Indent) {
                self.pos += 1;
                Ok(())
            } else {
                Err(self.err("ブロックの字下げが必要です"))
            }
        } else {
            self.expect("{")?;
            self.skip();
            Ok(())
        }
    }
    fn ended(&self) -> bool {
        if self.high {
            matches!(self.t().kind, K::Dedent)
        } else {
            matches!(&self.t().kind,K::Sym(s)if s=="}")
        }
    }
    fn close(&mut self) -> Result<(), String> {
        if self.high {
            if matches!(self.t().kind, K::Dedent) {
                self.pos += 1;
                Ok(())
            } else {
                Err(self.err("ブロックの終端が必要です"))
            }
        } else {
            self.expect("}")
        }
    }
    fn program(mut self) -> Result<Program, String> {
        let mut p = Program::default();
        let mut attrs = vec![];
        self.skip();
        while !matches!(self.t().kind, K::Eof) {
            let import_start = self.pos;
            let from = self.eat("from");
            if from || self.eat("import") {
                if !attrs.is_empty() {
                    return Err(self.err("importに属性は付けられません"));
                }
                let line = self.ts[import_start].line;
                let (path, source) = if let K::Str(file) = self.t().kind.clone() {
                    self.pos += 1;
                    (file, ImportSource::File)
                } else {
                    let name = self.qualified_name()?;
                    if !name.starts_with("std.") {
                        return Err(
                            self.err("importには相対ファイルパスの文字列かstd module名が必要です")
                        );
                    }
                    (name, ImportSource::Standard)
                };
                let kind = if from {
                    self.expect("import")?;
                    let mut names = vec![];
                    loop {
                        let start = self.pos;
                        let name = self.name()?;
                        let name_span = self.span(start);
                        let (alias, alias_span) = if self.eat("as") {
                            let start = self.pos;
                            let alias = self.name()?;
                            (alias, self.span(start))
                        } else {
                            (name.clone(), name_span)
                        };
                        names.push(ImportName {
                            name,
                            alias,
                            name_span,
                            alias_span,
                        });
                        if !self.eat(",") {
                            break;
                        }
                    }
                    ImportKind::Names(names)
                } else if self.eat("as") {
                    let start = self.pos;
                    let alias = self.name()?;
                    ImportKind::Module {
                        alias,
                        alias_span: self.span(start),
                    }
                } else if source == ImportSource::File {
                    p.imports.push((path.clone(), line));
                    ImportKind::Flat
                } else {
                    return Err(self.err("std moduleのimportにはasの名前が必要です"));
                };
                let span = self.span(import_start);
                self.end_stmt()?;
                p.module_imports.push(ModuleImport {
                    path,
                    source,
                    line,
                    span,
                    kind,
                });
                continue;
            }
            if self.eat("@") {
                let n = self.name()?;
                let val = if self.eat("(") {
                    let s = if let K::Str(s) = self.t().kind.clone() {
                        self.pos += 1;
                        s
                    } else {
                        return Err(self.err("属性の引数は文字列です"));
                    };
                    self.expect(")")?;
                    s
                } else {
                    let mut s = self.name()?;
                    while self.eat("::") {
                        s.push_str("::");
                        s.push_str(&self.name()?);
                    }
                    s
                };
                attrs.push((n, val));
                self.end_stmt()?;
                continue;
            }
            let line = self.t().line;
            if self.eat(if self.high { "class" } else { "record" }) {
                if !attrs.is_empty() {
                    return Err(self.err("class属性は未対応です"));
                }
                let name = self.name()?;
                self.begin()?;
                let mut fields = vec![];
                let mut field_lines = vec![];
                while !self.ended() {
                    field_lines.push(self.t().line);
                    let n = self.name()?;
                    self.expect(":")?;
                    let t = self.ty()?;
                    self.end_stmt()?;
                    fields.push((n, t));
                }
                self.close()?;
                p.classes.push(Class {
                    name,
                    fields,
                    field_lines,
                    line,
                });
            } else if self.eat("enum") {
                if !attrs.is_empty() {
                    return Err(self.err("enumに属性は付けられません"));
                }
                let name = self.name()?;
                self.begin()?;
                let mut variants = vec![];
                while !self.ended() {
                    let line = self.t().line;
                    let start = self.pos;
                    let name = self.name()?;
                    let name_span = self.span(start);
                    let mut fields = vec![];
                    let mut field_lines = vec![];
                    if self.eat("(") {
                        loop {
                            field_lines.push(self.t().line);
                            let field = self.name()?;
                            self.expect(":")?;
                            fields.push((field, self.ty()?));
                            if self.eat(")") {
                                break;
                            }
                            self.expect(",")?;
                        }
                    }
                    self.end_stmt()?;
                    variants.push(EnumVariant {
                        name,
                        fields,
                        field_lines,
                        line,
                        name_span,
                    });
                }
                self.close()?;
                p.enums.push(Enum {
                    name,
                    variants,
                    line,
                });
            } else {
                let external = self.eat("extern");
                let asynchronous = self.eat("async");
                self.expect(if self.high { "def" } else { "fn" })?;
                let name = self.name()?;
                self.expect("(")?;
                let mut params = vec![];
                let mut parameter_spans = vec![];
                if !self.eat(")") {
                    loop {
                        let start = self.pos;
                        let n = self.name()?;
                        parameter_spans.push(self.span(start));
                        self.expect(":")?;
                        params.push((n, self.ty()?));
                        if self.eat(")") {
                            break;
                        }
                        self.expect(",")?;
                    }
                }
                let ret = if self.eat("->") {
                    self.ty()?
                } else {
                    Type::named("unit")
                };
                let body = if external {
                    self.end_stmt()?;
                    Vec::new()
                } else {
                    self.block()?
                };
                p.functions.push(Function {
                    name,
                    params,
                    parameter_spans,
                    ret,
                    asynchronous,
                    external,
                    body,
                    attrs: std::mem::take(&mut attrs),
                    line,
                });
            }
            self.skip();
        }
        if !attrs.is_empty() {
            return Err("属性の対象がありません".into());
        }
        Ok(p)
    }
    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err(self.err("ブロックの入れ子は64段までです"));
        }
        self.begin()?;
        let mut ss = vec![];
        while !self.ended() {
            if matches!(self.t().kind, K::Eof) {
                return Err(self.err("ブロックが閉じていません"));
            }
            ss.push(self.stmt()?);
            self.skip();
        }
        self.close()?;
        self.depth -= 1;
        Ok(ss)
    }
    fn stmt(&mut self) -> Result<Stmt, String> {
        let line = self.t().line;
        let mut binding_span = None;
        let kind = if self.eat("return") {
            let e = if matches!(self.t().kind, K::Newline | K::Dedent | K::Eof)
                || matches!(&self.t().kind,K::Sym(s)if s==";"||s=="}")
            {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.end_stmt()?;
            S::Return(e)
        } else if self.eat("if") {
            let c = self.expr(0)?;
            let y = self.block()?;
            self.skip();
            let n = if self.eat("else") {
                self.block()?
            } else {
                vec![]
            };
            S::If(c, y, n)
        } else if self.eat("while") {
            let c = self.expr(0)?;
            S::While(c, self.block()?)
        } else if self.eat("match") {
            let value = self.expr(0)?;
            self.begin()?;
            let mut arms = vec![];
            while !self.ended() {
                let line = self.t().line;
                self.expect("case")?;
                let start = self.pos;
                let name = self.qualified_name()?;
                let span = self.span(start);
                let pattern = if name == "Ok" || name == "Err" {
                    self.expect("(")?;
                    let binding = self.pattern_binding()?;
                    self.expect(")")?;
                    MatchPattern::Result {
                        ok: name == "Ok",
                        binding,
                    }
                } else if name == "Some" {
                    self.expect("(")?;
                    let binding = self.pattern_binding()?;
                    self.expect(")")?;
                    MatchPattern::Option {
                        binding: Some(binding),
                    }
                } else if name == "None" {
                    MatchPattern::Option { binding: None }
                } else {
                    if !name.contains('.') {
                        return Err(
                            self.err("caseにはOk、Err、Some、None、またはenumの種類名が必要です")
                        );
                    }
                    let mut bindings = vec![];
                    if self.eat("(") {
                        loop {
                            bindings.push(self.pattern_binding()?);
                            if self.eat(")") {
                                break;
                            }
                            self.expect(",")?;
                        }
                    }
                    MatchPattern::Enum {
                        name,
                        bindings,
                        span,
                    }
                };
                arms.push(MatchArm {
                    pattern,
                    body: self.block()?,
                    line,
                });
                self.skip();
            }
            self.close()?;
            S::Match(value, arms)
        } else if self.eat("for") {
            let start = self.pos;
            let n = self.name()?;
            binding_span = Some(self.span(start));
            self.expect("in")?;
            let e = self.expr(0)?;
            S::For(n, e, self.block()?)
        } else if self.eat("scope") {
            S::Scope(self.block()?)
        } else if self.eat("async") {
            self.expect("with")?;
            self.expect("scope")?;
            S::Scope(self.block()?)
        } else if self.eat("spawn") {
            let e = self.expr(0)?;
            self.end_stmt()?;
            S::Spawn(e)
        } else {
            let explicit = self.eat("let");
            let assignment=matches!(&self.t().kind,K::Id(_)) && self.ts.get(self.pos+1).is_some_and(|t|matches!(&t.kind,K::Sym(x)if ["=",":","+=","-=","*="].contains(&x.as_str())));
            if explicit || assignment {
                let start = self.pos;
                let n = self.name()?;
                let name_span = self.span(start);
                binding_span = Some(name_span);
                let annotation = if self.eat(":") {
                    Some(self.ty()?)
                } else {
                    None
                };
                let value = if self.eat("=") {
                    self.expr(0)?
                } else {
                    let op = if self.eat("+=") {
                        "+"
                    } else if self.eat("-=") {
                        "-"
                    } else if self.eat("*=") {
                        "*"
                    } else {
                        return Err(self.err("代入演算子が必要です"));
                    };
                    let right = self.expr(0)?;
                    Expr {
                        line,
                        ty: None,
                        resolution: None,
                        span: self.span(start),
                        kind: E::Binary(
                            Box::new(Expr {
                                line,
                                ty: None,
                                resolution: None,
                                kind: E::Name(n.clone()),
                                span: name_span,
                            }),
                            op.into(),
                            Box::new(right),
                        ),
                    }
                };
                self.end_stmt()?;
                S::Assign {
                    name: n,
                    annotation,
                    value,
                    declare: explicit,
                }
            } else {
                let e = self.expr(0)?;
                self.end_stmt()?;
                S::Expr(e)
            }
        };
        Ok(Stmt {
            kind,
            line,
            binding_span,
            binding_type: None,
        })
    }
    fn pattern_binding(&mut self) -> Result<PatternBinding, String> {
        let start = self.pos;
        let name = self.name()?;
        Ok(PatternBinding {
            name: (name != "_").then_some(name),
            span: self.span(start),
            ty: None,
        })
    }
    // Pratt parser。演算子の優先順位を一か所に集め、曖昧な構文を避ける。
    fn expr(&mut self, min: u8) -> Result<Expr, String> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(self.err("式の入れ子が深すぎます"));
        }
        let line = self.t().line;
        let start = self.pos;
        let mut e = if self.eat("await") {
            Expr {
                line,
                ty: None,
                resolution: None,
                kind: E::Await(Box::new(self.expr(7)?)),
                span: self.span(start),
            }
        } else if self.eat("try") {
            Expr {
                line,
                ty: None,
                resolution: None,
                kind: E::Try(Box::new(self.expr(7)?)),
                span: self.span(start),
            }
        } else if self.eat("-") {
            Expr {
                line,
                ty: None,
                resolution: None,
                kind: E::Unary("-".into(), Box::new(self.expr(7)?)),
                span: self.span(start),
            }
        } else if self.eat("not") {
            Expr {
                line,
                ty: None,
                resolution: None,
                kind: E::Unary("not".into(), Box::new(self.expr(7)?)),
                span: self.span(start),
            }
        } else {
            let k = self.t().kind.clone();
            self.pos += 1;
            let kind = match k {
                K::Num(s) => {
                    if s.contains('.') {
                        E::Float(s)
                    } else {
                        E::Int(s)
                    }
                }
                K::Str(s) => E::Str(s),
                K::Id(s) if s == "true" || s == "True" => E::Bool(true),
                K::Id(s) if s == "false" || s == "False" => E::Bool(false),
                K::Id(s) if s == "None" || s == "null" => E::Null,
                K::Id(s) => E::Name(s),
                K::Sym(s) if s == "(" => {
                    let e = self.expr(0)?;
                    self.expect(")")?;
                    e.kind
                }
                K::Sym(s) if s == "[" => {
                    let mut a = vec![];
                    if !self.eat("]") {
                        loop {
                            a.push(self.expr(0)?);
                            if self.eat("]") {
                                break;
                            }
                            self.expect(",")?;
                        }
                    }
                    E::List(a)
                }
                _ => return Err(self.err("式が必要です")),
            };
            Expr {
                line,
                ty: None,
                resolution: None,
                kind,
                span: self.span(start),
            }
        };
        loop {
            if self.eat(".") {
                let f = self.name()?;
                e = Expr {
                    line,
                    ty: None,
                    resolution: None,
                    kind: E::Field(Box::new(e), f),
                    span: self.span(start),
                };
                continue;
            }
            // 型引数は呼び出し直前に限定。a[i]とf[T](x)は後続の ( で区別する。
            let saved = self.pos;
            let saved_depth = self.depth;
            let mut generics = vec![];
            if Self::callee_name(&e).is_some() && self.eat("[") {
                let attempt = (|| {
                    loop {
                        generics.push(self.ty()?);
                        if self.eat("]") {
                            break;
                        }
                        self.expect(",")?;
                    }
                    Ok::<_, String>(())
                })();
                if attempt.is_err() || !matches!(&self.t().kind,K::Sym(s)if s=="(") {
                    self.pos = saved;
                    self.depth = saved_depth;
                    generics.clear();
                }
            }
            if self.eat("(") {
                let n = Self::callee_name(&e).ok_or_else(|| self.err("呼び出し先は関数名です"))?;
                let mut args = vec![];
                let mut fields = vec![];
                if !self.eat(")") {
                    loop {
                        if matches!(&self.t().kind, K::Id(_))
                            && self
                                .ts
                                .get(self.pos + 1)
                                .is_some_and(|t| matches!(&t.kind,K::Sym(s)if s=="="))
                        {
                            let key = self.name()?;
                            self.expect("=")?;
                            fields.push((key, self.expr(0)?));
                        } else {
                            args.push(self.expr(0)?);
                        }
                        if self.eat(")") {
                            break;
                        }
                        self.expect(",")?;
                    }
                }
                if !args.is_empty() && !fields.is_empty() {
                    return Err(self.err("位置引数とclassフィールドは混在できません"));
                }
                if !fields.is_empty() && !generics.is_empty() {
                    return Err(self.err("classの型引数は未対応です"));
                }
                e = Expr {
                    line,
                    ty: None,
                    resolution: None,
                    kind: if fields.is_empty() {
                        E::Call(n, generics, args)
                    } else {
                        E::Record(n, fields)
                    },
                    span: self.span(start),
                };
                continue;
            }
            if self.eat("[") {
                let idx = self.expr(0)?;
                self.expect("]")?;
                e = Expr {
                    line,
                    ty: None,
                    resolution: None,
                    kind: E::Index(Box::new(e), Box::new(idx)),
                    span: self.span(start),
                };
                continue;
            }
            let op = match &self.t().kind {
                K::Sym(x) | K::Id(x) => x.clone(),
                _ => break,
            };
            let bp = match op.as_str() {
                "or" => 1,
                "and" => 2,
                "==" | "!=" => 3,
                "<" | ">" | "<=" | ">=" => 4,
                "+" | "-" => 5,
                "*" | "/" | "%" => 6,
                _ => break,
            };
            if bp < min {
                break;
            }
            self.pos += 1;
            let r = self.expr(bp + 1)?;
            e = Expr {
                line,
                ty: None,
                resolution: None,
                kind: E::Binary(Box::new(e), op, Box::new(r)),
                span: self.span(start),
            };
        }
        self.depth -= 1;
        Ok(e)
    }
}
