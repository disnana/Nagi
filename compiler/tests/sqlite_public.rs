#[path = "support/native_triple.rs"]
mod native_triple;
use native_triple::Fixture;

#[test]
fn public_sqlite_high_saved_low_and_handwritten_low_execute_real_transactions() {
    Fixture::new().run_three("sqlite-public", HIGH, LOW, "", ASSERTIONS);
}

const HIGH: &str = r#"import std.db.sqlite as sqlite
class Total:
    total: i64
def configuration() -> Result[sqlite.Options, Error]:
    return sqlite.options(1, 2, 1000, 0)
async def exercise(config: sqlite.Options) -> Result[i64, sqlite.Failure]:
    pool = try await sqlite.open(":memory:", config)
    alias = sqlite.clone_pool(pool)
    tx = try await sqlite.begin(pool, sqlite.BeginMode.DEFERRED)
    try await sqlite.exec(tx, "CREATE TABLE amounts(amount INTEGER NOT NULL)", sqlite.parameters())
    params = sqlite.bind_i64(sqlite.parameters(), 7)
    try await sqlite.exec(tx, "INSERT INTO amounts VALUES (?)", params)
    try await sqlite.commit(tx)
    tx = try await sqlite.begin(alias, sqlite.BeginMode.DEFERRED)
    sql = "SELECT SUM(amount) AS total FROM amounts"
    row = try await sqlite.query[Total](tx, view(sql), sqlite.parameters())
    try await sqlite.rollback(tx)
    try await sqlite.close(alias, 1000)
    match row:
        case Some(value):
            return ok(value.total)
        case None:
            return ok(0)
"#;
const LOW: &str = r#"import std.db.sqlite as sqlite;
record Total { total: i64; }
fn configuration() -> Result[sqlite.Options, Error] {
    return sqlite.options(1, 2, 1000, 0);
}
async fn exercise(config: sqlite.Options) -> Result[i64, sqlite.Failure] {
    let pool = try await sqlite.open(":memory:", config);
    let alias = sqlite.clone_pool(pool);
    let tx = try await sqlite.begin(pool, sqlite.BeginMode.DEFERRED);
    try await sqlite.exec(tx, "CREATE TABLE amounts(amount INTEGER NOT NULL)", sqlite.parameters());
    let params = sqlite.bind_i64(sqlite.parameters(), 7);
    try await sqlite.exec(tx, "INSERT INTO amounts VALUES (?)", params);
    try await sqlite.commit(tx);
    tx = try await sqlite.begin(alias, sqlite.BeginMode.DEFERRED);
    let sql = "SELECT SUM(amount) AS total FROM amounts";
    let row = try await sqlite.query[Total](tx, view(sql), sqlite.parameters());
    try await sqlite.rollback(tx);
    try await sqlite.close(alias, 1000);
    match row {
        case Some(value) { return ok(value.total); }
        case None { return ok(0); }
    }
}
"#;
const ASSERTIONS: &str = r#"
#[test]
fn real_public_sqlite() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let options = configuration().unwrap();
    assert_eq!(rt.block_on(exercise(options)).unwrap(), 7);
}
"#;

#[test]
fn matrix_positive_pairs_execute_high_saved_and_handwritten_low() {
    const CASES: &[(&str, &str)] = &[
        (
            "01-same-task-affine",
            r#"
#[test] fn invoked_contract() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = nagi_runtime::sqlite::open(":memory:", nagi_runtime::sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let tx = nagi_runtime::sqlite::begin(&pool, nagi_runtime::sqlite::BeginMode::Deferred).await.unwrap();
        local(tx).await.unwrap();
        let tx = nagi_runtime::sqlite::begin(&pool, nagi_runtime::sqlite::BeginMode::Deferred).await.unwrap();
        finish(tx).await.unwrap();
        nagi_runtime::sqlite::close(&pool, 1000).await.unwrap();
    });
}
"#,
        ),
        (
            "02-pool-row-sql",
            r#"
#[test] fn invoked_contract() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use nagi_runtime::sqlite as sqlite;
        let pool = sqlite::open(":memory:", sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let published = publish(sqlite::clone_pool(&pool));
        let tx = sqlite::begin(&pool, sqlite::BeginMode::Deferred).await.unwrap();
        sqlite::exec(&tx, nagi_runtime::Sql::Static("CREATE TABLE items(id INTEGER, label TEXT, data BLOB, enabled INTEGER)"), sqlite::parameters()).await.unwrap();
        sqlite::exec(&tx, nagi_runtime::Sql::Static("INSERT INTO items VALUES (1,NULL,x'0102',1)"), sqlite::parameters()).await.unwrap();
        read(tx, "SELECT id,label,data,enabled FROM items".into(), sqlite::bind_i64(sqlite::parameters(),1)).await.unwrap();
        drop(published);
        sqlite::close(&pool,1000).await.unwrap();
    });
}
"#,
        ),
        (
            "03-user-type-names",
            r#"
#[test] fn invoked_contract() {
    let shared = publish(Tx { id: 7 });
    assert_eq!(shared.id,7);
    let holder = Holder { pool: Pool { id:1 }, tx: Tx { id:2 }, options: Options { id:3 }, failure: Failure { id:4 } };
    assert_eq!(holder.pool.id+holder.tx.id+holder.options.id+holder.failure.id,10);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = nagi_runtime::sqlite::open(":memory:",nagi_runtime::sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let alias = native_identity(nagi_runtime::sqlite::clone_pool(&pool));
        nagi_runtime::sqlite::close(&alias,1000).await.unwrap();
    });
}
"#,
        ),
        (
            "04-pointer-signature",
            r#"
#[test] fn invoked_contract() {
    tokio::runtime::Runtime::new().unwrap().block_on(launch()).unwrap();
}
"#,
        ),
        (
            "16-parameters-field",
            r#"
#[test] fn invoked_contract() {
    let holder = hold(nagi_runtime::sqlite::bind_i64(nagi_runtime::sqlite::parameters(),3));
    let envelope = wrap(holder.params);
    match envelope { Envelope::Data { params } => { drop(params); }, _ => panic!("wrong variant") }
}
"#,
        ),
        (
            "20-copy-pointer-signature",
            r#"
fn callback(_tx: nagi_runtime::sqlite::Tx) {}
#[test] fn invoked_contract() {
    let callbacks: Vec<fn(nagi_runtime::sqlite::Tx)> = vec![callback];
    let duplicated = duplicate(&callbacks);
    assert_eq!(duplicated.len(),1);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = nagi_runtime::sqlite::open(":memory:",nagi_runtime::sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let tx = nagi_runtime::sqlite::begin(&pool,nagi_runtime::sqlite::BeginMode::Deferred).await.unwrap();
        duplicated[0](tx);
        nagi_runtime::sqlite::close(&pool,1000).await.unwrap();
    });
}
"#,
        ),
        (
            "21-failure-shared",
            r#"
fn problem() -> nagi_runtime::sqlite::Failure {
    let mut failure = tokio::runtime::Runtime::new().unwrap().block_on(nagi_runtime::sqlite::open("",nagi_runtime::sqlite::options(1,2,1000,0).unwrap())).unwrap_err();
    failure.message = "PRIVATE_FAILURE_MESSAGE".into();
    failure
}
#[test] fn invoked_contract() {
    let holder = publish(problem());
    let debug = format!("{:?}",holder);
    assert!(debug.contains("Invalid"));
    assert!(!debug.contains("PRIVATE_FAILURE_MESSAGE"));
    let direct = publish_direct(problem());
    assert_eq!(direct.message,"PRIVATE_FAILURE_MESSAGE");
    let optional = publish_option(problem());
    assert!(optional.is_some());
    let result = publish_result(problem());
    assert!(result.is_err());
}
"#,
        ),
    ];
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sqlite-contract");
    for (case, assertions) in CASES {
        let high = std::fs::read_to_string(root.join(format!("{case}.nagi"))).unwrap();
        let low = std::fs::read_to_string(root.join(format!("{case}.low"))).unwrap();
        Fixture::new().run_three(case, &high, &low, "", assertions);
    }
}

#[test]
fn matrix_checker_rejections_keep_stage_raw_cause_and_original_line() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sqlite-contract");
    let matrix: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/sqlite-contract/matrix.json")).unwrap();
    for case in matrix["cases"].as_array().unwrap() {
        for (form, high) in [("high", true), ("user_low", false)] {
            let path = root
                .join(case["inputs"][form].as_str().unwrap())
                .canonicalize()
                .unwrap();
            let mut loaded = nagic::source::load(&path, high).expect("fixture must reach checker");
            let outcome = nagic::check::check(&mut loaded.program);
            if case["expected_after_public_vertical_slice"] == "pass" {
                outcome.unwrap();
            } else {
                let error = outcome.expect_err("negative must reject in checker");
                let line: usize = error
                    .strip_prefix("line ")
                    .unwrap()
                    .split(':')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                let location = loaded.location(line).unwrap();
                assert_eq!(location.path, path);
                assert_eq!(
                    location.line as u64,
                    case["expected_origin"][form]["line"].as_u64().unwrap(),
                    "{error}"
                );
                assert!(
                    error.contains(case["checker_diagnostic_fragment"].as_str().unwrap()),
                    "{error}"
                );
            }
        }
    }
}

#[test]
fn local_containers_shared_copy_and_failure_accessor_execute() {
    Fixture::new().run_three("sqlite-payloads", r#"import std.db.sqlite as sqlite
def local_list(tx: sqlite.Tx) -> List[sqlite.Tx]:
    return [tx]
def local_map(values: Map[str, sqlite.Tx]) -> Map[str, sqlite.Tx]:
    return values
def duplicate(values: view[Option[shared[sqlite.Failure]]]) -> List[Option[shared[sqlite.Failure]]]:
    return copy(values)
def message(problem: sqlite.Failure) -> str:
    return copy(problem.message)
class State:
    pool: sqlite.Pool
class FailureState:
    failure: sqlite.Failure
async def begin_shared(state: shared[State]) -> Result[sqlite.Tx, sqlite.Failure]:
    return await sqlite.begin(state.pool, sqlite.BeginMode.DEFERRED)
def inspect_shared(state: shared[FailureState]) -> str:
    return copy(state.failure.message)
def classify(problem: view[sqlite.Failure]) -> sqlite.FailureKind:
    return problem.kind
def outcome(problem: view[sqlite.Failure]) -> sqlite.Outcome:
    return problem.outcome
def retired(problem: view[sqlite.Failure]) -> bool:
    return problem.retired
async def child(value: unit):
    print(0)
async def completed(tx: sqlite.Tx) -> Result[unit, Error]:
    result = await sqlite.commit(tx)
    match result:
        case Ok(done):
            return ok(done)
        case Err(problem):
            return error("commit failed")
async def launch_completed(tx: sqlite.Tx) -> Result[unit, Error]:
    async with scope:
        spawn child(try await completed(tx))
    return ok(print(0))
"#, r#"import std.db.sqlite as sqlite;
fn local_list(tx: sqlite.Tx) -> List[sqlite.Tx] { return [tx]; }
fn local_map(values: Map[str, sqlite.Tx]) -> Map[str, sqlite.Tx] { return values; }
fn duplicate(values: view[Option[shared[sqlite.Failure]]]) -> List[Option[shared[sqlite.Failure]]] { return copy(values); }
fn message(problem: sqlite.Failure) -> str { return copy(problem.message); }
record State { pool: sqlite.Pool; }
record FailureState { failure: sqlite.Failure; }
async fn begin_shared(state: shared[State]) -> Result[sqlite.Tx, sqlite.Failure] { return await sqlite.begin(state.pool, sqlite.BeginMode.DEFERRED); }
fn inspect_shared(state: shared[FailureState]) -> str { return copy(state.failure.message); }
fn classify(problem: view[sqlite.Failure]) -> sqlite.FailureKind { return problem.kind; }
fn outcome(problem: view[sqlite.Failure]) -> sqlite.Outcome { return problem.outcome; }
fn retired(problem: view[sqlite.Failure]) -> bool { return problem.retired; }
async fn child(value: unit) { print(0); }
async fn completed(tx: sqlite.Tx) -> Result[unit, Error] {
    let result = await sqlite.commit(tx);
    match result {
        case Ok(done) { return ok(done); }
        case Err(problem) { return error("commit failed"); }
    }
}
async fn launch_completed(tx: sqlite.Tx) -> Result[unit, Error] {
    scope { spawn child(try await completed(tx)); }
    return ok(print(0));
}
"#, "", r#"
#[test] fn invoked_payloads() {
    use nagi_runtime::sqlite as sqlite;
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = sqlite::open(":memory:",sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let tx = sqlite::begin(&pool,sqlite::BeginMode::Deferred).await.unwrap();
        let mut list = local_list(tx);
        assert_eq!(list.len(),1);
        sqlite::rollback(list.pop().unwrap()).await.unwrap();
        let tx = sqlite::begin(&pool,sqlite::BeginMode::Deferred).await.unwrap();
        let values = std::collections::HashMap::from([("tx".to_string(),tx)]);
        let mut returned = local_map(values);
        sqlite::commit(returned.remove("tx").unwrap()).await.unwrap();
        let problem = sqlite::open("",sqlite::options(1,2,1000,0).unwrap()).await.unwrap_err();
        let shared = std::sync::Arc::new(problem);
        let original = vec![Some(std::sync::Arc::clone(&shared))];
        let copied = duplicate(&original);
        assert!(std::sync::Arc::ptr_eq(original[0].as_ref().unwrap(),copied[0].as_ref().unwrap()));
        let problem = sqlite::open("",sqlite::options(1,2,1000,0).unwrap()).await.unwrap_err();
        assert_eq!(message(problem),"unsupported SQLite path/options combination");
        let problem = sqlite::open("",sqlite::options(1,2,1000,0).unwrap()).await.unwrap_err();
        assert_eq!(classify(&problem),sqlite::FailureKind::Invalid);
        assert_eq!(outcome(&problem),sqlite::Outcome::NotApplicable);
        assert!(!retired(&problem));
        let state = std::sync::Arc::new(FailureState { failure: problem });
        assert_eq!(inspect_shared(state),"unsupported SQLite path/options combination");
        let state = std::sync::Arc::new(State { pool: sqlite::clone_pool(&pool) });
        let tx = begin_shared(state).await.unwrap();
        launch_completed(tx).await.unwrap();

        sqlite::close(&pool,1000).await.unwrap();
    });
}
"#);
}

#[test]
fn sqlite_task_result_nested_actual_capture_and_actor_state_reject_in_checker() {
    let cases = [
        (
            r#"import std.db.sqlite as sqlite
async def acquire(pool: sqlite.Pool) -> Result[sqlite.Tx, sqlite.Failure]:
    return await sqlite.begin(pool, sqlite.BeginMode.DEFERRED)
async def invalid(pool: sqlite.Pool) -> Result[unit, Error]:
    async with scope:
        task = spawn acquire(pool)
        result = await task
    return ok(print(0))
"#,
            r#"import std.db.sqlite as sqlite;
async fn acquire(pool: sqlite.Pool) -> Result[sqlite.Tx, sqlite.Failure] { return await sqlite.begin(pool, sqlite.BeginMode.DEFERRED); }
async fn invalid(pool: sqlite.Pool) -> Result[unit, Error] {
    scope {
        let task = spawn acquire(pool);
        let result = await task;
    }
    return ok(print(0));
}
"#,
            6,
            5,
            "Taskの結果",
        ),
        (
            r#"import std.db.sqlite as sqlite
async def child(tx: Result[sqlite.Tx, sqlite.Failure]):
    print(0)
async def invalid(pool: sqlite.Pool) -> Result[unit, Error]:
    async with scope:
        spawn child(await sqlite.begin(pool, sqlite.BeginMode.DEFERRED))
    return ok(print(0))
"#,
            r#"import std.db.sqlite as sqlite;
async fn child(tx: Result[sqlite.Tx, sqlite.Failure]) { print(0); }
async fn invalid(pool: sqlite.Pool) -> Result[unit, Error] {
    scope {
        spawn child(await sqlite.begin(pool, sqlite.BeginMode.DEFERRED));
    }
    return ok(print(0));
}
"#,
            6,
            5,
            "実payloadを別task",
        ),
        (
            r#"import std.db.sqlite as sqlite
import std.actor as actor
def invalid(tx: sqlite.Tx) -> actor.Turn[sqlite.Tx, i64, Error]:
    return actor.turn[sqlite.Tx, i64, Error](tx, ok(1))
"#,
            r#"import std.db.sqlite as sqlite;
import std.actor as actor;
fn invalid(tx: sqlite.Tx) -> actor.Turn[sqlite.Tx, i64, Error] {
    return actor.turn[sqlite.Tx, i64, Error](tx, ok(1));
}
"#,
            3,
            3,
            "native stateの所有field",
        ),
        (
            r#"import std.db.sqlite as sqlite
from std.ownership import move
def invalid(problem: sqlite.Failure) -> sqlite.Failure:
    borrowed = problem.message
    moved = move(problem)
    print(borrowed)
    return moved
"#,
            r#"import std.db.sqlite as sqlite;
from std.ownership import move;
fn invalid(problem: sqlite.Failure) -> sqlite.Failure {
    let borrowed = problem.message;
    let moved = move(problem);
    print(borrowed);
    return moved;
}
"#,
            5,
            5,
            "借用",
        ),
    ];
    for (high, low, high_line, low_line, fragment) in cases {
        let fixture = Fixture::new();
        for (name, text, line) in [
            ("extra.nagi", high, high_line),
            ("extra.low", low, low_line),
        ] {
            fixture.write(name, text);
            let error = fixture
                .checked(name)
                .expect_err("must reject before emission");
            assert!(
                error.starts_with(&format!("line {line}:")),
                "{name}: {error}"
            );
            assert!(error.contains(fragment), "{name}: {error}");
        }
    }
}

#[test]
fn sqlite_sql_evaluation_order_and_row_decode_execute() {
    Fixture::new().run_three("sqlite-sql-order", r#"import std.db.sqlite as sqlite
class Row:
    id: i64
    tiny: u8
    label: Option[str]
    data: bytes
    enabled: bool
@rust("native::sql")
extern def sql() -> str
@rust("native::params")
extern def params() -> sqlite.Parameters
@rust("native::fallible_sql")
extern def fallible_sql() -> Result[str, Error]
@rust("native::panic_sql")
extern def panic_sql() -> str
async def ordered(tx: view[sqlite.Tx]) -> Result[i64, sqlite.Failure]:
    return await sqlite.exec(tx, sql(), params())
async def failed(tx: view[sqlite.Tx]) -> Result[unit, Error]:
    result = await sqlite.exec(tx, try fallible_sql(), params())
    return ok(print(0))
async def panicked(tx: view[sqlite.Tx]) -> Result[i64, sqlite.Failure]:
    return await sqlite.exec(tx, panic_sql(), params())
async def decoded(tx: view[sqlite.Tx]) -> Result[List[Row], sqlite.Failure]:
    return await sqlite.all[Row](tx, "SELECT 7 AS id, 255 AS tiny, NULL AS label, x'0102' AS data, 1 AS enabled", sqlite.parameters())
async def invalid_decode(tx: view[sqlite.Tx]) -> Result[List[Row], sqlite.Failure]:
    return await sqlite.all[Row](tx, "SELECT 7 AS id, 256 AS tiny, NULL AS label, x'0102' AS data, 1 AS enabled", sqlite.parameters())
"#, r#"import std.db.sqlite as sqlite;
record Row { id: i64; tiny: u8; label: Option[str]; data: bytes; enabled: bool; }
@rust("native::sql")
extern fn sql() -> str;
@rust("native::params")
extern fn params() -> sqlite.Parameters;
@rust("native::fallible_sql")
extern fn fallible_sql() -> Result[str, Error];
@rust("native::panic_sql")
extern fn panic_sql() -> str;
async fn ordered(tx: view[sqlite.Tx]) -> Result[i64, sqlite.Failure] { return await sqlite.exec(tx, sql(), params()); }
async fn failed(tx: view[sqlite.Tx]) -> Result[unit, Error] { let result = await sqlite.exec(tx, try fallible_sql(), params()); return ok(print(0)); }
async fn panicked(tx: view[sqlite.Tx]) -> Result[i64, sqlite.Failure] { return await sqlite.exec(tx, panic_sql(), params()); }
async fn decoded(tx: view[sqlite.Tx]) -> Result[List[Row], sqlite.Failure] { return await sqlite.all[Row](tx, "SELECT 7 AS id, 255 AS tiny, NULL AS label, x'0102' AS data, 1 AS enabled", sqlite.parameters()); }
async fn invalid_decode(tx: view[sqlite.Tx]) -> Result[List[Row], sqlite.Failure] { return await sqlite.all[Row](tx, "SELECT 7 AS id, 256 AS tiny, NULL AS label, x'0102' AS data, 1 AS enabled", sqlite.parameters()); }
"#, r#"
mod native {
    pub static EVENTS: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    pub fn sql() -> String { EVENTS.lock().unwrap().push("sql"); "DELETE FROM items".into() }
    pub fn params() -> nagi_runtime::sqlite::Parameters { EVENTS.lock().unwrap().push("params"); nagi_runtime::sqlite::parameters() }
    pub fn fallible_sql() -> Result<String,nagi_runtime::Error> {
        EVENTS.lock().unwrap().push("error_sql");
        Err(nagi_runtime::Error { kind: nagi_runtime::ErrorKind::Invalid, message: "expected error".into() })
    }
    pub fn panic_sql() -> String { EVENTS.lock().unwrap().push("panic_sql"); panic!("expected SQL panic") }
}
"#, r#"
#[test] fn evaluation_and_decode() {
    use nagi_runtime::sqlite as sqlite;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (pool, tx) = rt.block_on(async {
        let pool = sqlite::open(":memory:",sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let tx = sqlite::begin(&pool,sqlite::BeginMode::Deferred).await.unwrap();
        sqlite::exec(&tx,nagi_runtime::Sql::Static("CREATE TABLE items(id INTEGER)"),sqlite::parameters()).await.unwrap();
        assert_eq!(ordered(&tx).await.unwrap(),0);
        assert_eq!(*native::EVENTS.lock().unwrap(),["sql","params"]);
        native::EVENTS.lock().unwrap().clear();
        assert!(failed(&tx).await.is_err());
        assert_eq!(*native::EVENTS.lock().unwrap(),["error_sql"]);
        let rows = decoded(&tx).await.unwrap();
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0].id,7);
        assert_eq!(rows[0].tiny,255);
        assert!(rows[0].label.is_none());
        assert_eq!(rows[0].data,[1,2]);
        assert!(rows[0].enabled);
        let failure = invalid_decode(&tx).await.unwrap_err();
        assert_eq!(failure.kind,sqlite::FailureKind::Decode);
        assert_eq!(failure.outcome,sqlite::Outcome::Active);
        (pool, tx)
    });
    native::EVENTS.lock().unwrap().clear();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(||rt.block_on(panicked(&tx)))).is_err());
        assert_eq!(*native::EVENTS.lock().unwrap(),["panic_sql"]);
    rt.block_on(async {
        sqlite::rollback(tx).await.unwrap();
        sqlite::close(&pool,1000).await.unwrap();
    });
}
"#);
}

#[test]
fn materialized_sql_allows_later_parameter_move_of_the_same_string() {
    Fixture::new().run_three(
        "sqlite-sql-alias",
        r#"import std.db.sqlite as sqlite
class Row:
    value: str
async def insert(tx: view[sqlite.Tx], sql: str) -> Result[i64, sqlite.Failure]:
    return await sqlite.exec(tx, view(sql), sqlite.bind_text(sqlite.parameters(), sql))
async def one(tx: view[sqlite.Tx], sql: str) -> Result[Row?, sqlite.Failure]:
    return await sqlite.query[Row](tx, view(sql), sqlite.bind_text(sqlite.parameters(), sql))
async def many(tx: view[sqlite.Tx], sql: str) -> Result[List[Row], sqlite.Failure]:
    return await sqlite.all[Row](tx, view(sql), sqlite.bind_text(sqlite.parameters(), sql))
"#,
        r#"import std.db.sqlite as sqlite;
record Row { value: str; }
async fn insert(tx: view[sqlite.Tx], sql: str) -> Result[i64, sqlite.Failure] {
    return await sqlite.exec(tx, view(sql), sqlite.bind_text(sqlite.parameters(), sql));
}
async fn one(tx: view[sqlite.Tx], sql: str) -> Result[Row?, sqlite.Failure] {
    return await sqlite.query[Row](tx, view(sql), sqlite.bind_text(sqlite.parameters(), sql));
}
async fn many(tx: view[sqlite.Tx], sql: str) -> Result[List[Row], sqlite.Failure] {
    return await sqlite.all[Row](tx, view(sql), sqlite.bind_text(sqlite.parameters(), sql));
}
"#,
        "",
        r#"
#[test]
fn own_sql_before_parameter_move() {
    use nagi_runtime::{sqlite as db,Sql};
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool=db::open(":memory:",db::options(1,2,1000,0).unwrap()).await.unwrap();
        let tx=db::begin(&pool,db::BeginMode::Deferred).await.unwrap();
        db::exec(&tx,Sql::Static("CREATE TABLE data(value TEXT)"),db::parameters()).await.unwrap();
        let insert_sql="INSERT INTO data VALUES (?)";
        assert_eq!(insert(&tx,insert_sql.into()).await.unwrap(),1);
        let stored=one(&tx,"SELECT ? AS value".into()).await.unwrap().unwrap();
        assert_eq!(stored.value,"SELECT ? AS value");
        let rows=many(&tx,"SELECT ? AS value FROM data".into()).await.unwrap();
        assert_eq!(rows.len(),1);
        assert_eq!(rows[0].value,"SELECT ? AS value FROM data");
        db::rollback(tx).await.unwrap();
        db::close(&pool,1000).await.unwrap();
    });
}
"#,
    );
}

#[test]
fn sql_materialization_does_not_release_the_transaction_borrow() {
    Fixture::new().reject(
        r#"import std.db.sqlite as sqlite
def consume(tx: sqlite.Tx) -> sqlite.Parameters:
    return sqlite.parameters()
async def invalid(tx: sqlite.Tx) -> Result[i64, sqlite.Failure]:
    return await sqlite.exec(tx, "DELETE FROM data", consume(tx))
"#,
        r#"import std.db.sqlite as sqlite;
fn consume(tx: sqlite.Tx) -> sqlite.Parameters { return sqlite.parameters(); }
# The Tx loan still lasts through the async SQL operation.
async fn invalid(tx: sqlite.Tx) -> Result[i64, sqlite.Failure] {
    return await sqlite.exec(tx, "DELETE FROM data", consume(tx));
}
"#,
        5,
        "先に参照",
    );
}
