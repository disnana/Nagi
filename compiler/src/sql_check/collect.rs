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
            let (operation, bind_count) = match standard {
                Some(
                    crate::stdlib::Operation::SqliteQuery | crate::stdlib::Operation::SqliteAll,
                ) => (Some(Operation::SqliteRows), None),
                Some(crate::stdlib::Operation::SqliteExec) => (Some(Operation::SqliteExec), None),
                _ if expression.resolution == Some(NameResolution::Builtin) => {
                    match name.as_str() {
                        "db_all" => (Some(Operation::All), Some(0)),
                        "db_query" => (Some(Operation::Query), Some(1)),
                        "db_insert" => (Some(Operation::Insert), Some(2)),
                        "db_update" => (Some(Operation::Update), Some(3)),
                        "db_write" => (Some(Operation::Write), Some(1)),
                        "db_exec" => (None, Some(0)),
                        _ => return,
                    }
                }
                _ => return,
            };
            if sites.len() >= MAX_QUERIES {
                failure = Some(format!(
                    "line {}: SQL check exceeds the {MAX_QUERIES} call-site limit",
                    expression.line
                ));
                return;
            }
            let sql = arguments.get(1).and_then(|argument| {
                if let E::Str(sql) = &argument.kind {
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
            let reason = if operation.is_none() {
                Some("db_exec scripts are checked at runtime")
            } else if sql.is_none() {
                Some("dynamic SQL is checked at runtime")
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

    fn checked(source: &str) -> Program {
        let mut program = parser::parse(source, true).unwrap();
        check::check(&mut program).unwrap();
        program
    }

    const OPERATIONS: &str = r#"class Row:
    id: i64
    type: str
async def read(db: Db) -> Result[unit, Error]:
    rows = try await db_all[Row](db, "SELECT id, type FROM items")
    row = try await db_query[Row](db, "SELECT id, type FROM items WHERE id = ?", 1)
    inserted = try await db_insert[Row](db, "INSERT INTO items(type, id) VALUES (?, ?) RETURNING id, type", "name", 1)
    updated = try await db_update[Row](db, "UPDATE items SET type = ?, id = ? WHERE id = ? RETURNING id, type", 1, "name", 2)
    changed = try await db_write(db, "DELETE FROM items WHERE id = ?", 1)
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
                (Operation::All, 0),
                (Operation::Query, 1),
                (Operation::Insert, 2),
                (Operation::Update, 3),
                (Operation::Write, 1),
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
                    if index == 4 {
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
            vec![5, 6, 7, 8, 9]
        );
    }

    #[test]
    fn same_named_user_function_and_local_function_value_are_not_sql_calls() {
        let functions = checked(
            "def db_all(db: Db, sql: str) -> i64:\n    return 1\ndef read(db: Db) -> i64:\n    return db_all(db, \"this is not SQL\")\n",
        );
        assert!(collect(&functions).unwrap().is_empty());
        let locals = checked(
            "def fake(db: Db, sql: str) -> i64:\n    return 1\ndef read(db: Db) -> i64:\n    db_all = fake\n    return db_all(db, \"this is not SQL\")\n",
        );
        assert!(collect(&locals).unwrap().is_empty());
    }

    #[test]
    fn dynamic_sql_and_exec_scripts_remain_visible_as_excluded_sites() {
        let program = checked(
            "class Row:\n    id: i64\nasync def read(db: Db, sql: str) -> Result[unit, Error]:\n    rows = try await db_all[Row](db, sql)\n    borrowed = try await db_all[Row](db, view(\"SELECT id FROM items\"))\n    changed = try await db_exec(db, \"CREATE TABLE items(id INTEGER); DELETE FROM items\")\n    return ok(print(changed))\n",
        );
        let sites = collect(&program).unwrap();
        assert_eq!(sites.len(), 3);
        assert!(sites[..2]
            .iter()
            .all(|site| site.sql.is_none() && site.reason.is_some()));
        assert!(sites[2].operation.is_none());
        assert_eq!(sites[2].operation_name, "db_exec");
        assert!(sites[2].sql.as_ref().unwrap().contains("CREATE TABLE"));
        assert!(sites[2].reason.as_ref().unwrap().contains("scripts"));
    }

    #[test]
    fn conditional_loop_and_match_bodies_are_collected_once() {
        let program = checked(
            "class Row:\n    id: i64\nasync def read(db: Db) -> Result[unit, Error]:\n    if true:\n        rows = try await db_all[Row](db, \"SELECT id FROM first\")\n    else:\n        rows = try await db_all[Row](db, \"SELECT id FROM second\")\n    for index in range(2):\n        rows = try await db_all[Row](db, \"SELECT id FROM looped\")\n    while false:\n        rows = try await db_all[Row](db, \"SELECT id FROM waited\")\n    result = await db_all[Row](db, \"SELECT id FROM matched\")\n    match result:\n        case Ok(rows):\n            print(len(view(rows)))\n        case Err(error):\n            rows = try await db_all[Row](db, \"SELECT id FROM failed\")\n    return ok(print(1))\n",
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
            "class Row:\n    id: i64\nclass Batch:\n    rows: List[Row]\nasync def work(count: i64):\n    print(count)\nasync def read(db: Db) -> Result[i64, Error]:\n    batch = Batch(rows=try await db_all[Row](db, \"SELECT id FROM records\"))\n    batches = [try await db_all[Row](db, \"SELECT id FROM lists\")]\n    total = (try await db_all[Row](db, \"SELECT id FROM indexed\"))[0].id + len(view(try await db_all[Row](db, \"SELECT id FROM counted\")))\n    negative = -(try await db_all[Row](db, \"SELECT id FROM negated\"))[0].id\n    async with scope:\n        spawn work(try await db_write(db, \"DELETE FROM spawned WHERE id = ?\", 1))\n    return await db_write(db, \"DELETE FROM returned WHERE id = ?\", 1)\n",
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
                "DELETE FROM spawned WHERE id = ?",
                "DELETE FROM returned WHERE id = ?",
            ]
        );
    }

    #[test]
    fn integrated_native_replacements_only_collect_the_final_function_body() {
        let mut program = checked(
            "class Row:\n    id: i64\nasync def read(db: Db) -> Result[List[Row], Error]:\n    return await db_all[Row](db, \"SELECT id FROM discarded\")\n",
        );
        let native = parser::parse(
            "@replace generated::read\nasync fn read(db: Db) -> Result[List[Row], Error] { return await db_all[Row](db, \"SELECT id FROM actual\"); }\n",
            false,
        )
        .unwrap();
        check::integrate(&mut program, native).unwrap();
        let sites = collect(&program).unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].sql.as_deref(), Some("SELECT id FROM actual"));
        assert_eq!(sites[0].fields, ["id"]);
        assert_eq!(sites[0].line, 2);
    }

    #[test]
    fn repeated_row_metadata_is_bounded_before_building_the_worker_request() {
        let mut program = checked(
            "class Row:\n    id: i64\nasync def read(db: Db) -> Result[List[Row], Error]:\n    return await db_all[Row](db, \"SELECT id FROM users\")\n",
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
        assert!(error.starts_with("line 4:"), "{error}");
        assert!(error.contains("metadata/input limit"), "{error}");
    }

    #[test]
    fn collection_rejects_excessive_sites_at_the_first_over_limit_call() {
        let mut program = checked(
            "async def read(db: Db) -> Result[i64, Error]:\n    return await db_write(db, \"DELETE FROM users WHERE id = ?\", 1)\n",
        );
        let statement = program.functions[0].body[0].clone();
        program.functions[0].body = vec![statement; MAX_QUERIES + 1];
        let error = collect(&program).err().expect("too many sites must fail");
        assert!(error.starts_with("line 2:"), "{error}");
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
        fixture.write("main.nagi", "import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nasync def read(tx: sqlite.Tx, sql: str) -> Result[unit, sqlite.Failure]:\n    row = try await sqlite.query[Row](tx, \"SELECT id FROM users WHERE id=?\", sqlite.parameters())\n    rows = try await sqlite.all[Row](tx, sql, sqlite.parameters())\n    count = try await sqlite.exec(tx, \"CREATE TABLE new_table(id INTEGER)\", sqlite.parameters())\n    return await sqlite.rollback(tx)\n");
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program).unwrap();
        let mut low = parser::parse(&emit::low(&loaded.program), false).unwrap();
        check::check(&mut low).unwrap();
        for program in [&loaded.program, &low] {
            let sites = collect(program).unwrap();
            assert_eq!(sites.len(), 3);
            assert_eq!(sites[0].operation, Some(Operation::SqliteRows));
            assert_eq!(sites[2].operation, Some(Operation::SqliteExec));
            assert!(sites.iter().all(|s| s.bind_count.is_none()));
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
        fixture.write("right.nagi", "class Row:\n    other: u64\n");
        fixture.write(
            "main.nagi",
            "import \"left.nagi\" as left\nfrom \"right.nagi\" import Row as Other\nasync def read(db: Db) -> Result[unit, Error]:\n    rows = try await db_all[owned[owned[left.Row]]](db, \"SELECT id, type FROM left_rows\")\n    others = try await db_all[Other](db, \"SELECT other FROM right_rows\")\n    return ok(print(1))\n",
        );
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program).unwrap();
        let mut low = parser::parse(&emit::low(&loaded.program), false).unwrap();
        check::check(&mut low).unwrap();
        for program in [&loaded.program, &low] {
            let sites = collect(program).unwrap();
            assert_eq!(sites.len(), 2);
            assert_eq!(sites[0].fields, ["id", "type"]);
            // Unsupported auto-FromRow fields can still have a Rust impl;
            // collection must not reject that existing escape hatch.
            assert_eq!(sites[1].fields, ["other"]);
        }
    }
}
