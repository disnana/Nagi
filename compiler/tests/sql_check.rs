use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nagi sql check {} {}",
            std::process::id(),
            FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let fixture = Self(root);
        fixture.write("schema.sql", SCHEMA);
        fixture.write("main.nagi", "def main():\n    print(42)\n");
        fixture
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_from(&self.0, args)
    }

    fn run_from(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_nagic"))
            .args(args)
            .current_dir(cwd)
            // SQL checking must not need Cargo, runtime installation, or a DB server.
            .env("PATH", &self.0)
            .env("NAGI_ROOT", self.0.join("missing-runtime"))
            .output()
            .unwrap()
    }

    fn sql_check(&self, source: &str) -> Output {
        self.run(&[
            "check",
            source,
            "--no-project",
            "--out",
            "build",
            "--sql-schema",
            "schema.sql",
            "--sql-dialect",
            "sqlite",
        ])
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const SCHEMA: &str =
    "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER);\n";

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        stderr(&output)
    );
    assert!(output.stdout.is_empty(), "{output:?}");
    stderr(&output)
}

fn failure(output: Output) -> String {
    assert!(!output.status.success(), "{output:?}");
    stderr(&output)
}

#[cfg(not(feature = "sql-check"))]
#[test]
fn compiler_without_sql_engine_preserves_regular_check_and_reports_feature_requirement() {
    let fixture = Fixture::new();
    success(fixture.run(&["check", "main.nagi", "--no-project", "--out", "build"]));
    let diagnostic = failure(fixture.sql_check("main.nagi"));
    assert!(diagnostic.contains("sql-check"), "{diagnostic}");
}

#[cfg(feature = "sql-check")]
mod enabled {
    use super::*;
    use nagic::{check, emit, parser, source};

    fn row_program(body: &str) -> String {
        format!(
            "import std.db.sqlite as sqlite\nclass User:\n    id: i64\n    name: str\n\nasync def query(db: view[sqlite.Tx]) -> Result[unit, sqlite.Failure]:\n{body}\n    return ok(print(1))\n\ndef main():\n    print(0)\n"
        )
    }

    fn write_high_and_independent_low(fixture: &Fixture, text: &str) -> String {
        fixture.write("main.nagi", text);
        let mut loaded = source::load(&fixture.0.join("main.nagi"), true).unwrap();
        check::check(&mut loaded.program).unwrap();
        let low = emit::low(&loaded.program);
        let mut independent = parser::parse(&low, false).unwrap();
        check::check(&mut independent).unwrap();
        fixture.write("saved.low", &low);
        low
    }

    fn query_line(text: &str, needle: &str) -> usize {
        text.lines()
            .position(|line| line.contains(needle))
            .map(|line| line + 1)
            .unwrap()
    }

    fn diagnostic_on(diagnostic: &str, source: &str, line: usize) {
        assert!(
            diagnostic.contains(&format!("{source}:{line}")),
            "SQL error must point to the query in {source}:{line}: {diagnostic}"
        );
    }

    #[test]
    fn sqlite_public_literal_shape_columns_and_unknown_bind_count_use_source_origins() {
        let fixture = Fixture::new();
        let good = "import std.db.sqlite as sqlite\nclass Row:\n    id: i64\n    name: str\nasync def read(tx: sqlite.Tx, sql: sqlite.Query, params: sqlite.Parameters) -> Result[unit, sqlite.Failure]:\n    row = try await sqlite.query[Row](tx, sqlite.literal(\"SELECT id,name FROM users WHERE id=?\"), params)\n    rows = try await sqlite.all[Row](tx, sql, sqlite.parameters())\n    count = try await sqlite.exec(tx, sqlite.literal(\"CREATE TABLE prepared_only(id INTEGER)\"), sqlite.parameters())\n    return await sqlite.rollback(tx)\n";
        write_high_and_independent_low(&fixture, good);
        for source in ["main.nagi", "saved.low"] {
            let output = fixture.sql_check(source);
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(output.status.success(), "{diagnostic}");
            assert!(
                diagnostic.contains("SQL checked 2 literal queries; 1 runtime/unsupported sites"),
                "{diagnostic}"
            );
            assert!(diagnostic.contains("SQL bind unchecked:"), "{diagnostic}");
            assert!(
                diagnostic.contains("Parameters bind count is unknown"),
                "{diagnostic}"
            );
            assert!(
                diagnostic.contains("Query structure is not statically available"),
                "{diagnostic}"
            );
        }
        for (sql, fragment) in [
            (
                "SELECT id FROM users WHERE id=?",
                "missing row fields: name",
            ),
            ("SELECT id,name FROM users WHERE id=?1", "anonymous ?"),
            ("SELECT id,name FROM users WHERE id=:id", "anonymous ?"),
            ("DELETE FROM users RETURNING id,name", "read-only"),
        ] {
            let source = good.replace("SELECT id,name FROM users WHERE id=?", sql);
            let low = write_high_and_independent_low(&fixture, &source);
            for (name, source) in [("main.nagi", source.as_str()), ("saved.low", low.as_str())] {
                let output = fixture.sql_check(name);
                let diagnostic = String::from_utf8_lossy(&output.stderr);
                assert!(!output.status.success(), "{diagnostic}");
                assert!(diagnostic.contains(fragment), "{diagnostic}");
                diagnostic_on(&diagnostic, name, query_line(source, sql));
            }
        }
        for (sql, fragment) in [
            ("SELECT id FROM users", "without returned columns"),
            ("DELETE FROM users RETURNING id", "without returned columns"),
            (
                "CREATE TABLE prepared_only(id); SELECT id FROM users",
                "only one SQL statement",
            ),
        ] {
            let source = good.replace("CREATE TABLE prepared_only(id INTEGER)", sql);
            let low = write_high_and_independent_low(&fixture, &source);
            for (name, source) in [("main.nagi", source.as_str()), ("saved.low", low.as_str())] {
                let output = fixture.sql_check(name);
                let diagnostic = String::from_utf8_lossy(&output.stderr);
                assert!(!output.status.success(), "{diagnostic}");
                assert!(diagnostic.contains(fragment), "{diagnostic}");
                diagnostic_on(&diagnostic, name, query_line(source, sql));
            }
        }
    }

    #[test]
    fn schema_check_is_opt_in_and_reports_bad_columns_in_high_and_saved_low() {
        let fixture = Fixture::new();
        let high = row_program(
            "    rows = try await sqlite.all[User](db, sqlite.literal(\"SELECT id, naem AS name FROM users\"), sqlite.parameters())",
        );
        let low = write_high_and_independent_low(&fixture, &high);
        for (name, text) in [("main.nagi", &high), ("saved.low", &low)] {
            success(fixture.run(&["check", name, "--no-project", "--out", "build"]));
            let diagnostic = failure(fixture.sql_check(name));
            diagnostic_on(&diagnostic, name, query_line(text, "naem"));
            assert!(diagnostic.contains("naem"), "{diagnostic}");
        }
        fs::remove_file(fixture.0.join("main.nagi")).unwrap();
        // A saved Low file keeps the diagnostic on its own physical query line.
        let diagnostic = failure(fixture.sql_check("saved.low"));
        diagnostic_on(&diagnostic, "saved.low", query_line(&low, "naem"));
    }

    #[test]
    fn missing_return_columns_and_wrong_bind_count_use_query_source_lines() {
        for (body, needle) in [
            (
                "    rows = try await sqlite.all[User](db, sqlite.literal(\"SELECT id FROM users\"), sqlite.parameters())",
                "name",
            ),
            (
                "    row = try await sqlite.query[User](db, sqlite.literal(\"SELECT id, name FROM users\"), sqlite.bind_i64(sqlite.parameters(), 7))",
                "bind",
            ),
            (
                "    row = try await sqlite.query[User](db, sqlite.literal(\"SELECT id, name FROM users WHERE id = ? OR age = ?\"), sqlite.bind_i64(sqlite.parameters(), 7))",
                "bind",
            ),
        ] {
            let fixture = Fixture::new();
            let high = row_program(body);
            let low = write_high_and_independent_low(&fixture, &high);
            for (name, text) in [("main.nagi", &high), ("saved.low", &low)] {
                let diagnostic = failure(fixture.sql_check(name));
                diagnostic_on(&diagnostic, name, query_line(text, "SELECT"));
                assert!(diagnostic.to_lowercase().contains(needle), "{diagnostic}");
            }
        }
    }

    #[test]
    fn aliases_case_reordering_extra_columns_and_repeated_bound_values_are_valid() {
        for body in [
            "    rows = try await sqlite.all[User](db, sqlite.literal(\"SELECT upper(name) AS NAME, id AS ID, age FROM users\"), sqlite.parameters())",
            "    row = try await sqlite.query[User](db, sqlite.literal(\"SELECT name, id FROM users WHERE id = ? OR age = ?\"), sqlite.bind_i64(sqlite.bind_i64(sqlite.parameters(), 7), 7))",
        ] {
            let fixture = Fixture::new();
            let high = row_program(body);
            let low = write_high_and_independent_low(&fixture, &high);
            for name in ["main.nagi", "saved.low"] {
                let report = success(fixture.sql_check(name));
                assert!(report.contains("SQL checked 1"), "{report}");
            }
            assert!(low.contains("SELECT"));
        }
    }

    #[test]
    fn literal_insert_update_delete_are_prepared_without_executing_queries() {
        let fixture = Fixture::new();
        fixture.write(
            "schema.sql",
            "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER, CHECK(0));\n",
        );
        let high = row_program(
            "    inserted = try await sqlite.exec(db, sqlite.literal(\"INSERT INTO users(name, age) VALUES (?, ?)\"), sqlite.bind_i64(sqlite.bind_text(sqlite.parameters(), \"alice\"), 18))\n    updated = try await sqlite.exec(db, sqlite.literal(\"UPDATE users SET name = ?, age = ? WHERE id = ?\"), sqlite.bind_i64(sqlite.bind_text(sqlite.bind_i64(sqlite.parameters(), 1), \"bob\"), 20))\n    deleted = try await sqlite.exec(db, sqlite.literal(\"DELETE FROM users WHERE id = ?\"), sqlite.bind_i64(sqlite.parameters(), 1))",
        );
        write_high_and_independent_low(&fixture, &high);
        for name in ["main.nagi", "saved.low"] {
            let report = success(fixture.sql_check(name));
            assert!(report.contains("SQL checked 3"), "{report}");
        }
        // Executing the INSERT would violate CHECK(0). Successful checking proves
        // that this literal DML is prepared without being stepped.
    }

    #[test]
    fn helper_statement_kind_and_single_statement_contracts_are_checked() {
        for body in [
            "    inserted = try await sqlite.exec(db, sqlite.literal(\"SELECT 1 AS id, ? AS name, ? AS age\"), sqlite.bind_i64(sqlite.bind_text(sqlite.parameters(), \"alice\"), 18))",
            "    updated = try await sqlite.exec(db, sqlite.literal(\"SELECT ? AS id, ? AS name, ? AS age\"), sqlite.bind_i64(sqlite.bind_text(sqlite.bind_i64(sqlite.parameters(), 1), \"bob\"), 18))",
            "    rows = try await sqlite.all[User](db, sqlite.literal(\"SELECT id, name FROM users; SELECT id, name FROM users\"), sqlite.parameters())",
            "    deleted = try await sqlite.exec(db, sqlite.literal(\"DELETE FROM users WHERE id = ? RETURNING id\"), sqlite.bind_i64(sqlite.parameters(), 1))",
            "    deleted = try await sqlite.exec(db, sqlite.literal(\"SELECT ?\"), sqlite.bind_i64(sqlite.parameters(), 1))",
        ] {
            let fixture = Fixture::new();
            let high = row_program(body);
            let low = write_high_and_independent_low(&fixture, &high);
            for (name, text) in [("main.nagi", &high), ("saved.low", &low)] {
                let diagnostic = failure(fixture.sql_check(name));
                diagnostic_on(&diagnostic, name, query_line(text, "try await"));
            }
        }
    }

    #[test]
    fn query_values_are_reported_as_runtime_structure_checks() {
        let fixture = Fixture::new();
        let high=row_program("    selected = sqlite.literal(\"SELECT id, name FROM users\")\n    rows = try await sqlite.all[User](db, selected, sqlite.parameters())\n    changed = try await sqlite.exec(db, sqlite.literal(\"CREATE TABLE local_table(id INTEGER)\"), sqlite.parameters())\n    literal = try await sqlite.all[User](db, sqlite.literal(\"SELECT id, name FROM users\"), sqlite.parameters())");
        let low = write_high_and_independent_low(&fixture, &high);
        for (name, text) in [("main.nagi", &high), ("saved.low", &low)] {
            let report = success(fixture.sql_check(name));
            assert!(report.contains("SQL checked 2"), "{report}");
            assert!(report.contains("1 runtime"), "{report}");
            diagnostic_on(&report, name, query_line(text, "(db, selected"));
        }
    }

    #[test]
    fn a_user_function_named_db_all_is_not_a_builtin_sql_query() {
        let fixture = Fixture::new();
        let high = "def db_all(sql: str) -> i64:\n    return 1\ndef main():\n    print(db_all(\"SELECT no_such_column FROM missing_table\"))\n";
        write_high_and_independent_low(&fixture, high);
        for name in ["main.nagi", "saved.low"] {
            let report = success(fixture.sql_check(name));
            assert!(report.contains("SQL checked 0"), "{report}");
            assert!(!report.contains("no_such_column"), "{report}");
        }
    }

    #[test]
    fn imported_queries_report_the_original_module_file_and_line() {
        let fixture = Fixture::new();
        fixture.write(
            "queries.nagi",
            "import std.db.sqlite as sqlite\nclass Row:\n    id: i64\nasync def users(db: view[sqlite.Tx]) -> Result[List[Row], sqlite.Failure]:\n    return await sqlite.all[Row](db, sqlite.literal(\"SELECT typo FROM users\"), sqlite.parameters())\n",
        );
        fixture.write(
            "main.nagi",
            "import \"queries.nagi\" as queries\ndef main():\n    print(0)\n",
        );
        let diagnostic = failure(fixture.sql_check("main.nagi"));
        diagnostic_on(&diagnostic, "queries.nagi", 5);
        assert!(diagnostic.contains("typo"), "{diagnostic}");
    }

    #[test]
    fn schema_and_dialect_are_paired_check_only_options() {
        let fixture = Fixture::new();
        for args in [
            vec!["check", "main.nagi", "--sql-schema", "schema.sql"],
            vec!["check", "main.nagi", "--sql-dialect", "sqlite"],
            vec![
                "check",
                "main.nagi",
                "--sql-schema",
                "schema.sql",
                "--sql-dialect",
                "postgres",
            ],
            vec![
                "check",
                "main.nagi",
                "--sql-schema",
                "schema.sql",
                "--sql-schema",
                "schema.sql",
                "--sql-dialect",
                "sqlite",
            ],
            vec![
                "check",
                "main.nagi",
                "--sql-schema",
                "schema.sql",
                "--sql-dialect",
                "sqlite",
                "--editor-input",
            ],
        ] {
            let diagnostic = failure(fixture.run(&args));
            assert!(
                diagnostic.contains("sql") || diagnostic.contains("SQL"),
                "{diagnostic}"
            );
        }
        for command in ["build", "run", "lower", "map", "symbols"] {
            let diagnostic = failure(fixture.run(&[
                command,
                "main.nagi",
                "--sql-schema",
                "schema.sql",
                "--sql-dialect",
                "sqlite",
            ]));
            assert!(
                diagnostic.contains("sql") || diagnostic.contains("SQL"),
                "{diagnostic}"
            );
            assert!(!diagnostic.contains("Cargo"), "{diagnostic}");
        }
    }

    #[test]
    fn explicit_project_schema_path_is_relative_to_invocation_directory() {
        let fixture = Fixture::new();
        fixture.write("app/nagi.toml", "entry = 'src/main.nagi'\n");
        fixture.write(
            "app/src/main.nagi",
            &row_program("    rows = try await sqlite.all[User](db, sqlite.literal(\"SELECT id, name FROM users\"), sqlite.parameters())"),
        );
        fixture.write("app/schema.sql", "CREATE TABLE users(wrong INTEGER);\n");
        let report = success(fixture.run(&[
            "check",
            "--project",
            "app",
            "--sql-schema",
            "schema.sql",
            "--sql-dialect",
            "sqlite",
        ]));
        assert!(report.contains("SQL checked 1"), "{report}");
        let report = success(fixture.run_from(
            &fixture.0.join("app/src"),
            &[
                "check",
                "--sql-schema",
                "../../schema.sql",
                "--sql-dialect",
                "sqlite",
            ],
        ));
        assert!(report.contains("SQL checked 1"), "{report}");
    }

    #[test]
    fn unsafe_schema_operations_cannot_create_or_modify_external_files() {
        let fixture = Fixture::new();
        let untouched = fixture.0.join("outside.db");
        let forbidden = fixture.0.join("created-by-schema.db");
        let bytes = b"external file must remain unchanged";
        fs::write(&untouched, bytes).unwrap();
        let quote = |path: &Path| path.to_string_lossy().replace('\'', "''");
        for schema in [
            format!("ATTACH DATABASE '{}' AS external;", quote(&untouched)),
            format!("ATTACH DATABASE '{}' AS external;", quote(&forbidden)),
            "DETACH DATABASE main;".into(),
            "PRAGMA journal_mode=WAL;".into(),
            "PRAGMA user_version=7;".into(),
            format!("VACUUM INTO '{}';", quote(&forbidden)),
            format!("SELECT load_extension('{}');", quote(&untouched)),
            "CREATE TEMP TABLE secrets(id INTEGER);".into(),
            "CREATE VIRTUAL TABLE search USING fts5(value);".into(),
            "CREATE TABLE users(id INTEGER); INSERT INTO users VALUES (1);".into(),
            "CREATE TABLE users(id INTEGER); CREATE TRIGGER changed AFTER INSERT ON users BEGIN DELETE FROM users; END;".into(),
        ] {
            fixture.write("schema.sql", &schema);
            let diagnostic = failure(fixture.sql_check("main.nagi"));
            assert!(diagnostic.contains("schema.sql"), "{schema}: {diagnostic}");
            assert_eq!(fs::read(&untouched).unwrap(), bytes, "{schema}");
            assert!(!forbidden.exists(), "{schema}");
            assert!(!fixture.0.join("outside.db-wal").exists(), "{schema}");
            assert!(!fixture.0.join("outside.db-journal").exists(), "{schema}");
        }
    }

    #[test]
    fn literal_queries_cannot_change_schema_or_access_external_functions() {
        for sql in [
            "ATTACH DATABASE 'outside.db' AS external",
            "PRAGMA user_version",
            "CREATE TABLE unexpected(value INTEGER)",
            "SELECT load_extension('outside.db') AS id, 'alice' AS name",
            "SELECT readfile('outside.db') AS id, 'alice' AS name",
        ] {
            let fixture = Fixture::new();
            let high = row_program(&format!("    rows = try await sqlite.all[User](db, sqlite.literal(\"{sql}\"), sqlite.parameters())"));
            write_high_and_independent_low(&fixture, &high);
            for name in ["main.nagi", "saved.low"] {
                failure(fixture.sql_check(name));
            }
            assert!(!fixture.0.join("outside.db").exists());
        }
    }

    #[test]
    fn bounded_schema_and_query_inputs_fail_before_sql_preparation() {
        let fixture = Fixture::new();
        fixture.write("schema.sql", &" ".repeat(2 * 1024 * 1024 + 1));
        let diagnostic = failure(fixture.sql_check("main.nagi"));
        assert!(diagnostic.contains("schema.sql"), "{diagnostic}");
        fixture.write("schema.sql", SCHEMA);
        let sql = format!("SELECT id, name FROM users /*{}*/", "x".repeat(256 * 1024));
        let high = row_program(&format!("    rows = try await sqlite.all[User](db, sqlite.literal(\"{sql}\"), sqlite.parameters())"));
        write_high_and_independent_low(&fixture, &high);
        let diagnostic = failure(fixture.sql_check("main.nagi"));
        diagnostic_on(&diagnostic, "main.nagi", query_line(&high, "SELECT"));
    }
}
