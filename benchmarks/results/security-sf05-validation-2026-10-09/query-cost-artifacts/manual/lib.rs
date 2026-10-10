use nagi_runtime::{sqlite as db};
pub async fn literal(tx: &db::Tx, value: i64) -> Result<i64, db::Failure> {
    let params = db::bind_i64(db::parameters(), value);
    db::exec(tx, db::literal("INSERT INTO data VALUES (?)"), params).await
}
pub async fn selected(tx: &db::Tx, sql: db::Query, value: i64) -> Result<i64, db::Failure> {
    let params = db::bind_i64(db::parameters(), value);
    db::exec(tx, sql, params).await
}
