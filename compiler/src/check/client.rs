//! SF06 canonical operations use the existing move/borrow/Future facts.
use super::*;

impl Checker {
    pub(super) fn client_input(&self, input: &Type, line: usize) -> Result<(), String> {
        if crate::capabilities::contains_auth_proof(input, &self.classes, &self.enums)
            || crate::capabilities::contains_same_task_resource(input, &self.classes, &self.enums)
            || crate::capabilities::contains_task_owner(input, &self.classes, &self.enums)
        {
            return Err(error(
                line,
                "SF06 inputにauth proof/Tx/Taskの実payloadを保持できません",
            ));
        }
        let mut pending = vec![input];
        let mut seen = HashSet::new();
        while let Some(current) = pending.pop() {
            if current.0 == "view" || current.0 == "fn" || self.resource(&current.0).is_some() {
                return Err(error(line, "SF06 inputにはownedな非proofの値を使ってください。view/関数/opaque native resourceは保持できません"));
            }
            pending.extend(&current.1);
            if !seen.insert(current.0.as_str()) {
                continue;
            }
            if let Some(class) = self.classes.get(&current.0) {
                pending.extend(class.fields.iter().map(|(_, field)| field));
            } else if let Some(enumeration) = self.enums.get(&current.0) {
                pending.extend(
                    enumeration
                        .variants
                        .iter()
                        .flat_map(|v| v.fields.iter().map(|(_, field)| field)),
                );
            }
        }
        Ok(())
    }

    pub(super) fn client_standard(
        &mut self,
        operation: crate::stdlib::Operation,
        types: &[Type],
        args: &mut [Expr],
        line: usize,
    ) -> Result<Type, String> {
        use crate::stdlib::{Operation as O, Passing, Resource as R};
        let info = crate::stdlib::operation_info(operation);
        if args.len() != info.arity {
            return Err(error(
                line,
                format!("{}の引数は{}個です", info.name, info.arity),
            ));
        }
        if !types.is_empty() && types.len() != info.generic_arity
            || info.generic_arity == 0 && !types.is_empty()
        {
            return Err(error(
                line,
                format!("{}の型引数は{}個です", info.name, info.generic_arity),
            ));
        }
        for ty in types {
            self.valid(ty, line)?;
            self.emittable(ty, line, false)?;
        }
        let resource = |kind| crate::stdlib::resource_type(kind, vec![]);
        let view = |ty| Type::generic("view", vec![ty]);
        let output;
        let hints;
        if matches!(operation, O::ClientExecute | O::ClientExecutePublic) {
            let first = self.expr(&mut args[0], None)?;
            let handle = crate::stdlib::native_view_element(&first).unwrap_or(&first);
            let kind = if operation == O::ClientExecute {
                R::ProtectedOperation
            } else {
                R::PublicOperation
            };
            if self.resource(&handle.0) != Some(kind) {
                return Err(error(
                    args[0].line,
                    format!(
                        "{}には{}が必要です。protected/public操作は互換ではありません",
                        info.name,
                        crate::stdlib::resource_info(kind).name
                    ),
                ));
            }
            self.valid(handle, args[0].line)?;
            if !types.is_empty() {
                self.demand(
                    handle,
                    &crate::stdlib::resource_type(kind, types.to_vec()),
                    args[0].line,
                )?;
            }
            let input = handle.1.last().expect("validated operation arity").clone();
            self.client_input(&input, line)?;
            hints = if operation == O::ClientExecute {
                vec![
                    view(handle.clone()),
                    crate::stdlib::resource_type(R::Grant, vec![handle.1[1].clone()]),
                    input,
                ]
            } else {
                vec![view(handle.clone()), input]
            };
            output = Type::generic(
                "Result",
                vec![resource(R::ClientResponse), resource(R::ClientFailure)],
            );
            // The first argument was evaluated once above to infer the handle.
            self.reference(&first, &hints[0], args[0].line)?;
            self.available(&args[0], false)?;
            self.hold_value(&args[0], true);
        } else {
            let response = resource(R::ClientResponse);
            let failure = resource(R::ClientFailure);
            let bytes_view = view(Type::named("bytes"));
            (hints, output) = match operation {
                O::ClientStatus => (vec![view(response)], Type::named("i64")),
                O::ClientHeaders => (
                    vec![view(response), view(Type::named("str"))],
                    Type::generic(
                        "Result",
                        vec![
                            Type::generic("List", vec![bytes_view]),
                            Type::named("Error"),
                        ],
                    ),
                ),
                O::ClientBody => (vec![view(response)], bytes_view),
                O::ClientTakeBody => (vec![response], Type::named("bytes")),
                O::ClientKind => (vec![view(failure)], resource(R::ClientFailureKind)),
                O::ClientMessage => (vec![view(failure)], view(Type::named("str"))),
                O::ClientAuthKind => (
                    vec![view(failure)],
                    Type::generic("Option", vec![resource(R::AuthFailureKind)]),
                ),
                O::ClientOutcome => (vec![view(failure)], resource(R::ClientOutcome)),
                _ => unreachable!("only canonical client operations reach this checker"),
            };
        }
        let start = usize::from(matches!(
            operation,
            O::ClientExecute | O::ClientExecutePublic
        ));
        for (index, arg) in args.iter_mut().enumerate().skip(start) {
            let ty = self.expr(arg, Some(&hints[index]))?;
            if info.parameters[index] == Passing::Reference {
                self.reference(&ty, &hints[index], arg.line)?;
                self.available(arg, false)?;
            } else {
                self.demand(&ty, &hints[index], arg.line)?;
                self.consume(arg)?;
            }
            self.hold_value(arg, info.parameters[index] == Passing::Reference);
        }
        Ok(if info.asynchronous {
            future(output)
        } else {
            output
        })
    }
}
