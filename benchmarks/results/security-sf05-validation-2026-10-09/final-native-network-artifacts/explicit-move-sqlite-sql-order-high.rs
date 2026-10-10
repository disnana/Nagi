#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_c_526f77 {
    pub id: ::std::primitive::i64,
    pub tiny: ::std::primitive::u8,
    pub label: ::std::option::Option<::std::string::String>,
    pub data: ::std::vec::Vec<::std::primitive::u8>,
    pub enabled: ::std::primitive::bool,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_c_526f77 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Row").field("id", &self.id).field("tiny", &self.tiny).field("label", &self.label).field("data", &self.data).field("enabled", &self.enabled).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_c_526f77 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["id","tiny","label","data","enabled"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
id: row.get(ix[0])?,
tiny: row.get(ix[1])?,
label: row.get(ix[2])?,
data: row.get(ix[3])?,
enabled: row.get(ix[4])?,
}) }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_73716c() -> ::nagi_runtime::sqlite::Query {
    native::sql()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_706172616d73() -> ::nagi_runtime::sqlite::Parameters {
    native::params()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_66616c6c69626c655f73716c() -> ::std::result::Result<::nagi_runtime::sqlite::Query, ::nagi_runtime::Error> {
    native::fallible_sql()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_70616e69635f73716c() -> ::nagi_runtime::sqlite::Query {
    native::panic_sql()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_6f726465726564<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    return (::nagi_runtime::sqlite::exec(tx, crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_73716c(), crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_706172616d73())).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_6661696c6564<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    let mut result: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::exec(tx, (crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_66616c6c69626c655f73716c())?, crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_706172616d73())).await;
    return ::std::result::Result::Ok(println!("{}", 0i64));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_70616e69636b6564<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    return (::nagi_runtime::sqlite::exec(tx, crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_70616e69635f73716c(), crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_706172616d73())).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_6465636f646564<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx) -> ::std::result::Result<::std::vec::Vec<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_c_526f77>, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    return (::nagi_runtime::sqlite::all::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_c_526f77>(tx, ::nagi_runtime::sqlite::literal("SELECT 7 AS id, 255 AS tiny, NULL AS label, x'0102' AS data, 1 AS enabled"), ::nagi_runtime::sqlite::parameters())).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_696e76616c69645f6465636f6465<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx) -> ::std::result::Result<::std::vec::Vec<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_c_526f77>, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    return (::nagi_runtime::sqlite::all::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_c_526f77>(tx, ::nagi_runtime::sqlite::literal("SELECT 7 AS id, 256 AS tiny, NULL AS label, x'0102' AS data, 1 AS enabled"), ::nagi_runtime::sqlite::parameters())).await;
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_c_526f77 as Row;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_73716c as sql;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_706172616d73 as params;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_66616c6c69626c655f73716c as fallible_sql;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_70616e69635f73716c as panic_sql;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_6f726465726564 as ordered;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_6661696c6564 as failed;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_70616e69636b6564 as panicked;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_6465636f646564 as decoded;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d352f6d61696e2e6e616769_f_696e76616c69645f6465636f6465 as invalid_decode;
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


mod native {
    pub static EVENTS: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    pub fn sql() -> nagi_runtime::sqlite::Query { EVENTS.lock().unwrap().push("sql"); nagi_runtime::sqlite::literal("DELETE FROM items") }
    pub fn params() -> nagi_runtime::sqlite::Parameters { EVENTS.lock().unwrap().push("params"); nagi_runtime::sqlite::parameters() }
    pub fn fallible_sql() -> Result<nagi_runtime::sqlite::Query,nagi_runtime::Error> {
        EVENTS.lock().unwrap().push("error_sql");
        Err(nagi_runtime::Error { kind: nagi_runtime::ErrorKind::Invalid, message: "expected error".into() })
    }
    pub fn panic_sql() -> nagi_runtime::sqlite::Query { EVENTS.lock().unwrap().push("panic_sql"); panic!("expected SQL panic") }
}


#[test] fn evaluation_and_decode() {
    use nagi_runtime::sqlite as sqlite;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (pool, tx) = rt.block_on(async {
        let pool = sqlite::open(":memory:",sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        let tx = sqlite::begin(&pool,sqlite::BeginMode::Deferred).await.unwrap();
        sqlite::exec(&tx,sqlite::literal("CREATE TABLE items(id INTEGER)"),sqlite::parameters()).await.unwrap();
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
