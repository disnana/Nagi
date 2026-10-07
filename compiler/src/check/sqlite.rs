//! SQLite operation typing. All identities and passing modes come from the registry.
use super::*;

impl Checker {
    pub(super) fn sqlite_standard(
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
        let resource = |kind| crate::stdlib::resource_type(kind, vec![]);
        let view = |ty| Type::generic("view", vec![ty]);
        let failure = |ty| Type::generic("Result", vec![ty, resource(R::SqliteFailure)]);
        let parameters = resource(R::SqliteParameters);
        let tx = resource(R::SqliteTx);
        let pool = resource(R::SqlitePool);
        let i64_ty = Type::named("i64");
        let unit = Type::named("unit");
        let (hints, output) = match operation {
            O::SqliteOptions => (vec![i64_ty.clone(); 4], result(resource(R::SqliteOptions))),
            O::SqliteOpen => (
                vec![view(Type::named("str")), resource(R::SqliteOptions)],
                failure(pool.clone()),
            ),
            O::SqliteClonePool => (vec![view(pool.clone())], pool.clone()),
            O::SqliteBegin => (
                vec![view(pool.clone()), resource(R::SqliteBeginMode)],
                failure(tx.clone()),
            ),
            O::SqliteParameters => (vec![], parameters.clone()),
            O::SqliteBindI64 => (vec![parameters.clone(), i64_ty.clone()], parameters.clone()),
            O::SqliteBindF64 => (
                vec![parameters.clone(), Type::named("f64")],
                result(parameters.clone()),
            ),
            O::SqliteBindText => (
                vec![parameters.clone(), Type::named("str")],
                parameters.clone(),
            ),
            O::SqliteBindBytes => (
                vec![parameters.clone(), Type::named("bytes")],
                parameters.clone(),
            ),
            O::SqliteBindNull => (vec![parameters.clone()], parameters.clone()),
            O::SqliteQuery | O::SqliteAll => {
                let row = &types[0];
                if !self.classes.get(&row.0).is_some_and(|class| {
                    crate::capabilities::generated_row_supported(class, &self.classes, false)
                }) {
                    return Err(error(line, "SQLite行型には標準FromRow生成に対応するscalar fieldのclassを指定してください"));
                }
                let collection = if operation == O::SqliteQuery {
                    "Option"
                } else {
                    "List"
                };
                (
                    vec![
                        view(tx.clone()),
                        view(Type::named("str")),
                        parameters.clone(),
                    ],
                    failure(Type::generic(collection, vec![row.clone()])),
                )
            }
            O::SqliteExec => (
                vec![
                    view(tx.clone()),
                    view(Type::named("str")),
                    parameters.clone(),
                ],
                failure(i64_ty.clone()),
            ),
            O::SqliteCommit | O::SqliteRollback => (vec![tx], failure(unit.clone())),
            O::SqliteClose => (vec![view(pool), i64_ty], failure(unit)),
            O::SqliteCopyPrimaryError | O::SqliteCopyCleanupError => (
                vec![view(resource(R::SqliteFailure))],
                Type::generic("Option", vec![Type::named("Error")]),
            ),
            _ => unreachable!("not a SQLite operation"),
        };
        for (index, arg) in args.iter_mut().enumerate() {
            let ty = self.expr(arg, Some(&hints[index]))?;
            if info.parameters[index] == Passing::Reference {
                self.reference(&ty, &hints[index], arg.line)?;
                self.available(arg, false)?;
            } else {
                self.demand(&ty, &hints[index], arg.line)?;
                self.consume(arg)?;
            }
            // The sealed SQL plan owns argument 1 before evaluating Parameters.
            // Its temporary input loan ends there, unlike the Tx loan retained
            // by the async operation. Keep persistent local-view loans intact.
            let materialized_sql =
                index == 1 && matches!(operation, O::SqliteQuery | O::SqliteAll | O::SqliteExec);
            if !materialized_sql {
                self.hold_value(arg, info.parameters[index] == Passing::Reference);
            }
        }
        Ok(if info.asynchronous {
            future(output)
        } else {
            output
        })
    }
}
