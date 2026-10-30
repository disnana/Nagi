//! Initial, deliberately bounded Go backend.
//!
//! The frontend and final checker remain shared with the Rust backend.  This
//! module accepts only the G1 subset and rejects everything else before `go`
//! is invoked, so target compiler failures cannot silently define Nagi
//! semantics.
use crate::{
    ast::{Expr, Function, MatchPattern, NameResolution, Program, Stmt, Type, E, S},
    checked::{
        ArgumentCaptureFacts, ArgumentRelease, CaptureStorageId, CheckedProgram, EnumFacts,
        EnumVariantFact, ExpressionKey, FinalCheckFacts, FutureCapture, OperationPlan,
    },
    project::BuildProfile,
    source::Sources,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);

pub(crate) struct BuildResult {
    pub binary: PathBuf,
    pub go_ms: f64,
    pub publish_ms: f64,
    pub toolchain: String,
}

fn error(line: usize, message: impl std::fmt::Display) -> String {
    format!("line {line}: Go backend: {message}")
}

fn ident(raw: &str) -> String {
    // Hex-encode the UTF-8 spelling instead of replacing punctuation.  Module
    // qualification and Unicode identifiers must never collapse to the same
    // Go identifier, and user names must not collide with Go keywords or our
    // generated temporaries.
    let mut value = String::with_capacity(5 + raw.len() * 2);
    value.push_str("nagi_");
    for byte in raw.as_bytes() {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02x}").expect("String writes cannot fail");
    }
    value
}

fn function_name(name: &str) -> String {
    if name == "main" {
        "nagiMain".into()
    } else {
        format!("nagiFn_{}", ident(name))
    }
}

fn display_symbol(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn class_name(name: &str) -> String {
    format!("nagiType_{}", ident(name))
}

fn enum_name(name: &str) -> String {
    format!("nagiEnum_{}", ident(name))
}

fn enum_variant_name(fact: &EnumVariantFact) -> String {
    format!(
        "nagiVariant_{}_{}",
        ident(&fact.enumeration),
        ident(&fact.variant)
    )
}

fn integer_type(ty: &Type) -> Option<(bool, &'static str)> {
    Some(match ty.0.as_str() {
        "i8" => (true, "int8(-128)"),
        "i16" => (true, "int16(-32768)"),
        "i32" => (true, "int32(-2147483648)"),
        "i64" => (true, "int64(-9223372036854775808)"),
        "u8" | "u16" | "u32" | "u64" => (false, ""),
        _ => return None,
    })
}

fn negative_boundary(ty: &Type, literal: &str) -> Option<&'static str> {
    let magnitude = literal.parse::<u128>().ok()?;
    Some(match (ty.0.as_str(), magnitude) {
        ("i8", 128) => "int8(-128)",
        ("i16", 32_768) => "int16(-32768)",
        ("i32", 2_147_483_648) => "int32(-2147483648)",
        ("i64", 9_223_372_036_854_775_808) => "int64(-9223372036854775808)",
        _ => return None,
    })
}

fn decimal_integer_literal(literal: &str, line: usize) -> Result<String, String> {
    // Nagi integer literals are decimal.  Go interprets a leading zero as
    // octal, so carrying the checked source spelling across can either change
    // the value (`010`) or make valid Nagi fail to compile (`08`).
    literal
        .parse::<u128>()
        .map(|value| value.to_string())
        .map_err(|problem| error(line, format!("invalid checked integer literal: {problem}")))
}

fn float_type(ty: &Type) -> Option<u8> {
    match ty.0.as_str() {
        "f32" => Some(32),
        "f64" => Some(64),
        _ => None,
    }
}

fn go_type(
    ty: &Type,
    classes: &BTreeSet<String>,
    enums: &BTreeSet<String>,
    line: usize,
) -> Result<String, String> {
    if ty.is_async_function() {
        let (result, arguments) =
            ty.1.split_last()
                .expect("async function type has a Future result");
        let result = result
            .1
            .first()
            .expect("async function result is a checked Future");
        let arguments = arguments
            .iter()
            .map(|argument| go_type(argument, classes, enums, line))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        return Ok(format!(
            "func({arguments}) *nagiFuture[{}]",
            go_type(result, classes, enums, line)?
        ));
    }
    let primitive = match ty.0.as_str() {
        "i8" => Some("int8"),
        "i16" => Some("int16"),
        "i32" => Some("int32"),
        "i64" => Some("int64"),
        "u8" => Some("uint8"),
        "u16" => Some("uint16"),
        "u32" => Some("uint32"),
        "u64" => Some("uint64"),
        "f32" => Some("float32"),
        "f64" => Some("float64"),
        "bool" => Some("bool"),
        "str" => Some("string"),
        "bytes" => Some("[]byte"),
        "unit" => Some("nagiUnit"),
        "Error" => Some("nagiError"),
        _ => None,
    };
    if let Some(primitive) = primitive {
        if !ty.1.is_empty() {
            return Err(error(line, format!("type {ty} has unexpected arguments")));
        }
        return Ok(primitive.into());
    }
    if classes.contains(&ty.0) && ty.1.is_empty() {
        return Ok(class_name(&ty.0));
    }
    if enums.contains(&ty.0) && ty.1.is_empty() {
        return Ok(enum_name(&ty.0));
    }
    match (ty.0.as_str(), ty.1.as_slice()) {
        ("Range", []) => Ok("nagiRange".into()),
        ("List", [item]) => Ok(format!("[]{}", go_type(item, classes, enums, line)?)),
        ("Option", [item]) => Ok(format!(
            "nagiOption[{}]",
            go_type(item, classes, enums, line)?
        )),
        ("Result", [value, problem]) => Ok(format!(
            "nagiResult[{}, {}]",
            go_type(value, classes, enums, line)?,
            go_type(problem, classes, enums, line)?
        )),
        ("Future", [value]) => Ok(format!(
            "*nagiFuture[{}]",
            go_type(value, classes, enums, line)?
        )),
        _ => Err(error(
            line,
            format!("type {ty} is not supported by the initial Go backend"),
        )),
    }
}

fn go_borrow_input_type(
    ty: &Type,
    classes: &BTreeSet<String>,
    enums: &BTreeSet<String>,
    line: usize,
) -> Result<String, String> {
    if ty == &Type::generic("view", vec![Type::named("str")]) {
        return Ok("string".into());
    }
    go_type(ty, classes, enums, line)
}

fn go_local_type(
    ty: &Type,
    classes: &BTreeSet<String>,
    enums: &BTreeSet<String>,
    line: usize,
) -> Result<String, String> {
    if ty.is_async_function() {
        let (result, arguments) =
            ty.1.split_last()
                .expect("async function type has a Future result");
        let result = result
            .1
            .first()
            .expect("async function result is a checked Future");
        let arguments = arguments
            .iter()
            .map(|argument| go_borrow_input_type(argument, classes, enums, line))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        return Ok(format!(
            "func({arguments}) *nagiFuture[{}]",
            go_type(result, classes, enums, line)?
        ));
    }
    go_borrow_input_type(ty, classes, enums, line)
}

fn walk_expr_calls(expr: &Expr, calls: &mut Vec<String>) {
    match &expr.kind {
        E::Call(name, _, args) => {
            if expr.resolution == Some(NameResolution::Function) {
                calls.push(name.clone());
            }
            for argument in args {
                walk_expr_calls(argument, calls);
            }
        }
        E::Binary(left, _, right) | E::Index(left, right) => {
            walk_expr_calls(left, calls);
            walk_expr_calls(right, calls);
        }
        E::Unary(_, value) | E::Field(value, _) | E::Await(value) | E::Try(value) => {
            walk_expr_calls(value, calls)
        }
        E::Record(_, fields) => {
            for (_, value) in fields {
                walk_expr_calls(value, calls);
            }
        }
        E::List(values) => {
            for value in values {
                walk_expr_calls(value, calls);
            }
        }
        E::Name(name) => {
            if expr.resolution == Some(NameResolution::Function) {
                calls.push(name.clone());
            }
        }
        E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null => {}
    }
}

fn expression_uses_builtin_sleep(expression: &Expr) -> bool {
    let nested = match &expression.kind {
        E::Call(_, _, arguments) | E::List(arguments) => {
            arguments.iter().any(expression_uses_builtin_sleep)
        }
        E::Binary(left, _, right) | E::Index(left, right) => {
            expression_uses_builtin_sleep(left) || expression_uses_builtin_sleep(right)
        }
        E::Unary(_, value) | E::Field(value, _) | E::Await(value) | E::Try(value) => {
            expression_uses_builtin_sleep(value)
        }
        E::Record(_, fields) => fields
            .iter()
            .any(|(_, value)| expression_uses_builtin_sleep(value)),
        E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null | E::Name(_) => false,
    };
    nested
        || matches!(
            &expression.kind,
            E::Call(name, _, _)
                if name == "sleep" && expression.resolution == Some(NameResolution::Builtin)
        )
}

fn statement_uses_builtin_sleep(statement: &Stmt) -> bool {
    match &statement.kind {
        S::Assign { value, .. } | S::SpawnBind { value, .. } | S::Expr(value) | S::Spawn(value) => {
            expression_uses_builtin_sleep(value)
        }
        S::Return(value) => value.as_ref().is_some_and(expression_uses_builtin_sleep),
        S::If(condition, yes, no) => {
            expression_uses_builtin_sleep(condition)
                || yes.iter().any(statement_uses_builtin_sleep)
                || no.iter().any(statement_uses_builtin_sleep)
        }
        S::Match(value, arms) => {
            expression_uses_builtin_sleep(value)
                || arms
                    .iter()
                    .flat_map(|arm| &arm.body)
                    .any(statement_uses_builtin_sleep)
        }
        S::While(condition, body) => {
            expression_uses_builtin_sleep(condition)
                || body.iter().any(statement_uses_builtin_sleep)
        }
        S::For(_, value, body) => {
            expression_uses_builtin_sleep(value) || body.iter().any(statement_uses_builtin_sleep)
        }
        S::Scope(body) => body.iter().any(statement_uses_builtin_sleep),
    }
}

fn walk_statement_calls(statement: &Stmt, calls: &mut Vec<String>) {
    match &statement.kind {
        S::Assign { value, .. } | S::SpawnBind { value, .. } | S::Expr(value) | S::Spawn(value) => {
            walk_expr_calls(value, calls)
        }
        S::Return(value) => {
            if let Some(value) = value {
                walk_expr_calls(value, calls);
            }
        }
        S::If(condition, left, right) => {
            walk_expr_calls(condition, calls);
            for statement in left.iter().chain(right) {
                walk_statement_calls(statement, calls);
            }
        }
        S::Match(value, arms) => {
            walk_expr_calls(value, calls);
            for statement in arms.iter().flat_map(|arm| &arm.body) {
                walk_statement_calls(statement, calls);
            }
        }
        S::While(condition, body) => {
            walk_expr_calls(condition, calls);
            for statement in body {
                walk_statement_calls(statement, calls);
            }
        }
        S::For(_, value, body) => {
            walk_expr_calls(value, calls);
            for statement in body {
                walk_statement_calls(statement, calls);
            }
        }
        S::Scope(body) => {
            for statement in body {
                walk_statement_calls(statement, calls);
            }
        }
    }
}

fn reachable<'a>(
    program: &'a Program,
    future_facts: &FinalCheckFacts,
) -> Result<Vec<(&'a Function, Vec<String>)>, String> {
    let functions: BTreeMap<_, _> = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect();
    if !functions.contains_key("main") {
        return Err("Go backend requires def main()".into());
    }
    let mut pending = vec![("main".to_owned(), vec!["main".to_owned()])];
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    while let Some((name, chain)) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let function = functions
            .get(name.as_str())
            .copied()
            .ok_or_else(|| format!("Go backend cannot resolve checked function {name}"))?;
        result.push((function, chain.clone()));
        let mut calls = Vec::new();
        for statement in &function.body {
            walk_statement_calls(statement, &mut calls);
        }
        if let Some(captures) = future_facts.functions.get(&function.name) {
            calls.extend(
                captures
                    .values()
                    .filter(|capture| {
                        matches!(
                            capture.resolution,
                            NameResolution::Function | NameResolution::Local
                        ) && functions.contains_key(capture.callee.as_str())
                    })
                    .map(|capture| capture.callee.clone()),
            );
        }
        calls.sort();
        calls.dedup();
        for called in calls.into_iter().rev() {
            let mut next = chain.clone();
            next.push(called.clone());
            pending.push((called, next));
        }
    }
    result.sort_by_key(|(function, _)| function.line);
    Ok(result)
}

struct Emitter<'a> {
    program: &'a Program,
    classes: BTreeSet<String>,
    enums: BTreeSet<String>,
    enum_facts: &'a EnumFacts,
    future_facts: &'a FinalCheckFacts,
    reachable: Vec<(&'a Function, Vec<String>)>,
    operations: BTreeMap<String, BTreeMap<ExpressionKey, OperationPlan>>,
    current_function: Option<String>,
    profile: BuildProfile,
    uses_builtin_sleep: bool,
    temporary: usize,
}

impl<'a> Emitter<'a> {
    fn new(
        checked: &'a CheckedProgram,
        go_files: &[PathBuf],
        profile: BuildProfile,
    ) -> Result<Self, String> {
        checked.validate_for_emission()?;
        if let Some(file) = go_files.first() {
            return Err(format!(
                "Go native adapter files are not supported by the initial backend: {}",
                file.display()
            ));
        }
        let program = checked.program();
        let classes = program
            .classes
            .iter()
            .map(|class| class.name.clone())
            .collect();
        let enums = program
            .enums
            .iter()
            .map(|enumeration| enumeration.name.clone())
            .collect();
        let future_facts = checked.final_check_facts();
        let reachable = reachable(program, future_facts)?;
        let uses_builtin_sleep = reachable
            .iter()
            .any(|(function, _)| function.body.iter().any(statement_uses_builtin_sleep));
        if program.functions.len() != checked.emission.functions.len() {
            return Err(
                "internal compiler error: Go backend function plans do not match the checked program"
                    .into(),
            );
        }
        // Keep the frontend's original checked AST. The Rust emission copy may
        // contain Rust-specific names, while these operation facts are the
        // target-neutral proof that a standard call is the canonical move.
        let operations = program
            .functions
            .iter()
            .zip(&checked.emission.functions)
            .map(|(function, plan)| {
                let expressions = plan
                    .expressions
                    .iter()
                    .filter_map(|(key, expression)| {
                        expression
                            .operation
                            .clone()
                            .map(|operation| (*key, operation))
                    })
                    .collect();
                (function.name.clone(), expressions)
            })
            .collect();
        let mut emitter = Self {
            program,
            classes,
            enums,
            enum_facts: &checked.enums,
            future_facts,
            reachable,
            operations,
            current_function: None,
            profile,
            uses_builtin_sleep,
            temporary: 0,
        };
        emitter.validate()?;
        Ok(emitter)
    }

    fn temp(&mut self, prefix: &str) -> String {
        let result = format!("__nagi_{prefix}_{}", self.temporary);
        self.temporary += 1;
        result
    }

    fn validate(&mut self) -> Result<(), String> {
        for enumeration in &self.program.enums {
            if let Some(variant) = enumeration
                .variants
                .iter()
                .find(|variant| !variant.fields.is_empty())
            {
                return Err(error(
                    variant.line,
                    format!(
                        "enum {} has payload variant {}; the initial Go enum stage supports only payload-free variants",
                        enumeration.name, variant.name
                    ),
                ));
            }
        }
        for class in &self.program.classes {
            for (_, ty) in &class.fields {
                go_type(ty, &self.classes, &self.enums, class.line)?;
            }
        }
        let reachable = self.reachable.clone();
        for (function, chain) in reachable {
            self.current_function = Some(function.name.clone());
            if function.external {
                let available = function
                    .attrs
                    .iter()
                    .find(|(name, _)| name == "rust")
                    .map(|(_, value)| value.as_str())
                    .unwrap_or("Rust adapter");
                return Err(error(
                    function.line,
                    format!(
                        "selected backend go cannot provide native function {}\nrequired by {}\navailable binding: rust ({available})\nadd a Go binding or select --backend rust",
                        display_symbol(&function.name),
                        chain
                            .iter()
                            .map(|name| display_symbol(name))
                            .collect::<Vec<_>>()
                            .join(" -> ")
                    ),
                ));
            }
            for (_, ty) in &function.params {
                go_borrow_input_type(ty, &self.classes, &self.enums, function.line)?;
            }
            go_type(&function.ret, &self.classes, &self.enums, function.line)?;
            if function.name == "main"
                && function.ret.0 == "Result"
                && function
                    .ret
                    .1
                    .get(1)
                    .is_some_and(|problem| self.type_contains_enum(problem))
            {
                return Err(error(
                    function.line,
                    "main Result error types containing enum are not supported by the initial Go enum stage",
                ));
            }
            for statement in &function.body {
                self.validate_statement(statement, &function.ret)?;
            }
        }
        self.current_function = None;
        Ok(())
    }

    fn operation_plan(&self, expression: &Expr) -> Result<&OperationPlan, String> {
        let function = self.current_function.as_ref().ok_or_else(|| {
            error(
                expression.line,
                "internal compiler error: Go backend has no active function",
            )
        })?;
        self.operations
            .get(function)
            .and_then(|expressions| expressions.get(&ExpressionKey::of(expression)))
            .ok_or_else(|| {
                error(
                    expression.line,
                    format!(
                        "internal compiler error: Go backend is missing the sealed operation plan for {function}"
                    ),
                )
            })
    }

    fn future_capture(&self, expression: &Expr) -> Result<&FutureCapture, String> {
        let function = self.current_function.as_ref().ok_or_else(|| {
            error(
                expression.line,
                "internal compiler error: Go backend has no active function",
            )
        })?;
        self.future_facts
            .functions
            .get(function)
            .and_then(|captures| captures.get(&crate::ast::ExprUseId::of(expression)))
            .ok_or_else(|| {
                error(
                    expression.line,
                    format!(
                        "internal compiler error: Go backend is missing sealed Future capture identity for {function}"
                    ),
                )
            })
    }

    fn user_capture_storage(
        &self,
        expression: &Expr,
        arguments: &[Expr],
    ) -> Result<Option<Vec<CaptureStorageId>>, String> {
        let user_async = matches!(
            expression.resolution,
            Some(NameResolution::Function | NameResolution::Local)
        ) && expression.ty.as_ref().is_some_and(Type::is_future);
        if !user_async {
            return Ok(None);
        }
        let capture = self.future_capture(expression)?;
        let invalid = || {
            error(
                expression.line,
                "internal compiler error: invalid sealed user Future capture storage/release plan",
            )
        };
        if capture.resolution != expression.resolution.ok_or_else(invalid)?
            || capture.arguments.len() != arguments.len()
            || matches!(&expression.kind, E::Call(name, _, _)
                if expression.resolution == Some(NameResolution::Function) && *name != capture.callee)
        {
            return Err(invalid());
        }
        let mut storage = Vec::new();
        for (index, (argument, plan)) in arguments.iter().zip(&capture.arguments).enumerate() {
            let ArgumentCaptureFacts::UserAsync(facts) = &plan.capture else {
                return Err(invalid());
            };
            let expected = CaptureStorageId {
                call: crate::ast::ExprUseId::of(expression),
                argument: index,
            };
            if plan.index != index
                || plan.passing != crate::stdlib::Passing::Move
                || plan.expression != crate::ast::ExprUseId::of(argument)
                || argument.ty.as_ref() != Some(&plan.ty)
                || facts.slot.call != expected.call
                || facts.slot.argument != index
                || facts.storage != expected
                || !matches!(
                    facts.transfer,
                    crate::ast::ExprUseMode::Copy | crate::ast::ExprUseMode::Move
                )
            {
                return Err(invalid());
            }
            match facts.release {
                ArgumentRelease::NotRequired | ArgumentRelease::FutureInvocationExit => {}
                ArgumentRelease::Deferred => {
                    return Err(error(
                        argument.line,
                        "deferred user Future capture release is not supported by the Go backend",
                    ))
                }
            }
            storage.push(facts.storage);
        }
        Ok(Some(storage))
    }

    fn constructor_fact(&self, expression: &Expr) -> Result<&EnumVariantFact, String> {
        let function = self.current_function.as_ref().ok_or_else(|| {
            error(
                expression.line,
                "internal compiler error: Go backend has no active function",
            )
        })?;
        self.enum_facts
            .functions
            .get(function)
            .and_then(|facts| facts.constructors.get(&ExpressionKey::of(expression)))
            .ok_or_else(|| {
                error(
                    expression.line,
                    format!(
                        "internal compiler error: Go backend is missing the sealed enum constructor identity for {function}"
                    ),
                )
            })
    }

    fn match_facts(&self, value: &Expr) -> Result<&[EnumVariantFact], String> {
        let function = self.current_function.as_ref().ok_or_else(|| {
            error(
                value.line,
                "internal compiler error: Go backend has no active function",
            )
        })?;
        self.enum_facts
            .functions
            .get(function)
            .and_then(|facts| facts.matches.get(&ExpressionKey::of(value)))
            .map(Vec::as_slice)
            .ok_or_else(|| {
                error(
                    value.line,
                    format!(
                        "internal compiler error: Go backend is missing the sealed enum match identities for {function}"
                    ),
                )
            })
    }

    fn type_contains_enum(&self, ty: &Type) -> bool {
        fn visit(emitter: &Emitter<'_>, ty: &Type, classes: &mut BTreeSet<String>) -> bool {
            if emitter.enums.contains(&ty.0)
                || ty.1.iter().any(|item| visit(emitter, item, classes))
            {
                return true;
            }
            let Some(class) = emitter
                .program
                .classes
                .iter()
                .find(|class| class.name == ty.0)
            else {
                return false;
            };
            if !classes.insert(class.name.clone()) {
                return false;
            }
            let found = class
                .fields
                .iter()
                .any(|(_, field)| visit(emitter, field, classes));
            classes.remove(&class.name);
            found
        }
        visit(self, ty, &mut BTreeSet::new())
    }

    fn validate_statement(&self, statement: &Stmt, ret: &Type) -> Result<(), String> {
        match &statement.kind {
            S::Assign { value, .. } => self.validate_expr(value, ret),
            S::Return(value) => value
                .as_ref()
                .map(|value| self.validate_expr(value, ret))
                .unwrap_or(Ok(())),
            S::Expr(value) => self.validate_expr(value, ret),
            S::If(condition, left, right) => {
                self.validate_expr(condition, ret)?;
                for statement in left.iter().chain(right) {
                    self.validate_statement(statement, ret)?;
                }
                Ok(())
            }
            S::Match(value, arms) => {
                self.validate_expr(value, ret)?;
                let subject = value.ty.as_ref().expect("checked match type");
                if self.enums.contains(&subject.0) {
                    let facts = self.match_facts(value)?;
                    if facts.len() != arms.len()
                        || facts.iter().any(|fact| fact.enumeration != subject.0)
                    {
                        return Err(error(
                            statement.line,
                            "internal compiler error: inconsistent sealed enum match identities",
                        ));
                    }
                    for (arm, fact) in arms.iter().zip(facts) {
                        let MatchPattern::Enum { bindings, .. } = &arm.pattern else {
                            return Err(error(
                                arm.line,
                                "internal compiler error: mixed sealed enum match",
                            ));
                        };
                        if !bindings.is_empty() {
                            return Err(error(
                                arm.line,
                                format!(
                                    "enum {} variant {} has a payload unsupported by the initial Go enum stage",
                                    fact.enumeration, fact.variant
                                ),
                            ));
                        }
                    }
                } else if matches!(subject.0.as_str(), "Option" | "Result") {
                    if arms.len() != 2 {
                        return Err(error(
                            statement.line,
                            "Option/Result match must have exactly two exhaustive arms",
                        ));
                    }
                    if arms
                        .iter()
                        .any(|arm| matches!(arm.pattern, MatchPattern::Enum { .. }))
                    {
                        return Err(error(statement.line, "invalid checked Option/Result match"));
                    }
                } else {
                    return Err(error(
                        statement.line,
                        "only Option/Result/payload-free enum match is supported",
                    ));
                }
                for arm in arms {
                    for statement in &arm.body {
                        self.validate_statement(statement, ret)?;
                    }
                }
                Ok(())
            }
            S::While(condition, body) => {
                self.validate_expr(condition, ret)?;
                for statement in body {
                    self.validate_statement(statement, ret)?;
                }
                Ok(())
            }
            S::For(_, iterable, body) => {
                self.validate_expr(iterable, ret)?;
                match iterable.ty.as_ref().map(|ty| ty.0.as_str()) {
                    Some("Range") => {}
                    Some("List") if !statement.binding_borrowed => {}
                    Some("List") => {
                        return Err(error(
                            statement.line,
                            "for over non-Copy List elements is not supported by the Go backend yet",
                        ));
                    }
                    _ => {
                        return Err(error(
                            statement.line,
                            "Go backend for requires range or a List with Copy elements",
                        ));
                    }
                }
                for statement in body {
                    self.validate_statement(statement, ret)?;
                }
                Ok(())
            }
            S::SpawnBind { .. } | S::Spawn(_) | S::Scope(_) => Err(error(
                statement.line,
                "async task operations are not supported by the initial Go backend",
            )),
        }
    }

    fn validate_expr(&self, expression: &Expr, ret: &Type) -> Result<(), String> {
        let ty = expression
            .ty
            .as_ref()
            .ok_or_else(|| error(expression.line, "checked expression has no type"))?;
        if expression.resolution == Some(NameResolution::Standard)
            && matches!(
                self.operation_plan(expression)?,
                OperationPlan::NativeCall { .. }
            )
        {
            let E::Call(name, _, _) = &expression.kind else {
                return Err(error(
                    expression.line,
                    "internal compiler error: sealed standard operation is not a call",
                ));
            };
            return Err(error(
                expression.line,
                format!("operation {name} is not supported by the initial Go backend"),
            ));
        }
        go_local_type(ty, &self.classes, &self.enums, expression.line)?;
        match &expression.kind {
            E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null => Ok(()),
            E::Name(_) => match expression.resolution {
                Some(
                    NameResolution::Local
                    | NameResolution::BorrowedLocal
                    | NameResolution::Function,
                ) => Ok(()),
                _ => Err(error(
                    expression.line,
                    "function/module values are not supported",
                )),
            },
            E::Binary(left, operator, right) => {
                if !matches!(
                    operator.as_str(),
                    "+" | "-"
                        | "*"
                        | "/"
                        | "%"
                        | "=="
                        | "!="
                        | "<"
                        | "<="
                        | ">"
                        | ">="
                        | "and"
                        | "or"
                ) {
                    return Err(error(
                        expression.line,
                        format!("operator {operator} is not supported"),
                    ));
                }
                self.validate_expr(left, ret)?;
                self.validate_expr(right, ret)
            }
            E::Unary(operator, value) => {
                if !matches!(operator.as_str(), "-" | "+" | "not") {
                    return Err(error(
                        expression.line,
                        format!("operator {operator} is not supported"),
                    ));
                }
                self.validate_expr(value, ret)
            }
            E::Call(name, _, arguments) => {
                if expression.resolution == Some(NameResolution::Enum) {
                    let fact = self.constructor_fact(expression)?;
                    if !arguments.is_empty() {
                        return Err(error(
                            expression.line,
                            format!(
                                "enum {} variant {} has a payload unsupported by the initial Go enum stage",
                                fact.enumeration, fact.variant
                            ),
                        ));
                    }
                    return Ok(());
                }
                for argument in arguments {
                    self.validate_expr(argument, ret)?;
                }
                self.user_capture_storage(expression, arguments)?;
                match expression.resolution {
                    Some(NameResolution::Function) => Ok(()),
                    Some(NameResolution::Local) => {
                        let capture = self.future_capture(expression)?;
                        if capture.resolution != NameResolution::Local
                            || !self.program.functions.iter().any(|function| {
                                function.name == capture.callee && function.asynchronous
                            })
                        {
                            return Err(error(
                                expression.line,
                                "internal compiler error: invalid sealed async callback identity",
                            ));
                        }
                        Ok(())
                    }
                    Some(NameResolution::Standard) => match self.operation_plan(expression)? {
                        OperationPlan::IdentityTransfer { argument }
                            if *argument == 0 && arguments.len() == 1 =>
                        {
                            Ok(())
                        }
                        OperationPlan::IdentityTransfer { .. } => Err(error(
                            expression.line,
                            "internal compiler error: invalid sealed identity transfer shape",
                        )),
                        OperationPlan::NativeCall { .. } => Err(error(
                            expression.line,
                            format!("operation {name} is not supported by the initial Go backend"),
                        )),
                    },
                    Some(NameResolution::Builtin)
                        if matches!(
                            name.as_str(),
                            "print"
                                | "write"
                                | "assert_true"
                                | "ok"
                                | "fail"
                                | "error"
                                | "some"
                                | "parse_i64"
                                | "error_message"
                                | "i64"
                                | "len"
                                | "range"
                                | "sleep"
                                | "append"
                                | "view"
                        ) =>
                    {
                        if name == "view" {
                            let Some(argument_ty) =
                                arguments.first().and_then(|argument| argument.ty.as_ref())
                            else {
                                return Err(error(
                                    expression.line,
                                    "internal compiler error: invalid checked view shape",
                                ));
                            };
                            if argument_ty != &Type::named("str") {
                                return Err(error(
                                    expression.line,
                                    format!(
                                        "view of {argument_ty} is not supported by the initial Go borrow stage; only a read-only string borrowed from str is supported"
                                    ),
                                ));
                            }
                        }
                        if matches!(name.as_str(), "print" | "write")
                            && arguments
                                .first()
                                .and_then(|argument| argument.ty.as_ref())
                                .is_some_and(|ty| self.type_contains_enum(ty))
                        {
                            return Err(error(
                                expression.line,
                                "printing enum values is not supported by the initial Go enum stage",
                            ));
                        }
                        Ok(())
                    }
                    _ => Err(error(
                        expression.line,
                        format!("operation {name} is not supported by the initial Go backend"),
                    )),
                }
            }
            E::Record(name, fields) => {
                if !self.classes.contains(name) {
                    return Err(error(
                        expression.line,
                        format!("record {name} is not supported"),
                    ));
                }
                for (_, value) in fields {
                    self.validate_expr(value, ret)?;
                }
                Ok(())
            }
            E::Field(_, _) if expression.resolution == Some(NameResolution::Enum) => {
                self.constructor_fact(expression)?;
                Ok(())
            }
            E::Field(value, _) => self.validate_expr(value, ret),
            E::Try(value) => {
                self.validate_expr(value, ret)?;
                let Some(value_ty) = value
                    .ty
                    .as_ref()
                    .filter(|ty| ty.0 == "Result" && ty.1.len() == 2)
                else {
                    return Err(error(expression.line, "try operand is not Result"));
                };
                if ret.0 != "Result" || ret.1.len() != 2 || value_ty.1[1] != ret.1[1] {
                    return Err(error(
                        expression.line,
                        "try error conversion is not supported by the initial Go backend",
                    ));
                }
                Ok(())
            }
            E::Index(values, index) => {
                self.validate_expr(values, ret)?;
                self.validate_expr(index, ret)
            }
            E::List(values) => {
                for value in values {
                    self.validate_expr(value, ret)?;
                }
                Ok(())
            }
            E::Await(value) => self.validate_expr(value, ret),
        }
    }

    fn expression(
        &mut self,
        expression: &Expr,
        ret: &Type,
    ) -> Result<(Vec<String>, String), String> {
        let mut before = Vec::new();
        let value = match &expression.kind {
            E::Int(value) => decimal_integer_literal(value, expression.line)?,
            E::Float(value) => {
                let bits = float_type(expression.ty.as_ref().expect("validated float"))
                    .expect("validated float has a Go floating type");
                if bits == 32 {
                    let parsed = value
                        .parse::<f32>()
                        .map_err(|problem| error(expression.line, problem))?;
                    format!("math.Float32frombits(0x{:08x})", parsed.to_bits())
                } else {
                    let parsed = value
                        .parse::<f64>()
                        .map_err(|problem| error(expression.line, problem))?;
                    format!("math.Float64frombits(0x{:016x})", parsed.to_bits())
                }
            }
            E::Str(value) => serde_json::to_string(value).map_err(|error| error.to_string())?,
            E::Bool(value) => value.to_string(),
            E::Null => {
                let item = &expression.ty.as_ref().expect("validated type").1[0];
                format!(
                    "nagiNone[{}]()",
                    go_type(item, &self.classes, &self.enums, expression.line)?
                )
            }
            E::Name(name) => {
                if expression.resolution == Some(NameResolution::Function) {
                    function_name(name)
                } else {
                    ident(name)
                }
            }
            E::Binary(left, operator, right) => {
                let (left_before, left) = self.evaluated(left, ret)?;
                before.extend(left_before);
                if matches!(operator.as_str(), "and" | "or") {
                    let result = self.temp("logical");
                    before.push(format!("{result} := {left}"));
                    let guard = if operator == "and" {
                        result.clone()
                    } else {
                        format!("!{result}")
                    };
                    before.push(format!("if {guard} {{"));
                    let (right_before, right) = self.evaluated(right, ret)?;
                    before.extend(right_before.into_iter().map(|line| format!("    {line}")));
                    before.push(format!("    {result} = {right}"));
                    before.push("}".into());
                    result
                } else {
                    let (right_before, right) = self.evaluated(right, ret)?;
                    before.extend(right_before);
                    let ty = expression.ty.as_ref().expect("validated binary type");
                    if let Some(bits) = float_type(ty).filter(|_| operator == "%") {
                        if bits == 32 {
                            format!("float32(math.Mod(float64({left}), float64({right})))")
                        } else {
                            format!("math.Mod({left}, {right})")
                        }
                    } else if let Some((signed, minimum)) = integer_type(ty) {
                        let checked = self.profile == BuildProfile::Dev
                            && matches!(operator.as_str(), "+" | "-" | "*");
                        let division_overflow = signed && matches!(operator.as_str(), "/" | "%");
                        if checked || division_overflow {
                            let result = self.temp("arithmetic");
                            let go_ty = go_type(ty, &self.classes, &self.enums, expression.line)?;
                            if division_overflow {
                                before.push(format!(
                                    "if {left} == {minimum} && {right} == -1 {{ panic(\"integer overflow\") }}"
                                ));
                            }
                            match (signed, operator.as_str()) {
                                (false, "-") if checked => before.push(format!(
                                    "if {left} < {right} {{ panic(\"integer overflow\") }}"
                                )),
                                (true, "*") if checked => before.push(format!(
                                    "if ({left} == {minimum} && {right} == -1) || ({right} == {minimum} && {left} == -1) {{ panic(\"integer overflow\") }}"
                                )),
                                _ => {}
                            }
                            before
                                .push(format!("var {result} {go_ty} = {left} {operator} {right}"));
                            match (signed, operator.as_str()) {
                                (false, "+") if checked => before.push(format!(
                                    "if {result} < {left} {{ panic(\"integer overflow\") }}"
                                )),
                                (false, "*") if checked => before.push(format!(
                                    "if {left} != 0 && {result} / {left} != {right} {{ panic(\"integer overflow\") }}"
                                )),
                                (true, "+") if checked => before.push(format!(
                                    "if ({right} > 0 && {result} < {left}) || ({right} < 0 && {result} > {left}) {{ panic(\"integer overflow\") }}"
                                )),
                                (true, "-") if checked => before.push(format!(
                                    "if ({right} > 0 && {result} > {left}) || ({right} < 0 && {result} < {left}) {{ panic(\"integer overflow\") }}"
                                )),
                                (true, "*") if checked => before.push(format!(
                                    "if {left} != 0 && {result} / {left} != {right} {{ panic(\"integer overflow\") }}"
                                )),
                                _ => {}
                            }
                            result
                        } else {
                            format!("({left} {operator} {right})")
                        }
                    } else {
                        format!("({left} {operator} {right})")
                    }
                }
            }
            E::Unary(operator, inner) => {
                if operator == "-" {
                    if let E::Int(literal) = &inner.kind {
                        if let Some(boundary) = negative_boundary(
                            expression.ty.as_ref().expect("validated unary type"),
                            literal,
                        ) {
                            return Ok((before, boundary.into()));
                        }
                    }
                }
                let (inner_before, inner) = self.evaluated(inner, ret)?;
                before.extend(inner_before);
                if operator == "-" && self.profile == BuildProfile::Dev {
                    if let Some((true, minimum)) =
                        integer_type(expression.ty.as_ref().expect("validated unary type"))
                    {
                        before.push(format!(
                            "if {inner} == {minimum} {{ panic(\"integer overflow\") }}"
                        ));
                    }
                }
                format!(
                    "({}{inner})",
                    if operator == "not" { "!" } else { operator }
                )
            }
            E::Call(name, _, arguments) => {
                if expression.resolution == Some(NameResolution::Enum) {
                    if !arguments.is_empty() {
                        return Err(error(
                            expression.line,
                            "internal compiler error: validated payload-free enum constructor has arguments",
                        ));
                    }
                    return Ok((
                        before,
                        enum_variant_name(self.constructor_fact(expression)?),
                    ));
                }
                if expression.resolution == Some(NameResolution::Standard) {
                    return match self.operation_plan(expression)?.clone() {
                        OperationPlan::IdentityTransfer { argument }
                            if argument == 0 && arguments.len() == 1 =>
                        {
                            self.evaluated(&arguments[argument], ret)
                        }
                        OperationPlan::IdentityTransfer { .. } => Err(error(
                            expression.line,
                            "internal compiler error: invalid sealed identity transfer shape",
                        )),
                        OperationPlan::NativeCall { .. } => Err(error(
                            expression.line,
                            format!("operation {name} is not supported by the initial Go backend"),
                        )),
                    };
                }
                if expression.resolution == Some(NameResolution::Builtin) && name == "append" {
                    let E::Name(receiver) = &arguments[0].kind else {
                        unreachable!("checker restricts append receiver to a local name")
                    };
                    let (item_before, item) = self.evaluated(&arguments[1], ret)?;
                    before.extend(item_before);
                    let receiver = ident(receiver);
                    before.push(format!("{receiver} = append({receiver}, {item})"));
                    return Ok((before, "nagiUnit{}".into()));
                }
                let mut values = Vec::new();
                let storage = self.user_capture_storage(expression, arguments)?;
                for (index, argument) in arguments.iter().enumerate() {
                    let (argument_before, argument) = if let Some(storage) = &storage {
                        self.evaluated_storage(argument, ret, Some(storage[index]))?
                    } else {
                        self.evaluated(argument, ret)?
                    };
                    before.extend(argument_before);
                    values.push(argument);
                }
                match expression.resolution {
                    Some(NameResolution::Function) => {
                        format!("{}({})", function_name(name), values.join(", "))
                    }
                    Some(NameResolution::Local) => {
                        format!("{}({})", ident(name), values.join(", "))
                    }
                    Some(NameResolution::Builtin) => match name.as_str() {
                        "print" => format!("nagiPrint({})", values[0]),
                        "write" => format!("nagiWrite({})", values[0]),
                        "assert_true" => format!("nagiAssertTrue({})", values[0]),
                        "some" => format!("nagiSome({})", values[0]),
                        "parse_i64" => format!("nagiParseI64({})", values[0]),
                        "error_message" => format!("({}).message", values[0]),
                        "i64" => format!("int64({})", values[0]),
                        "len" => format!("int64(len({}))", values[0]),
                        "range" => format!("nagiRange{{end: {}}}", values[0]),
                        "sleep" => format!("nagiSleep({})", values[0]),
                        "view" => values[0].clone(),
                        "ok" => {
                            let ty = expression.ty.as_ref().expect("validated result");
                            format!(
                                "nagiOk[{}, {}]({})",
                                go_type(&ty.1[0], &self.classes, &self.enums, expression.line)?,
                                go_type(&ty.1[1], &self.classes, &self.enums, expression.line)?,
                                values[0]
                            )
                        }
                        "fail" => {
                            let ty = expression.ty.as_ref().expect("validated result");
                            format!(
                                "nagiErr[{}, {}]({})",
                                go_type(&ty.1[0], &self.classes, &self.enums, expression.line)?,
                                go_type(&ty.1[1], &self.classes, &self.enums, expression.line)?,
                                values[0]
                            )
                        }
                        "error" => {
                            let ty = expression.ty.as_ref().expect("validated result");
                            format!(
                                "nagiErr[{}, nagiError](nagiError{{message: {}}})",
                                go_type(&ty.1[0], &self.classes, &self.enums, expression.line)?,
                                values[0]
                            )
                        }
                        _ => unreachable!("validated builtin"),
                    },
                    _ => unreachable!("validated call"),
                }
            }
            E::Record(name, fields) => {
                let mut rendered = Vec::new();
                for (name, value) in fields {
                    let (value_before, value) = self.evaluated(value, ret)?;
                    before.extend(value_before);
                    rendered.push(format!("{}: {value}", ident(name)));
                }
                format!("{}{{{}}}", class_name(name), rendered.join(", "))
            }
            E::Field(_, _) if expression.resolution == Some(NameResolution::Enum) => {
                enum_variant_name(self.constructor_fact(expression)?)
            }
            E::Field(value, field) => {
                let (value_before, value) = self.evaluated(value, ret)?;
                before.extend(value_before);
                format!("({value}).{}", ident(field))
            }
            E::Try(value) => {
                let (value_before, value) = self.expression(value, ret)?;
                before.extend(value_before);
                let result = self.temp("try");
                before.push(format!("{result} := {value}"));
                let ret_value = go_type(&ret.1[0], &self.classes, &self.enums, expression.line)?;
                let ret_error = go_type(&ret.1[1], &self.classes, &self.enums, expression.line)?;
                before.push(format!(
                    "if !{result}.ok {{ return nagiErr[{ret_value}, {ret_error}]({result}.problem) }}"
                ));
                format!("{result}.value")
            }
            E::Index(values, index) => {
                let (values_before, values) = self.evaluated(values, ret)?;
                before.extend(values_before);
                let (index_before, index) = self.evaluated(index, ret)?;
                before.extend(index_before);
                before.push(format!(
                    "if {index} < 0 || {index} >= int64(len({values})) {{ panic(\"index out of bounds\") }}"
                ));
                format!("{values}[int({index})]")
            }
            E::List(values) => {
                let mut rendered = Vec::new();
                for value in values {
                    let (value_before, value) = self.evaluated(value, ret)?;
                    before.extend(value_before);
                    rendered.push(value);
                }
                let item = &expression.ty.as_ref().expect("validated List type").1[0];
                format!(
                    "[]{}{{{}}}",
                    go_type(item, &self.classes, &self.enums, expression.line)?,
                    rendered.join(", ")
                )
            }
            E::Await(value) => {
                let (value_before, value) = self.evaluated(value, ret)?;
                before.extend(value_before);
                format!("({value}).await()")
            }
        };
        Ok((before, value))
    }

    fn evaluated(
        &mut self,
        expression: &Expr,
        ret: &Type,
    ) -> Result<(Vec<String>, String), String> {
        self.evaluated_storage(expression, ret, None)
    }

    fn evaluated_storage(
        &mut self,
        expression: &Expr,
        ret: &Type,
        storage: Option<CaptureStorageId>,
    ) -> Result<(Vec<String>, String), String> {
        let (mut before, value) = self.expression(expression, ret)?;
        let name = if let Some(storage) = storage {
            format!(
                "__nagi_capture_{}_{}_{}_{}",
                storage.call.line, storage.call.start, storage.call.end, storage.argument
            )
        } else {
            self.temp("value")
        };
        let ty = go_local_type(
            expression.ty.as_ref().expect("validated expression type"),
            &self.classes,
            &self.enums,
            expression.line,
        )?;
        before.push(format!("var {name} {ty} = {value}"));
        Ok((before, name))
    }

    fn line(output: &mut String, indent: usize, line: impl AsRef<str>) {
        output.push_str(&"    ".repeat(indent));
        output.push_str(line.as_ref());
        output.push('\n');
    }

    fn statements(
        &mut self,
        statements: &[Stmt],
        ret: &Type,
        indent: usize,
        output: &mut String,
    ) -> Result<(), String> {
        for statement in statements {
            match &statement.kind {
                S::Assign {
                    name,
                    annotation,
                    value,
                    declare,
                    ..
                } => {
                    let (before, value) = self.expression(value, ret)?;
                    for line in before {
                        Self::line(output, indent, line);
                    }
                    let name = ident(name);
                    if *declare {
                        let ty = go_local_type(
                            annotation.as_ref().expect("checked declaration type"),
                            &self.classes,
                            &self.enums,
                            statement.line,
                        )?;
                        Self::line(output, indent, format!("var {name} {ty} = {value}"));
                    } else {
                        Self::line(output, indent, format!("{name} = {value}"));
                    }
                    Self::line(output, indent, format!("_ = {name}"));
                }
                S::Return(value) => {
                    if let Some(value) = value {
                        let (before, value) = self.expression(value, ret)?;
                        for line in before {
                            Self::line(output, indent, line);
                        }
                        Self::line(output, indent, format!("return {value}"));
                    } else {
                        Self::line(output, indent, "return nagiUnit{}");
                    }
                }
                S::Expr(value) => {
                    let (before, value) = self.expression(value, ret)?;
                    for line in before {
                        Self::line(output, indent, line);
                    }
                    Self::line(output, indent, format!("_ = {value}"));
                }
                S::If(condition, left, right) => {
                    let (before, condition) = self.expression(condition, ret)?;
                    for line in before {
                        Self::line(output, indent, line);
                    }
                    Self::line(output, indent, format!("if {condition} {{"));
                    self.statements(left, ret, indent + 1, output)?;
                    if right.is_empty() {
                        Self::line(output, indent, "}");
                    } else {
                        Self::line(output, indent, "} else {");
                        self.statements(right, ret, indent + 1, output)?;
                        Self::line(output, indent, "}");
                    }
                }
                S::While(condition, body) => {
                    Self::line(output, indent, "for {");
                    let (before, condition) = self.expression(condition, ret)?;
                    for line in before {
                        Self::line(output, indent + 1, line);
                    }
                    Self::line(output, indent + 1, format!("if !({condition}) {{ break }}"));
                    self.statements(body, ret, indent + 1, output)?;
                    Self::line(output, indent, "}");
                }
                S::Match(value, arms) => {
                    let enum_facts = value
                        .ty
                        .as_ref()
                        .is_some_and(|ty| self.enums.contains(&ty.0))
                        .then(|| self.match_facts(value).map(<[_]>::to_vec))
                        .transpose()?;
                    let (before, value) = self.expression(value, ret)?;
                    for line in before {
                        Self::line(output, indent, line);
                    }
                    let matched = self.temp("match");
                    Self::line(output, indent, format!("{matched} := {value}"));
                    if let Some(facts) = enum_facts {
                        Self::line(output, indent, format!("switch {matched} {{"));
                        for (arm, fact) in arms.iter().zip(facts) {
                            Self::line(
                                output,
                                indent,
                                format!("case {}:", enum_variant_name(&fact)),
                            );
                            self.statements(&arm.body, ret, indent + 1, output)?;
                        }
                        Self::line(output, indent, "default:");
                        Self::line(output, indent + 1, "panic(\"invalid generated enum tag\")");
                        Self::line(output, indent, "}");
                        continue;
                    }
                    for (index, arm) in arms.iter().enumerate() {
                        let (condition, projected) = match &arm.pattern {
                            MatchPattern::Result { ok, binding } => (
                                if *ok {
                                    format!("{matched}.ok")
                                } else {
                                    format!("!{matched}.ok")
                                },
                                binding.name.as_ref().map(|name| {
                                    (ident(name), if *ok { "value" } else { "problem" })
                                }),
                            ),
                            MatchPattern::Option { binding } => (
                                if binding.is_some() {
                                    format!("{matched}.some")
                                } else {
                                    format!("!{matched}.some")
                                },
                                binding.as_ref().and_then(|binding| {
                                    binding.name.as_ref().map(|name| (ident(name), "value"))
                                }),
                            ),
                            MatchPattern::Enum { .. } => unreachable!("validated match"),
                        };
                        Self::line(
                            output,
                            indent,
                            if index == 0 {
                                format!("if {condition} {{")
                            } else if index + 1 == arms.len() {
                                "} else {".into()
                            } else {
                                format!("}} else if {condition} {{")
                            },
                        );
                        if let Some((binding, field)) = projected {
                            Self::line(
                                output,
                                indent + 1,
                                format!("{binding} := {matched}.{field}"),
                            );
                            Self::line(output, indent + 1, format!("_ = {binding}"));
                        }
                        self.statements(&arm.body, ret, indent + 1, output)?;
                    }
                    Self::line(output, indent, "}");
                }
                S::For(name, iterable, body) => {
                    let (before, iterable_value) = self.evaluated(iterable, ret)?;
                    for line in before {
                        Self::line(output, indent, line);
                    }
                    let name = ident(name);
                    if iterable.ty.as_ref().is_some_and(|ty| ty.0 == "Range") {
                        Self::line(
                            output,
                            indent,
                            format!(
                                "for {name} := int64(0); {name} < {iterable_value}.end; {name}++ {{"
                            ),
                        );
                    } else {
                        Self::line(
                            output,
                            indent,
                            format!("for _, {name} := range {iterable_value} {{"),
                        );
                    }
                    Self::line(output, indent + 1, format!("_ = {name}"));
                    self.statements(body, ret, indent + 1, output)?;
                    Self::line(output, indent, "}");
                }
                S::SpawnBind { .. } | S::Spawn(_) | S::Scope(_) => {
                    unreachable!("validated statement")
                }
            }
        }
        Ok(())
    }

    fn generate(mut self) -> Result<String, String> {
        let mut body = String::new();
        for enumeration in &self.program.enums {
            Self::line(
                &mut body,
                0,
                format!("type {} uint32", enum_name(&enumeration.name)),
            );
            Self::line(&mut body, 0, "const (");
            for (ordinal, variant) in enumeration.variants.iter().enumerate() {
                let fact = EnumVariantFact {
                    enumeration: enumeration.name.clone(),
                    variant: variant.name.clone(),
                    ordinal,
                };
                Self::line(
                    &mut body,
                    1,
                    format!(
                        "{} {} = {}",
                        enum_variant_name(&fact),
                        enum_name(&enumeration.name),
                        fact.ordinal + 1
                    ),
                );
            }
            Self::line(&mut body, 0, ")");
            body.push('\n');
        }
        for class in &self.program.classes {
            Self::line(
                &mut body,
                0,
                format!("type {} struct {{", class_name(&class.name)),
            );
            for (field, ty) in &class.fields {
                Self::line(
                    &mut body,
                    1,
                    format!(
                        "{} {}",
                        ident(field),
                        go_type(ty, &self.classes, &self.enums, class.line)?
                    ),
                );
            }
            Self::line(&mut body, 0, "}");
            body.push('\n');
        }
        let reachable = self.reachable.clone();
        for (function, _) in reachable {
            self.current_function = Some(function.name.clone());
            let parameters = function
                .params
                .iter()
                .map(|(name, ty)| {
                    Ok(format!(
                        "{} {}",
                        ident(name),
                        go_borrow_input_type(ty, &self.classes, &self.enums, function.line)?
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?
                .join(", ");
            let result = go_type(&function.ret, &self.classes, &self.enums, function.line)?;
            let signature_result = if function.asynchronous {
                format!("*nagiFuture[{result}]")
            } else {
                result.clone()
            };
            Self::line(
                &mut body,
                0,
                format!(
                    "func {}({parameters}) {signature_result} {{",
                    function_name(&function.name)
                ),
            );
            let body_indent = if function.asynchronous {
                Self::line(&mut body, 1, format!("return nagiCold(func() {result} {{"));
                2
            } else {
                1
            };
            for (name, _) in &function.params {
                Self::line(&mut body, body_indent, format!("_ = {}", ident(name)));
            }
            self.statements(&function.body, &function.ret, body_indent, &mut body)?;
            if function.ret.0 == "unit" && !crate::ast::block_returns(&function.body) {
                Self::line(&mut body, body_indent, "return nagiUnit{}");
            }
            if function.asynchronous {
                Self::line(&mut body, 1, "})");
            }
            Self::line(&mut body, 0, "}");
            body.push('\n');
        }
        self.current_function = None;
        let main = self
            .program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("validated main");
        Self::line(&mut body, 0, "func main() {");
        if main.ret.0 == "Result" {
            Self::line(
                &mut body,
                1,
                if main.asynchronous {
                    "result := nagiMain().await()"
                } else {
                    "result := nagiMain()"
                },
            );
            Self::line(&mut body, 1, "if !result.ok {");
            Self::line(&mut body, 2, "fmt.Fprintln(os.Stderr, result.problem)");
            Self::line(&mut body, 2, "os.Exit(1)");
            Self::line(&mut body, 1, "}");
        } else {
            Self::line(
                &mut body,
                1,
                if main.asynchronous {
                    "_ = nagiMain().await()"
                } else {
                    "_ = nagiMain()"
                },
            );
        }
        Self::line(&mut body, 0, "}");

        let mut imports = vec![
            "\"fmt\"",
            "\"math\"",
            "\"math/big\"",
            "\"strconv\"",
            "\"strings\"",
        ];
        if main.ret.0 == "Result" {
            imports.push("\"os\"");
        }
        let sleep_runtime = if self.uses_builtin_sleep {
            imports.push("\"time\"");
            "const nagiMaxSleepMilliseconds int64 = 9223372036854\n\
func nagiSleep(milliseconds int64) *nagiFuture[nagiUnit] {\n\
    return nagiCold(func() nagiUnit {\n\
        for milliseconds > nagiMaxSleepMilliseconds {\n\
            time.Sleep(time.Duration(nagiMaxSleepMilliseconds) * time.Millisecond)\n\
            milliseconds -= nagiMaxSleepMilliseconds\n\
        }\n\
        if milliseconds > 0 { time.Sleep(time.Duration(milliseconds) * time.Millisecond) }\n\
        return nagiUnit{}\n\
    })\n\
}\n"
        } else {
            ""
        };
        let imports = format!("import (\n    {}\n)\n\n", imports.join("\n    "));
        Ok(format!(
            "// Code generated by nagic for the experimental Go backend. DO NOT EDIT.\npackage main\n\n{imports}\
type nagiUnit struct{{}}\n\
type nagiError struct{{ message string }}\n\
func (problem nagiError) Error() string {{ return problem.message }}\n\
type nagiRange struct{{ end int64 }}\n\
type nagiOption[T any] struct{{ some bool; value T }}\n\
func nagiSome[T any](value T) nagiOption[T] {{ return nagiOption[T]{{some: true, value: value}} }}\n\
func nagiNone[T any]() nagiOption[T] {{ return nagiOption[T]{{}} }}\n\
type nagiResult[T any, E any] struct{{ ok bool; value T; problem E }}\n\
func nagiOk[T any, E any](value T) nagiResult[T, E] {{ return nagiResult[T, E]{{ok: true, value: value}} }}\n\
func nagiErr[T any, E any](problem E) nagiResult[T, E] {{ return nagiResult[T, E]{{problem: problem}} }}\n\
type nagiFuture[T any] struct{{ body func() T; consumed bool }}\n\
func nagiCold[T any](body func() T) *nagiFuture[T] {{ return &nagiFuture[T]{{body: body}} }}\n\
func (future *nagiFuture[T]) await() T {{\n\
    if future == nil || future.consumed {{ panic(\"future already consumed\") }}\n\
    future.consumed = true\n\
    body := future.body\n\
    future.body = nil\n\
    defer func() {{ body = nil }}()\n\
    return body()\n\
}}\n\
{sleep_runtime}\
func nagiFixedDecimal(digits string, scale int) string {{\n\
    if scale >= 0 {{ return digits + strings.Repeat(\"0\", scale) }}\n\
    point := len(digits) + scale\n\
    if point > 0 {{ return digits[:point] + \".\" + digits[point:] }}\n\
    return \"0.\" + strings.Repeat(\"0\", -point) + digits\n\
}}\n\
func nagiDecimalRat(digits string, scale int) *big.Rat {{\n\
    numerator, valid := new(big.Int).SetString(digits, 10)\n\
    if !valid {{ panic(\"invalid generated decimal\") }}\n\
    powerScale := scale\n\
    if powerScale < 0 {{ powerScale = -powerScale }}\n\
    power := new(big.Int).Exp(big.NewInt(10), big.NewInt(int64(powerScale)), nil)\n\
    if scale >= 0 {{ return new(big.Rat).SetInt(numerator.Mul(numerator, power)) }}\n\
    return new(big.Rat).SetFrac(numerator, power)\n\
}}\n\
func nagiRustFloat(value float64, bits int) string {{\n\
    negative := math.Signbit(value)\n\
    magnitude := math.Abs(value)\n\
    scientific := strconv.FormatFloat(magnitude, 'e', -1, bits)\n\
    mantissa, exponentText, valid := strings.Cut(scientific, \"e\")\n\
    if !valid {{ panic(\"invalid generated float\") }}\n\
    digits := strings.ReplaceAll(mantissa, \".\", \"\")\n\
    exponent, problem := strconv.Atoi(exponentText)\n\
    if problem != nil {{ panic(\"invalid generated float exponent\") }}\n\
    scale := exponent - (len(digits) - 1)\n\
    text := nagiFixedDecimal(digits, scale)\n\
    candidate, valid := new(big.Int).SetString(digits, 10)\n\
    if !valid {{ panic(\"invalid generated float digits\") }}\n\
    candidate.Add(candidate, big.NewInt(1))\n\
    candidateDigits := candidate.String()\n\
    exact := new(big.Rat).SetFloat64(magnitude)\n\
    twiceExact := new(big.Rat).Mul(exact, big.NewRat(2, 1))\n\
    neighbours := new(big.Rat).Add(nagiDecimalRat(digits, scale), nagiDecimalRat(candidateDigits, scale))\n\
    if twiceExact.Cmp(neighbours) == 0 {{\n\
        candidateText := nagiFixedDecimal(candidateDigits, scale)\n\
        parsed, parseProblem := strconv.ParseFloat(candidateText, bits)\n\
        same := parseProblem == nil\n\
        if bits == 32 {{\n\
            same = same && math.Float32bits(float32(parsed)) == math.Float32bits(float32(magnitude))\n\
        }} else {{\n\
            same = same && math.Float64bits(parsed) == math.Float64bits(magnitude)\n\
        }}\n\
        if same {{ text = candidateText }}\n\
    }}\n\
    if negative {{ return \"-\" + text }}\n\
    return text\n\
}}\n\
func nagiDisplay[T any](value T) string {{\n\
    switch number := any(value).(type) {{\n\
    case float32:\n\
        if math.IsInf(float64(number), 1) {{ return \"inf\" }}\n\
        if math.IsInf(float64(number), -1) {{ return \"-inf\" }}\n\
        if math.IsNaN(float64(number)) {{ return \"NaN\" }}\n\
        return nagiRustFloat(float64(number), 32)\n\
    case float64:\n\
        if math.IsInf(number, 1) {{ return \"inf\" }}\n\
        if math.IsInf(number, -1) {{ return \"-inf\" }}\n\
        if math.IsNaN(number) {{ return \"NaN\" }}\n\
        return nagiRustFloat(number, 64)\n\
    default:\n\
        return fmt.Sprint(value)\n\
    }}\n\
}}\n\
func nagiPrint[T any](value T) nagiUnit {{ fmt.Println(nagiDisplay(value)); return nagiUnit{{}} }}\n\
func nagiWrite[T any](value T) nagiUnit {{ fmt.Print(nagiDisplay(value)); return nagiUnit{{}} }}\n\
func nagiAssertTrue(value bool) nagiUnit {{ if !value {{ panic(\"assertion failed\") }}; return nagiUnit{{}} }}\n\
func nagiParseI64(value string) nagiResult[int64, nagiError] {{ parsed, problem := strconv.ParseInt(value, 10, 64); if problem != nil {{ return nagiErr[int64](nagiError{{message: problem.Error()}}) }}; return nagiOk[int64, nagiError](parsed) }}\n\n{body}"
        ))
    }
}

pub(crate) fn source(
    checked: &CheckedProgram,
    go_files: &[PathBuf],
    profile: BuildProfile,
) -> Result<String, String> {
    Emitter::new(checked, go_files, profile)?.generate()
}

fn application_name(source: &Path, out: &Path) -> Result<String, String> {
    let stem = source.file_stem().ok_or("source has no file stem")?;
    let mut label = format!(
        "nagi-go-{}",
        stem.to_string_lossy()
            .chars()
            .map(|character| {
                if character == '_' || character == '-' || character.is_ascii_alphanumeric() {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>()
    );
    let mut identity = std::collections::hash_map::DefaultHasher::new();
    (
        fs::canonicalize(source).map_err(|error| error.to_string())?,
        out,
        "go",
    )
        .hash(&mut identity);
    label.push_str(&format!("-{:016x}", identity.finish()));
    Ok(label)
}

pub(crate) fn latest_path(source: &Path, out: &Path) -> Result<PathBuf, String> {
    Ok(out
        .join(".nagi/apps")
        .join(application_name(source, out)?)
        .join("latest.json"))
}

struct Staging {
    path: PathBuf,
    published: bool,
}
impl Drop for Staging {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    fs::write(path, bytes).map_err(|error| format!("Cannot write {}: {error}", path.display()))
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn require_plain_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|problem| format!("Cannot inspect Go projection {}: {problem}", path.display()))?;
    if is_link(&metadata) || !metadata.is_dir() {
        return Err(format!(
            "Go compatibility projection requires a plain directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn project_file(source: &Path, destination: &Path, id: &str) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| format!("Go projection has no parent: {}", destination.display()))?;
    require_plain_directory(parent)?;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "Go projection has an invalid name: {}",
                destination.display()
            )
        })?;
    let temporary = parent.join(format!(".{name}-{id}.tmp"));
    let result = (|| -> Result<(), String> {
        let mut input = fs::File::open(source).map_err(|problem| {
            format!("Cannot read Go generation {}: {problem}", source.display())
        })?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|problem| {
                format!(
                    "Cannot create Go projection temporary {}: {problem}",
                    temporary.display()
                )
            })?;
        std::io::copy(&mut input, &mut output).map_err(|problem| {
            format!(
                "Cannot copy Go projection {}: {problem}",
                destination.display()
            )
        })?;
        output
            .sync_all()
            .map_err(|problem| format!("Cannot flush Go projection: {problem}"))?;
        drop(output);
        match fs::symlink_metadata(destination) {
            Ok(metadata) if is_link(&metadata) => {
                return Err(format!(
                    "Go compatibility projection cannot replace a symlink: {}",
                    destination.display()
                ));
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(format!(
                    "Go compatibility projection requires a file: {}",
                    destination.display()
                ));
            }
            Ok(_) => fs::remove_file(destination).map_err(|problem| {
                format!(
                    "Cannot detach existing Go projection {}: {problem}",
                    destination.display()
                )
            })?,
            Err(problem) if problem.kind() == std::io::ErrorKind::NotFound => {}
            Err(problem) => {
                return Err(format!(
                    "Cannot inspect Go projection {}: {problem}",
                    destination.display()
                ));
            }
        }
        fs::rename(&temporary, destination).map_err(|problem| {
            format!(
                "Cannot publish Go projection {}: {problem}",
                destination.display()
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn go_command() -> std::ffi::OsString {
    std::env::var_os("NAGI_GO").unwrap_or_else(|| "go".into())
}

fn go_output(arguments: &[&str], directory: &Path) -> Result<std::process::Output, String> {
    Command::new(go_command())
        .args(arguments)
        .current_dir(directory)
        .env("CGO_ENABLED", "0")
        .env("GOTOOLCHAIN", "local")
        .env("GOWORK", "off")
        .output()
        .map_err(|problem| {
            if problem.kind() == std::io::ErrorKind::NotFound {
                format!("Go toolchain was not found. Install Go or set NAGI_GO to the go executable: {problem}")
            } else {
                format!("Cannot start Go toolchain: {problem}")
            }
        })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build(
    checked: &CheckedProgram,
    source_path: &Path,
    canonical_out: &Path,
    sources: &Sources,
    low: &str,
    inputs: &[PathBuf],
    go_files: &[PathBuf],
    profile: BuildProfile,
) -> Result<BuildResult, String> {
    let generated = source(checked, go_files, profile)?;
    let package = application_name(source_path, canonical_out)?;
    let app = canonical_out.join(".nagi/apps").join(&package);
    let generations = app.join("generations");
    fs::create_dir_all(&generations).map_err(|error| error.to_string())?;
    let (id, staging_path) = loop {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let id = format!(
            "g-{:x}-{now:x}-{:x}",
            std::process::id(),
            NEXT_GENERATION.fetch_add(1, Ordering::Relaxed)
        );
        let path = generations.join(format!(".staging-{id}"));
        match fs::create_dir(&path) {
            Ok(()) => break (id, path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Cannot create Go build generation: {error}")),
        }
    };
    let mut staging = Staging {
        path: staging_path,
        published: false,
    };
    fs::write(staging.path.join("main.go"), &generated).map_err(|error| error.to_string())?;
    let module = format!("module nagi.local/{package}\n\ngo 1.23.0\n");
    fs::write(staging.path.join("go.mod"), &module).map_err(|error| error.to_string())?;
    fs::write(staging.path.join("generated.low"), low).map_err(|error| error.to_string())?;
    let files = sources
        .files()
        .map(|(path, text, _)| {
            serde_json::json!({"path":path.to_string_lossy(),"path_os":crate::generation::os_path(path),"text":text})
        })
        .collect::<Vec<_>>();
    write_json(
        &staging.path.join("sources.json"),
        &serde_json::json!({"schema_version":1,"files":files}),
    )?;
    let executable_name = format!(
        "nagi-go-{}-{id}{}",
        package.rsplit('-').next().unwrap(),
        std::env::consts::EXE_SUFFIX
    );
    let executable = staging.path.join(&executable_name);
    let version = go_output(&["version"], &staging.path)?;
    let toolchain = String::from_utf8_lossy(&version.stdout).trim().to_owned();
    if !version.status.success() || !toolchain.starts_with("go version go") {
        let diagnostic = String::from_utf8_lossy(&version.stderr);
        return Err(format!(
            "Selected Go executable is not a usable Go toolchain. Set NAGI_GO to the official go executable: {diagnostic}"
        ));
    }
    let build_started = Instant::now();
    let mut command = Command::new(go_command());
    command
        .args(["build", "-trimpath", "-buildvcs=false", "-mod=readonly"])
        .arg("-o")
        .arg(&executable);
    if profile == BuildProfile::Release {
        command.args(["-ldflags", "-s -w"]);
    }
    let built = command
        .arg(".")
        .current_dir(&staging.path)
        .env("CGO_ENABLED", "0")
        .env("GOTOOLCHAIN", "local")
        .env("GOWORK", "off")
        .output()
        .map_err(|problem| {
            if problem.kind() == std::io::ErrorKind::NotFound {
                format!("Go toolchain was not found. Install Go or set NAGI_GO to the go executable: {problem}")
            } else {
                format!("Cannot start Go build: {problem}")
            }
        })?;
    let go_ms = build_started.elapsed().as_secs_f64() * 1000.0;
    if !built.status.success() {
        let diagnostic = String::from_utf8_lossy(&built.stderr);
        return Err(format!(
            "Go build failed after Nagi checking:\n{diagnostic}"
        ));
    }
    let environment = go_output(
        &[
            "env",
            "-json",
            "GOOS",
            "GOARCH",
            "GOVERSION",
            "GOROOT",
            "GOTOOLCHAIN",
        ],
        &staging.path,
    )?;
    if !environment.status.success() {
        return Err("go env failed after a successful build".into());
    }
    let environment: serde_json::Value = serde_json::from_slice(&environment.stdout)
        .map_err(|error| format!("Cannot parse go env output: {error}"))?;
    let modules = go_output(&["list", "-m", "-json", "all"], &staging.path)?;
    if !modules.status.success() {
        return Err("go list -m failed after a successful build".into());
    }
    let input_records = inputs
        .iter()
        .filter(|path| path.is_file())
        .map(|path| {
            Ok(serde_json::json!({
                "path":path.to_string_lossy(),
                "path_os":crate::generation::os_path(path),
                "sha256":crate::build_analysis::file_digest(path)?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    write_json(
        &staging.path.join("build-receipt.json"),
        &serde_json::json!({
            "schema_version":1,
            "backend":"go",
            "compiler":crate::build_analysis::file_digest(&std::env::current_exe().map_err(|error| error.to_string())?)?,
            "toolchain":toolchain,
            "environment":environment,
            "module_graph":String::from_utf8_lossy(&modules.stdout),
            "cgo_enabled":false,
            "toolchain_auto_download":false,
            "profile":profile.as_str(),
            "inputs":input_records,
            "artifacts":{
                "generated.low":crate::build_analysis::file_digest(&staging.path.join("generated.low"))?,
                "main.go":crate::build_analysis::file_digest(&staging.path.join("main.go"))?,
                "go.mod":crate::build_analysis::file_digest(&staging.path.join("go.mod"))?,
                "executable":crate::build_analysis::file_digest(&executable)?,
            }
        }),
    )?;
    let publish_started = Instant::now();
    let published = generations.join(&id);
    fs::rename(&staging.path, &published)
        .map_err(|error| format!("Cannot publish Go generation: {error}"))?;
    staging.published = true;
    let compatibility = canonical_out.join("go");
    match fs::create_dir(&compatibility) {
        Ok(()) => {}
        Err(problem) if problem.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(problem) => return Err(format!("Cannot create Go projection: {problem}")),
    }
    require_plain_directory(&compatibility)?;
    for name in ["main.go", "go.mod"] {
        project_file(&published.join(name), &compatibility.join(name), &id).map_err(|error| {
            format!("Compatibility projection may be partial; prior latest unchanged: {error}")
        })?;
    }
    let prefix = format!("generations/{id}/");
    let metadata = serde_json::json!({
        "schema_version":1,
        "backend":"go",
        "app_id":package,
        "generation":id,
        "executable":format!("{prefix}{executable_name}"),
        "low":format!("{prefix}generated.low"),
        "go":format!("{prefix}main.go"),
        "module":format!("{prefix}go.mod"),
        "sources":format!("{prefix}sources.json"),
        "receipt":format!("{prefix}build-receipt.json"),
    });
    fs::create_dir_all(&app).map_err(|error| error.to_string())?;
    let latest = app.join("latest.json");
    let temporary = app.join(format!(".latest-{id}.tmp"));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| {
                format!("Cannot create Go latest metadata; prior latest unchanged: {error}")
            })?;
        let bytes = serde_json::to_vec_pretty(&metadata).map_err(|error| error.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| {
                format!("Cannot write Go latest metadata; prior latest unchanged: {error}")
            })?;
    }
    fs::rename(&temporary, &latest).map_err(|error| {
        format!("Cannot replace Go latest metadata; prior latest unchanged: {error}")
    })?;
    Ok(BuildResult {
        binary: published.join(executable_name),
        go_ms,
        publish_ms: publish_started.elapsed().as_secs_f64() * 1000.0,
        toolchain,
    })
}
