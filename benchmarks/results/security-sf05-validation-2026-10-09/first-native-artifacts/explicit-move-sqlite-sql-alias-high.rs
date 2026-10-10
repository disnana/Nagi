#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_c_526f77 {
    pub value: ::std::string::String,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_c_526f77 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Row").field("value", &self.value).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_c_526f77 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["value"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
value: row.get(ix[0])?,
}) }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_f_696e73657274<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx, mut sql: ::std::string::String) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    return (::nagi_runtime::sqlite::exec(tx, ::nagi_runtime::sqlite::literal("INSERT INTO data VALUES (?)"), ::nagi_runtime::sqlite::bind_text(::nagi_runtime::sqlite::parameters(), sql))).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_f_6f6e65<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx, mut sql: ::std::string::String) -> ::std::result::Result<::std::option::Option<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_c_526f77>, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    return (::nagi_runtime::sqlite::query::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_c_526f77>(tx, ::nagi_runtime::sqlite::literal("SELECT ? AS value"), ::nagi_runtime::sqlite::bind_text(::nagi_runtime::sqlite::parameters(), sql))).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_f_6d616e79<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx, mut sql: ::std::string::String) -> ::std::result::Result<::std::vec::Vec<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_c_526f77>, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    return (::nagi_runtime::sqlite::all::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_c_526f77>(tx, ::nagi_runtime::sqlite::literal("SELECT ? AS value FROM data"), ::nagi_runtime::sqlite::bind_text(::nagi_runtime::sqlite::parameters(), sql))).await;
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_c_526f77 as Row;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_f_696e73657274 as insert;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_f_6f6e65 as one;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d322f6d61696e2e6e616769_f_6d616e79 as many;
#[allow(non_snake_case)]
pub mod sqlite {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Pool as Pool;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Tx as Tx;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Query as Query;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Parameters as Parameters;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Options as Options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::BeginMode as BeginMode;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Failure as Failure;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::FailureKind as FailureKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::Outcome as Outcome;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::literal as literal;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::options as options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::open as open;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::clone_pool as clone_pool;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::begin as begin;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::parameters as parameters;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_i64 as bind_i64;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_f64 as bind_f64;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_text as bind_text;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_bytes as bind_bytes;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::bind_null as bind_null;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::query as query;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::all as all;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::exec as exec;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::commit as commit;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::rollback as rollback;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::close as close;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::copy_primary_error as copy_primary_error;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::sqlite::copy_cleanup_error as copy_cleanup_error;
}
fn main() {}



#[test]
fn own_sql_before_parameter_move() {
    use nagi_runtime::{sqlite as db};
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let pool=db::open(":memory:",db::options(1,2,1000,0).unwrap()).await.unwrap();
        let tx=db::begin(&pool,db::BeginMode::Deferred).await.unwrap();
        db::exec(&tx,db::literal("CREATE TABLE data(value TEXT)"),db::parameters()).await.unwrap();
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
