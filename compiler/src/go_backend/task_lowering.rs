//! Private proof of a single checked Task path. This module is test-build only.
use super::{error, ident, Emitter};
use crate::{
    ast::{Expr, ExprUseId, NameResolution, Stmt, Type, E, S},
    checked::{
        CheckedProgram, ErrorDestination, ExpressionKey, ScopeBodyErrorExit, ScopeBridge,
        ScopeNormalExit, ScopeOwnerTeardown, TaskSpawnMode, TaskUseMode,
    },
    project::BuildProfile,
};

#[derive(Clone, Copy)]
pub(super) struct SingleTask<'a> {
    pub(super) scope: &'a Stmt,
    pub(super) invocation: &'a Expr,
    handle: &'a str,
    received: &'a str,
    body_try: Option<BodyTry<'a>>,
}

#[derive(Clone, Copy)]
enum BodyTry<'a> {
    Before(&'a Stmt),
    After(&'a Stmt),
}

impl<'a> SingleTask<'a> {
    fn from_checked(checked: &'a CheckedProgram) -> Result<Self, String> {
        let program = checked.program();
        let (index, main) = program
            .functions
            .iter()
            .enumerate()
            .find(|(_, function)| function.name == "main")
            .ok_or("missing checked main for internal Go Task lowering")?;
        let unsupported = || {
            error(
                main.line,
                "unsupported shape for internal single-child Go Task lowering",
            )
        };
        let [scope, end] = main.body.as_slice() else {
            return Err(unsupported());
        };
        if !main.asynchronous
            || main.ret != Type("Result".into(), vec![scalar("unit"), scalar("Error")])
            || !matches!(&end.kind, S::Return(Some(value))
                if builtin_argument(value, &["ok"]).is_some_and(|argument|
                    literal_builtin(argument, &["print"], |kind| matches!(kind, E::Str(_)))))
        {
            return Err(unsupported());
        }
        let S::Scope(body) = &scope.kind else {
            return Err(unsupported());
        };
        let (spawn, receive, body_try) = match body.as_slice() {
            [spawn, receive] => (spawn, receive, None),
            [spawn, first, second] if literal_body_try(first) => {
                (spawn, second, Some(BodyTry::Before(first)))
            }
            [spawn, first, second] if literal_body_try(second) => {
                (spawn, first, Some(BodyTry::After(second)))
            }
            _ => return Err(unsupported()),
        };
        let S::SpawnBind {
            name: handle,
            value: invocation,
            declare: true,
            ..
        } = &spawn.kind
        else {
            return Err(unsupported());
        };
        let S::Assign {
            name: received,
            value: awaiting,
            declare: true,
            ..
        } = &receive.kind
        else {
            return Err(unsupported());
        };
        let E::Await(operand) = &awaiting.kind else {
            return Err(unsupported());
        };
        if !matches!(&operand.kind, E::Name(name) if name == handle)
            || operand.resolution != Some(NameResolution::Local)
        {
            return Err(unsupported());
        }
        let E::Call(child_name, types, arguments) = &invocation.kind else {
            return Err(unsupported());
        };
        if invocation.resolution != Some(NameResolution::Function)
            || !types.is_empty()
            || !arguments.is_empty()
        {
            return Err(unsupported());
        }
        let result = Type("Result".into(), vec![scalar("i64"), scalar("i64")]);
        let child = program
            .functions
            .iter()
            .find(|child| &child.name == child_name)
            .ok_or_else(unsupported)?;
        if !child.asynchronous || child.external || !child.params.is_empty() || child.ret != result
        {
            return Err(unsupported());
        }
        // Limit the admitted child to literal outcomes and an optional builtin
        // assertion. No other calls, captures or resource-bearing paths enter.
        let child_return = match child.body.as_slice() {
            [last] => last,
            [first, last]
                if matches!(&first.kind, S::Expr(value)
                if literal_builtin(value, &["assert_true"], |kind| matches!(kind, E::Bool(_)))) =>
            {
                last
            }
            _ => return Err(unsupported()),
        };
        if !matches!(&child_return.kind, S::Return(Some(value))
            if literal_builtin(value, &["ok", "fail"], |kind| matches!(kind, E::Int(_))))
        {
            return Err(unsupported());
        }

        let plan = &checked.emission.functions[index];
        let scope_statement = plan.body.statements.first().ok_or_else(unsupported)?;
        let exit = scope_statement
            .scope_exit
            .as_ref()
            .ok_or_else(unsupported)?;
        let [block] = scope_statement.children.as_slice() else {
            return Err(unsupported());
        };
        let start = block
            .statements
            .first()
            .and_then(|entry| entry.task_spawn.as_ref())
            .ok_or_else(unsupported)?;
        let take = plan
            .expressions
            .get(&ExpressionKey::of(awaiting))
            .and_then(|entry| entry.task.as_ref())
            .ok_or_else(unsupported)?;
        if exit.bridge != ScopeBridge::TaskScope
            || exit.enclosing_error_destination != ErrorDestination::Function
            || exit.normal_exit != ScopeNormalExit::JoinAndPropagate
            || exit.owner_teardown != ScopeOwnerTeardown::AbortRequestOnly
            || exit.body_error_exit
                != ScopeBodyErrorExit::LeaveBodyThenCancelDrainAndPropagateOriginal
            || exit.body_error_destination
                != (ErrorDestination::Scope {
                    id: exit.scope,
                    nesting: 1,
                })
            || Some(exit.scope) != scope.task.scope
            || start.bridge != ScopeBridge::TaskScope
            || start.scope != exit.scope
            || take.scope != exit.scope
            || start.invocation != ExprUseId::of(invocation)
            || take.expression != ExprUseId::of(awaiting)
            || take.operand != ExprUseId::of(operand)
            || !matches!(&start.mode, TaskSpawnMode::ValueHandle { binding, output }
                if Some(*binding) == spawn.task.spawn_binding && output == &result)
            || take.mode != (TaskUseMode::ReceiveJoinedValue { output: result })
        {
            return Err(unsupported());
        }
        Ok(Self {
            scope,
            invocation,
            handle,
            received,
            body_try,
        })
    }

    pub(super) fn validate(&self, emitter: &Emitter<'a>, ret: &Type) -> Result<(), String> {
        emitter.validate_expr(self.invocation, ret)?;
        if let Some(BodyTry::Before(statement) | BodyTry::After(statement)) = self.body_try {
            emitter.validate_statement(statement, ret)?;
        }
        Ok(())
    }

    pub(super) fn lower(
        self,
        emitter: &mut Emitter<'a>,
        ret: &Type,
        indent: usize,
        output: &mut String,
    ) -> Result<(), String> {
        let scope = emitter.temp("task_scope");
        let (before, future) = emitter.evaluated(self.invocation, ret)?;
        let handle = ident(self.handle);
        let received = ident(self.received);
        Emitter::line(output, indent, "{");
        let indent = indent + 1;
        Emitter::line(output, indent, format!("{scope} := taskscope.New()"));
        let body_result = self.body_try.map(|_| emitter.temp("task_body"));
        // The ordinary try lowering returns to this lexical body first.
        // Outside it, FinishBodyError retires the output and drains the child.
        if let Some(body) = &body_result {
            Emitter::line(
                output,
                indent,
                format!("{body} := func() nagiResult[nagiUnit, nagiError] {{"),
            );
        }
        let body_indent = indent + usize::from(body_result.is_some());
        for line in before {
            Emitter::line(output, body_indent, line);
        }
        Emitter::line(
            output,
            body_indent,
            format!("{handle} := nagiTaskSpawn({scope}, {future}, nagiTaskCleanup)"),
        );
        if let Some(BodyTry::Before(statement)) = self.body_try {
            Emitter::line(
                output,
                body_indent,
                "if nagiTaskBeforeBodyTry != nil { nagiTaskBeforeBodyTry() }",
            );
            emitter.statements(std::slice::from_ref(statement), ret, body_indent, output)?;
        }
        Emitter::line(
            output,
            body_indent,
            format!("{received} := nagiTaskReceive({scope}, {handle})"),
        );
        Emitter::line(
            output,
            body_indent,
            format!("if nagiTaskObserve != nil {{ nagiTaskObserve({received}) }}"),
        );
        if let Some(BodyTry::After(statement)) = self.body_try {
            emitter.statements(std::slice::from_ref(statement), ret, body_indent, output)?;
        }
        if let Some(body) = body_result {
            Emitter::line(
                output,
                body_indent,
                "return nagiOk[nagiUnit, nagiError](nagiUnit{})",
            );
            Emitter::line(output, indent, "}()");
            Emitter::line(output, indent, format!(
                "if !{body}.ok {{ return nagiErr[nagiUnit](taskscope.FinishBodyError({scope}, {body}.problem)) }}"
            ));
        }
        Emitter::line(
            output,
            indent,
            format!("if exit := nagiTaskFinish({scope}); !exit.ok {{ return exit }}"),
        );
        Emitter::line(output, indent - 1, "}");
        Ok(())
    }
}

fn literal_body_try(statement: &Stmt) -> bool {
    matches!(&statement.kind, S::Assign { annotation: Some(ty), value, declare: true, .. }
        if ty == &scalar("i64")
        && matches!(&value.kind, E::Try(error)
            if literal_builtin(error, &["error"], |kind| matches!(kind, E::Str(_)))))
}

fn scalar(name: &str) -> Type {
    Type(name.into(), vec![])
}

fn literal_builtin(value: &Expr, names: &[&str], literal: impl FnOnce(&E) -> bool) -> bool {
    builtin_argument(value, names).is_some_and(|argument| literal(&argument.kind))
}

fn builtin_argument<'a>(value: &'a Expr, names: &[&str]) -> Option<&'a Expr> {
    let E::Call(name, types, arguments) = &value.kind else {
        return None;
    };
    if value.resolution != Some(NameResolution::Builtin)
        || !names.contains(&name.as_str())
        || !types.is_empty()
    {
        return None;
    }
    let [argument] = arguments.as_slice() else {
        return None;
    };
    Some(argument)
}

pub(super) fn source(checked: &CheckedProgram) -> Result<String, String> {
    let mut emitter = Emitter::from_checked(checked, &[], BuildProfile::Dev)?;
    emitter.single_task = Some(SingleTask::from_checked(checked)?);
    emitter.validate()?;
    emitter.generate()
}

// Shared by the generated scope's typed receive and exit. These kinds are
// confined to the internal module, and unknown private faults fail closed.
pub(super) const RUNTIME: &str = r#"
type nagiTaskFailure struct { kind uint8; message string }
const nagiTaskPanicked uint8 = 1
const nagiTaskInternal uint8 = nagiBusy + 1
const nagiChildPanicked = "child panicked"

var nagiTaskCleanup func()
var nagiTaskObserve func(nagiResult[nagiResult[int64, int64], nagiTaskFailure])
var nagiTaskChildDone func(<-chan struct{})
var nagiTaskBeforeBodyTry func()

func nagiTaskSpawn[T any](scope *taskscope.Scope, future *nagiFuture[T], cleanup func()) taskscope.Handle[T] {
    handle, fault := taskscope.Spawn(scope, func(ctx context.Context) T {
        if nagiTaskChildDone != nil { nagiTaskChildDone(ctx.Done()) }
        return future.await()
    }, cleanup)
    if fault != nil { panic(fault) }
    return handle
}

func nagiTaskReceive[T any](scope *taskscope.Scope, handle taskscope.Handle[T]) nagiResult[T, nagiTaskFailure] {
    value, fault := taskscope.Receive(scope, handle)
    if fault == nil { return nagiOk[T, nagiTaskFailure](value) }
    if fault != taskscope.ErrTaskPanicked { panic(fault) }
    return nagiErr[T](nagiTaskFailure{kind: nagiTaskPanicked, message: nagiChildPanicked})
}

func nagiTaskFinish(scope *taskscope.Scope) nagiResult[nagiUnit, nagiError] {
    if fault := scope.Finish(); fault != nil {
        if fault != taskscope.ErrTaskPanicked { panic(fault) }
        return nagiErr[nagiUnit](nagiError{kind: nagiTaskInternal, message: nagiChildPanicked})
    }
    return nagiOk[nagiUnit, nagiError](nagiUnit{})
}
"#;
