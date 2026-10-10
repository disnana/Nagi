#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_5374617465 {
    pub pool: ::nagi_runtime::sqlite::Pool,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_5374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("State").field("pool", &self.pool).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_526f77 {
    pub id: ::std::primitive::i64,
    pub label: ::std::option::Option<::std::string::String>,
    pub data: ::std::vec::Vec<::std::primitive::u8>,
    pub enabled: ::std::primitive::bool,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_526f77 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Row").field("id", &self.id).field("label", &self.label).field("data", &self.data).field("enabled", &self.enabled).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_526f77 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id","label","data","enabled"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
label: row.get(ix[1])?,
data: row.get(ix[2])?,
enabled: row.get(ix[3])?,
}) }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_f_7075626c697368(mut pool: ::nagi_runtime::sqlite::Pool) -> ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_5374617465> {
    return ::std::sync::Arc::new(__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_5374617465 { pool: ::nagi_runtime::sqlite::clone_pool(&(pool)) });
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_f_72656164(mut tx: ::nagi_runtime::sqlite::Tx, mut sql: ::nagi_runtime::sqlite::Query, mut params: ::nagi_runtime::sqlite::Parameters) -> ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> {
    let mut row: ::std::option::Option<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_526f77> = ((::nagi_runtime::sqlite::query::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_526f77>(&(tx), ::nagi_runtime::sqlite::literal("SELECT id, label, data, enabled FROM items WHERE id = ?"), params)).await)?;
    let mut rows: ::std::vec::Vec<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_526f77> = ((::nagi_runtime::sqlite::all::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_526f77>(&(tx), sql, ::nagi_runtime::sqlite::parameters())).await)?;
    let mut count: ::std::primitive::i64 = ((::nagi_runtime::sqlite::exec(&(tx), ::nagi_runtime::sqlite::literal("DELETE FROM items"), ::nagi_runtime::sqlite::parameters())).await)?;
    return (::nagi_runtime::sqlite::rollback(tx)).await;
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_5374617465 as State;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_c_526f77 as Row;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_f_7075626c697368 as publish;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3738333133392d352f6d61696e2e6e616769_f_72656164 as read;
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



#[test] fn invoked_contract() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        use nagi_runtime::sqlite as sqlite;
        let pool = sqlite::open(":memory:", sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let published = publish(sqlite::clone_pool(&pool));
        let tx = sqlite::begin(&pool, sqlite::BeginMode::Deferred).await.unwrap();
        sqlite::exec(&tx, sqlite::literal("CREATE TABLE items(id INTEGER, label TEXT, data BLOB, enabled INTEGER)"), sqlite::parameters()).await.unwrap();
        sqlite::exec(&tx, sqlite::literal("INSERT INTO items VALUES (1,NULL,x'0102',1)"), sqlite::parameters()).await.unwrap();
        read(tx, sqlite::literal("SELECT id,label,data,enabled FROM items"), sqlite::bind_i64(sqlite::parameters(),1)).await.unwrap();
        drop(published);
        sqlite::close(&pool,1000).await.unwrap();
    });
}
