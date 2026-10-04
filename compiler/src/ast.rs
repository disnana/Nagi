use serde::{Deserialize, Serialize};
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
    pub fn is_future(&self) -> bool {
        self.0 == "Future" && self.1.len() == 1
    }
    pub fn is_async_function(&self) -> bool {
        self.0 == "fn" && self.1.last().is_some_and(Self::is_future)
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
    /// A capture-free function's output views need either a borrowing input
    /// or a static origin. Nested function signatures bind their own views.
    pub fn function_view_return_is_static(&self) -> bool {
        self.0 == "fn"
            && self.1.last().is_some_and(Self::contains_view)
            && !self.1[..self.1.len() - 1].iter().any(Self::contains_view)
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
        write!(f, "{}", crate::modules::display_symbol(&self.0))?;
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
    /// A loop-local reference to a non-Copy element. This is not a source type.
    BorrowedLocal,
    Module,
    Enum,
    Standard,
    ResourceConstant,
    ResourceField,
}
/// Half-open token range in the original source file. Import loading shifts
/// diagnostic lines, but keeps these ranges local to each file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    pub pattern: MatchPattern,
    pub body: Vec<Stmt>,
    pub line: usize,
}
#[derive(Clone, Debug)]
pub enum MatchPattern {
    Result {
        ok: bool,
        binding: PatternBinding,
    },
    Option {
        binding: Option<PatternBinding>,
    },
    Enum {
        name: String,
        bindings: Vec<PatternBinding>,
        span: Span,
    },
}
impl MatchPattern {
    pub fn bindings(&self) -> &[PatternBinding] {
        match self {
            Self::Result { binding, .. } => std::slice::from_ref(binding),
            Self::Option { binding } => binding.as_slice(),
            Self::Enum { bindings, .. } => bindings,
        }
    }
    pub fn bindings_mut(&mut self) -> &mut [PatternBinding] {
        match self {
            Self::Result { binding, .. } => std::slice::from_mut(binding),
            Self::Option { binding } => binding.as_mut_slice(),
            Self::Enum { bindings, .. } => bindings,
        }
    }
}
#[derive(Clone, Debug)]
pub struct PatternBinding {
    pub name: Option<String>,
    pub span: Span,
    pub ty: Option<Type>,
}
#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: S,
    pub line: usize,
    pub binding_span: Option<Span>,
    pub binding_type: Option<Type>,
    pub binding_borrowed: bool,
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
#[derive(Clone, Debug)]
pub struct Enum {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub line: usize,
}
#[derive(Clone, Debug)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub field_lines: Vec<usize>,
    pub line: usize,
    pub name_span: Span,
}
#[derive(Clone, Debug, Default)]
pub struct Program {
    pub imports: Vec<(String, usize)>,
    pub module_imports: Vec<ModuleImport>,
    pub modules: ModuleMetadata,
    pub classes: Vec<Class>,
    pub enums: Vec<Enum>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug)]
pub struct ModuleImport {
    pub path: String,
    pub source: ImportSource,
    pub line: usize,
    pub span: Span,
    pub kind: ImportKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportSource {
    File,
    Standard,
}

#[derive(Clone, Debug)]
pub enum ImportKind {
    Flat,
    Module { alias: String, alias_span: Span },
    Names(Vec<ImportName>),
}

#[derive(Clone, Debug)]
pub struct ImportName {
    pub name: String,
    pub alias: String,
    pub name_span: Span,
    pub alias_span: Span,
}

/// A canonical source file or compiler-registered standard ID, retained in Low.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModuleId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefKind {
    Class,
    Enum,
    Function,
    Resource,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefId {
    pub module: ModuleId,
    pub kind: DefKind,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleInfo {
    pub id: ModuleId,
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionInfo {
    pub id: DefId,
    pub symbol: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingTarget {
    Definition(DefId),
    Module(ModuleId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleBinding {
    pub module: ModuleId,
    pub name: String,
    pub target: BindingTarget,
    /// Own definitions and legacy flat imports may be exposed by a flat load.
    #[serde(default)]
    pub public: bool,
    pub line: usize,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleReference {
    pub module: ModuleId,
    pub line: usize,
    pub span: Span,
    pub target: DefId,
    pub spelling: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModuleMetadata {
    /// Generated Low's explicit intrinsic type namespace. Resolved ASTs clear it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builtin_types: Option<String>,
    pub root: Option<ModuleId>,
    pub modules: Vec<ModuleInfo>,
    pub definitions: Vec<DefinitionInfo>,
    pub bindings: Vec<ModuleBinding>,
    pub references: Vec<ModuleReference>,
}
