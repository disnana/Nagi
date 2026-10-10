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
        if operation == O::SqliteLiteral && !matches!(args[0].kind, E::Str(_)) {
            return Err(error(args[0].line, "SF05 literal Query: sqlite.literalには直接文字列literalを指定してください。変数/連結/format/動的SQLは不可です。値はParametersへbindし、構造はレビュー済みliteral Queryから選んでください"));
        }
        for ty in types {
            self.valid(ty, line)?;
            self.emittable(ty, line, false)?;
        }
        let resource = |kind| crate::stdlib::resource_type(kind, vec![]);
        let view = |ty| Type::generic("view", vec![ty]);
        let failure = |ty| Type::generic("Result", vec![ty, resource(R::SqliteFailure)]);
        let parameters = resource(R::SqliteParameters);
        let query = resource(R::SqliteQueryValue);
        let tx = resource(R::SqliteTx);
        let pool = resource(R::SqlitePool);
        let i64_ty = Type::named("i64");
        let unit = Type::named("unit");
        let (hints, output) = match operation {
            O::SqliteLiteral => (vec![Type::named("str")], query.clone()),
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
                    vec![view(tx.clone()), query.clone(), parameters.clone()],
                    failure(Type::generic(collection, vec![row.clone()])),
                )
            }
            O::SqliteExec => (
                vec![view(tx.clone()), query.clone(), parameters.clone()],
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
            if index == 1
                && matches!(operation, O::SqliteQuery | O::SqliteAll | O::SqliteExec)
                && (ty == Type::named("str") || ty == view(Type::named("str")))
            {
                return Err(error(arg.line, "SF05 migration: SQLite query/all/execはQueryが必須です。直接literalをsqlite.literal(\"…\")で封印し、値をParametersへbindしてください。動的SQL/旧Dbはtrusted host管理処理へ分離してください"));
            }
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
