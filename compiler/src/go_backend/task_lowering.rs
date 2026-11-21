//! Private proof of a single checked Task path. This module is test-build only.
use super::{error, ident, Emitter};
use crate::{
    ast::{Expr, ExprUseId, NameResolution, Stmt, Type, E, S},
    checked::{
        CheckedProgram, ErrorDestination, ExpressionKey, ScopeBodyErrorExit, ScopeBridge,
        ScopeNormalExit, ScopeOwnerTeardown, TaskSpawnMode, TaskUseMode,
    },
    project::BuildProfile,
    stdlib::{self, Operation},
};

#[derive(Clone, Copy)]
pub(super) struct SingleTask<'a> {
    pub(super) scope: &'a Stmt,
    pub(super) invocation: &'a Expr,
    handle: &'a str,
    completion: TaskCompletion<'a>,
    body_step: Option<BodyStep<'a>>,
    child_sleep: Option<ChildSleep<'a>>,
}

#[derive(Clone, Copy)]
struct ChildSleep<'a> {
    milliseconds: i32,
    result: &'a Expr,
}

#[derive(Clone, Copy)]
enum BodyStep<'a> {
    TryBefore(&'a Stmt),
    TryAfter(&'a Stmt),
    AssertBefore(&'a Stmt),
    ParentWait(&'a Stmt, i32),
}

#[derive(Clone, Copy)]
enum TaskCompletion<'a> {
    Receive(&'a str),
    Discard,
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
        let (spawn, consume, body_step) = match body.as_slice() {
            [spawn, consume] => (spawn, consume, None),
            [spawn, first, second] if literal_body_try(first) => {
                (spawn, second, Some(BodyStep::TryBefore(first)))
            }
            [spawn, first, second] if literal_body_try(second) => {
                (spawn, first, Some(BodyStep::TryAfter(second)))
            }
            [spawn, assertion, consume] if literal_assertion(assertion) => {
                (spawn, consume, Some(BodyStep::AssertBefore(assertion)))
            }
            [spawn, wait, consume] => (
                spawn,
                consume,
                Some(BodyStep::ParentWait(
                    wait,
                    literal_sleep(wait).ok_or_else(unsupported)?,
                )),
            ),
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
        let (completion, command, operand) = match &consume.kind {
            S::Assign {
                name,
                value,
                declare: true,
                ..
            } => {
                let E::Await(operand) = &value.kind else {
                    return Err(unsupported());
                };
                (TaskCompletion::Receive(name), value, operand.as_ref())
            }
            S::Expr(value) if body_step.is_none() => {
                let E::Call(name, types, arguments) = &value.kind else {
                    return Err(unsupported());
                };
                let [operand] = arguments.as_slice() else {
                    return Err(unsupported());
                };
                if value.resolution != Some(NameResolution::Standard)
                    || stdlib::operation(name) != Some(Operation::TaskDiscard)
                    || !types.is_empty()
                {
                    return Err(unsupported());
                }
                (TaskCompletion::Discard, value, operand)
            }
            _ => return Err(unsupported()),
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
        // The timer variant has a single suspension point before its literal
        // result. Discard and post-receive body errors keep the existing shape.
        let (child_return, sleep) = match child.body.as_slice() {
            [last] => (last, None),
            [first, last] if literal_assertion(first) => (last, None),
            [first, last]
                if matches!(completion, TaskCompletion::Receive(_))
                    && !matches!(body_step, Some(BodyStep::TryAfter(_))) =>
            {
                (last, Some(literal_sleep(first).ok_or_else(unsupported)?))
            }
            _ => return Err(unsupported()),
        };
        if matches!(body_step, Some(BodyStep::ParentWait(..))) && sleep.is_none() {
            return Err(unsupported());
        }
        let S::Return(Some(child_result)) = &child_return.kind else {
            return Err(unsupported());
        };
        if !literal_builtin(child_result, &["ok", "fail"], |kind| {
            matches!(kind, E::Int(_))
        }) {
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
            .get(&ExpressionKey::of(command))
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
            || take.expression != ExprUseId::of(command)
            || take.operand != ExprUseId::of(operand)
            || !matches!(&start.mode, TaskSpawnMode::ValueHandle { binding, output }
                if Some(*binding) == spawn.task.spawn_binding && output == &result)
            || take.mode
                != match completion {
                    TaskCompletion::Receive(_) => {
                        TaskUseMode::ReceiveJoinedValue { output: result }
                    }
                    TaskCompletion::Discard => TaskUseMode::DiscardOutputKeepScopeJoin,
                }
        {
            return Err(unsupported());
        }
        Ok(Self {
            scope,
            invocation,
            handle,
            completion,
            body_step,
            child_sleep: sleep.map(|milliseconds| ChildSleep {
                milliseconds,
                result: child_result,
            }),
        })
    }

    pub(super) fn validate(&self, emitter: &Emitter<'a>, ret: &Type) -> Result<(), String> {
        emitter.validate_expr(self.invocation, ret)?;
        if let Some(
            BodyStep::TryBefore(statement)
            | BodyStep::TryAfter(statement)
            | BodyStep::AssertBefore(statement)
            | BodyStep::ParentWait(statement, _),
        ) = self.body_step
        {
            emitter.validate_statement(statement, ret)?;
        }
        Ok(())
    }

    pub(super) fn has_parent_wait(self) -> bool {
        matches!(self.body_step, Some(BodyStep::ParentWait(..)))
    }

    pub(super) fn lower_parent_main(
        self,
        emitter: &mut Emitter<'a>,
        ret: &Type,
        end: &Stmt,
        output: &mut String,
    ) -> Result<(), String> {
        let S::Return(Some(value)) = &end.kind else {
            unreachable!("validated single-task main return");
        };
        Emitter::line(
            output,
            0,
            "func nagiTaskParent(parent context.Context) (nagiResult[nagiUnit, nagiError], bool) {",
        );
        self.lower(emitter, ret, 1, output)?;
        let (before, value) = emitter.expression(value, ret)?;
        for line in before {
            Emitter::line(output, 1, line);
        }
        Emitter::line(output, 1, format!("return {value}, true"));
        Emitter::line(output, 0, "}");
        // Keep the regular entry usable with its non-cancelling background.
        output.push_str("func nagiMain() *nagiFuture[nagiResult[nagiUnit, nagiError]] {\n    return nagiCold(func() nagiResult[nagiUnit, nagiError] {\n        result, _ := nagiTaskParent(context.Background())\n        return result\n    })\n}\n\n");
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
        let handle = ident(self.handle);
        Emitter::line(output, indent, "{");
        let indent = indent + 1;
        Emitter::line(output, indent, format!("{scope} := taskscope.New()"));
        let parent_finished = self
            .has_parent_wait()
            .then(|| emitter.temp("parent_finished"));
        if let Some(finished) = &parent_finished {
            Emitter::line(output, indent, format!("{finished} := false"));
            Emitter::line(
                output,
                indent,
                format!("defer func() {{ if !{finished} {{ {scope}.RequestAbnormalExit() }} }}()"),
            );
        }
        let body_result = matches!(
            self.body_step,
            Some(BodyStep::TryBefore(_) | BodyStep::TryAfter(_))
        )
        .then(|| emitter.temp("task_body"));
        let body_asserts = matches!(self.body_step, Some(BodyStep::AssertBefore(_)));
        // The ordinary try lowering returns to this lexical body first.
        // Outside it, FinishBodyError retires the output and drains the child.
        if let Some(body) = &body_result {
            Emitter::line(
                output,
                indent,
                format!("{body} := func() nagiResult[nagiUnit, nagiError] {{"),
            );
        } else if body_asserts {
            Emitter::line(output, indent, format!("nagiTaskBody({scope}, func() {{"));
        }
        let body_indent = indent + usize::from(body_result.is_some() || body_asserts);
        if let Some(sleep) = self.child_sleep {
            sleep.emit_spawn(emitter, &scope, &handle, body_indent, output)?;
        } else {
            let (before, future) = emitter.evaluated(self.invocation, ret)?;
            for line in before {
                Emitter::line(output, body_indent, line);
            }
            Emitter::line(
                output,
                body_indent,
                format!("{handle} := nagiTaskSpawn({scope}, {future}, nagiTaskCleanup)"),
            );
        }
        match self.body_step {
            Some(BodyStep::TryBefore(statement)) => {
                Emitter::line(
                    output,
                    body_indent,
                    "if nagiTaskBeforeBodyTry != nil { nagiTaskBeforeBodyTry() }",
                );
                emitter.statements(std::slice::from_ref(statement), ret, body_indent, output)?;
            }
            Some(BodyStep::AssertBefore(statement)) => {
                Emitter::line(output, body_indent, format!(
                    "if nagiTaskBeforeBodyAssert != nil {{ nagiTaskBeforeBodyAssert({scope}, {handle}) }}"
                ));
                emitter.statements(std::slice::from_ref(statement), ret, body_indent, output)?;
            }
            Some(BodyStep::ParentWait(_, milliseconds)) => {
                Emitter::line(
                    output,
                    body_indent,
                    format!("if !nagiTaskWaitParent(parent, {milliseconds}, {scope}, {handle}) {{"),
                );
                Emitter::line(
                    output,
                    body_indent + 1,
                    "return nagiResult[nagiUnit, nagiError]{}, false",
                );
                Emitter::line(output, body_indent, "}");
            }
            _ => {}
        }
        match self.completion {
            TaskCompletion::Receive(name) => {
                let received = ident(name);
                if self.has_parent_wait() {
                    let completed = emitter.temp("receive_completed");
                    Emitter::line(output, body_indent, format!(
                        "{received}, {completed} := nagiTaskReceiveParent(parent, {scope}, {handle})"
                    ));
                    Emitter::line(
                        output,
                        body_indent,
                        format!(
                        "if !{completed} {{ return nagiResult[nagiUnit, nagiError]{{}}, false }}"
                    ),
                    );
                } else {
                    Emitter::line(
                        output,
                        body_indent,
                        format!("{received} := nagiTaskReceive({scope}, {handle})"),
                    );
                }
                Emitter::line(
                    output,
                    body_indent,
                    format!("if nagiTaskObserve != nil {{ nagiTaskObserve({received}) }}"),
                );
            }
            TaskCompletion::Discard => {
                Emitter::line(
                    output,
                    body_indent,
                    format!("if fault := taskscope.Discard({scope}, {handle}); fault != nil {{ panic(fault) }}"),
                );
                Emitter::line(
                    output,
                    body_indent,
                    "if nagiTaskDiscarded != nil { nagiTaskDiscarded() }",
                );
            }
        }
        if let Some(BodyStep::TryAfter(statement)) = self.body_step {
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
        } else if body_asserts {
            Emitter::line(output, indent, "})");
        }
        if let Some(finished) = &parent_finished {
            Emitter::line(output, indent, format!("exit := nagiTaskFinish({scope})"));
            Emitter::line(output, indent, format!("{finished} = true"));
            Emitter::line(output, indent, "if !exit.ok { return exit, true }");
        } else {
            Emitter::line(
                output,
                indent,
                format!("if exit := nagiTaskFinish({scope}); !exit.ok {{ return exit }}"),
            );
        }
        Emitter::line(output, indent - 1, "}");
        Ok(())
    }
}

impl ChildSleep<'_> {
    fn emit_spawn(
        self,
        emitter: &mut Emitter<'_>,
        scope: &str,
        handle: &str,
        indent: usize,
        output: &mut String,
    ) -> Result<(), String> {
        let fault = emitter.temp("task_spawn_fault");
        let result_type = Type("Result".into(), vec![scalar("i64"), scalar("i64")]);
        let (before, result) = emitter.expression(self.result, &result_type)?;
        Emitter::line(output, indent, format!(
            "{handle}, {fault} := taskscope.SpawnInterruptible({scope}, func(ctx context.Context) (nagiResult[int64, int64], bool) {{"
        ));
        Emitter::line(
            output,
            indent + 1,
            format!(
                "timer := time.NewTimer(time.Duration({}) * time.Millisecond)",
                self.milliseconds
            ),
        );
        Emitter::line(output, indent + 1, "defer timer.Stop()");
        Emitter::line(
            output,
            indent + 1,
            "if nagiTaskSleepStarted != nil { nagiTaskSleepStarted() }",
        );
        Emitter::line(output, indent + 1, "select {");
        Emitter::line(output, indent + 1, "case <-ctx.Done():");
        Emitter::line(
            output,
            indent + 2,
            "if nagiTaskSleepInterrupted != nil { nagiTaskSleepInterrupted() }",
        );
        Emitter::line(
            output,
            indent + 2,
            "return nagiResult[int64, int64]{}, false",
        );
        Emitter::line(output, indent + 1, "case <-timer.C:");
        Emitter::line(output, indent + 1, "}");
        for line in before {
            Emitter::line(output, indent + 1, line);
        }
        Emitter::line(output, indent + 1, format!("return {result}, true"));
        Emitter::line(output, indent, "}, nagiTaskCleanup)");
        Emitter::line(
            output,
            indent,
            format!("if {fault} != nil {{ panic({fault}) }}"),
        );
        Ok(())
    }
}

fn literal_sleep(statement: &Stmt) -> Option<i32> {
    let S::Expr(Expr {
        kind: E::Await(future),
        ..
    }) = &statement.kind
    else {
        return None;
    };
    let E::Int(value) = &builtin_argument(future, &["sleep"])?.kind else {
        return None;
    };
    // Keep the private timer within one safe Go time.Duration.
    value.parse::<i32>().ok().filter(|value| *value >= 0)
}

fn literal_assertion(statement: &Stmt) -> bool {
    matches!(&statement.kind, S::Expr(value)
        if literal_builtin(value, &["assert_true"], |kind| matches!(kind, E::Bool(_))))
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
    let has_parent_wait = emitter.single_task.is_some_and(SingleTask::has_parent_wait);
    let mut output = emitter.generate()?;
    if has_parent_wait {
        output.push_str(PARENT_WAIT_RUNTIME);
    }
    Ok(output)
}

const PARENT_WAIT_RUNTIME: &str = r#"
var nagiTaskParentWaiting func(*taskscope.Scope, taskscope.Handle[nagiResult[int64, int64]])
var nagiTaskReceiveClaimed func(*taskscope.Scope, taskscope.Handle[nagiResult[int64, int64]])

func nagiTaskWaitParent(parent context.Context, milliseconds int64, scope *taskscope.Scope, handle taskscope.Handle[nagiResult[int64, int64]]) bool {
    timer := time.NewTimer(time.Duration(milliseconds) * time.Millisecond)
    defer timer.Stop()
    if nagiTaskParentWaiting != nil { nagiTaskParentWaiting(scope, handle) }
    select {
    case <-parent.Done():
        return false
    case <-timer.C:
        return true
    }
}

func nagiTaskReceiveParent(parent context.Context, scope *taskscope.Scope, handle taskscope.Handle[nagiResult[int64, int64]]) (nagiResult[nagiResult[int64, int64], nagiTaskFailure], bool) {
    value, fault, completed := taskscope.ReceiveUntil(scope, handle, parent.Done(), func() {
        if nagiTaskReceiveClaimed != nil { nagiTaskReceiveClaimed(scope, handle) }
    })
    if !completed { return nagiResult[nagiResult[int64, int64], nagiTaskFailure]{}, false }
    return nagiTaskReceived(value, fault), true
}
"#;

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
var nagiTaskBeforeBodyAssert func(*taskscope.Scope, taskscope.Handle[nagiResult[int64, int64]])
var nagiTaskDiscarded func()
var nagiTaskSleepStarted func()
var nagiTaskSleepInterrupted func()

func nagiTaskBody(scope *taskscope.Scope, body func()) {
    completed := false
    defer func() {
        if !completed {
            scope.RequestAbnormalExit()
        }
    }()
    body()
    completed = true
}

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
    return nagiTaskReceived(value, fault)
}

func nagiTaskReceived[T any](value T, fault error) nagiResult[T, nagiTaskFailure] {
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
