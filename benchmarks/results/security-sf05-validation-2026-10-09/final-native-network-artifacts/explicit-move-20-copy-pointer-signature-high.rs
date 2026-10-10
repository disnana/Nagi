#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31342f6d61696e2e6e616769_f_6475706c6963617465<'a>(mut callbacks: &'a [fn(::nagi_runtime::sqlite::Tx) -> ()]) -> ::std::vec::Vec<fn(::nagi_runtime::sqlite::Tx) -> ()> {
    let mut callbacks: &[fn(::nagi_runtime::sqlite::Tx) -> ()] = callbacks;
    return (callbacks).to_vec();
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d31342f6d61696e2e6e616769_f_6475706c6963617465 as duplicate;
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
