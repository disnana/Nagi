//! Validation of typed scalar expressions, without rewriting runtime evaluation.
use crate::ast::{block_returns, BindingId, Expr, Function, NameResolution, Stmt, Type, E, S};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IntegerType {
    bits: u32,
    signed: bool,
}

impl IntegerType {
    fn of(ty: &Type) -> Option<Self> {
        if !ty.1.is_empty() {
            return None;
        }
        let (bits, signed) = match ty.0.as_str() {
            "i8" => (8, true),
            "i16" => (16, true),
            "i32" => (32, true),
            "i64" => (64, true),
            "u8" => (8, false),
            "u16" => (16, false),
            "u32" => (32, false),
            "u64" => (64, false),
            _ => return None,
        };
        Some(Self { bits, signed })
    }
    fn minimum(self) -> i128 {
        if self.signed {
            -(1_i128 << (self.bits - 1))
        } else {
            0
        }
    }
    fn maximum(self) -> i128 {
        (1_i128 << (self.bits - u32::from(self.signed))) - 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Integer(IntegerType, i128),
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unknown {
    Dynamic,
    ProfileDependent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Evaluation {
    Known(Value),
    Unknown(Unknown),
}

const DYNAMIC: Evaluation = Evaluation::Unknown(Unknown::Dynamic);

#[derive(Clone)]
struct Binding {
    id: BindingId,
    value: Evaluation,
}
type Facts = HashMap<String, Binding>;

fn internal(expr: &Expr) -> String {
    format!(
        "line {}: [E_CONST_INTERNAL] 定数検査に正常な型付き式がありません",
        expr.line
    )
}

fn zero_error(expr: &Expr, op: &str) -> String {
    format!("line {}: [E_CONST_ZERO_DIVISOR] 整数の{op}の除数に0は指定できません。ゼロ以外の値を指定してください", expr.line)
}

/// The positive magnitude of a signed minimum is valid only under this unary
/// leaf. Share recognition with Rust printing, rather than folding an expression.
pub(crate) fn signed_minimum(expr: &Expr) -> Option<&Type> {
    let E::Unary(op, operand) = &expr.kind else {
        return None;
    };
    let E::Int(magnitude) = &operand.kind else {
        return None;
    };
    let ty = expr.ty.as_ref()?;
    let integer = IntegerType::of(ty)?;
    (op == "-" && integer.signed && magnitude.parse::<i128>().ok() == Some(-integer.minimum()))
        .then_some(ty)
}

/// Keep the established literal-zero diagnostic at its original checker point.
/// Compound and binding facts are validated only after type checking succeeds.
pub(crate) fn validate_literal_divisor(expr: &Expr, op: &str) -> Result<(), String> {
    fn literal_zero(expr: &Expr) -> bool {
        match &expr.kind {
            E::Int(value) => value.parse::<u128>().ok() == Some(0),
            E::Unary(op, value) if op == "-" => literal_zero(value),
            _ => false,
        }
    }
    if literal_zero(expr) {
        Err(zero_error(expr, op))
    } else {
        Ok(())
    }
}

fn evaluate(expr: &Expr, facts: &Facts) -> Result<Evaluation, String> {
    let ty = expr.ty.as_ref().ok_or_else(|| internal(expr))?;
    if signed_minimum(expr).is_some() {
        let integer = IntegerType::of(ty).ok_or_else(|| internal(expr))?;
        return Ok(Evaluation::Known(Value::Integer(
            integer,
            integer.minimum(),
        )));
    }
    let known = |value| Ok(Evaluation::Known(value));
    match &expr.kind {
        E::Int(text) => {
            let integer = IntegerType::of(ty).ok_or_else(|| internal(expr))?;
            let value = text.parse::<i128>().map_err(|_| internal(expr))?;
            if value < integer.minimum() || value > integer.maximum() {
                return Err(internal(expr));
            }
            known(Value::Integer(integer, value))
        }
        E::Bool(value) => known(Value::Boolean(*value)),
        E::Name(name)
            if matches!(
                expr.resolution,
                Some(NameResolution::Local | NameResolution::BorrowedLocal)
            ) =>
        {
            facts
                .get(name)
                .map(|binding| binding.value)
                .ok_or_else(|| internal(expr))
        }
        E::Unary(op, operand) => match evaluate(operand, facts)? {
            Evaluation::Known(Value::Integer(integer, value)) if op == "-" => {
                bounded(integer, value.checked_neg())
            }
            Evaluation::Known(Value::Boolean(value)) if op == "not" => {
                known(Value::Boolean(!value))
            }
            Evaluation::Unknown(reason) => Ok(Evaluation::Unknown(reason)),
            _ => Ok(DYNAMIC),
        },
        E::Binary(left, op, right) => {
            // Visit both operands for syntactic validation, including a runtime
            // short-circuit RHS. This pass does not execute either operand.
            let lhs = evaluate(left, facts)?;
            let rhs = evaluate(right, facts)?;
            if matches!(op.as_str(), "/" | "%")
                && matches!(rhs, Evaluation::Known(Value::Integer(_, 0)))
            {
                return Err(zero_error(right, op));
            }
            match (lhs, rhs) {
                (
                    Evaluation::Known(Value::Integer(integer, a)),
                    Evaluation::Known(Value::Integer(other, b)),
                ) => {
                    if integer != other {
                        return Err(internal(expr));
                    }
                    match op.as_str() {
                        "+" => bounded(integer, a.checked_add(b)),
                        "-" => bounded(integer, a.checked_sub(b)),
                        "*" => bounded(integer, a.checked_mul(b)),
                        "/" | "%" => {
                            if integer.signed && a == integer.minimum() && b == -1 {
                                return Err(format!("line {}: [E_CONST_SIGNED_DIV_OVERFLOW] {}の整数の{op}は最小値と-1の組合せでoverflowし、常にpanicします", expr.line, left.ty.as_ref().ok_or_else(|| internal(left))?));
                            }
                            bounded(
                                integer,
                                if op == "/" {
                                    a.checked_div(b)
                                } else {
                                    a.checked_rem(b)
                                },
                            )
                        }
                        "==" => known(Value::Boolean(a == b)),
                        "!=" => known(Value::Boolean(a != b)),
                        "<" => known(Value::Boolean(a < b)),
                        ">" => known(Value::Boolean(a > b)),
                        "<=" => known(Value::Boolean(a <= b)),
                        ">=" => known(Value::Boolean(a >= b)),
                        _ => Err(internal(expr)),
                    }
                }
                (Evaluation::Known(Value::Boolean(a)), Evaluation::Known(Value::Boolean(b))) => {
                    match op.as_str() {
                        "and" => known(Value::Boolean(a && b)),
                        "or" => known(Value::Boolean(a || b)),
                        "==" => known(Value::Boolean(a == b)),
                        "!=" => known(Value::Boolean(a != b)),
                        "<" => known(Value::Boolean(!a & b)),
                        ">" => known(Value::Boolean(a & !b)),
                        "<=" => known(Value::Boolean(a <= b)),
                        ">=" => known(Value::Boolean(a >= b)),
                        _ => Err(internal(expr)),
                    }
                }
                (Evaluation::Known(Value::Boolean(false)), _) if op == "and" => {
                    known(Value::Boolean(false))
                }
                (Evaluation::Known(Value::Boolean(true)), _) if op == "or" => {
                    known(Value::Boolean(true))
                }
                (Evaluation::Unknown(Unknown::ProfileDependent), _)
                | (_, Evaluation::Unknown(Unknown::ProfileDependent)) => {
                    Ok(Evaluation::Unknown(Unknown::ProfileDependent))
                }
                _ => Ok(DYNAMIC),
            }
        }
        E::Call(name, _, arguments) => {
            let values = arguments
                .iter()
                .map(|arg| evaluate(arg, facts))
                .collect::<Result<Vec<_>, _>>()?;
            if name == "i64" && expr.resolution == Some(NameResolution::Builtin) {
                if let [Evaluation::Known(Value::Integer(_, value))] = values.as_slice() {
                    let integer = IntegerType::of(ty).ok_or_else(|| internal(expr))?;
                    return bounded(integer, Some(*value));
                }
            }
            Ok(DYNAMIC)
        }
        E::Field(base, _) => {
            // A registered constant's type name is not a runtime expression and
            // intentionally has no expression type in the checked AST.
            if expr.resolution != Some(NameResolution::ResourceConstant)
                && expr.resolution != Some(NameResolution::Enum)
            {
                evaluate(base, facts)?;
            }
            Ok(DYNAMIC)
        }
        E::Index(base, index) => {
            evaluate(base, facts)?;
            evaluate(index, facts)?;
            Ok(DYNAMIC)
        }
        E::List(values) => {
            for value in values {
                evaluate(value, facts)?;
            }
            Ok(DYNAMIC)
        }
        E::Record(_, fields) => {
            for (_, value) in fields {
                evaluate(value, facts)?;
            }
            Ok(DYNAMIC)
        }
        E::Await(value) | E::Try(value) => {
            evaluate(value, facts)?;
            Ok(DYNAMIC)
        }
        E::Float(_) | E::Str(_) | E::Null | E::Name(_) => Ok(DYNAMIC),
    }
}

fn bounded(integer: IntegerType, value: Option<i128>) -> Result<Evaluation, String> {
    Ok(match value {
        Some(value) if value >= integer.minimum() && value <= integer.maximum() => {
            Evaluation::Known(Value::Integer(integer, value))
        }
        _ => Evaluation::Unknown(Unknown::ProfileDependent),
    })
}

fn statement_id(statement: &Stmt) -> BindingId {
    BindingId {
        line: statement.line,
        token: statement.binding_span.unwrap_or_default().start,
    }
}

fn join(facts: &mut Facts, paths: &[Facts]) {
    for (name, binding) in facts.iter_mut() {
        let candidate = paths.first().and_then(|path| path.get(name));
        binding.value = match candidate {
            Some(candidate)
                if matches!(candidate.value, Evaluation::Known(_))
                    && paths.iter().all(|path| {
                        path.get(name).is_some_and(|other| {
                            other.id == binding.id && other.value == candidate.value
                        })
                    }) =>
            {
                candidate.value
            }
            _ => DYNAMIC,
        };
    }
}

/// Collect writes to identities visible at the loop header. A loop counter
/// with the same spelling is a different binding and must not kill its owner.
fn writes(
    statements: &[Stmt],
    visible: &mut HashMap<String, BindingId>,
    output: &mut HashSet<BindingId>,
) {
    for statement in statements {
        match &statement.kind {
            S::Assign { name, declare, .. } => {
                if *declare {
                    visible.insert(name.clone(), statement_id(statement));
                } else if let Some(id) = visible.get(name) {
                    output.insert(*id);
                }
            }
            S::If(_, a, b) => {
                writes(a, &mut visible.clone(), output);
                writes(b, &mut visible.clone(), output);
            }
            S::Match(_, arms) => {
                for arm in arms {
                    let mut local = visible.clone();
                    for binding in arm.pattern.bindings() {
                        if let Some(name) = &binding.name {
                            local.insert(
                                name.clone(),
                                BindingId {
                                    line: arm.line,
                                    token: binding.span.start,
                                },
                            );
                        }
                    }
                    writes(&arm.body, &mut local, output);
                }
            }
            S::For(name, _, body) => {
                let mut local = visible.clone();
                local.insert(name.clone(), statement_id(statement));
                writes(body, &mut local, output);
            }
            S::While(_, body) | S::Scope(body) => writes(body, &mut visible.clone(), output),
            _ => {}
        }
    }
}

fn kill_loop_writes(body: &[Stmt], facts: &mut Facts, counter: Option<(&str, BindingId)>) {
    let mut visible = facts
        .iter()
        .map(|(name, binding)| (name.clone(), binding.id))
        .collect::<HashMap<_, _>>();
    if let Some((name, id)) = counter {
        visible.insert(name.into(), id);
    }
    let mut written = HashSet::new();
    writes(body, &mut visible, &mut written);
    for binding in facts.values_mut() {
        if written.contains(&binding.id) {
            binding.value = DYNAMIC;
        }
    }
}

fn block(statements: &[Stmt], facts: &mut Facts) -> Result<(), String> {
    for statement in statements {
        match &statement.kind {
            S::Assign {
                name,
                value,
                declare,
                ..
            } => {
                let value = evaluate(value, facts)?;
                let id = if *declare {
                    statement_id(statement)
                } else {
                    facts
                        .get(name)
                        .ok_or_else(|| {
                            format!(
                                "line {}: [E_CONST_INTERNAL] 代入先のbindingがありません",
                                statement.line
                            )
                        })?
                        .id
                };
                facts.insert(name.clone(), Binding { id, value });
            }
            S::Return(Some(value)) | S::Expr(value) | S::Spawn(value) => {
                evaluate(value, facts)?;
            }
            S::Return(None) => {}
            S::If(condition, a, b) => {
                evaluate(condition, facts)?;
                let mut paths = Vec::new();
                for body in [a, b] {
                    let mut local = facts.clone();
                    block(body, &mut local)?;
                    if !block_returns(body) {
                        paths.push(local);
                    }
                }
                join(facts, &paths);
            }
            S::Match(value, arms) => {
                evaluate(value, facts)?;
                let mut paths = Vec::new();
                for arm in arms {
                    let mut local = facts.clone();
                    for binding in arm.pattern.bindings() {
                        if let Some(name) = &binding.name {
                            local.insert(
                                name.clone(),
                                Binding {
                                    id: BindingId {
                                        line: arm.line,
                                        token: binding.span.start,
                                    },
                                    value: DYNAMIC,
                                },
                            );
                        }
                    }
                    block(&arm.body, &mut local)?;
                    if !block_returns(&arm.body) {
                        paths.push(local);
                    }
                }
                join(facts, &paths);
            }
            S::While(condition, body) => {
                kill_loop_writes(body, facts, None);
                evaluate(condition, facts)?;
                block(body, &mut facts.clone())?;
            }
            S::For(name, iterator, body) => {
                evaluate(iterator, facts)?;
                let id = statement_id(statement);
                kill_loop_writes(body, facts, Some((name, id)));
                let mut local = facts.clone();
                local.insert(name.clone(), Binding { id, value: DYNAMIC });
                block(body, &mut local)?;
            }
            S::Scope(body) => {
                let mut local = facts.clone();
                block(body, &mut local)?;
                join(facts, &[local]);
            }
        }
    }
    Ok(())
}

pub(crate) fn validate(function: &Function) -> Result<(), String> {
    let mut facts = Facts::new();
    for ((name, _), span) in function.params.iter().zip(&function.parameter_spans) {
        facts.insert(
            name.clone(),
            Binding {
                id: BindingId {
                    line: function.line,
                    token: span.start,
                },
                value: DYNAMIC,
            },
        );
    }
    block(&function.body, &mut facts)
}
