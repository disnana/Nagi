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
    pub(crate) fn is_view_string_list(&self) -> bool {
        if self.0 != "List" || self.1.len() != 1 {
            return false;
        }
        let element = &self.1[0];
        element.0 == "view"
            && element.1.len() == 1
            && element.1[0].0 == "str"
            && element.1[0].1.is_empty()
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
    pub(crate) flow: Option<StmtFlowFacts>,
}

/// Stable identity of a source binding. Token offsets survive Rust-only name
/// rewriting, while the line disambiguates tokens from imported source files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct BindingId {
    pub line: usize,
    pub token: usize,
}

/// One origin contributing to the contents of a checked List[view[str]].
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlowOrigin {
    pub binding: BindingId,
    pub fields: Vec<String>,
    pub owner_loan: bool,
    pub static_origin: bool,
}

/// Checker state for one exact List[view[str]] binding at a statement edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ViewListBindingSnapshot {
    pub name: String,
    pub binding: BindingId,
    pub moved: bool,
    pub origins: Vec<FlowOrigin>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FlowAssignment {
    pub target: BindingId,
    /// True when evaluating this assignment's RHS consumed the previous value
    /// of the target binding before it was replaced.
    pub rhs_consumed_target: bool,
}

/// A checked mutation of an existing container's content borrow origins.
/// Unlike statement-edge snapshots, this survives replacement of the binding
/// later in the same expression. It does not represent replacing the Vec value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlowContentMutation {
    pub binding: BindingId,
    pub name: String,
    pub added_origins: Vec<FlowOrigin>,
}

/// Statement-boundary checker facts used to plan owning-view lowering. These
/// are deliberately absent from Low serialization and are recomputed by check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StmtFlowFacts {
    pub before: Vec<ViewListBindingSnapshot>,
    pub after: Vec<ViewListBindingSnapshot>,
    pub condition_after: Option<Vec<ViewListBindingSnapshot>>,
    pub content_mutations: Vec<FlowContentMutation>,
    pub assignment: Option<FlowAssignment>,
}

/// The initial owning-view flow pass handles straight-line statements and If
/// joins. A control-flow form outside that subset disables metadata for the
/// containing function until its checker state can be represented precisely.
pub(crate) fn supports_view_flow(statements: &[Stmt]) -> bool {
    statements.iter().all(|statement| match &statement.kind {
        S::If(_, then_body, else_body) => {
            supports_view_flow(then_body) && supports_view_flow(else_body)
        }
        S::Match(..) | S::While(..) | S::For(..) | S::Scope(..) => false,
        S::Assign { .. } | S::Return(_) | S::Expr(_) | S::Spawn(_) => true,
    })
}

/// Shared by ownership checking and Rust emission. A loop or scope can
/// continue; only explicit returns and exhaustive returning branches qualify.
pub(crate) fn block_returns(statements: &[Stmt]) -> bool {
    statements.iter().any(|statement| match &statement.kind {
        S::Return(_) => true,
        S::If(_, a, b) => block_returns(a) && block_returns(b),
        S::Match(_, arms) => !arms.is_empty() && arms.iter().all(|arm| block_returns(&arm.body)),
        _ => false,
    })
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
