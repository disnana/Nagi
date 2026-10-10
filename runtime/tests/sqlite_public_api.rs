//! Uses only the published Rust boundary; cfg(test) session seams are unavailable.
use nagi_runtime::{sqlite as db, ErrorKind, FromRow};
use rusqlite::Row;

#[derive(Debug, PartialEq)]
struct Number {
    n: i64,
}
impl FromRow for Number {
    fn columns() -> &'static [&'static str] {
        &["n"]
    }
    fn read(row: &Row<'_>, indices: &[usize]) -> rusqlite::Result<Self> {
        Ok(Self {
            n: row.get(indices[0])?,
        })
    }
}
fn failed<T>(result: Result<T, db::Failure>) -> db::Failure {
    match result {
        Err(problem) => problem,
        Ok(_) => panic!("expected a business failure"),
    }
}
#[tokio::test]
async fn published_sqlite_pool_api_builds_without_test_seams() {
    fn shared<T: Send + Sync>() {}
    shared::<db::Pool>();
    let config = db::options(1, 2, 0, 0).unwrap();
    let pool = db::open(":memory:", config).await.unwrap();
    let clone = db::clone_pool(&pool);
    let tx = db::begin(&pool, db::BeginMode::Deferred).await.unwrap();
    db::exec(&tx, db::literal("CREATE TABLE data(n)"), db::parameters())
        .await
        .unwrap();
    db::exec(
        &tx,
        db::literal("INSERT INTO data VALUES (?)"),
        db::bind_i64(db::parameters(), 7),
    )
    .await
    .unwrap();
    assert_eq!(
        db::query::<Number>(&tx, db::literal("SELECT n FROM data"), db::parameters())
            .await
            .unwrap(),
        Some(Number { n: 7 })
    );
    assert_eq!(
        failed(db::begin(&clone, db::BeginMode::Deferred).await).kind,
        db::FailureKind::AcquireTimeout
    );
    let problem = failed(
        db::exec(
            &tx,
            db::literal("PRAGMA journal_mode=WAL"),
            db::parameters(),
        )
        .await,
    );
    assert_eq!(problem.kind, db::FailureKind::Sql);
    assert_eq!(problem.outcome, db::Outcome::Active);
    assert!(matches!(
        db::copy_primary_error(&problem).unwrap().kind,
        ErrorKind::Database
    ));
    assert!(db::copy_cleanup_error(&problem).is_none());
    db::commit(tx).await.unwrap();
    let tx = db::begin(&clone, db::BeginMode::Immediate).await.unwrap();
    assert_eq!(
        db::all::<Number>(&tx, db::literal("SELECT n FROM data"), db::parameters())
            .await
            .unwrap(),
        vec![Number { n: 7 }]
    );
    db::rollback(tx).await.unwrap();
    db::close(&pool, 2_000).await.unwrap();
    assert_eq!(
        failed(db::begin(&clone, db::BeginMode::Deferred).await).kind,
        db::FailureKind::Closed
    );
    db::close(&clone, 0).await.unwrap();
}
