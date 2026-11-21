//! Private probes of checked Task lowering. This module is test-build only.
use super::{error, ident, statement_matches, Emitter};
use crate::checked::{ArgumentCaptureFacts, ArgumentRelease};
use crate::{
    ast::{
        BindingId, Expr, ExprUseId, ExprUseMode, Function, NameResolution, Program, Stmt, Type, E,
        S,
    },
    checked::{
        CheckedProgram, ErrorDestination, ExpressionKey, FunctionPlan, ScopeBodyErrorExit,
        ScopeBridge, ScopeExitPlan, ScopeNormalExit, ScopeOwnerTeardown, StatementPlan,
        TaskSpawnMode, TaskUseMode,
    },
    project::BuildProfile,
    stdlib::{self, Operation},
};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(super) struct TaskScopeProbe<'a> {
    pub(super) scope: &'a Stmt,
    pub(super) invocation: &'a Expr,
    handle: &'a str,
    completion: TaskCompletion<'a>,
    body_step: Option<BodyStep<'a>>,
    child_sleep: Option<ChildSleep<'a>>,
    sibling: Option<TimerSibling<'a>>,
    connection_probe: bool,
}

#[derive(Clone, Copy)]
struct TimerSibling<'a> {
    invocation: &'a Expr,
    handle: &'a str,
    sleep: ChildSleep<'a>,
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
    CopyRebind(&'a [Stmt]),
}

#[derive(Clone, Copy)]
enum TaskCompletion<'a> {
    Receive(&'a str),
    Discard,
}

impl<'a> TaskScopeProbe<'a> {
    fn from_checked(checked: &'a CheckedProgram) -> Result<Self, String> {
        let program = checked.program();
        let (index, main) = program
            .functions
            .iter()
            .enumerate()
            .find(|(_, function)| function.name == "main")
            .ok_or("missing checked main for internal Go Task lowering")?;
        let unsupported = || error(main.line, "unsupported shape for internal Go Task lowering");
        let [locals @ .., scope, end] = main.body.as_slice() else {
            return Err(unsupported());
        };
        if locals
            .iter()
            .any(|local| scalar_literal_assignment(local, true).is_none())
        {
            return Err(unsupported());
        }
        let printed = match &end.kind {
            S::Return(Some(value)) => builtin_argument(value, &["ok"])
                .and_then(|argument| builtin_argument(argument, &["print"])),
            _ => None,
        };
        if !main.asynchronous
            || main.ret != Type("Result".into(), vec![scalar("unit"), scalar("Error")])
            || !printed.is_some_and(|value| {
                if locals.is_empty() {
                    matches!(value.kind, E::Str(_))
                } else {
                    capture_local(locals, value).is_some()
                }
            })
        {
            return Err(unsupported());
        }
        let S::Scope(body) = &scope.kind else {
            return Err(unsupported());
        };
        // The only two-child form discards a timer child and receives a
        // literal panic or business error from the other child.
        let (spawn, consume, body_step, sibling_nodes) = match (locals.is_empty(), body.as_slice())
        {
            (false, [spawn, rebinds @ .., consume])
                if rebinds.len() == locals.len()
                    && locals.iter().zip(rebinds).all(|(local, rebind)| {
                        scalar_literal_assignment(local, true)
                            == scalar_literal_assignment(rebind, false)
                    }) =>
            {
                (spawn, consume, Some(BodyStep::CopyRebind(rebinds)), None)
            }
            (false, _) => return Err(unsupported()),
            (true, [sibling, primary, discard, await_primary]) => {
                (primary, await_primary, None, Some((sibling, discard)))
            }
            (true, [spawn, consume]) => (spawn, consume, None, None),
            (true, [spawn, first, second]) if literal_body_try(first) => {
                (spawn, second, Some(BodyStep::TryBefore(first)), None)
            }
            (true, [spawn, first, second]) if literal_body_try(second) => {
                (spawn, first, Some(BodyStep::TryAfter(second)), None)
            }
            (true, [spawn, assertion, consume]) if literal_assertion(assertion) => (
                spawn,
                consume,
                Some(BodyStep::AssertBefore(assertion)),
                None,
            ),
            (true, [spawn, wait, consume]) => (
                spawn,
                consume,
                Some(BodyStep::ParentWait(
                    wait,
                    literal_sleep(wait).ok_or_else(unsupported)?,
                )),
                None,
            ),
            _ => return Err(unsupported()),
        };
        let result = Type("Result".into(), vec![scalar("i64"), scalar("i64")]);
        let (handle, invocation, child) =
            spawned_child(program, spawn, &result, locals).ok_or_else(unsupported)?;
        if !locals.is_empty() && !checked_copy_arguments(checked, main, locals, invocation, child) {
            return Err(unsupported());
        }
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
                let operand = discard_operand(value).ok_or_else(unsupported)?;
                (TaskCompletion::Discard, value, operand)
            }
            _ => return Err(unsupported()),
        };
        if !matches!(&operand.kind, E::Name(name) if name == handle)
            || operand.resolution != Some(NameResolution::Local)
        {
            return Err(unsupported());
        }
        let child_sleep = if locals.is_empty() {
            let (child_return, sleep) = match child.body.as_slice() {
                [last] => (last, None),
                [first, last] if literal_assertion(first) => (last, None),
                [first, last] if !matches!(body_step, Some(BodyStep::TryAfter(_))) => {
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
            sleep.map(|milliseconds| ChildSleep {
                milliseconds,
                result: child_result,
            })
        } else {
            // The capture probe reuses ordinary nonsuspending child code.
            if child.body.iter().any(|statement| {
                statement_matches(statement, &|value| matches!(value.kind, E::Await(_)))
            }) {
                return Err(unsupported());
            }
            None
        };

        let plan = &checked.emission.functions[index];
        let scope_statement = plan
            .body
            .statements
            .get(locals.len())
            .ok_or_else(unsupported)?;
        let exit = scope_statement
            .scope_exit
            .as_ref()
            .ok_or_else(unsupported)?;
        let [block] = scope_statement.children.as_slice() else {
            return Err(unsupported());
        };
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
        {
            return Err(unsupported());
        }
        let primary_index = usize::from(sibling_nodes.is_some());
        let Some(primary_plan) = block.statements.get(primary_index) else {
            return Err(unsupported());
        };
        let use_mode = match completion {
            TaskCompletion::Receive(_) => TaskUseMode::ReceiveJoinedValue {
                output: result.clone(),
            },
            TaskCompletion::Discard => TaskUseMode::DiscardOutputKeepScopeJoin,
        };
        if !checked_task_pair(
            plan,
            primary_plan,
            exit,
            (spawn, invocation),
            (command, operand),
            (&result, use_mode),
        ) {
            return Err(unsupported());
        }
        let sibling = if let Some((sibling_spawn, discard)) = sibling_nodes {
            let (sibling_handle, sibling_invocation, sibling_child) =
                spawned_child(program, sibling_spawn, &result, &[]).ok_or_else(unsupported)?;
            let (S::Expr(discard), [wait, last]) = (&discard.kind, sibling_child.body.as_slice())
            else {
                return Err(unsupported());
            };
            let operand = discard_operand(discard).ok_or_else(unsupported)?;
            let milliseconds = literal_sleep(wait).ok_or_else(unsupported)?;
            let S::Return(Some(sibling_result)) = &last.kind else {
                return Err(unsupported());
            };
            let supported_pair = match child.body.as_slice() {
                [assertion, _] => {
                    milliseconds > 0
                        && matches!(&assertion.kind, S::Expr(value)
                    if literal_builtin(value, &["assert_true"], |kind| matches!(kind, E::Bool(false))))
                }
                [Stmt {
                    kind: S::Return(Some(value)),
                    ..
                }] => {
                    milliseconds == 0
                        && literal_builtin(value, &["fail"], |kind| matches!(kind, E::Int(_)))
                }
                _ => false,
            };
            if !supported_pair
                || !matches!(completion, TaskCompletion::Receive(_))
                || sibling_handle == handle
                || sibling_spawn.task.spawn_binding == spawn.task.spawn_binding
                || operand.resolution != Some(NameResolution::Local)
                || !matches!(&operand.kind, E::Name(name) if name == sibling_handle)
                || !literal_builtin(sibling_result, &["ok", "fail"], |kind| {
                    matches!(kind, E::Int(_))
                })
                || !block.statements.first().is_some_and(|sibling_plan| {
                    checked_task_pair(
                        plan,
                        sibling_plan,
                        exit,
                        (sibling_spawn, sibling_invocation),
                        (discard, operand),
                        (&result, TaskUseMode::DiscardOutputKeepScopeJoin),
                    )
                })
            {
                return Err(unsupported());
            }
            Some(TimerSibling {
                invocation: sibling_invocation,
                handle: sibling_handle,
                sleep: ChildSleep {
                    milliseconds,
                    result: sibling_result,
                },
            })
        } else {
            None
        };
        Ok(Self {
            scope,
            invocation,
            handle,
            completion,
            body_step,
            sibling,
            connection_probe: false,
            child_sleep,
        })
    }

    pub(super) fn validate(&self, emitter: &Emitter<'a>, ret: &Type) -> Result<(), String> {
        emitter.validate_expr(self.invocation, ret)?;
        if let Some(sibling) = self.sibling {
            emitter.validate_expr(sibling.invocation, ret)?;
        }
        if let Some(
            BodyStep::TryBefore(statement)
            | BodyStep::TryAfter(statement)
            | BodyStep::AssertBefore(statement)
            | BodyStep::ParentWait(statement, _),
        ) = self.body_step
        {
            emitter.validate_statement(statement, ret)?;
        }
        if let Some(BodyStep::CopyRebind(rebinds)) = self.body_step {
            for rebind in rebinds {
                emitter.validate_statement(rebind, ret)?;
            }
        }
        Ok(())
    }

    pub(super) fn has_parent_wait(self) -> bool {
        matches!(self.body_step, Some(BodyStep::ParentWait(..)))
    }

    fn interrupts_finish(self) -> bool {
        self.child_sleep.is_some() && matches!(self.completion, TaskCompletion::Discard)
    }

    pub(super) fn has_parent_context(self) -> bool {
        self.has_parent_wait() || self.interrupts_finish()
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
        let (param, arg) = if self.connection_probe {
            (", owner *nagiTaskConnOwner", ", nil")
        } else {
            ("", "")
        };
        Emitter::line(output, 0, format!(
            "func nagiTaskParent(parent context.Context{param}) (nagiResult[nagiUnit, nagiError], bool) {{"
        ));
        self.lower(emitter, ret, 1, output)?;
        let (before, value) = emitter.expression(value, ret)?;
        for line in before {
            Emitter::line(output, 1, line);
        }
        Emitter::line(output, 1, format!("return {value}, true"));
        Emitter::line(output, 0, "}");
        // Keep the regular entry usable with its non-cancelling background.
        output.push_str(&format!("func nagiMain() *nagiFuture[nagiResult[nagiUnit, nagiError]] {{\n    return nagiCold(func() nagiResult[nagiUnit, nagiError] {{\n        result, _ := nagiTaskParent(context.Background(){arg})\n        return result\n    }})\n}}\n\n"));
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
            .has_parent_context()
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
        let sibling_handle = if let Some(sibling) = self.sibling {
            let name = ident(sibling.handle);
            sibling.sleep.emit_spawn(
                emitter,
                &scope,
                &name,
                "taskscope.SpawnInterruptible",
                body_indent,
                output,
            )?;
            Some(name)
        } else {
            None
        };
        if let Some(sleep) = self.child_sleep {
            let (spawn, args) = if self.connection_probe {
                ("nagiTaskSpawnWithConn", format!("owner, {scope}"))
            } else {
                ("taskscope.SpawnInterruptible", scope.clone())
            };
            sleep.emit_spawn(emitter, &args, &handle, spawn, body_indent, output)?;
        } else {
            let (before, future) = emitter.evaluated(self.invocation, ret)?;
            for line in before {
                Emitter::line(output, body_indent, line);
            }
            let cleanup = if self.sibling.is_some() {
                "nil"
            } else {
                "nagiTaskCleanup"
            };
            Emitter::line(
                output,
                body_indent,
                format!("{handle} := nagiTaskSpawn({scope}, {future}, {cleanup})"),
            );
        }
        if let Some(sibling) = sibling_handle {
            emit_discard(&scope, &sibling, body_indent, output);
            Emitter::line(
                output,
                body_indent,
                format!("if nagiTaskSiblingScope != nil {{ nagiTaskSiblingScope({scope}) }}"),
            );
        }
        match self.body_step {
            Some(BodyStep::CopyRebind(rebinds)) => {
                emitter.statements(rebinds, ret, body_indent, output)?;
                Emitter::line(
                    output,
                    body_indent,
                    "if nagiTaskCopyRebound != nil { nagiTaskCopyRebound() }",
                );
            }
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
                emit_discard(&scope, &handle, body_indent, output);
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
            if self.interrupts_finish() {
                Emitter::line(
                    output,
                    indent,
                    format!("exit, completed := nagiTaskFinishParent(parent, {scope}, {handle})"),
                );
                Emitter::line(
                    output,
                    indent,
                    "if !completed { return nagiResult[nagiUnit, nagiError]{}, false }",
                );
            } else {
                Emitter::line(output, indent, format!("exit := nagiTaskFinish({scope})"));
            }
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

fn spawned_child<'a>(
    program: &'a Program,
    spawn: &'a Stmt,
    result: &Type,
    locals: &[Stmt],
) -> Option<(&'a str, &'a Expr, &'a Function)> {
    let S::SpawnBind {
        name,
        value: invocation,
        declare: true,
        ..
    } = &spawn.kind
    else {
        return None;
    };
    let E::Call(callee, types, arguments) = &invocation.kind else {
        return None;
    };
    if invocation.resolution != Some(NameResolution::Function) || !types.is_empty() {
        return None;
    }
    let child = program
        .functions
        .iter()
        .find(|child| &child.name == callee)?;
    let arguments_match = if locals.is_empty() {
        arguments.is_empty() && child.params.is_empty()
    } else {
        !arguments.is_empty() && arguments.len() == child.params.len()
    };
    (child.asynchronous && !child.external && arguments_match && &child.ret == result).then_some((
        name.as_str(),
        invocation,
        child,
    ))
}

fn scalar_literal_assignment(statement: &Stmt, declaration: bool) -> Option<(&str, &Type)> {
    let S::Assign {
        name,
        value,
        declare,
        ..
    } = &statement.kind
    else {
        return None;
    };
    // The AST shape limits the probe to literal locals. The checked Copy
    // facts, not the literal spelling or its Go representation, authorize capture.
    (*declare == declaration
        && matches!(value.kind, E::Int(_) | E::Float(_) | E::Bool(_) | E::Str(_)))
    .then_some((name, value.ty.as_ref()?))
}

fn capture_local<'a>(locals: &'a [Stmt], operand: &Expr) -> Option<&'a Stmt> {
    let (Some(NameResolution::Local), E::Name(name)) = (operand.resolution, &operand.kind) else {
        return None;
    };
    let ty = operand.ty.as_ref()?;
    locals
        .iter()
        .find(|stmt| scalar_literal_assignment(stmt, true) == Some((name, ty)))
}

fn copy_expression_owners(locals: &[Stmt], operand: &Expr) -> Option<Vec<(ExprUseId, BindingId)>> {
    let mut owners = Vec::new();
    crate::checked::visit_expression(operand, &mut |expression| {
        match &expression.kind {
            E::Int(_) | E::Float(_) | E::Bool(_) | E::Unary(..) | E::Binary(..) => {}
            E::Name(_) => {
                let local = capture_local(locals, expression)
                    .filter(|local| matches!(&local.kind, S::Assign { value, .. } if !matches!(value.kind, E::Str(_))))
                    .ok_or("not a scalar literal local")?;
                let span = local.binding_span.ok_or("missing capture binding span")?;
                owners.push((ExprUseId::of(expression), BindingId { line: local.line, token: span.start }));
            }
            _ => return Err("not a pure scalar capture expression".into()),
        }
        Ok(())
    }).ok()?;
    Some(owners)
}

fn checked_copy_arguments(
    checked: &CheckedProgram,
    parent: &Function,
    locals: &[Stmt],
    invocation: &Expr,
    child: &Function,
) -> bool {
    let E::Call(callee, _, args) = &invocation.kind else {
        return false;
    };
    let call = ExprUseId::of(invocation);
    let Some(capture) = checked
        .final_check_facts()
        .functions
        .get(&parent.name)
        .and_then(|calls| calls.get(&call))
    else {
        return false;
    };
    if capture.callee != *callee
        || capture.resolution != NameResolution::Function
        || capture.arguments.len() != args.len()
        || child.params.len() != args.len()
    {
        return false;
    }
    let mut bindings = BTreeSet::new();
    for (index, ((operand, argument), (_, parameter_type))) in args
        .iter()
        .zip(&capture.arguments)
        .zip(&child.params)
        .enumerate()
    {
        let (Some(owners), ArgumentCaptureFacts::UserAsync(facts)) =
            (copy_expression_owners(locals, operand), &argument.capture)
        else {
            return false;
        };
        let operand_id = ExprUseId::of(operand);
        if argument.index != index
            || argument.expression != operand_id
            || operand.ty.as_ref() != Some(parameter_type)
            || &argument.ty != parameter_type
            || argument.passing != stdlib::Passing::Move
            || argument.owners != owners
            || facts.slot.call != call
            || facts.slot.argument != index
            || facts.storage.call != call
            || facts.storage.argument != index
            || facts.transfer != ExprUseMode::Copy
            || !facts.views_absent
            || !facts.depths.is_empty()
            || facts.release != ArgumentRelease::NotRequired
        {
            return false;
        }
        bindings.extend(owners.into_iter().map(|(_, binding)| binding));
    }
    bindings.len() == locals.len()
}

fn discard_operand(value: &Expr) -> Option<&Expr> {
    let E::Call(name, types, arguments) = &value.kind else {
        return None;
    };
    let [operand] = arguments.as_slice() else {
        return None;
    };
    (value.resolution == Some(NameResolution::Standard)
        && stdlib::operation(name) == Some(Operation::TaskDiscard)
        && types.is_empty())
    .then_some(operand)
}

fn checked_task_pair(
    plan: &FunctionPlan,
    statement: &StatementPlan,
    exit: &ScopeExitPlan,
    (spawn, invocation): (&Stmt, &Expr),
    (command, operand): (&Expr, &Expr),
    (result, mode): (&Type, TaskUseMode),
) -> bool {
    let Some(start) = &statement.task_spawn else {
        return false;
    };
    let Some(take) = plan
        .expressions
        .get(&ExpressionKey::of(command))
        .and_then(|entry| entry.task.as_ref())
    else {
        return false;
    };
    start.bridge == ScopeBridge::TaskScope
        && start.scope == exit.scope
        && take.scope == exit.scope
        && start.invocation == ExprUseId::of(invocation)
        && take.expression == ExprUseId::of(command)
        && take.operand == ExprUseId::of(operand)
        && matches!(&start.mode, TaskSpawnMode::ValueHandle { binding, output }
            if Some(*binding) == spawn.task.spawn_binding && output == result)
        && take.mode == mode
}

fn emit_discard(scope: &str, handle: &str, indent: usize, output: &mut String) {
    Emitter::line(
        output,
        indent,
        format!(
            "if fault := taskscope.Discard({scope}, {handle}); fault != nil {{ panic(fault) }}"
        ),
    );
}

impl ChildSleep<'_> {
    fn emit_spawn(
        self,
        emitter: &mut Emitter<'_>,
        spawn_args: &str,
        handle: &str,
        spawn: &str,
        indent: usize,
        output: &mut String,
    ) -> Result<(), String> {
        let fault = emitter.temp("task_spawn_fault");
        let result_type = Type("Result".into(), vec![scalar("i64"), scalar("i64")]);
        let (before, result) = emitter.expression(self.result, &result_type)?;
        Emitter::line(output, indent, format!(
            "{handle}, {fault} := {spawn}({spawn_args}, func(ctx context.Context) (nagiResult[int64, int64], bool) {{"
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
    source_for_probe(checked, false)
}

pub(super) fn source_with_connection(checked: &CheckedProgram) -> Result<String, String> {
    source_for_probe(checked, true)
}

fn source_for_probe(checked: &CheckedProgram, connection_probe: bool) -> Result<String, String> {
    let mut task = TaskScopeProbe::from_checked(checked)?;
    if connection_probe
        && (!task.interrupts_finish() || task.sibling.is_some() || task.body_step.is_some())
    {
        return Err(error(
            task.scope.line,
            "connection probe requires a single discarded timer child",
        ));
    }
    task.connection_probe = connection_probe;
    let mut emitter = Emitter::from_checked(checked, &[], BuildProfile::Dev)?;
    emitter.task_scope_probe = Some(task);
    emitter.validate()?;
    let has_parent_context = emitter
        .task_scope_probe
        .is_some_and(TaskScopeProbe::has_parent_context);
    let mut output = emitter.generate()?;
    if has_parent_context {
        output.push_str(PARENT_WAIT_RUNTIME);
    }
    Ok(output)
}

const PARENT_WAIT_RUNTIME: &str = r#"
var nagiTaskParentWaiting func(*taskscope.Scope, taskscope.Handle[nagiResult[int64, int64]])
var nagiTaskReceiveClaimed func(*taskscope.Scope, taskscope.Handle[nagiResult[int64, int64]])
var nagiTaskFinishWaiting func(*taskscope.Scope, taskscope.Handle[nagiResult[int64, int64]])

func nagiTaskFinishParent(parent context.Context, scope *taskscope.Scope, handle taskscope.Handle[nagiResult[int64, int64]]) (nagiResult[nagiUnit, nagiError], bool) {
    fault, completed := scope.FinishUntil(parent.Done(), func() {
        if nagiTaskFinishWaiting != nil { nagiTaskFinishWaiting(scope, handle) }
    })
    if !completed { return nagiResult[nagiUnit, nagiError]{}, false }
    return nagiTaskFinished(fault), true
}

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
var nagiTaskSiblingScope func(*taskscope.Scope)
var nagiTaskFinishPending func()
var nagiTaskCopyRebound func()
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
    fault, _ := scope.FinishUntil(nil, nagiTaskFinishPending)
    return nagiTaskFinished(fault)
}

func nagiTaskFinished(fault error) nagiResult[nagiUnit, nagiError] {
    if fault != nil {
        if fault != taskscope.ErrTaskPanicked { panic(fault) }
        return nagiErr[nagiUnit](nagiError{kind: nagiTaskInternal, message: nagiChildPanicked})
    }
    return nagiOk[nagiUnit, nagiError](nagiUnit{})
}
"#;
