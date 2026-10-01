use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type(pub String, pub Vec<Type>);
impl Type {
    pub fn named(s: &str) -> Self {
        Self(s.into(), vec![])
    }
    pub fn generic(s: &str, args: Vec<Type>) -> Self {
        Self(s.into(), args)
    }
    pub fn is_view(&self) -> bool {
        self.0 == "view"
    }
    pub fn contains_view(&self) -> bool {
        // Plain function values capture no data. Their view parameters have
        // lifetimes bound by the function pointer, not by the value's scope.
        if self.0 == "fn" && !self.1.is_empty() {
            return false;
        }
        self.is_view() || self.1.iter().any(Self::contains_view)
    }
    pub fn is_copy(&self) -> bool {
        matches!(
            self.0.as_str(),
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
                | "unit"
                | "view"
                | "UUID"
                | "timestamp"
        ) || self.0 == "fn" && !self.1.is_empty()
    }
    pub fn inner(&self) -> Type {
        self.1
            .first()
            .cloned()
            .unwrap_or_else(|| Self::named("unit"))
    }
}
impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == "Option" {
            return write!(f, "{}?", self.inner());
        }
        write!(f, "{}", self.0)?;
        if !self.1.is_empty() {
            write!(
                f,
                "[{}]",
                self.1
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: E,
    pub line: usize,
    pub ty: Option<Type>,
    pub resolution: Option<NameResolution>,
    pub span: Span,
}
/// Preserve the checker's decision through Rust emission. Low keeps the name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameResolution {
    Builtin,
    Function,
    Local,
}
/// Half-open token range in the original source file. Import loading shifts
/// diagnostic lines, but keeps these ranges local to each file.
#[derive(Clone, Copy, Debug, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug)]
pub enum E {
    Int(String),
    Float(String),
    Str(String),
    Bool(bool),
    Null,
    Name(String),
    Binary(Box<Expr>, String, Box<Expr>),
    Unary(String, Box<Expr>),
    Call(String, Vec<Type>, Vec<Expr>),
    Record(String, Vec<(String, Expr)>),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    List(Vec<Expr>),
    Await(Box<Expr>),
    Try(Box<Expr>),
}
#[derive(Clone, Debug)]
pub enum S {
    Assign {
        name: String,
        annotation: Option<Type>,
        value: Expr,
        declare: bool,
    },
    Return(Option<Expr>),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    Match(Expr, Vec<MatchArm>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Vec<Stmt>),
    Scope(Vec<Stmt>),
    Spawn(Expr),
}
#[derive(Clone, Debug)]
pub struct MatchArm {
    pub ok: bool,
    pub binding: Option<String>,
    pub body: Vec<Stmt>,
    pub line: usize,
    pub binding_span: Span,
    pub binding_type: Option<Type>,
}
#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: S,
    pub line: usize,
    pub binding_span: Option<Span>,
    pub binding_type: Option<Type>,
}
#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub parameter_spans: Vec<Span>,
    pub ret: Type,
    pub asynchronous: bool,
    pub external: bool,
    pub body: Vec<Stmt>,
    pub attrs: Vec<(String, String)>,
    pub line: usize,
}
#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub field_lines: Vec<usize>,
    pub line: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Program {
    pub imports: Vec<(String, usize)>,
    pub classes: Vec<Class>,
    pub functions: Vec<Function>,
}
