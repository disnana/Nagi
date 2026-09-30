use crate::ast::*;
use crate::lexer::{lex, Token, K};

pub fn parse(src: &str, high: bool) -> Result<Program, String> {
    if src.len() > 2_000_000 {
        return Err("source limit: 2 MB".into());
    }
    Parser {
        ts: lex(src, high)?,
        pos: 0,
        high,
        depth: 0,
    }
    .program()
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
            let n = self.name()?;
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
                while !self.ended() {
                    let n = self.name()?;
                    self.expect(":")?;
                    let t = self.ty()?;
                    self.end_stmt()?;
                    fields.push((n, t));
                }
                self.close()?;
                p.classes.push(Class { name, fields, line });
            } else {
                let asynchronous = self.eat("async");
                self.expect(if self.high { "def" } else { "fn" })?;
                let name = self.name()?;
                self.expect("(")?;
                let mut params = vec![];
                if !self.eat(")") {
                    loop {
                        let n = self.name()?;
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
                let body = self.block()?;
                p.functions.push(Function {
                    name,
                    params,
                    ret,
                    asynchronous,
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
        } else if self.eat("for") {
            let n = self.name()?;
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
                let n = self.name()?;
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
                    Expr {
                        line,
                        ty: None,
                        kind: E::Binary(
                            Box::new(Expr {
                                line,
                                ty: None,
                                kind: E::Name(n.clone()),
                            }),
                            op.into(),
                            Box::new(self.expr(0)?),
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
        Ok(Stmt { kind, line })
    }
    // Pratt parser。演算子の優先順位を一か所に集め、曖昧な構文を避ける。
    fn expr(&mut self, min: u8) -> Result<Expr, String> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(self.err("式の入れ子が深すぎます"));
        }
        let line = self.t().line;
        let mut e = if self.eat("await") {
            Expr {
                line,
                ty: None,
                kind: E::Await(Box::new(self.expr(7)?)),
            }
        } else if self.eat("try") {
            Expr {
                line,
                ty: None,
                kind: E::Try(Box::new(self.expr(7)?)),
            }
        } else if self.eat("-") {
            Expr {
                line,
                ty: None,
                kind: E::Unary("-".into(), Box::new(self.expr(7)?)),
            }
        } else if self.eat("not") {
            Expr {
                line,
                ty: None,
                kind: E::Unary("not".into(), Box::new(self.expr(7)?)),
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
                kind,
            }
        };
        loop {
            if self.eat(".") {
                let f = self.name()?;
                e = Expr {
                    line,
                    ty: None,
                    kind: E::Field(Box::new(e), f),
                };
                continue;
            }
            // 型引数は呼び出し直前に限定。a[i]とf[T](x)は後続の ( で区別する。
            let saved = self.pos;
            let mut generics = vec![];
            if matches!(e.kind, E::Name(_)) && self.eat("[") {
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
                    generics.clear();
                }
            }
            if self.eat("(") {
                let n = if let E::Name(n) = e.kind {
                    n
                } else {
                    return Err(self.err("呼び出し先は関数名です"));
                };
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
                e = Expr {
                    line,
                    ty: None,
                    kind: if fields.is_empty() {
                        E::Call(n, generics, args)
                    } else {
                        E::Record(n, fields)
                    },
                };
                continue;
            }
            if self.eat("[") {
                let idx = self.expr(0)?;
                self.expect("]")?;
                e = Expr {
                    line,
                    ty: None,
                    kind: E::Index(Box::new(e), Box::new(idx)),
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
                kind: E::Binary(Box::new(e), op, Box::new(r)),
            };
        }
        self.depth -= 1;
        Ok(e)
    }
}
