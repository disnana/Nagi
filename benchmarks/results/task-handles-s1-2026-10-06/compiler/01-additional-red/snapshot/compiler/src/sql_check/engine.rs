use super::{
    Operation, Query, QueryOutcome, Request, Response, MAX_QUERIES, MAX_SCHEMA_BYTES, MAX_SQL_BYTES,
};
use rusqlite::config::DbConfig;
use rusqlite::fallible_iterator::FallibleIterator;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::{Batch, Connection};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_SCHEMA_STATEMENTS: usize = 1024;
const ENGINE_DEADLINE: Duration = Duration::from_millis(4500);

#[derive(Clone, Copy)]
enum Phase {
    Schema,
    Query,
    Metadata,
}

#[derive(Default)]
struct Actions {
    creates: usize,
    created_indexes: Vec<String>,
    created_tables: Vec<String>,
    created_views: Vec<String>,
    selects: bool,
    writes: bool,
    denied: Option<String>,
}

pub(super) fn validate(request: Request) -> Response {
    let mut response = Response {
        schema_error: None,
        queries: Vec::new(),
    };
    if request.schema.len() > MAX_SCHEMA_BYTES {
        response.schema_error = Some(format!("schema exceeds {MAX_SCHEMA_BYTES} bytes"));
        return response;
    }
    if request.queries.len() > MAX_QUERIES {
        response.schema_error = Some(format!("SQL check exceeds {MAX_QUERIES} queries"));
        return response;
    }
    let deadline = Instant::now() + ENGINE_DEADLINE;
    let connection = match open_scratch(deadline) {
        Ok(connection) => connection,
        Err(error) => {
            response.schema_error = Some(error);
            return response;
        }
    };
    let catalog = match load_schema(&connection, &request.schema, deadline) {
        Ok(catalog) => catalog,
        Err(error) => {
            response.schema_error = Some(error);
            return response;
        }
    };
    if let Err(error) = connection.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, MAX_SQL_BYTES as i32) {
        response.schema_error = Some(error.to_string());
        return response;
    }
    for (index, query) in request.queries.iter().enumerate() {
        let error = check_query(&connection, query, deadline, &catalog).err();
        response.queries.push(QueryOutcome { index, error });
    }
    response
}

fn open_scratch(deadline: Instant) -> Result<Connection, String> {
    let connection = Connection::open_in_memory().map_err(|error| error.to_string())?;
    // These connection settings are not loaded from user SQL. Triggers and
    // foreign-key actions are unnecessary when DML is only prepared.
    for (config, value) in [
        (DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true),
        (DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, false),
    ] {
        connection
            .set_db_config(config, value)
            .map_err(|error| error.to_string())?;
    }
    for (limit, value) in [
        (Limit::SQLITE_LIMIT_LENGTH, MAX_SCHEMA_BYTES as i32),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, MAX_SCHEMA_BYTES as i32),
        (Limit::SQLITE_LIMIT_COLUMN, 256),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 128),
        (Limit::SQLITE_LIMIT_PARSER_DEPTH, 256),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 64),
        (Limit::SQLITE_LIMIT_VDBE_OP, 250_000),
        (Limit::SQLITE_LIMIT_FUNCTION_ARG, 64),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_LIKE_PATTERN_LENGTH, 4096),
        (Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 4096),
        (Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 0),
        (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
    ] {
        connection
            .set_limit(limit, value)
            .map_err(|error| error.to_string())?;
    }
    connection
        .progress_handler(1000, Some(move || Instant::now() >= deadline))
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

fn install_authorizer(
    connection: &Connection,
    phase: Phase,
    catalog: Arc<HashSet<String>>,
) -> Result<Arc<Mutex<Actions>>, String> {
    let actions = Arc::new(Mutex::new(Actions::default()));
    let hook_actions = Arc::clone(&actions);
    connection
        .authorizer(Some(move |context: AuthContext<'_>| {
            let Ok(mut actions) = hook_actions.lock() else {
                return Authorization::Deny;
            };
            let allowed = authorize(context, phase, &mut actions, &catalog);
            if allowed {
                Authorization::Allow
            } else {
                if actions.denied.is_none() {
                    actions.denied = Some(format!("SQL check does not allow {:?}", context.action));
                }
                Authorization::Deny
            }
        }))
        .map_err(|error| error.to_string())?;
    Ok(actions)
}

fn authorize(
    context: AuthContext<'_>,
    phase: Phase,
    actions: &mut Actions,
    catalog: &HashSet<String>,
) -> bool {
    if context.database_name.is_some_and(|name| name != "main") {
        return false;
    }
    match phase {
        // This phase is used only for the fixed metadata statement constructed
        // in check_defaults. User schema and queries never run with it.
        Phase::Metadata => matches!(
            context.action,
            AuthAction::Pragma { pragma_name: "table_xinfo", pragma_value: Some(table) }
                if context.database_name == Some("main") && catalog.contains(table)
        ),
        Phase::Schema => match context.action {
            AuthAction::CreateTable { table_name } => {
                actions.creates += 1;
                if !table_name.starts_with("sqlite_") {
                    actions.created_tables.push(table_name.to_owned());
                }
                true
            }
            AuthAction::CreateView { view_name } => {
                actions.creates += 1;
                actions.created_views.push(view_name.to_owned());
                true
            }
            AuthAction::CreateIndex { index_name, .. } => {
                actions.creates += 1;
                actions.created_indexes.push(index_name.to_owned());
                true
            }
            // SQLite uses this action to initialize a newly created index.
            // A standalone REINDEX has no matching CreateIndex action.
            AuthAction::Reindex { index_name } => actions
                .created_indexes
                .iter()
                .any(|name| name == index_name),
            // SQLite emits these writes while creating schema objects. User
            // DML is never stepped: the prepared statement must also be DDL.
            AuthAction::Insert { table_name } | AuthAction::Update { table_name, .. } => {
                is_schema_table(table_name)
            }
            AuthAction::Read { .. } => true,
            AuthAction::Function { function_name } => pure_function(function_name),
            // In particular, CREATE TABLE AS SELECT cannot populate tables.
            // A CREATE VIEW stores its SELECT without executing it.
            _ => false,
        },
        Phase::Query => match context.action {
            // Eponymous virtual tables (including pragma_* tables) need not
            // authorize their underlying operation until step. Only declared
            // ordinary tables/views and SQLite schema metadata are readable.
            AuthAction::Read { table_name, .. } => {
                catalog.contains(table_name) || is_schema_table(table_name)
            }
            AuthAction::Select => {
                actions.selects = true;
                true
            }
            AuthAction::Insert { table_name }
            | AuthAction::Update { table_name, .. }
            | AuthAction::Delete { table_name } => {
                if is_schema_table(table_name) || table_name.starts_with("sqlite_") {
                    return false;
                }
                actions.writes = true;
                true
            }
            AuthAction::Function { function_name } => pure_function(function_name),
            AuthAction::Recursive => true,
            _ => false,
        },
    }
}

fn is_schema_table(name: &str) -> bool {
    matches!(name, "sqlite_master" | "sqlite_schema")
}

fn pure_function(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "abs"
            | "avg"
            | "char"
            | "coalesce"
            | "concat"
            | "concat_ws"
            | "count"
            | "format"
            | "glob"
            | "group_concat"
            | "hex"
            | "if"
            | "ifnull"
            | "iif"
            | "instr"
            | "length"
            | "like"
            | "likelihood"
            | "likely"
            | "lower"
            | "ltrim"
            | "max"
            | "min"
            | "nullif"
            | "octet_length"
            | "printf"
            | "quote"
            | "replace"
            | "round"
            | "rtrim"
            | "sign"
            | "soundex"
            | "string_agg"
            | "substr"
            | "substring"
            | "sum"
            | "total"
            | "trim"
            | "typeof"
            | "unhex"
            | "unicode"
            | "unlikely"
            | "upper"
            | "zeroblob"
            | "json"
            | "json_array"
            | "json_array_length"
            | "json_error_position"
            | "json_extract"
            | "json_group_array"
            | "json_group_object"
            | "json_insert"
            | "json_object"
            | "json_patch"
            | "json_quote"
            | "json_remove"
            | "json_replace"
            | "json_set"
            | "json_type"
            | "json_valid"
            | "jsonb"
            | "jsonb_array"
            | "jsonb_extract"
            | "jsonb_group_array"
            | "jsonb_group_object"
            | "jsonb_insert"
            | "jsonb_object"
            | "jsonb_patch"
            | "jsonb_remove"
            | "jsonb_replace"
            | "jsonb_set"
            | "->"
            | "->>"
            | "row_number"
            | "rank"
            | "dense_rank"
            | "percent_rank"
            | "cume_dist"
            | "ntile"
            | "lag"
            | "lead"
            | "first_value"
            | "last_value"
            | "nth_value"
    )
}

fn deadline_check(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("SQL check exceeded its time limit".to_owned())
    } else {
        Ok(())
    }
}

fn sqlite_error(error: rusqlite::Error, actions: &Arc<Mutex<Actions>>) -> String {
    actions
        .lock()
        .ok()
        .and_then(|actions| actions.denied.clone())
        .unwrap_or_else(|| error.to_string())
}

fn load_schema(
    connection: &Connection,
    schema: &str,
    deadline: Instant,
) -> Result<Arc<HashSet<String>>, String> {
    let mut batch = Batch::new(connection, schema);
    let mut count = 0;
    let mut tables = Vec::new();
    let mut views = Vec::new();
    loop {
        deadline_check(deadline)?;
        let actions = install_authorizer(connection, Phase::Schema, Arc::new(HashSet::new()))?;
        let Some(mut statement) = batch
            .next()
            .map_err(|error| sqlite_error(error, &actions))?
        else {
            break;
        };
        count += 1;
        if count > MAX_SCHEMA_STATEMENTS {
            return Err(format!("schema exceeds {MAX_SCHEMA_STATEMENTS} statements"));
        }
        let creates = actions
            .lock()
            .map_err(|_| "SQL authorizer state is unavailable".to_owned())?
            .creates;
        if creates == 0 || statement.is_explain() != 0 || statement.column_count() != 0 {
            return Err(
                "schema must contain only CREATE TABLE, INDEX, or VIEW statements".to_owned(),
            );
        }
        if statement.parameter_count() != 0 {
            return Err("schema statements cannot have SQL parameters".to_owned());
        }
        statement
            .execute([])
            .map_err(|error| sqlite_error(error, &actions))?;
        let actions = actions
            .lock()
            .map_err(|_| "SQL authorizer state is unavailable".to_owned())?;
        tables.extend(actions.created_tables.iter().cloned());
        views.extend(actions.created_views.iter().cloned());
    }
    let catalog = Arc::new(tables.iter().chain(&views).cloned().collect::<HashSet<_>>());
    for table in &tables {
        check_defaults(connection, table, deadline, &catalog)?;
    }
    // SQLite defers preparing view bodies and CHECK expressions until they
    // are used. These SELECT/INSERT statements are never stepped.
    for (names, prefix, suffix) in [
        (views, "SELECT * FROM main.\"", "\""),
        (tables, "INSERT INTO main.\"", "\" DEFAULT VALUES"),
    ] {
        for name in names {
            deadline_check(deadline)?;
            let actions = install_authorizer(connection, Phase::Query, Arc::clone(&catalog))?;
            let sql = format!("{prefix}{}{suffix}", name.replace('"', "\"\""));
            connection
                .prepare(&sql)
                .map_err(|error| sqlite_error(error, &actions))?;
        }
    }
    Ok(catalog)
}

fn check_defaults(
    connection: &Connection,
    table: &str,
    deadline: Instant,
    catalog: &Arc<HashSet<String>>,
) -> Result<(), String> {
    deadline_check(deadline)?;
    let actions = install_authorizer(connection, Phase::Metadata, Arc::clone(catalog))?;
    let sql = format!(
        "PRAGMA main.table_xinfo(\"{}\")",
        table.replace('"', "\"\"")
    );
    let defaults = {
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| sqlite_error(error, &actions))?;
        let mut rows = statement
            .query([])
            .map_err(|error| sqlite_error(error, &actions))?;
        let mut defaults = Vec::new();
        while let Some(row) = rows.next().map_err(|error| sqlite_error(error, &actions))? {
            deadline_check(deadline)?;
            if let Some(default) = row
                .get::<_, Option<String>>(4)
                .map_err(|error| error.to_string())?
            {
                defaults.push(default);
            }
        }
        defaults
    };
    // INSERT preparation can omit unused defaults, notably an INTEGER PRIMARY
    // KEY default. Prepare each expression independently under the usual
    // allowlist. Neither defaults nor application rows are evaluated.
    for default in defaults {
        deadline_check(deadline)?;
        let actions = install_authorizer(connection, Phase::Query, Arc::clone(catalog))?;
        let default = default_literal(&default).unwrap_or(default);
        let sql = format!("SELECT ({default})");
        connection
            .prepare(&sql)
            .map_err(|error| sqlite_error(error, &actions))?;
    }
    Ok(())
}

/// SQLite's DEFAULT ID|INDEXED grammar converts a complete identifier token
/// into a string (apart from TRUE/FALSE). A SELECT parses the same token as a
/// column. Preserve that grammar when projecting reflected defaults; compound
/// expressions and CURRENT_* function terms still go through normal prepare.
fn default_literal(default: &str) -> Option<String> {
    let token = default.trim_matches([' ', '\t', '\n', '\r', '\u{000b}', '\u{000c}']);
    let bytes = token.as_bytes();
    let first = *bytes.first()?;
    let value = if first == b'[' {
        let inner = token.strip_prefix('[')?.strip_suffix(']')?;
        if inner.contains(']') {
            return None;
        }
        inner.to_owned()
    } else if matches!(first, b'`' | b'"') {
        let mut value = Vec::new();
        let mut position = 1;
        loop {
            let byte = *bytes.get(position)?;
            if byte == first {
                if position + 1 == bytes.len() {
                    break;
                }
                if bytes.get(position + 1) != Some(&first) {
                    return None;
                }
                position += 1;
            }
            value.push(byte);
            position += 1;
        }
        String::from_utf8(value).ok()?
    } else {
        if !(first.is_ascii_alphabetic() || first == b'_' || first >= 0x80)
            || !bytes.iter().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'$') || *byte >= 0x80
            })
            || [
                "NULL",
                "TRUE",
                "FALSE",
                "CURRENT_TIME",
                "CURRENT_DATE",
                "CURRENT_TIMESTAMP",
            ]
            .iter()
            .any(|keyword| token.eq_ignore_ascii_case(keyword))
        {
            return None;
        }
        token.to_owned()
    };
    Some(format!("'{}'", value.replace('\'', "''")))
}

fn check_query(
    connection: &Connection,
    query: &Query,
    deadline: Instant,
    catalog: &Arc<HashSet<String>>,
) -> Result<(), String> {
    deadline_check(deadline)?;
    if query.sql.len() > MAX_SQL_BYTES {
        return Err(format!("query exceeds {MAX_SQL_BYTES} bytes"));
    }
    let actions = install_authorizer(connection, Phase::Query, Arc::clone(catalog))?;
    // Batch uses SQLite's tail pointer rather than splitting on semicolons in
    // strings or comments. Neither the first nor any later statement is run.
    let mut batch = Batch::new(connection, &query.sql);
    let statement = batch
        .next()
        .map_err(|error| sqlite_error(error, &actions))?
        .ok_or_else(|| "query must contain one SQL statement".to_owned())?;
    if batch
        .next()
        .map_err(|error| sqlite_error(error, &actions))?
        .is_some()
    {
        return Err("query must contain only one SQL statement".to_owned());
    }
    if statement.is_explain() != 0 {
        return Err("EXPLAIN is not a supported database operation".to_owned());
    }
    let actions = actions
        .lock()
        .map_err(|_| "SQL authorizer state is unavailable".to_owned())?;
    let columns = statement.column_count();
    match query.operation {
        Operation::All | Operation::Query => {
            if !statement.readonly() || actions.writes || !actions.selects || columns == 0 {
                return Err("db_all/db_query require a read-only SELECT statement".to_owned());
            }
        }
        Operation::Insert | Operation::Update => {
            if statement.readonly() || !actions.writes || columns == 0 {
                return Err("db_insert/db_update require DML with RETURNING columns".to_owned());
            }
        }
        Operation::Write => {
            if statement.readonly() || !actions.writes || columns != 0 {
                return Err("db_write requires DML without returned columns".to_owned());
            }
        }
    }
    if statement.parameter_count() != query.bind_count {
        return Err(format!(
            "SQL requires {} bind parameters, but this operation supplies {}",
            statement.parameter_count(),
            query.bind_count
        ));
    }
    let missing: Vec<_> = query
        .fields
        .iter()
        .filter(|field| statement.column_index(field).is_err())
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "SQL result is missing row fields: {}",
            missing.join(", ")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const SCHEMA: &str = "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT, age INTEGER);";

    fn query(operation: Operation, sql: &str, binds: usize, fields: &[&str]) -> Query {
        Query {
            operation,
            sql: sql.to_owned(),
            bind_count: binds,
            fields: fields.iter().map(|field| (*field).to_owned()).collect(),
        }
    }

    fn result(schema: &str, queries: Vec<Query>) -> Response {
        validate(Request {
            schema: schema.to_owned(),
            queries,
        })
    }

    fn assert_ok(query: Query) {
        let response = result(SCHEMA, vec![query]);
        assert!(
            response.schema_error.is_none(),
            "{:?}",
            response.schema_error
        );
        assert!(
            response.queries[0].error.is_none(),
            "{:?}",
            response.queries[0].error
        );
    }

    #[test]
    fn columns_aliases_and_sqlite_parameter_numbering() {
        assert_ok(query(
            Operation::All,
            "SELECT name, age, id FROM users",
            0,
            &["id", "name"],
        ));
        assert_ok(query(
            Operation::Query,
            "SELECT id AS ID, name FROM users WHERE id=?1 OR age=?1",
            1,
            &["id", "name"],
        ));
        assert_ok(query(
            Operation::Query,
            "SELECT id FROM users WHERE id=?5",
            5,
            &["id"],
        ));
        assert_ok(query(
            Operation::All,
            "WITH named AS (SELECT name FROM users) SELECT name FROM named",
            0,
            &["name"],
        ));
        assert_ok(query(
            Operation::All,
            "SELECT ';' AS name FROM users; -- trailing SQL comment\n /* ; */",
            0,
            &["name"],
        ));
        let response = result(
            SCHEMA,
            vec![
                query(Operation::All, "SELECT naem FROM users", 0, &["name"]),
                query(Operation::All, "SELECT id FROM users", 0, &["id", "name"]),
                query(
                    Operation::Query,
                    "SELECT id FROM users WHERE id=?5",
                    1,
                    &["id"],
                ),
                query(
                    Operation::All,
                    "SELECT id FROM users; SELECT id FROM users",
                    0,
                    &["id"],
                ),
            ],
        );
        assert!(response.queries.iter().all(|query| query.error.is_some()));
        assert!(response.queries[1].error.as_ref().unwrap().contains("name"));
        assert!(response.queries[2]
            .error
            .as_ref()
            .unwrap()
            .contains("5 bind"));
    }

    #[test]
    fn prepare_only_dml_does_not_write_application_rows() {
        let deadline = Instant::now() + ENGINE_DEADLINE;
        let connection = open_scratch(deadline).unwrap();
        let catalog = load_schema(&connection, SCHEMA, deadline).unwrap();
        for query in [
            query(
                Operation::Insert,
                "INSERT INTO users(name,age) VALUES(?1,?2) RETURNING id,name,age",
                2,
                &["id", "name", "age"],
            ),
            query(
                Operation::Update,
                "UPDATE users SET name=?2,age=?3 WHERE id=?1 RETURNING id,name,age",
                3,
                &["id", "name", "age"],
            ),
            query(Operation::Write, "DELETE FROM users WHERE id=?1", 1, &[]),
            query(
                Operation::Update,
                "DELETE FROM users WHERE id=?1 RETURNING id",
                1,
                &["id"],
            ),
        ] {
            check_query(&connection, &query, deadline, &catalog).unwrap();
            assert_eq!(connection.total_changes(), 0);
        }
        let count: i64 = connection
            .query_row("SELECT count(*) FROM users", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn rejects_operation_shape_mismatches() {
        let response = result(
            SCHEMA,
            vec![
                query(Operation::All, "DELETE FROM users RETURNING id", 0, &["id"]),
                query(Operation::Write, "SELECT id FROM users WHERE id=?1", 1, &[]),
                query(
                    Operation::Write,
                    "DELETE FROM users WHERE id=?1 RETURNING id",
                    1,
                    &[],
                ),
                query(
                    Operation::Insert,
                    "INSERT INTO users(name) VALUES(?1)",
                    1,
                    &["id"],
                ),
                query(
                    Operation::Update,
                    "SELECT id FROM users WHERE id=?1",
                    1,
                    &["id"],
                ),
                query(Operation::All, "EXPLAIN SELECT id FROM users", 0, &["id"]),
            ],
        );
        for query in &response.queries {
            assert!(query.error.is_some(), "query {} was accepted", query.index);
        }
    }

    #[test]
    fn ordinary_tables_indexes_views_and_pure_functions_are_supported() {
        let schema = "/* schema ; */ CREATE TABLE users(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT CHECK(length(name) < 100), age INTEGER); CREATE UNIQUE INDEX names ON users(lower(name)); CREATE VIEW names_view AS SELECT id, name FROM users;";
        let response = result(
            schema,
            vec![query(
                Operation::All,
                "SELECT id, upper(name) AS name FROM names_view",
                0,
                &["id", "name"],
            )],
        );
        assert!(
            response.schema_error.is_none(),
            "{:?}",
            response.schema_error
        );
        assert!(
            response.queries[0].error.is_none(),
            "{:?}",
            response.queries[0].error
        );
    }

    #[test]
    fn schema_is_ddl_snapshot_not_migration_or_arbitrary_sql() {
        for schema in [
            "SELECT 1;",
            "PRAGMA writable_schema=ON;",
            "CREATE TEMP TABLE secrets(id);",
            "CREATE VIRTUAL TABLE secrets USING fts5(content);",
            "CREATE TABLE users(id); INSERT INTO users VALUES(1);",
            "CREATE TABLE users AS SELECT 1 AS id;",
            "CREATE TABLE users(id); CREATE TRIGGER log AFTER INSERT ON users BEGIN SELECT 1; END;",
            "CREATE TABLE users(id); DROP TABLE users;",
            "BEGIN; CREATE TABLE users(id); COMMIT;",
            "EXPLAIN CREATE TABLE users(id);",
            "CREATE TABLE users(id); ANALYZE users;",
        ] {
            assert!(
                result(schema, vec![]).schema_error.is_some(),
                "accepted {schema}"
            );
        }
    }

    #[test]
    fn external_files_and_extension_paths_are_never_opened_or_modified() {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "nagi-sql-engine-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let existing = base.with_extension("db");
        let absent = base.with_extension("created.db");
        {
            let connection = Connection::open(&existing).unwrap();
            connection
                .execute("CREATE TABLE marker(value)", [])
                .unwrap();
            connection
                .execute("INSERT INTO marker VALUES('unchanged')", [])
                .unwrap();
        }
        let before = std::fs::read(&existing).unwrap();
        let sql_path = |path: &std::path::Path| path.to_string_lossy().replace('\'', "''");
        for sql in [
            format!("ATTACH '{}' AS other", sql_path(&existing)),
            format!("ATTACH '{}' AS other", sql_path(&absent)),
            format!("VACUUM INTO '{}'", sql_path(&absent)),
            format!("SELECT load_extension('{}')", sql_path(&existing)),
            format!("SELECT writefile('{}','changed')", sql_path(&existing)),
            format!("SELECT readfile('{}')", sql_path(&existing)),
        ] {
            assert!(
                result(&sql, vec![]).schema_error.is_some(),
                "schema accepted {sql}"
            );
            let response = result(SCHEMA, vec![query(Operation::All, &sql, 0, &["id"])]);
            assert!(response.queries[0].error.is_some(), "query accepted {sql}");
        }
        assert_eq!(before, std::fs::read(&existing).unwrap());
        assert!(!absent.exists());
        std::fs::remove_file(existing).unwrap();
    }

    #[test]
    fn prepare_authorizer_also_rejects_unsafe_view_functions() {
        let response = result(
            "CREATE VIEW unsafe_view AS SELECT load_extension('missing') AS id;",
            vec![query(
                Operation::All,
                "SELECT id FROM unsafe_view",
                0,
                &["id"],
            )],
        );
        assert!(response.schema_error.is_some());
        assert!(result(
            "CREATE TABLE users(id DEFAULT(load_extension('missing')));",
            vec![]
        )
        .schema_error
        .is_some());
        let response = result(
            SCHEMA,
            vec![
                query(Operation::All, "PRAGMA table_info(users)", 0, &[]),
                query(Operation::All, "SELECT random() AS id", 0, &["id"]),
                query(
                    Operation::All,
                    "SELECT name FROM pragma_table_info('users')",
                    0,
                    &["name"],
                ),
                query(Operation::All, "DETACH main", 0, &[]),
            ],
        );
        for query in &response.queries {
            assert!(query.error.is_some(), "query {} was accepted", query.index);
        }
        let response = result(
            "CREATE TABLE pragma_table_info(name TEXT);",
            vec![query(
                Operation::All,
                "SELECT name FROM pragma_table_info",
                0,
                &["name"],
            )],
        );
        assert!(response.schema_error.is_none());
        assert!(response.queries[0].error.is_none());
    }

    #[test]
    fn every_default_expression_uses_the_function_allowlist_even_when_unused() {
        for default in [
            "(random())",
            "(date('now'))",
            "CURRENT_TIMESTAMP",
            "(load_extension('missing'))",
            "(no_such_function(1))",
        ] {
            for declaration in ["INTEGER", "INTEGER PRIMARY KEY"] {
                let schema =
                    format!("CREATE TABLE users(id {declaration} DEFAULT {default}, name TEXT);");
                assert!(
                    result(&schema, vec![]).schema_error.is_some(),
                    "accepted {schema}"
                );
            }
        }
        let deadline = Instant::now() + ENGINE_DEADLINE;
        let connection = open_scratch(deadline).unwrap();
        let schema = "CREATE TABLE \"weird\"\";table\"(id INTEGER PRIMARY KEY DEFAULT(abs(-1)), name TEXT DEFAULT(lower('ABC')), value TEXT DEFAULT('); ATTACH ''missing'' AS other; --'));";
        let catalog = load_schema(&connection, schema, deadline).unwrap();
        check_query(
            &connection,
            &query(
                Operation::All,
                "SELECT id, name, value FROM \"weird\"\";table\"",
                0,
                &["id", "name", "value"],
            ),
            deadline,
            &catalog,
        )
        .unwrap();
        assert_eq!(connection.total_changes(), 0);
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM \"weird\"\";table\"", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn sqlite_identifier_defaults_remain_literals_without_hiding_function_terms() {
        for default in [
            "abc",
            "cast",
            "CURRENT_USER",
            "abc$def",
            "日本語",
            "😇",
            "[abc]",
            "[CURRENT_TIMESTAMP]",
            "`abc`",
            "`ab``cd`",
            "\"abc\"",
            "\"ab\"\"cd\"",
            "[weird()'name]",
            "''",
            "'quoted''text'",
            "('abc')",
            "TRUE",
            "FALSE",
            "NULL",
            "+12",
            "-12",
            "X'6162'",
        ] {
            for declaration in ["TEXT", "INTEGER PRIMARY KEY"] {
                let schema =
                    format!("CREATE TABLE users(id {declaration} DEFAULT {default}, name TEXT);");
                let response = result(&schema, vec![]);
                assert!(
                    response.schema_error.is_none(),
                    "rejected {schema}: {:?}",
                    response.schema_error
                );
            }
        }
        for default in [
            "(random())",
            "((random()))",
            "(date('now'))",
            "CURRENT_TIME",
            "CURRENT_DATE",
            "CURRENT_TIMESTAMP",
            "(+CURRENT_TIME)",
            "(load_extension('missing'))",
            "(`load_extension`('missing'))",
            "(\"load_extension\"('missing'))",
        ] {
            let schema = format!("CREATE TABLE users(id INTEGER PRIMARY KEY DEFAULT {default});");
            assert!(
                result(&schema, vec![]).schema_error.is_some(),
                "accepted {schema}"
            );
        }
    }

    #[test]
    fn finite_limits_reject_large_and_deep_inputs() {
        assert!(result(&" ".repeat(MAX_SCHEMA_BYTES + 1), vec![])
            .schema_error
            .is_some());
        let response = result(
            SCHEMA,
            vec![query(
                Operation::All,
                &" ".repeat(MAX_SQL_BYTES + 1),
                0,
                &[],
            )],
        );
        assert!(response.queries[0].error.is_some());
        let schema = (0..=MAX_SCHEMA_STATEMENTS)
            .map(|index| format!("CREATE TABLE t{index}(id);\n"))
            .collect::<String>();
        assert!(result(&schema, vec![])
            .schema_error
            .unwrap()
            .contains("statements"));
        let response = result(
            SCHEMA,
            vec![
                query(Operation::All, "SELECT ?4097 AS id", 4097, &["id"]),
                query(
                    Operation::All,
                    &format!("SELECT {}1{} AS id", "(".repeat(300), ")".repeat(300)),
                    0,
                    &["id"],
                ),
                query(
                    Operation::All,
                    &(0..65)
                        .map(|_| "SELECT 1 AS id")
                        .collect::<Vec<_>>()
                        .join(" UNION ALL "),
                    0,
                    &["id"],
                ),
            ],
        );
        assert!(response.queries.iter().all(|query| query.error.is_some()));
        let deadline = Instant::now() - Duration::from_millis(1);
        let connection = open_scratch(deadline).unwrap();
        assert!(load_schema(&connection, SCHEMA, deadline)
            .unwrap_err()
            .contains("time limit"));
        assert!(check_query(
            &connection,
            &query(Operation::All, "SELECT 1", 0, &[]),
            deadline,
            &Arc::new(HashSet::new())
        )
        .unwrap_err()
        .contains("time limit"));
    }
}
