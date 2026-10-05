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
    pub(crate) fn is_view_containing_list(&self) -> bool {
        self.0 == "List" && self.1.len() == 1 && self.contains_view()
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

/// One origin contributing to a checked owning value containing views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlowOrigin {
    pub binding: BindingId,
    pub fields: Vec<String>,
    pub owner_loan: bool,
    pub static_origin: bool,
}

/// Checker state for an owning value containing views at a statement edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ViewListBindingSnapshot {
    pub ty: Type,
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
    /// Value inputs observed at the checker's common content update point,
    /// including updates that add no new borrow origin.
    pub inputs: Vec<BindingId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlowValueDependency {
    pub target: BindingId,
    pub inputs: Vec<BindingId>,
}

/// Identity of a checked expression within this compilation. Diagnostic origin
/// and generated storage identity remain separate from this token identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ExprUseId {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}
impl ExprUseId {
    pub fn of(expr: &Expr) -> Self {
        Self {
            line: expr.line,
            start: expr.span.start,
            end: expr.span.end,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExprUseMode {
    Move,
    Copy,
    Borrow,
    BorrowMut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlowExprUse {
    pub expression: ExprUseId,
    pub binding: BindingId,
    pub mode: ExprUseMode,
}

/// Statement-boundary checker facts used to plan owning-view lowering. These
/// are deliberately absent from Low serialization and are recomputed by check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StmtFlowFacts {
    pub before: Vec<ViewListBindingSnapshot>,
    pub after: Vec<ViewListBindingSnapshot>,
    /// State after evaluating the If condition or consuming the Match scrutinee.
    pub branch_entry: Option<Vec<ViewListBindingSnapshot>>,
    /// Fixed-point loop header and state after evaluating its condition (or
    /// installing the for binding). The checked body carries this same pass.
    pub loop_entry: Option<FlowLoopEntry>,
    pub content_mutations: Vec<FlowContentMutation>,
    pub assignment: Option<FlowAssignment>,
    pub value_dependencies: Vec<FlowValueDependency>,
    pub return_observers: Vec<BindingId>,
    /// Operand roles decided by the checker, rather than reconstructed from
    /// builtin spelling or the generated Rust expression's context.
    pub expression_uses: Vec<FlowExprUse>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FlowLoopEntry {
    pub header: Vec<ViewListBindingSnapshot>,
    pub body: Vec<ViewListBindingSnapshot>,
}

/// All existing structured regions can carry private owning-view metadata.
pub(crate) fn supports_view_flow(statements: &[Stmt]) -> bool {
    statements.iter().all(|statement| match &statement.kind {
        S::If(_, then_body, else_body) => {
            supports_view_flow(then_body) && supports_view_flow(else_body)
        }
        S::Match(_, arms) => arms.iter().all(|arm| supports_view_flow(&arm.body)),
        S::While(_, body) | S::For(_, _, body) | S::Scope(body) => supports_view_flow(body),
        S::Assign { .. } | S::Return(_) | S::Expr(_) | S::Spawn(_) => true,
    })
}

/// Syntactic names referenced by an expression, also usable before checking.
/// BindingId snapshots decide which source binding a collected name denotes.
pub(crate) fn expression_names(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    expression_names_inner(expr, names, false);
}

/// Follow checked values that can carry views. Scalar observations such as
/// len(container) do not carry that container's borrowed lifetime onward.
pub(crate) fn view_value_names(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    expression_names_inner(expr, names, true);
}

fn expression_names_inner(
    expr: &Expr,
    names: &mut std::collections::HashSet<String>,
    value_only: bool,
) {
    if value_only && !expr.ty.as_ref().is_some_and(Type::contains_view) {
        return;
    }
    match &expr.kind {
        E::Name(name) => {
            names.insert(name.clone());
        }
        E::Binary(a, _, b) | E::Index(a, b) => {
            expression_names_inner(a, names, value_only);
            expression_names_inner(b, names, value_only);
        }
        E::Unary(_, value) | E::Field(value, _) | E::Await(value) | E::Try(value) => {
            expression_names_inner(value, names, value_only)
        }
        E::Call(name, _, values) => {
            // Local call targets are values too; a global function name is not
            // a local use. Before check, unresolved names are conservative.
            if expr.resolution.is_none()
                || matches!(
                    expr.resolution,
                    Some(NameResolution::Local | NameResolution::BorrowedLocal)
                )
            {
                names.insert(name.clone());
            }
            for value in values {
                expression_names_inner(value, names, value_only);
            }
        }
        E::List(values) => {
            for value in values {
                expression_names_inner(value, names, value_only);
            }
        }
        E::Record(_, fields) => {
            for (_, value) in fields {
                expression_names_inner(value, names, value_only);
            }
        }
        E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null => {}
    }
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
