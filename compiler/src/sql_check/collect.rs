use super::{Operation, Site, MAX_MESSAGE_BYTES, MAX_QUERIES};
use crate::ast::{Expr, NameResolution, Program, Stmt, Type, E, S};
use std::collections::HashMap;

/// Collect only database calls whose identity was resolved by the checker.
/// User functions and function values can share these spellings safely.
pub(super) fn collect(program: &Program) -> Result<Vec<Site>, String> {
    let classes: HashMap<_, _> = program
        .classes
        .iter()
        .map(|class| (class.name.as_str(), class))
        .collect();
    let mut sites = Vec::new();
    let mut size = 0usize;
    let mut failure = None;
    for function in &program.functions {
        statements(&function.body, &mut |expression| {
            if failure.is_some() {
                return;
            }
            let E::Call(name, types, arguments) = &expression.kind else {
                return;
            };
            let standard = if expression.resolution == Some(NameResolution::Standard) {
                crate::stdlib::operation(name).filter(|op| {
                    program.modules.definition(name).map(|d| &d.id)
                        == Some(&crate::stdlib::function_id(*op))
                })
            } else {
                None
            };
            let operation = match standard {
                Some(
                    crate::stdlib::Operation::SqliteQuery | crate::stdlib::Operation::SqliteAll,
                ) => Some(Operation::SqliteRows),
                Some(crate::stdlib::Operation::SqliteExec) => Some(Operation::SqliteExec),
                _ => return,
            };
            let bind_count = arguments
                .get(2)
                .and_then(|arg| parameter_count(program, arg));
            if sites.len() >= MAX_QUERIES {
                failure = Some(format!(
                    "line {}: SQL check exceeds the {MAX_QUERIES} call-site limit",
                    expression.line
                ));
                return;
            }
            let sql = arguments.get(1).and_then(|argument| {
                let E::Call(_, _, args) = &argument.kind else {
                    return None;
                };
                if standard_call(program, argument) != Some(crate::stdlib::Operation::SqliteLiteral)
                {
                    return None;
                }
                if let E::Str(sql) = &args.first()?.kind {
                    Some(sql.as_str())
                } else {
                    None
                }
            });
            let fields = types
                .first()
                .map(unowned)
                .and_then(|row| classes.get(row.0.as_str()))
                .map(|class| class.fields.as_slice())
                .unwrap_or_default();
            let reason = if sql.is_none() {
                Some("Query structure is not statically available; checked at runtime")
            } else if bind_count.is_none() {
                Some("Parameters bind count is unknown; checked at runtime")
            } else {
                None
            };
            // One large row class can appear in thousands of tiny calls. Bound
            // its repeated metadata before cloning, rather than waiting for
            // the encoded request to exceed its limit after allocating it.
            let field_size = fields.iter().fold(0usize, |size, (name, _)| {
                size.saturating_add(name.len())
                    .saturating_add(std::mem::size_of::<String>())
            });
            size = size
                .saturating_add(std::mem::size_of::<Site>())
                .saturating_add(name.len())
                .saturating_add(sql.map(str::len).unwrap_or_default())
                .saturating_add(field_size)
                .saturating_add(reason.map(str::len).unwrap_or_default());
            if size > MAX_MESSAGE_BYTES {
                failure = Some(format!(
                    "line {}: SQL collection exceeds the 8 MiB metadata/input limit",
                    expression.line
                ));
                return;
            }
            sites.push(Site {
                line: expression.line,
                operation,
                operation_name: standard
                    .map(|op| format!("sqlite.{}", crate::stdlib::operation_info(op).name))
                    .unwrap_or_else(|| name.clone()),
                sql: sql.map(str::to_owned),
                bind_count,
                fields: fields.iter().map(|(name, _)| name.clone()).collect(),
                reason: reason.map(str::to_owned),
            });
        });
        if let Some(error) = failure.take() {
            return Err(error);
        }
    }
    Ok(sites)
}

fn standard_call(program: &Program, expression: &Expr) -> Option<crate::stdlib::Operation> {
    let E::Call(name, _, _) = &expression.kind else {
        return None;
    };
    if expression.resolution != Some(NameResolution::Standard) {
        return None;
    }
    let operation = crate::stdlib::operation(name)?;
    (program.modules.definition(name).map(|d| &d.id)
        == Some(&crate::stdlib::function_id(operation)))
    .then_some(operation)
}
// Count only canonical, checked constructor chains. No SQL parser or folding
// of arbitrary user functions/variables; unknown values retain runtime checks.
fn parameter_count(program: &Program, expression: &Expr) -> Option<usize> {
    use crate::stdlib::Operation as O;
    if let E::Try(inner) = &expression.kind {
        return parameter_count(program, inner);
    }
    let E::Call(_, _, args) = &expression.kind else {
        return None;
    };
    match standard_call(program, expression)? {
        O::SqliteParameters => Some(0),
        O::SqliteBindI64
        | O::SqliteBindF64
        | O::SqliteBindText
        | O::SqliteBindBytes
        | O::SqliteBindNull => parameter_count(program, args.first()?)?.checked_add(1),
        _ => None,
    }
}

fn unowned(mut ty: &Type) -> &Type {
    while ty.0 == "owned" && ty.1.len() == 1 {
        ty = &ty.1[0];
    }
    ty
}

fn statements(body: &[Stmt], visit: &mut impl FnMut(&Expr)) {
    for statement in body {
        match &statement.kind {
            S::Assign { value, .. }
            | S::SpawnBind { value, .. }
            | S::Expr(value)
            | S::Spawn(value) => expr(value, visit),
            S::Return(value) => {
                if let Some(value) = value {
                    expr(value, visit);
                }
            }
            S::If(condition, yes, no) => {
                expr(condition, visit);
                statements(yes, visit);
                statements(no, visit);
            }
            S::Match(value, arms) => {
                expr(value, visit);
                for arm in arms {
                    statements(&arm.body, visit);
                }
            }
            S::While(condition, body) | S::For(_, condition, body) => {
                expr(condition, visit);
                statements(body, visit);
            }
            S::Scope(body) => statements(body, visit),
        }
    }
}

fn expr(expression: &Expr, visit: &mut impl FnMut(&Expr)) {
    visit(expression);
    match &expression.kind {
        E::Binary(left, _, right) | E::Index(left, right) => {
            expr(left, visit);
            expr(right, visit);
        }
        E::Unary(_, value) | E::Field(value, _) | E::Await(value) | E::Try(value) => {
            expr(value, visit)
        }
        E::Call(_, _, values) | E::List(values) => {
            for value in values {
                expr(value, visit);
            }
        }
        E::Record(_, fields) => {
            for (_, value) in fields {
                expr(value, visit);
            }
        }
        E::Int(_) | E::Float(_) | E::Str(_) | E::Bool(_) | E::Null | E::Name(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check, emit, parser, source};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    fn checked(text: &str) -> Program {
        let fixture = Fixture::new();
        fixture.write("main.nagi", text);
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program).unwrap();
        loaded.program
    }

    const OPERATIONS: &str = r#"import std.db.sqlite as sqlite
class Row:
    id: i64
    type: str
async def read(db: view[sqlite.Tx]) -> Result[unit, sqlite.Failure]:
    rows = try await sqlite.all[Row](db, sqlite.literal("SELECT id, type FROM items"), sqlite.parameters())
    row = try await sqlite.query[Row](db, sqlite.literal("SELECT id, type FROM items WHERE id = ?"), sqlite.bind_i64(sqlite.parameters(),1))
    inserted = try await sqlite.exec(db, sqlite.literal("INSERT INTO items(type, id) VALUES (?, ?)"), sqlite.bind_i64(sqlite.bind_text(sqlite.parameters(),"name"),1))
    updated = try await sqlite.exec(db, sqlite.literal("UPDATE items SET type = ?, id = ? WHERE id = ?"), sqlite.bind_i64(sqlite.bind_i64(sqlite.bind_text(sqlite.parameters(),"name"),2),1))
    changed = try await sqlite.exec(db, sqlite.literal("DELETE FROM items WHERE id = ?"), sqlite.bind_i64(sqlite.parameters(),1))
    return ok(print(changed))
"#;

    #[test]
    fn checked_high_and_saved_low_preserve_supported_operations_and_source_fields() {
        let high = checked(OPERATIONS);
        let mut low = parser::parse(&emit::low(&high), false).unwrap();
        check::check(&mut low).unwrap();
        for program in [&high, &low] {
            let sites = collect(program).unwrap();
            assert_eq!(sites.len(), 5);
            for (index, (operation, binds)) in [
                (Operation::SqliteRows, 0),
                (Operation::SqliteRows, 1),
                (Operation::SqliteExec, 2),
                (Operation::SqliteExec, 3),
                (Operation::SqliteExec, 1),
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(sites[index].operation, Some(operation));
                assert_eq!(sites[index].bind_count, Some(binds));
                assert!(sites[index].sql.is_some());
                assert!(sites[index].reason.is_none());
                assert_eq!(
                    sites[index].fields,
                    if index >= 2 {
                        Vec::<String>::new()
                    } else {
                        vec!["id".into(), "type".into()]
                    }
                );
            }
        }
        assert_eq!(
            collect(&high)
                .unwrap()
                .iter()
                .map(|site| site.line)
                .collect::<Vec<_>>(),
            vec![6, 7, 8, 9, 10]
        );
    }

    #[test]
    fn same_named_user_function_and_local_function_value_are_not_sql_calls() {
        let functions = checked(
            "def db_all(db: i64, sql: str) -> i64:\n    return 1\ndef read(db: i64) -> i64:\n    return db_all(db, \"this is not SQL\")\n",
        );
        assert!(collect(&functions).unwrap().is_empty());
        let locals = checked(
            "def fake(db: i64, sql: str) -> i64:\n    return 1\ndef read(db: i64) -> i64:\n    db_all = fake\n    return db_all(db, \"this is not SQL\")\n",
        );
        assert!(collect(&locals).unwrap().is_empty());
    }

    #[test]
    fn query_values_remain_visible_as_sites_with_unavailable_static_structure() {
        let program=checked("import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nasync def read(db: view[sqlite.Tx], query: sqlite.Query) -> Result[unit, sqlite.Failure]:\n    rows = try await sqlite.all[Row](db, query, sqlite.parameters())\n    selected = sqlite.literal(\"SELECT id FROM items\")\n    borrowed = try await sqlite.all[Row](db, selected, sqlite.parameters())\n    changed = try await sqlite.exec(db, sqlite.literal(\"CREATE TABLE items(id INTEGER)\"), sqlite.parameters())\n    return ok(print(changed))\n");
        let sites = collect(&program).unwrap();
        assert_eq!(sites.len(), 3);
        assert!(sites[..2]
            .iter()
            .all(|site| site.sql.is_none() && site.reason.as_ref().unwrap().contains("structure")));
        assert_eq!(sites[2].operation, Some(Operation::SqliteExec));
        assert_eq!(sites[2].operation_name, "sqlite.exec");
        assert!(sites[2].sql.as_ref().unwrap().contains("CREATE TABLE"));
        assert!(sites[2].reason.is_none());
    }

    #[test]
    fn conditional_loop_and_match_bodies_are_collected_once() {
        let program = checked(
            "import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nasync def read(db: view[sqlite.Tx]) -> Result[unit, sqlite.Failure]:\n    if true:\n        rows = try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM first\"), sqlite.parameters())\n    else:\n        rows = try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM second\"), sqlite.parameters())\n    for index in range(2):\n        rows = try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM looped\"), sqlite.parameters())\n    while false:\n        rows = try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM waited\"), sqlite.parameters())\n    result = await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM matched\"), sqlite.parameters())\n    match result:\n        case Ok(rows):\n            print(len(view(rows)))\n        case Err(error):\n            rows = try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM failed\"), sqlite.parameters())\n    return ok(print(1))\n",
        );
        let sites = collect(&program).unwrap();
        assert_eq!(sites.len(), 6);
        let sql: Vec<_> = sites
            .iter()
            .map(|site| site.sql.as_deref().unwrap())
            .collect();
        assert_eq!(
            sql,
            [
                "SELECT id FROM first",
                "SELECT id FROM second",
                "SELECT id FROM looped",
                "SELECT id FROM waited",
                "SELECT id FROM matched",
                "SELECT id FROM failed",
            ]
        );
    }

    #[test]
    fn nested_value_expressions_returns_and_scope_spawn_keep_each_actual_call() {
        let program = checked(
            "import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nclass Batch:\n    rows: List[Row]\nasync def work(count: Result[i64, sqlite.Failure]):\n    print(1)\nasync def read(db: view[sqlite.Tx]) -> Result[i64, sqlite.Failure]:\n    batch = Batch(rows=try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM records\"), sqlite.parameters()))\n    batches = [try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM lists\"), sqlite.parameters())]\n    total = (try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM indexed\"), sqlite.parameters()))[0].id + len(view(try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM counted\"), sqlite.parameters())))\n    negative = -(try await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM negated\"), sqlite.parameters()))[0].id\n    return await sqlite.exec(db, sqlite.literal(\"DELETE FROM returned WHERE id = ?\"), sqlite.bind_i64(sqlite.parameters(), 1))\nasync def scoped(db: view[sqlite.Tx]) -> Result[unit, Error]:\n    async with scope:\n        spawn work(await sqlite.exec(db, sqlite.literal(\"DELETE FROM spawned WHERE id = ?\"), sqlite.bind_i64(sqlite.parameters(), 1)))\n    return ok(print(1))\n",
        );
        let sites = collect(&program).unwrap();
        assert_eq!(sites.len(), 7);
        assert_eq!(
            sites
                .iter()
                .map(|site| site.sql.as_deref().unwrap())
                .collect::<Vec<_>>(),
            [
                "SELECT id FROM records",
                "SELECT id FROM lists",
                "SELECT id FROM indexed",
                "SELECT id FROM counted",
                "SELECT id FROM negated",
                "DELETE FROM returned WHERE id = ?",
                "DELETE FROM spawned WHERE id = ?",
            ]
        );
    }

    #[test]
    fn integrated_native_replacements_only_collect_the_final_function_body() {
        let mut program = checked(
            "import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nasync def read(db: view[sqlite.Tx]) -> Result[List[Row], sqlite.Failure]:\n    return await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM discarded\"), sqlite.parameters())\n",
        );
        let fixture = Fixture::new();
        fixture.write("replacement.low", "import std.db.sqlite as sqlite;\n@replace generated::read\nasync fn read(db: view[sqlite.Tx]) -> Result[List[Row], sqlite.Failure] { return await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM actual\"), sqlite.parameters()); }\n");
        let native = source::load(&fixture.0.join("replacement.low"), false)
            .unwrap()
            .program;
        check::integrate(&mut program, native).unwrap();
        let sites = collect(&program).unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].sql.as_deref(), Some("SELECT id FROM actual"));
        assert_eq!(sites[0].fields, ["id"]);
        assert_eq!(sites[0].line, 3);
    }

    #[test]
    fn repeated_row_metadata_is_bounded_before_building_the_worker_request() {
        let mut program = checked(
            "import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nasync def read(db: view[sqlite.Tx]) -> Result[List[Row], sqlite.Failure]:\n    return await sqlite.all[Row](db, sqlite.literal(\"SELECT id FROM users\"), sqlite.parameters())\n",
        );
        // The declaration and calls together are small. Expanding each call's
        // required-field metadata would exceed the request's memory budget.
        program.classes[0].fields = (0..256)
            .map(|index| {
                (
                    format!("field{index}_{}", "x".repeat(64)),
                    Type::named("i64"),
                )
            })
            .collect();
        let statement = program.functions[0].body[0].clone();
        program.functions[0].body = vec![statement; 512];
        let error = collect(&program)
            .err()
            .expect("repeated fields must be bounded");
        assert!(error.starts_with("line 5:"), "{error}");
        assert!(error.contains("metadata/input limit"), "{error}");
    }

    #[test]
    fn collection_rejects_excessive_sites_at_the_first_over_limit_call() {
        let mut program = checked(
            "import std.db.sqlite as sqlite\nasync def read(db: view[sqlite.Tx]) -> Result[i64, sqlite.Failure]:\n    return await sqlite.exec(db, sqlite.literal(\"DELETE FROM users WHERE id = ?\"), sqlite.bind_i64(sqlite.parameters(), 1))\n",
        );
        let statement = program.functions[0].body[0].clone();
        program.functions[0].body = vec![statement; MAX_QUERIES + 1];
        let error = collect(&program).err().expect("too many sites must fail");
        assert!(error.starts_with("line 3:"), "{error}");
        assert!(error.contains("call-site limit"), "{error}");
    }

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "nagi sql collect {} {}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn write(&self, name: &str, contents: &str) {
            fs::write(self.0.join(name), contents).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn sqlite_canonical_calls_and_saved_low_keep_unknown_bind_metadata() {
        let fixture = Fixture::new();
        fixture.write("main.nagi", "import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nasync def read(tx: sqlite.Tx, sql: sqlite.Query, params: sqlite.Parameters) -> Result[unit, sqlite.Failure]:\n    row = try await sqlite.query[Row](tx, sqlite.literal(\"SELECT id FROM users WHERE id=?\"), params)\n    rows = try await sqlite.all[Row](tx, sql, sqlite.parameters())\n    count = try await sqlite.exec(tx, sqlite.literal(\"CREATE TABLE new_table(id INTEGER)\"), sqlite.parameters())\n    return await sqlite.rollback(tx)\n");
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program).unwrap();
        let mut low = parser::parse(&emit::low(&loaded.program), false).unwrap();
        check::check(&mut low).unwrap();
        for program in [&loaded.program, &low] {
            let sites = collect(program).unwrap();
            assert_eq!(sites.len(), 3);
            assert_eq!(sites[0].operation, Some(Operation::SqliteRows));
            assert_eq!(sites[2].operation, Some(Operation::SqliteExec));
            assert_eq!(
                sites.iter().map(|s| s.bind_count).collect::<Vec<_>>(),
                [None, Some(0), Some(0)]
            );
            assert_eq!(sites[0].fields, ["id"]);
            assert!(sites[0].reason.as_ref().unwrap().contains("unknown"));
            assert!(sites[1].sql.is_none());
        }
        fixture.write("names.nagi", "import std.db.sqlite as sqlite\ndef query(sql: str) -> str:\n    return sql\ndef main():\n    print(query(\"not SQL\"))\n");
        let mut named = source::load(&fixture.0.join("names.nagi"), true).unwrap();
        check::check(&mut named.program).unwrap();
        assert!(collect(&named.program).unwrap().is_empty());
    }

    #[test]
    fn aliased_row_classes_use_canonical_identity_and_original_field_names() {
        let fixture = Fixture::new();
        fixture.write(
            "left.nagi",
            "class Row:\n    id: owned[i64]\n    type: str\n",
        );
        fixture.write("right.nagi", "class Row:\n    other: i64\n");
        fixture.write(
            "main.nagi",
            "import std.db.sqlite as sqlite\nimport \"left.nagi\" as left\nfrom \"right.nagi\" import Row as Other\nasync def read(db: view[sqlite.Tx]) -> Result[unit, sqlite.Failure]:\n    rows = try await sqlite.all[left.Row](db, sqlite.literal(\"SELECT id, type FROM left_rows\"), sqlite.parameters())\n    others = try await sqlite.all[Other](db, sqlite.literal(\"SELECT other FROM right_rows\"), sqlite.parameters())\n    return ok(print(1))\n",
        );
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program).unwrap();
        let mut low = parser::parse(&emit::low(&loaded.program), false).unwrap();
        check::check(&mut low).unwrap();
        for program in [&loaded.program, &low] {
            let sites = collect(program).unwrap();
            assert_eq!(sites.len(), 2);
            assert_eq!(sites[0].fields, ["id", "type"]);
            // Same-named row classes retain distinct canonical field shapes.
            assert_eq!(sites[1].fields, ["other"]);
        }
    }
}
