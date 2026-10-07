use nagi_runtime::{sqlite as db, Sql};
pub async fn literal(tx: &db::Tx, value: i64) -> Result<i64, db::Failure> {
    let params = db::bind_i64(db::parameters(), value);
    db::exec(tx, Sql::Static("INSERT INTO data VALUES (?)"), params).await
}
pub async fn dynamic(tx: &db::Tx, sql: &str, value: i64) -> Result<i64, db::Failure> {
    let params = db::bind_i64(db::parameters(), value);
    db::exec(tx, Sql::Owned(sql.to_owned()), params).await
}
