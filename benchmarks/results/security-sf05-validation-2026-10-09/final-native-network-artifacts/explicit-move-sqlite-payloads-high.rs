#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_c_5374617465 {
    pub pool: ::nagi_runtime::sqlite::Pool,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_c_5374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("State").field("pool", &self.pool).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_c_4661696c7572655374617465 {
    pub failure: ::nagi_runtime::sqlite::Failure,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_c_4661696c7572655374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("FailureState").field("failure", &self.failure).finish()
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6c6f63616c5f6c697374(mut tx: ::nagi_runtime::sqlite::Tx) -> ::std::vec::Vec<::nagi_runtime::sqlite::Tx> {
    return vec![tx];
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6c6f63616c5f6d6170(mut values: ::std::collections::HashMap<::std::string::String, ::nagi_runtime::sqlite::Tx>) -> ::std::collections::HashMap<::std::string::String, ::nagi_runtime::sqlite::Tx> {
    return values;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6475706c6963617465<'a>(mut values: &'a [::std::option::Option<::std::sync::Arc<::nagi_runtime::sqlite::Failure>>]) -> ::std::vec::Vec<::std::option::Option<::std::sync::Arc<::nagi_runtime::sqlite::Failure>>> {
    let mut values: &[::std::option::Option<::std::sync::Arc<::nagi_runtime::sqlite::Failure>>] = values;
    return (values).to_vec();
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6d657373616765(mut problem: ::nagi_runtime::sqlite::Failure) -> ::std::string::String {
    return ((problem).message.as_str()).to_owned();
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_626567696e5f736861726564(mut state: ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_c_5374617465>) -> ::std::result::Result<::nagi_runtime::sqlite::Tx, ::nagi_runtime::sqlite::Failure> {
    return (::nagi_runtime::sqlite::begin(&((state).pool), ::nagi_runtime::sqlite::BeginMode::Deferred)).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_696e73706563745f736861726564(mut state: ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_c_4661696c7572655374617465>) -> ::std::string::String {
    return (((state).failure).message.as_str()).to_owned();
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_636c617373696679<'a>(mut problem: &'a ::nagi_runtime::sqlite::Failure) -> ::nagi_runtime::sqlite::FailureKind {
    let mut problem: &::nagi_runtime::sqlite::Failure = problem;
    return (problem).kind;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6f7574636f6d65<'a>(mut problem: &'a ::nagi_runtime::sqlite::Failure) -> ::nagi_runtime::sqlite::Outcome {
    let mut problem: &::nagi_runtime::sqlite::Failure = problem;
    return (problem).outcome;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_72657469726564<'a>(mut problem: &'a ::nagi_runtime::sqlite::Failure) -> ::std::primitive::bool {
    let mut problem: &::nagi_runtime::sqlite::Failure = problem;
    return (problem).retired;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6368696c64(mut value: ()) -> () {
    println!("{}", 0i64);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_636f6d706c65746564(mut tx: ::nagi_runtime::sqlite::Tx) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut result: ::std::result::Result<(), ::nagi_runtime::sqlite::Failure> = (::nagi_runtime::sqlite::commit(tx)).await;
    match result {
        ::std::result::Result::Ok(mut done) => {
            return ::std::result::Result::Ok(done);
        },
        ::std::result::Result::Err(mut problem) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("commit failed")));
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6c61756e63685f636f6d706c65746564(mut tx: ::nagi_runtime::sqlite::Tx) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    {
        let mut __scope = ::nagi_runtime::Scope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6368696c64(match ((crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_636f6d706c65746564(tx)).await) { ::std::result::Result::Ok(__nagi_try_value) => __nagi_try_value, ::std::result::Result::Err(__nagi_try_error) => break '__nagi_scope_body_1 ::std::result::Result::Err(::std::convert::From::from(__nagi_try_error)) }); __scope.spawn(async move { __nagi_spawn_future.await; ::std::result::Result::Ok(()) }); }
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { __scope.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__scope.join().await)?;
    }
    return ::std::result::Result::Ok(println!("{}", 0i64));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_c_5374617465 as State;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_c_4661696c7572655374617465 as FailureState;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6c6f63616c5f6c697374 as local_list;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6c6f63616c5f6d6170 as local_map;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6475706c6963617465 as duplicate;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6d657373616765 as message;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_626567696e5f736861726564 as begin_shared;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_696e73706563745f736861726564 as inspect_shared;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_636c617373696679 as classify;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6f7574636f6d65 as outcome;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_72657469726564 as retired;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6368696c64 as child;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_636f6d706c65746564 as completed;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930363033312d302f6d61696e2e6e616769_f_6c61756e63685f636f6d706c65746564 as launch_completed;
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
