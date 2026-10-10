//! The final check boundary owns the accepted AST and emission decisions.
use crate::{ast::*, source::SourceProvenance};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
};

/// Final Low/native integration and checking, with immutable emission facts.
///
/// Its fields and constructor are private to the checker. A parser result or
/// editor recovery AST is not evidence that this boundary has been crossed.
///
/// ```compile_fail,E0451
/// use nagic::checked::CheckedProgram;
/// let _ = CheckedProgram { program: nagic::ast::Program::default() };
/// ```
///
/// ```compile_fail,E0596
/// use nagic::{check, ast::Program, source::SourceProvenance};
/// let mut checked = check::finalize(Program::default(), Program::default(),
///     SourceProvenance::user_low_unmapped()).unwrap();
/// checked.program().functions.clear();
/// ```
pub struct CheckedProgram {
    program: Program,
    provenance: SourceProvenance,
    pub(crate) emission: EmissionPlan,
    _final_check_facts: FinalCheckFacts,
}
impl CheckedProgram {
    pub fn program(&self) -> &Program {
        &self.program
    }
    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }
    pub(crate) fn validate_for_emission(&self) -> Result<(), String> {
        self.emission
            .validate()
            .map_err(|e| FinalizeError::defect(e).to_string())
    }
    pub(super) fn seal(
        program: Program,
        provenance: SourceProvenance,
        facts: FinalCheckFacts,
    ) -> Result<Self, FinalizeError> {
        validate_facts(&program).map_err(FinalizeError::defect)?;
        // Reuse the ownership checker to verify the private proof against the
        // final source AST. This avoids a second, subtly different owner solver.
        let rebuilt =
            super::check_mode(&mut program.clone(), false).map_err(FinalizeError::defect)?;
        if rebuilt != facts {
            return Err(FinalizeError::defect(
                "missing or inconsistent final Future capture facts".into(),
            ));
        }
        let emission = EmissionPlan::build(&program, &provenance).map_err(FinalizeError::defect)?;
        Ok(Self {
            program,
            provenance,
            emission,
            _final_check_facts: facts,
        })
    }
}

// Private checker proof: no syntax field can assert safe task transfer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct FinalCheckFacts {
    pub functions: BTreeMap<String, BTreeMap<ExprUseId, FutureCapture>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FutureCapture {
    pub callee: String,
    pub resolution: NameResolution,
    pub arguments: Vec<FutureArgument>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FutureArgument {
    pub index: usize,
    pub expression: ExprUseId,
    pub ty: Type,
    pub passing: crate::stdlib::Passing,
    pub owners: Vec<(ExprUseId, BindingId)>,
}

#[cfg(test)]
impl CheckedProgram {
    pub(super) fn seal_rechecked(
        program: Program,
        provenance: SourceProvenance,
    ) -> Result<Self, FinalizeError> {
        validate_facts(&program).map_err(FinalizeError::defect)?;
        let facts =
            super::check_mode(&mut program.clone(), false).map_err(FinalizeError::defect)?;
        Self::seal(program, provenance, facts)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    UserError,
    CompilerDefect,
    Unclassified,
}
#[derive(Debug)]
pub struct FinalizeError {
    message: String,
    kind: FailureKind,
}
impl FinalizeError {
    pub fn kind(&self) -> FailureKind {
        self.kind
    }
    pub(super) fn checked(message: String, provenance: &SourceProvenance, mixed: bool) -> Self {
        let line = message
            .strip_prefix("line ")
            .and_then(|s| s.split(':').next())
            .and_then(|s| s.parse().ok());
        let kind = match provenance.origin(line.unwrap_or(0)).kind {
            crate::source::LoweringKind::GeneratedLow if mixed => FailureKind::Unclassified,
            crate::source::LoweringKind::GeneratedLow => FailureKind::CompilerDefect,
            crate::source::LoweringKind::Synthetic | crate::source::LoweringKind::Unknown => {
                FailureKind::Unclassified
            }
            _ => FailureKind::UserError,
        };
        Self { message, kind }
    }
    fn defect(message: String) -> Self {
        Self {
            message: format!("internal compiler error: {message}"),
            kind: FailureKind::CompilerDefect,
        }
    }
}
impl fmt::Display for FinalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for FinalizeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StaticRead {
    None,
    Literal,
    ErrorKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewRead {
    Native,
    Str,
    Slice,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CopyRead {
    Native,
    List,
    Owned,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompareRead {
    Value,
    Static,
    Str,
    Slice,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OperationPlan {
    NativeCall {
        path: String,
        parameters: Vec<crate::stdlib::Passing>,
        emit_type_arguments: bool,
        sql: Option<SqlArgumentPlan>,
    },
    /// Validated type-preserving transfer, realized as a Rust by-value result.
    /// View origins remain checked facts; the operand's place is not preserved.
    IdentityTransfer { argument: usize },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SqlArgumentPlan {
    pub index: usize,
    pub literal: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldRead {
    Method(String),
    Value(String),
    BorrowStr(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExpressionPlan {
    pub task: Option<TaskUse>,
    pub numeric_suffix: Option<String>,
    pub static_read: StaticRead,
    pub reference_is_view: bool,
    pub comparison: Option<(CompareRead, CompareRead)>,
    pub minimum: Option<String>,
    pub symbol_path: Option<String>,
    pub field: Option<FieldRead>,
    pub operation: Option<OperationPlan>,
    pub view: ViewRead,
    pub copy: CopyRead,
    pub json_string: bool,
    pub slice_string: bool,
    pub argument_resolution: Option<NameResolution>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExpressionKey {
    use_id: ExprUseId,
    kind: u8,
}
impl ExpressionKey {
    pub(crate) fn of(e: &Expr) -> Self {
        let kind = match e.kind {
            E::Int(_) => 0,
            E::Float(_) => 1,
            E::Str(_) => 2,
            E::Bool(_) => 3,
            E::Null => 4,
            E::Name(_) => 5,
            E::Binary(..) => 6,
            E::Unary(..) => 7,
            E::Call(..) => 8,
            E::Record(..) => 9,
            E::Field(..) => 10,
            E::Index(..) => 11,
            E::List(..) => 12,
            E::Await(..) => 13,
            E::Try(..) => 14,
        };
        Self {
            use_id: ExprUseId::of(e),
            kind,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ItemPlan {
    identity: String,
    pub copy: bool,
    pub serde: bool,
    pub debug: bool,
    pub readable_debug: bool,
    pub charge: Option<bool>,
    pub from_row: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainError {
    None,
    Display,
    Debug,
    Opaque,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IteratorRead {
    Range,
    Borrowed,
    Copied,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BlockPlan {
    pub shadow: Vec<(String, Type)>,
    pub statements: Vec<StatementPlan>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct StatementPlan {
    pub scope: Option<ScopeId>,
    pub task_bridge: bool,
    pub rebind: bool,
    pub annotate: bool,
    pub iterator: Option<IteratorRead>,
    pub spawn_result: bool,
    pub children: Vec<BlockPlan>,
}
pub(crate) struct FunctionPlan {
    pub origin: crate::source::SourceOrigin,
    pub flow: Option<crate::view_flow::Plan>,
    expected_flow_slots: Option<BTreeSet<String>>,
    pub rebind_parameters: BTreeSet<String>,
    pub lifetime: bool,
    pub error_type: Option<Type>,
    pub body: BlockPlan,
    pub expressions: BTreeMap<ExpressionKey, ExpressionPlan>,
}
pub(crate) struct EmissionPlan {
    pub program: Program,
    pub rust_types: BTreeMap<Type, String>,
    rust_type_count: usize,
    pub names: crate::rust_names::RustNames,
    pub classes: Vec<ItemPlan>,
    pub enums: Vec<ItemPlan>,
    pub functions: Vec<FunctionPlan>,
    pub native_paths: BTreeMap<String, String>,
    pub native_reexports: BTreeSet<String>,
    pub main_error: MainError,
}

pub(crate) fn unmapped_rust_type(t: &Type) -> String {
    render_type_at(t, 0, &[], &BTreeMap::new(), &BTreeSet::new())
}
fn render_type_at(
    t: &Type,
    depth: usize,
    raw_classes: &[Class],
    native_paths: &BTreeMap<String, String>,
    native_view_types: &BTreeSet<String>,
) -> String {
    if let Some(path) = native_paths.get(&t.0) {
        return if t.1.is_empty() {
            path.clone()
        } else {
            format!(
                "{path}<{}>",
                t.1.iter()
                    .map(|arg| render_type_at(
                        arg,
                        depth + 1,
                        raw_classes,
                        native_paths,
                        native_view_types
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
    }
    match t.0.as_str() {
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" | "bool" => {
            if raw_classes.iter().any(|class| class.name == t.0) {
                t.0.clone()
            } else {
                format!("::std::primitive::{}", t.0)
            }
        }
        "str" => "::std::string::String".into(),
        "bytes" => "::std::vec::Vec<::std::primitive::u8>".into(),
        "unit" => "()".into(),
        "Error" => "::nagi_runtime::Error".into(),
        "Html" => "::nagi_runtime::axum::response::Html<::std::string::String>".into(),
        "UUID" => "::nagi_runtime::Uuid".into(),
        "timestamp" => "::nagi_runtime::Timestamp".into(),
        "fn" if !t.1.is_empty() => {
            let signature = format!(
                "fn({}) -> {}",
                t.1[..t.1.len() - 1]
                    .iter()
                    .map(|arg| render_type_at(
                        arg,
                        depth + 1,
                        raw_classes,
                        native_paths,
                        native_view_types
                    ))
                    .collect::<Vec<_>>()
                    .join(", "),
                render_type_at(
                    t.1.last().unwrap(),
                    depth + 1,
                    raw_classes,
                    native_paths,
                    native_view_types
                )
            );
            // Nested function types bind their own views during recursion.
            // Only this function's inputs can bind its remaining view lifetimes.
            if t.1[..t.1.len() - 1].iter().any(Type::contains_view) {
                let lifetime = format!("'nagi_fn_{depth}");
                format!(
                    "for<{lifetime}> {}",
                    signature.replace("&'a ", &format!("&{lifetime} "))
                )
            } else if t.function_view_return_is_static() {
                signature.replace("&'a ", "&'static ")
            } else {
                signature
            }
        }
        "view" => {
            let a = t.inner();
            let native_view = {
                let mut element = &a;
                while element.0 == "owned" && element.1.len() == 1 {
                    element = &element.1[0];
                }
                native_view_types.contains(&element.0)
            };
            if native_view {
                return format!(
                    "&'a {}",
                    render_type_at(&a, depth + 1, raw_classes, native_paths, native_view_types)
                );
            }
            match a.0.as_str() {
                "str" => "&'a ::std::primitive::str".into(),
                "bytes" => "&'a [::std::primitive::u8]".into(),
                _ => format!(
                    "&'a [{}]",
                    render_type_at(&a, depth + 1, raw_classes, native_paths, native_view_types)
                ),
            }
        }
        "owned" => render_type_at(
            &t.inner(),
            depth + 1,
            raw_classes,
            native_paths,
            native_view_types,
        ),
        "shared" => format!(
            "::std::sync::Arc<{}>",
            render_type_at(
                &t.inner(),
                depth + 1,
                raw_classes,
                native_paths,
                native_view_types
            )
        ),
        "List" => format!(
            "::std::vec::Vec<{}>",
            render_type_at(
                &t.inner(),
                depth + 1,
                raw_classes,
                native_paths,
                native_view_types
            )
        ),
        "Map" => format!(
            "::std::collections::HashMap<{}, {}>",
            render_type_at(
                &t.1[0],
                depth + 1,
                raw_classes,
                native_paths,
                native_view_types
            ),
            render_type_at(
                &t.1[1],
                depth + 1,
                raw_classes,
                native_paths,
                native_view_types
            )
        ),
        "Option" | "Result" => format!(
            "::std::{}::{}<{}>",
            if t.0 == "Option" { "option" } else { "result" },
            t.0,
            t.1.iter()
                .map(|arg| render_type_at(
                    arg,
                    depth + 1,
                    raw_classes,
                    native_paths,
                    native_view_types
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => t.0.clone(),
    }
}
fn registered_resource(symbol: &str, modules: &ModuleMetadata) -> Option<crate::stdlib::Resource> {
    let resource = crate::stdlib::resource(symbol)?;
    (modules.definition(symbol)?.id == crate::stdlib::resource_id(resource)).then_some(resource)
}
fn native_view(ty: &Type, modules: &ModuleMetadata) -> Option<crate::stdlib::Resource> {
    registered_resource(&crate::stdlib::native_view_element(ty)?.0, modules)
}
fn resource_type(mut ty: &Type, modules: &ModuleMetadata) -> Option<crate::stdlib::Resource> {
    while matches!(ty.0.as_str(), "owned" | "shared" | "view") {
        ty = &ty.1[0];
    }
    registered_resource(&ty.0, modules)
}
fn copy_type(t: &Type, p: &Program, depth: usize) -> bool {
    if let Some(resource) = registered_resource(&t.0, &p.modules) {
        return crate::stdlib::resource_info(resource).copy;
    }
    if depth > 64 || matches!(t.0.as_str(), "str" | "bytes" | "Error" | "Html") {
        false
    } else if let Some(enumeration) = p.enums.iter().find(|d| d.name == t.0) {
        enumeration
            .variants
            .iter()
            .all(|v| v.fields.iter().all(|(_, t)| copy_type(t, p, depth + 1)))
    } else if t.is_copy() {
        true
    } else if matches!(t.0.as_str(), "Option" | "owned") {
        copy_type(&t.inner(), p, depth + 1)
    } else if let Some(class) = p.classes.iter().find(|d| d.name == t.0) {
        class.fields.iter().all(|(_, t)| copy_type(t, p, depth + 1))
    } else {
        false
    }
}
fn static_read(e: &Expr) -> StaticRead {
    match &e.kind {
        E::Str(_) => StaticRead::Literal,
        E::Call(name, _, _)
            if name == "error_kind" && e.resolution == Some(NameResolution::Builtin) =>
        {
            StaticRead::ErrorKind
        }
        _ => StaticRead::None,
    }
}
fn content(mut ty: &Type) -> Option<(&str, bool)> {
    while ty.0 == "owned" {
        ty = ty.1.first()?;
    }
    let borrowed = ty.is_view();
    if borrowed {
        ty = ty.1.first()?;
    }
    matches!(ty.0.as_str(), "str" | "bytes").then_some((ty.0.as_str(), borrowed))
}
fn comparison_read(e: &Expr, content: Option<&str>, equality: bool) -> CompareRead {
    if let Some(content) = content {
        if static_read(e) != StaticRead::None {
            return CompareRead::Static;
        }
        if e.ty
            .as_ref()
            .and_then(|t| self::content(t))
            .is_some_and(|(_, borrowed)| !borrowed)
        {
            return if content == "str" {
                CompareRead::Str
            } else {
                CompareRead::Slice
            };
        }
    }
    if equality && matches!(e.kind, E::Str(_)) {
        CompareRead::Static
    } else {
        CompareRead::Value
    }
}
fn expression_plan(e: &Expr, p: &Program) -> Result<ExpressionPlan, String> {
    let ty =
        e.ty.as_ref()
            .ok_or_else(|| "missing checked expression type".to_owned())?;
    if matches!(e.kind, E::Name(_) | E::Call(..)) && e.resolution.is_none() {
        return Err("missing checked name resolution".into());
    }
    let mut plan = ExpressionPlan {
        task: None,
        numeric_suffix: (matches!(e.kind, E::Int(_) | E::Float(_))
            && matches!(
                ty.0.as_str(),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64"
            ))
        .then(|| ty.0.clone()),
        static_read: static_read(e),
        reference_is_view: ty.is_view(),
        comparison: None,
        minimum: crate::constant_eval::signed_minimum(e).map(|t| t.0.clone()),
        symbol_path: None,
        field: None,
        operation: None,
        view: ViewRead::Slice,
        copy: CopyRead::Owned,
        json_string: false,
        slice_string: false,
        argument_resolution: e.resolution,
    };
    if e.resolution == Some(NameResolution::Standard) {
        let name = match &e.kind {
            E::Name(name) | E::Call(name, _, _) => name,
            _ => return Err("invalid checked standard expression".into()),
        };
        let resource = registered_resource(name, &p.modules);
        if let Some(resource) = resource {
            plan.symbol_path = Some(crate::stdlib::resource_info(resource).rust_path.into());
        } else {
            let op = crate::stdlib::operation(name).ok_or("missing checked standard operation")?;
            if p.modules.definition(name).map(|d| &d.id) != Some(&crate::stdlib::function_id(op)) {
                return Err("invalid checked standard operation identity".into());
            }
            let info = crate::stdlib::operation_info(op);
            let sql = if op == crate::stdlib::Operation::SqliteLiteral {
                let E::Call(_, _, args) = &e.kind else {
                    return Err("missing checked literal Query call".into());
                };
                let argument = args
                    .first()
                    .ok_or("missing checked literal Query argument")?;
                let E::Str(literal) = &argument.kind else {
                    return Err("invalid checked literal Query structure".into());
                };
                if argument.ty.as_ref() != Some(&Type::named("str")) {
                    return Err("invalid checked literal Query argument type".into());
                }
                Some(SqlArgumentPlan {
                    index: 0,
                    literal: literal.clone(),
                })
            } else {
                None
            };
            plan.operation = Some(match crate::stdlib::operation_semantics(op).emission {
                crate::stdlib::OperationEmission::NativeCall => {
                    plan.symbol_path = Some(info.rust_path.into());
                    OperationPlan::NativeCall {
                        path: info.rust_path.into(),
                        parameters: info.parameters.to_vec(),
                        emit_type_arguments: info.emit_type_arguments,
                        sql,
                    }
                }
                crate::stdlib::OperationEmission::IdentityTransfer { argument } => {
                    let E::Call(_, type_arguments, args) = &e.kind else {
                        return Err("invalid checked identity transfer".into());
                    };
                    if info.asynchronous
                        || !type_arguments.is_empty()
                        || args.len() != info.arity
                        || info.parameters.get(argument) != Some(&crate::stdlib::Passing::Move)
                        || args.get(argument).and_then(|input| input.ty.as_ref()) != Some(ty)
                        || crate::stdlib::operation_semantics(op).value_transfer
                            != (crate::stdlib::ValueTransfer::WholeValue { argument })
                    {
                        return Err("invalid checked identity transfer facts".into());
                    }
                    OperationPlan::IdentityTransfer { argument }
                }
            });
        }
    }
    match &e.kind {
        E::Binary(a, op, b) => {
            let matching_content = if ["==", "!=", "<", ">", "<=", ">="].contains(&op.as_str()) {
                a.ty.as_ref()
                    .and_then(|t| content(t))
                    .zip(b.ty.as_ref().and_then(|t| content(t)))
                    .filter(|((a, _), (b, _))| a == b)
                    .map(|((a, _), _)| a)
            } else {
                None
            };
            plan.comparison = Some((
                comparison_read(a, matching_content, matches!(op.as_str(), "==" | "!=")),
                comparison_read(b, matching_content, matches!(op.as_str(), "==" | "!=")),
            ));
        }
        E::Field(owner, name) if e.resolution == Some(NameResolution::Enum) => {
            let E::Name(symbol) = &owner.kind else {
                return Err("invalid checked enum owner".into());
            };
            if !p.enums.iter().any(|definition| {
                definition.name == *symbol
                    && definition
                        .variants
                        .iter()
                        .any(|variant| variant.name == *name && variant.fields.is_empty())
            }) {
                return Err("missing checked enum variant".into());
            }
            plan.symbol_path = Some(format!("{symbol}::{name}"));
        }
        E::Field(owner, name) if e.resolution == Some(NameResolution::ResourceConstant) => {
            let E::Name(symbol) = &owner.kind else {
                return Err("invalid checked constant owner".into());
            };
            let resource = registered_resource(symbol, &p.modules)
                .ok_or("missing checked constant resource")?;
            let constant = crate::stdlib::constant(resource, name)
                .ok_or("missing checked resource constant")?;
            plan.symbol_path = Some(format!(
                "{}::{}",
                crate::stdlib::resource_info(resource).rust_path,
                constant.native_name
            ));
        }
        E::Field(owner, name) if e.resolution == Some(NameResolution::ResourceField) => {
            let resource = resource_type(
                owner.ty.as_ref().ok_or("missing checked receiver type")?,
                &p.modules,
            )
            .ok_or("missing checked field resource")?;
            let field =
                crate::stdlib::field(resource, name).ok_or("missing checked resource field")?;
            plan.field = Some(
                if resource == crate::stdlib::Resource::SqliteFailure && field.ty.is_view() {
                    FieldRead::BorrowStr(field.accessor.into())
                } else if field.owned || resource == crate::stdlib::Resource::SqliteFailure {
                    FieldRead::Value(field.accessor.into())
                } else {
                    FieldRead::Method(field.accessor.into())
                },
            );
        }
        E::Call(_, _, args) if e.resolution == Some(NameResolution::Builtin) => {
            plan.view = if native_view(ty, &p.modules).is_some() {
                ViewRead::Native
            } else if args
                .first()
                .and_then(|a| a.ty.as_ref())
                .is_some_and(|t| t.0 == "str")
            {
                ViewRead::Str
            } else {
                ViewRead::Slice
            };
            plan.copy = if args
                .first()
                .and_then(|a| a.ty.as_ref())
                .and_then(|t| native_view(t, &p.modules))
                .is_some_and(|r| crate::stdlib::resource_info(r).copy)
            {
                CopyRead::Native
            } else if ty.0 == "List" {
                CopyRead::List
            } else {
                CopyRead::Owned
            };
            plan.json_string = args.first().and_then(|a| a.ty.as_ref()).is_some_and(|t| {
                t.0 == "str" || t == &Type::generic("view", vec![Type::named("str")])
            });
            plan.slice_string = args
                .first()
                .and_then(|a| a.ty.as_ref())
                .is_some_and(|t| t.inner().0 == "str");
        }
        _ => {}
    }
    Ok(plan)
}
pub(super) fn visit_expression(
    e: &Expr,
    visit: &mut impl FnMut(&Expr) -> Result<(), String>,
) -> Result<(), String> {
    visit(e)?;
    match &e.kind {
        E::Binary(a, _, b) | E::Index(a, b) => {
            visit_expression(a, visit)?;
            visit_expression(b, visit)?;
        }
        E::Field(_, _)
            if matches!(
                e.resolution,
                Some(NameResolution::Enum | NameResolution::ResourceConstant)
            ) => {}
        E::Unary(_, e) | E::Await(e) | E::Try(e) | E::Field(e, _) => visit_expression(e, visit)?,
        E::Call(_, _, args) | E::List(args) => {
            for arg in args {
                visit_expression(arg, visit)?;
            }
        }
        E::Record(_, fields) => {
            for (_, arg) in fields {
                visit_expression(arg, visit)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn visit_statement_expressions(
    s: &Stmt,
    visit: &mut impl FnMut(&Expr) -> Result<(), String>,
) -> Result<(), String> {
    match &s.kind {
        S::Assign { value, .. }
        | S::SpawnBind { value, .. }
        | S::Expr(value)
        | S::Spawn(value)
        | S::Return(Some(value))
        | S::Match(value, _)
        | S::If(value, _, _)
        | S::While(value, _)
        | S::For(_, value, _) => visit_expression(value, visit)?,
        S::Return(None) | S::Scope(_) => {}
    }
    Ok(())
}
fn visit_expressions(
    statements: &[Stmt],
    visit: &mut impl FnMut(&Expr) -> Result<(), String>,
) -> Result<(), String> {
    for statement in statements {
        visit_statement_expressions(statement, visit)?;
        match &statement.kind {
            S::If(_, a, b) => {
                visit_expressions(a, visit)?;
                visit_expressions(b, visit)?;
            }
            S::While(_, body) | S::For(_, _, body) | S::Scope(body) => {
                visit_expressions(body, visit)?
            }
            S::Match(_, arms) => {
                for arm in arms {
                    visit_expressions(&arm.body, visit)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn validate_facts(p: &Program) -> Result<(), String> {
    for f in &p.functions {
        if !f.external {
            validate_statement_tree(&f.body)?;
            visit_expressions(&f.body, &mut |e| {
                if e.ty.is_none() {
                    Err("missing checked expression type".into())
                } else {
                    Ok(())
                }
            })?;
        }
    }
    Ok(())
}

// Check only facts already recorded by the checker, without reconstructing
// types, control flow, or ownership after the final check.
fn validate_statement_facts(statement: &Stmt) -> Result<(), String> {
    if matches!(
        statement.kind,
        S::Scope(_) | S::Spawn(_) | S::SpawnBind { .. }
    ) && statement.task.scope.is_none()
    {
        return Err("missing checked scope identity".into());
    }
    if let S::SpawnBind { value, .. } = &statement.kind {
        let output = value
            .ty
            .as_ref()
            .filter(|t| t.is_future())
            .ok_or("missing checked spawn Future")?
            .inner();
        let wanted = crate::stdlib::resource_type(crate::stdlib::Resource::Task, vec![output]);
        if statement.binding_type.as_ref() != Some(&wanted) {
            return Err("inconsistent checked Task binding".into());
        }
    }
    match &statement.kind {
        S::Assign { annotation, .. } | S::SpawnBind { annotation, .. } => {
            let annotation = annotation
                .as_ref()
                .ok_or("missing checked declaration type")?;
            let binding = statement
                .binding_type
                .as_ref()
                .ok_or("missing checked binding type")?;
            if annotation != binding {
                return Err("inconsistent checked declaration and binding types".into());
            }
        }
        S::For(..) if statement.binding_type.is_none() => {
            return Err("missing checked loop binding type".into())
        }
        S::Match(_, arms)
            if arms
                .iter()
                .flat_map(|arm| arm.pattern.bindings())
                .any(|binding| binding.ty.is_none()) =>
        {
            return Err("missing checked pattern binding type".into());
        }
        _ => {}
    }
    Ok(())
}

fn validate_statement_tree(statements: &[Stmt]) -> Result<(), String> {
    for statement in statements {
        validate_statement_facts(statement)?;
        match &statement.kind {
            S::If(_, yes, no) => {
                validate_statement_tree(yes)?;
                validate_statement_tree(no)?;
            }
            S::While(_, body) | S::For(_, _, body) | S::Scope(body) => {
                validate_statement_tree(body)?
            }
            S::Match(_, arms) => {
                for arm in arms {
                    validate_statement_tree(&arm.body)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn assigned_views(
    statements: &[Stmt],
    bindings: &BTreeMap<String, Type>,
    assigned: &mut BTreeSet<String>,
) {
    for s in statements {
        match &s.kind {
            S::Assign { name, declare, .. } | S::SpawnBind { name, declare, .. }
                if !declare && bindings.contains_key(name) =>
            {
                assigned.insert(name.clone());
            }
            S::If(_, a, b) => {
                assigned_views(a, bindings, assigned);
                assigned_views(b, bindings, assigned);
            }
            S::While(_, b) | S::Scope(b) => assigned_views(b, bindings, assigned),
            S::For(name, _, b) => {
                let mut inner = bindings.clone();
                inner.remove(name);
                assigned_views(b, &inner, assigned);
            }
            S::Match(_, arms) => {
                for arm in arms {
                    let mut inner = bindings.clone();
                    for binding in arm.pattern.bindings() {
                        if let Some(name) = &binding.name {
                            inner.remove(name);
                        }
                    }
                    assigned_views(&arm.body, &inner, assigned);
                }
            }
            _ => {}
        }
    }
}
fn block_plan(
    statements: &[Stmt],
    outer: &BTreeMap<String, Type>,
    flow: Option<&crate::view_flow::Block>,
    child: bool,
) -> BlockPlan {
    let terminal = flow.map_or_else(
        || crate::ast::block_returns(statements),
        |block| block.terminal,
    );
    let mut bindings = outer.clone();
    let mut result = BlockPlan::default();
    if child && crate::ast::block_returns(statements) {
        let mut assigned = BTreeSet::new();
        assigned_views(statements, &bindings, &mut assigned);
        result.shadow = assigned
            .into_iter()
            .map(|name| {
                let ty = bindings[&name].clone();
                (name, ty)
            })
            .collect();
    }
    for (index, original) in statements.iter().enumerate() {
        let node = flow.map(|block| &block.statements[index]);
        let statement = node.map_or(original, |node| &node.stmt);
        let mut plan = StatementPlan {
            scope: statement.task.scope,
            task_bridge: statement.task.bridge,
            ..Default::default()
        };
        match &statement.kind {
            S::Assign {
                name,
                annotation,
                declare,
                ..
            }
            | S::SpawnBind {
                name,
                annotation,
                declare,
                ..
            } => {
                plan.rebind = terminal && annotation.as_ref().is_some_and(Type::is_view);
                plan.annotate = (*declare || plan.rebind)
                    && !annotation.as_ref().is_some_and(Type::is_async_function);
                if let Some(ty) = annotation.as_ref().filter(|t| t.is_view()) {
                    bindings.insert(name.clone(), ty.clone());
                }
            }
            S::If(_, a, b) => {
                plan.children
                    .push(block_plan(a, &bindings, node.map(|n| &n.children[0]), true));
                plan.children
                    .push(block_plan(b, &bindings, node.map(|n| &n.children[1]), true));
            }
            S::While(_, b) => {
                plan.children
                    .push(block_plan(b, &bindings, node.map(|n| &n.children[0]), true))
            }
            S::For(name, expr, b) => {
                plan.iterator = Some(if expr.ty.as_ref().is_some_and(|t| t.0 == "Range") {
                    IteratorRead::Range
                } else if statement.binding_borrowed {
                    IteratorRead::Borrowed
                } else {
                    IteratorRead::Copied
                });
                let mut inner = bindings.clone();
                inner.remove(name);
                if let Some(t) = statement.binding_type.as_ref().filter(|t| t.is_view()) {
                    inner.insert(name.clone(), t.clone());
                }
                plan.children
                    .push(block_plan(b, &inner, node.map(|n| &n.children[0]), true));
            }
            S::Match(_, arms) => {
                for (i, arm) in arms.iter().enumerate() {
                    let mut inner = bindings.clone();
                    for binding in arm.pattern.bindings() {
                        if let Some(name) = &binding.name {
                            inner.remove(name);
                            if let Some(t) = binding.ty.as_ref().filter(|t| t.is_view()) {
                                inner.insert(name.clone(), t.clone());
                            }
                        }
                    }
                    plan.children.push(block_plan(
                        &arm.body,
                        &inner,
                        node.map(|n| &n.children[i]),
                        true,
                    ));
                }
            }
            S::Spawn(e) => {
                plan.spawn_result = e.ty.as_ref().is_some_and(|t| t.inner().0 == "Result")
            }
            S::Scope(b) => plan.children.push(block_plan(
                b,
                &bindings,
                node.map(|n| &n.children[0]),
                false,
            )),
            _ => {}
        }
        result.statements.push(plan);
    }
    result
}
fn flow_expressions(
    block: &crate::view_flow::Block,
    visit: &mut impl FnMut(&Expr) -> Result<(), String>,
) -> Result<(), String> {
    for node in &block.statements {
        visit_statement_expressions(&node.stmt, visit)?;
        for child in &node.children {
            flow_expressions(child, visit)?;
        }
    }
    Ok(())
}
impl EmissionPlan {
    fn validate(&self) -> Result<(), String> {
        validate_facts(&self.program)?;
        if self.rust_types.len() != self.rust_type_count {
            return Err("missing checked Rust type plan".into());
        }
        if self.classes.len() != self.program.classes.len()
            || self.enums.len() != self.program.enums.len()
            || self.functions.len() != self.program.functions.len()
        {
            return Err("missing checked item plan".into());
        }
        let classes: HashMap<_, _> = self
            .program
            .classes
            .iter()
            .map(|item| (item.name.clone(), item.clone()))
            .collect();
        let enums: HashMap<_, _> = self
            .program
            .enums
            .iter()
            .map(|item| (item.name.clone(), item.clone()))
            .collect();
        for (name, plan) in self
            .program
            .classes
            .iter()
            .map(|item| &item.name)
            .zip(&self.classes)
            .chain(
                self.program
                    .enums
                    .iter()
                    .map(|item| &item.name)
                    .zip(&self.enums),
            )
        {
            if &plan.identity != name
                || plan.debug
                    != crate::capabilities::debug_supported(&Type::named(name), &classes, &enums)
            {
                return Err("inconsistent checked item Debug plan".into());
            }
        }
        fn block(
            statements: &[Stmt],
            plan: &BlockPlan,
            flow: Option<&crate::view_flow::Block>,
        ) -> Result<(), String> {
            if statements.len() != plan.statements.len()
                || flow.is_some_and(|f| f.statements.len() != statements.len())
            {
                return Err("missing checked statement plan".into());
            }
            for (i, original) in statements.iter().enumerate() {
                let node = flow.map(|f| &f.statements[i]);
                let s = node.map_or(original, |n| &n.stmt);
                validate_statement_facts(s)?;
                let children: Vec<&[Stmt]> = match &s.kind {
                    S::If(_, a, b) => vec![a, b],
                    S::While(_, b) | S::For(_, _, b) | S::Scope(b) => vec![b],
                    S::Match(_, arms) => arms.iter().map(|a| a.body.as_slice()).collect(),
                    _ => vec![],
                };
                if children.len() != plan.statements[i].children.len()
                    || node.is_some_and(|n| n.children.len() != children.len())
                {
                    return Err("missing checked child plan".into());
                }
                for (j, child) in children.iter().enumerate() {
                    block(
                        child,
                        &plan.statements[i].children[j],
                        node.map(|n| &n.children[j]),
                    )?;
                }
            }
            Ok(())
        }
        for (f, plan) in self.program.functions.iter().zip(&self.functions) {
            if f.external {
                continue;
            }
            match (&plan.expected_flow_slots, &plan.flow) {
                (Some(expected), Some(flow)) if expected != &flow.storage_slots => {
                    return Err("missing checked storage slots".into())
                }
                (Some(_), None) | (None, Some(_)) => return Err("missing checked flow plan".into()),
                _ => {}
            }
            block(&f.body, &plan.body, plan.flow.as_ref().map(|f| &f.body))?;
            let mut validate = |e: &Expr| {
                if !plan.expressions.contains_key(&ExpressionKey::of(e)) {
                    return Err("missing checked expression plan".into());
                }
                let actual = &plan.expressions[&ExpressionKey::of(e)];
                let expected = expression_plan(e, &self.program)?;
                if actual.field != expected.field {
                    return Err("inconsistent checked resource field plan".into());
                }
                if actual.operation != expected.operation {
                    return Err("inconsistent checked native operation/SQL argument plan".into());
                }
                if let (Some(flow), E::Name(name)) = (&plan.flow, &e.kind) {
                    if flow.storage_slots.contains(name)
                        && !flow.expression_uses.contains_key(&ExprUseId::of(e))
                    {
                        return Err("missing checked operand role".into());
                    }
                }
                Ok(())
            };
            visit_expressions(&f.body, &mut validate)?;
            if let Some(flow) = &plan.flow {
                flow_expressions(&flow.body, &mut validate)?;
            }
        }
        Ok(())
    }
    fn build(original: &Program, provenance: &SourceProvenance) -> Result<Self, String> {
        let names = crate::rust_names::RustNames::new(original);
        let program = names.program(original);
        let p = &program;
        let classes: HashMap<_, _> = p
            .classes
            .iter()
            .map(|c| (c.name.clone(), c.clone()))
            .collect();
        let enums: HashMap<_, _> = p
            .enums
            .iter()
            .map(|c| (c.name.clone(), c.clone()))
            .collect();
        let actor = p.modules.definitions.iter().any(|d| {
            d.id.module.0 == crate::stdlib::module_info(crate::stdlib::StandardModule::Actor).id
                && crate::stdlib::definition(&d.id)
        });
        let class_plans = p
            .classes
            .iter()
            .map(|c| ItemPlan {
                identity: c.name.clone(),
                copy: c.fields.iter().all(|(_, t)| copy_type(t, p, 0)),
                serde: c
                    .fields
                    .iter()
                    .all(|(_, t)| crate::capabilities::serde_type(t, &classes, &enums)),
                debug: crate::capabilities::debug_supported(
                    &Type::named(&c.name),
                    &classes,
                    &enums,
                ),
                readable_debug: p.modules.definition(&c.name).is_some()
                    || names.original(&c.name) != c.name
                    || c.fields.iter().any(|(n, _)| names.original(n) != n),
                charge: (actor
                    && crate::capabilities::charge_type_supported(
                        &Type::named(&c.name),
                        &classes,
                        &enums,
                    )
                    .is_ok())
                .then(|| {
                    crate::capabilities::charge_inline_only(&Type::named(&c.name), &classes, &enums)
                }),
                from_row: crate::capabilities::generated_row_supported(
                    c,
                    &classes,
                    p.modules.root.is_some(),
                ),
            })
            .collect();
        let enum_plans = p
            .enums
            .iter()
            .map(|e| ItemPlan {
                identity: e.name.clone(),
                copy: e
                    .variants
                    .iter()
                    .all(|v| v.fields.iter().all(|(_, t)| copy_type(t, p, 0))),
                serde: false,
                debug: crate::capabilities::debug_supported(
                    &Type::named(&e.name),
                    &classes,
                    &enums,
                ),
                readable_debug: e.variants.iter().any(|v| {
                    names.original(&v.name) != v.name
                        || v.fields.iter().any(|(n, _)| names.original(n) != n)
                }),
                charge: (actor
                    && crate::capabilities::charge_type_supported(
                        &Type::named(&e.name),
                        &classes,
                        &enums,
                    )
                    .is_ok())
                .then(|| {
                    crate::capabilities::charge_inline_only(&Type::named(&e.name), &classes, &enums)
                }),
                from_row: false,
            })
            .collect();
        let mut functions = Vec::new();
        for (index, f) in p.functions.iter().enumerate() {
            let flow = if f.external {
                None
            } else {
                crate::view_flow::plan(f)?
            };
            let ordered = f
                .params
                .iter()
                .any(|(_, t)| t.contains_view() && !copy_type(t, p, 0));
            let rebind_parameters = f
                .params
                .iter()
                .filter(|(n, t)| {
                    !flow
                        .as_ref()
                        .is_some_and(|f| f.parameter_actions.contains_key(n))
                        && (t.contains_view() || ordered && !copy_type(t, p, 0))
                })
                .map(|(n, _)| n.clone())
                .collect();
            let bindings = f
                .params
                .iter()
                .filter(|(_, t)| t.is_view())
                .cloned()
                .collect();
            let body = block_plan(&f.body, &bindings, flow.as_ref().map(|p| &p.body), false);
            fn task_uses(
                statements: &[Stmt],
                current: Option<(ScopeId, bool)>,
                uses: &mut BTreeMap<ExprUseId, TaskUse>,
            ) -> Result<(), String> {
                for s in statements {
                    for use_ in &s.task.uses {
                        if current != Some((use_.scope, true)) {
                            return Err("invalid checked Task scope use".into());
                        }
                        if uses
                            .insert(use_.expression, *use_)
                            .is_some_and(|old| old != *use_)
                        {
                            return Err("conflicting checked Task use".into());
                        }
                    }
                    match &s.kind {
                        S::Scope(body) => {
                            let scope = s.task.scope.ok_or("missing checked scope identity")?;
                            if s.task.bridge != scope_has_task_binding(body) {
                                return Err("inconsistent checked Task scope bridge".into());
                            }
                            task_uses(body, Some((scope, s.task.bridge)), uses)?;
                        }
                        S::SpawnBind { .. }
                            if current != s.task.scope.map(|scope| (scope, true)) =>
                        {
                            return Err("invalid checked Task spawn scope".into())
                        }
                        S::Spawn(_)
                            if current != s.task.scope.map(|scope| (scope, s.task.bridge)) =>
                        {
                            return Err("invalid checked legacy spawn scope".into())
                        }
                        S::If(_, a, b) => {
                            task_uses(a, current, uses)?;
                            task_uses(b, current, uses)?;
                        }
                        S::While(_, b) | S::For(_, _, b) => task_uses(b, current, uses)?,
                        S::Match(_, arms) => {
                            for a in arms {
                                task_uses(&a.body, current, uses)?;
                            }
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            let mut tasks = BTreeMap::new();
            task_uses(&f.body, None, &mut tasks)?;
            let mut expressions = BTreeMap::new();
            let mut add = |e: &Expr| {
                let key = ExpressionKey::of(e);
                let mut plan = expression_plan(e, p)?;
                plan.task = tasks.get(&ExprUseId::of(e)).copied();
                let requires_task = match &e.kind {
                    E::Await(x)
                        if x.ty.as_ref().is_some_and(|t| {
                            registered_resource(&t.0, &p.modules)
                                == Some(crate::stdlib::Resource::Task)
                        }) =>
                    {
                        Some(TaskAction::Receive)
                    }
                    E::Call(n, _, _)
                        if e.resolution == Some(NameResolution::Standard)
                            && crate::stdlib::operation(n)
                                == Some(crate::stdlib::Operation::TaskDiscard) =>
                    {
                        Some(TaskAction::Discard)
                    }
                    _ => None,
                };
                if requires_task != plan.task.map(|use_| use_.action) {
                    return Err("missing or inconsistent checked Task use plan".into());
                }
                if let Some(old) = expressions.insert(key, plan.clone()) {
                    if old != plan {
                        return Err(format!(
                            "conflicting checked expression identity at line {}",
                            e.line
                        ));
                    }
                }
                Ok(())
            };
            visit_expressions(&f.body, &mut add)?;
            if let Some(flow) = &flow {
                flow_expressions(&flow.body, &mut add)?;
            }
            functions.push(FunctionPlan {
                origin: provenance.function_origin(&original.functions[index]),
                expected_flow_slots: flow.as_ref().map(|flow| flow.storage_slots.clone()),
                flow,
                rebind_parameters,
                lifetime: f.params.iter().any(|(_, t)| t.contains_view()) || f.ret.contains_view(),
                error_type: (f.ret.0 == "Result").then(|| f.ret.1[1].clone()),
                body,
                expressions,
            });
        }
        let mut native_paths = BTreeMap::new();
        let mut native_view_types = BTreeSet::new();
        let mut native_reexports = BTreeSet::new();
        for d in &p.modules.definitions {
            if let Some(r) = registered_resource(&d.symbol, &p.modules) {
                // Deprecated symbols remain resolvable only for migration diagnostics.
                if r != crate::stdlib::Resource::Principal {
                    native_reexports.insert(d.symbol.clone());
                }
                native_paths.insert(
                    d.symbol.clone(),
                    crate::stdlib::resource_info(r).rust_path.into(),
                );
                if crate::stdlib::native_view_element(&Type::generic(
                    "view",
                    vec![Type::named(&d.symbol)],
                ))
                .is_some()
                {
                    native_view_types.insert(d.symbol.clone());
                }
            } else if let Some(op) = crate::stdlib::operation(&d.symbol) {
                if d.id == crate::stdlib::function_id(op) {
                    // Scope-dependent discard and failure getters are checked
                    // operations, not Rust module items. Calls use sealed plans;
                    // associated methods cannot be reexported with `pub use`.
                    if !matches!(
                        op,
                        crate::stdlib::Operation::Html
                            | crate::stdlib::Operation::TaskDiscard
                            | crate::stdlib::Operation::TaskKind
                            | crate::stdlib::Operation::TaskMessage
                    ) {
                        native_reexports.insert(d.symbol.clone());
                    }
                    native_paths.insert(
                        d.symbol.clone(),
                        crate::stdlib::operation_info(op).rust_path.into(),
                    );
                }
            }
        }
        let mut rust_types = BTreeMap::new();
        fn add(t: &Type, out: &mut BTreeSet<Type>) {
            out.insert(t.clone());
            for inner in &t.1 {
                add(inner, out);
            }
        }
        fn statements(ss: &[Stmt], out: &mut BTreeSet<Type>) {
            for s in ss {
                if let Some(t) = &s.binding_type {
                    add(t, out);
                }
                match &s.kind {
                    S::Assign {
                        annotation: Some(t),
                        ..
                    }
                    | S::SpawnBind {
                        annotation: Some(t),
                        ..
                    } => add(t, out),
                    S::If(_, a, b) => {
                        statements(a, out);
                        statements(b, out);
                    }
                    S::While(_, b) | S::For(_, _, b) | S::Scope(b) => statements(b, out),
                    S::Match(_, arms) => {
                        for a in arms {
                            for b in a.pattern.bindings() {
                                if let Some(t) = &b.ty {
                                    add(t, out);
                                }
                            }
                            statements(&a.body, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut types = BTreeSet::new();
        for c in &p.classes {
            for (_, t) in &c.fields {
                add(t, &mut types);
            }
        }
        for e in &p.enums {
            for v in &e.variants {
                for (_, t) in &v.fields {
                    add(t, &mut types);
                }
            }
        }
        for f in &p.functions {
            add(&f.ret, &mut types);
            for (_, t) in &f.params {
                add(t, &mut types);
            }
            statements(&f.body, &mut types);
            visit_expressions(&f.body, &mut |e| {
                if let Some(t) = &e.ty {
                    add(t, &mut types);
                }
                if let E::Call(_, ts, _) = &e.kind {
                    for t in ts {
                        add(t, &mut types);
                    }
                }
                Ok(())
            })?;
        }
        for t in types {
            let text = render_type_at(
                &t,
                0,
                if p.modules.root.is_none() {
                    &p.classes
                } else {
                    &[]
                },
                &native_paths,
                &native_view_types,
            );
            rust_types.insert(t, text);
        }
        let main_error =
            p.functions
                .iter()
                .find(|f| f.name == "main")
                .map_or(MainError::None, |f| {
                    let mut ret = &f.ret;
                    while ret.0 == "owned" {
                        ret = &ret.1[0];
                    }
                    if ret.0 != "Result" {
                        return MainError::None;
                    }
                    let mut error = &ret.1[1];
                    while error.0 == "owned" {
                        error = &error.1[0];
                    }
                    if crate::capabilities::debug_supported(error, &classes, &enums) {
                        if error.0 == "Error" {
                            MainError::Display
                        } else {
                            MainError::Debug
                        }
                    } else {
                        MainError::Opaque
                    }
                });
        let rust_type_count = rust_types.len();
        Ok(Self {
            program,
            rust_types,
            rust_type_count,
            names,
            classes: class_plans,
            enums: enum_plans,
            functions,
            native_paths,
            native_reexports,
            main_error,
        })
    }
}

#[cfg(test)]
#[path = "checked_tests.rs"]
mod tests;
