//! Canonical policy callbacks and auth operations.
use super::*;

impl Checker {
    fn security_callback(&mut self, callback: &mut Expr, line: usize) -> Result<Type, String> {
        let ty = self.expr(callback, None)?;
        let named = match (&callback.kind, callback.resolution) {
            (E::Name(n), Some(NameResolution::Function)) => {
                self.functions.get(n).is_some_and(|f| f.asynchronous)
            }
            (E::Name(n), Some(NameResolution::Local)) => {
                self.vars.get(n).is_some_and(|v| v.async_function.is_some())
            }
            _ => false,
        };
        if !named || !ty.is_async_function() {
            return Err(error(
                line,
                "security policyには名前付きasync関数またはそのローカルaliasが必要です",
            ));
        }
        self.hold_value(callback, false);
        Ok(ty)
    }
    pub(super) fn security_standard(
        &mut self,
        op: crate::stdlib::Operation,
        types: &[Type],
        args: &mut [Expr],
        line: usize,
    ) -> Result<Type, String> {
        use crate::stdlib::{Operation as O, Passing, Resource as R};
        let info = crate::stdlib::operation_info(op);
        if args.len() != info.arity {
            return Err(error(
                line,
                format!("{}の引数は{}個です", info.name, info.arity),
            ));
        }
        if types.len() != info.generic_arity {
            return Err(error(
                line,
                format!("{}の型引数は{}個です", info.name, info.generic_arity),
            ));
        }
        for ty in types {
            self.valid(ty, line)?;
            self.emittable(ty, line, false)?;
        }
        let resource = |r| crate::stdlib::resource_type(r, vec![]);
        let policy =
            |state, authority| crate::stdlib::resource_type(R::HttpPolicy, vec![state, authority]);
        let failure = resource(R::AuthFailure);
        if matches!(
            op,
            O::PublicPolicy | O::AuthenticatedPolicy | O::AuthorizedPolicy
        ) {
            let state = &types[0];
            let scope = resource(R::AuthScope);
            if op == O::PublicPolicy {
                let output = policy(state.clone(), Type::named("unit"));
                self.valid(&output, line)?;
                return Ok(output);
            }
            let expected = Type::generic(
                "fn",
                vec![
                    resource(R::Request),
                    Type::generic("shared", vec![state.clone()]),
                    future(Type::generic(
                        "Result",
                        vec![resource(R::VerifiedIdentity), failure.clone()],
                    )),
                ],
            );
            let verifier = self.security_callback(&mut args[0], line)?;
            self.demand(&verifier, &expected, line)?;
            let authority = if op == O::AuthorizedPolicy {
                let grant = crate::stdlib::resource_type(R::Grant, vec![types[1].clone()]);
                self.valid(&grant, line)?;
                let expected = Type::generic(
                    "fn",
                    vec![
                        scope,
                        resource(R::Request),
                        Type::generic("shared", vec![state.clone()]),
                        future(Type::generic("Result", vec![grant.clone(), failure])),
                    ],
                );
                let authorizer = self.security_callback(&mut args[1], line)?;
                self.demand(&authorizer, &expected, line)?;
                grant
            } else {
                scope
            };
            let output = policy(state.clone(), authority);
            self.valid(&output, line)?;
            return Ok(output);
        }
        if matches!(
            op,
            O::SessionAuthenticatedPolicy | O::SessionAuthorizedPolicy
        ) {
            let store = self.expr(&mut args[0], Some(&resource(R::SessionStore)))?;
            self.demand(&store, &resource(R::SessionStore), args[0].line)?;
            self.consume(&args[0])?;
            self.hold_value(&args[0], false);
            let authority = if op == O::SessionAuthorizedPolicy {
                let grant = crate::stdlib::resource_type(R::Grant, vec![types[1].clone()]);
                self.valid(&grant, line)?;
                let expected = Type::generic(
                    "fn",
                    vec![
                        resource(R::AuthScope),
                        resource(R::Request),
                        Type::generic("shared", vec![types[0].clone()]),
                        future(Type::generic("Result", vec![grant.clone(), failure])),
                    ],
                );
                let authorizer = self.security_callback(&mut args[1], line)?;
                self.demand(&authorizer, &expected, line)?;
                grant
            } else {
                resource(R::AuthScope)
            };
            let output = policy(types[0].clone(), authority);
            self.valid(&output, line)?;
            return Ok(output);
        }
        let session_result = |ty| Type::generic("Result", vec![ty, resource(R::SessionFailure)]);
        let view = |ty| Type::generic("view", vec![ty]);
        let (hints, output) = match op {
            O::SessionOptions => (
                vec![Type::named("i64"); 8],
                session_result(resource(R::SessionOptions)),
            ),
            O::SessionCookieOptions => (
                vec![view(Type::named("str")), resource(R::SessionSameSite)],
                session_result(resource(R::SessionCookieOptions)),
            ),
            O::SessionOpen => (
                vec![
                    view(resource(R::SqlitePool)),
                    resource(R::SessionOptions),
                    resource(R::SessionCookieOptions),
                ],
                session_result(resource(R::SessionStore)),
            ),
            O::SessionCloneStore => (
                vec![view(resource(R::SessionStore))],
                resource(R::SessionStore),
            ),
            O::SessionIssue | O::SessionRotate | O::SessionLogout => (
                vec![view(resource(R::SessionStore)), resource(R::AuthScope)],
                session_result(resource(R::SessionResponse)),
            ),
            O::SessionApply => (
                vec![resource(R::Response), resource(R::SessionResponse)],
                session_result(resource(R::Response)),
            ),
            O::SessionKind => (
                vec![view(resource(R::SessionFailure))],
                resource(R::AuthFailureKind),
            ),
            O::SessionMessage => (
                vec![view(resource(R::SessionFailure))],
                view(Type::named("str")),
            ),
            O::SessionOutcome => (
                vec![view(resource(R::SessionFailure))],
                resource(R::SqliteOutcome),
            ),
            O::SecurityTimeout => (
                vec![resource(R::Options), Type::named("i64")],
                result(resource(R::Options)),
            ),
            O::AuthSubject => (vec![view(resource(R::AuthScope))], Type::named("i64")),
            O::AuthKind => (vec![view(failure.clone())], resource(R::AuthFailureKind)),
            O::AuthMessage => (vec![view(failure.clone())], view(Type::named("str"))),
            O::AuthInvalidCredential
            | O::AuthDenied
            | O::AuthExpired
            | O::AuthInvalidRequest
            | O::AuthUnavailable
            | O::AuthInternal => (vec![], failure),
            _ => unreachable!("non-security operation"),
        };
        for (i, arg) in args.iter_mut().enumerate() {
            let ty = self.expr(arg, Some(&hints[i]))?;
            if info.parameters[i] == Passing::Reference {
                self.reference(&ty, &hints[i], arg.line)?;
                self.available(arg, false)?;
            } else {
                self.demand(&ty, &hints[i], arg.line)?;
                self.consume(arg)?;
            }
            self.hold_value(arg, info.parameters[i] == Passing::Reference);
        }
        Ok(if info.asynchronous {
            future(output)
        } else {
            output
        })
    }
}
