#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465 {
    pub pool: ::nagi_runtime::sqlite::Pool,
    pub subject: ::std::primitive::i64,
    pub target: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("State").field("pool", &self.pool).field("subject", &self.subject).field("target", &self.target).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(::nagi_runtime::serde::Serialize, ::nagi_runtime::serde::Deserialize, Clone, Copy)]
#[serde(crate = "::nagi_runtime::serde", deny_unknown_fields)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_56616c7565526f77 {
    pub value: ::std::primitive::i64,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_56616c7565526f77 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("ValueRow").field("value", &self.value).finish()
    }
}
impl ::nagi_runtime::FromRow for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_56616c7565526f77 {
 fn columns() -> &'static [&'static ::std::primitive::str] { &["value"] }
 fn read(row: &::nagi_runtime::rusqlite::Row<'_>, ix: &[::std::primitive::usize]) -> ::nagi_runtime::rusqlite::Result<Self> { ::std::result::Result::Ok(Self {
value: row.get(ix[0])?,
}) }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Clone, Copy)]
pub struct __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5175657279486f6c646572 {
    pub query: ::nagi_runtime::sqlite::Query,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5175657279486f6c646572 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("QueryHolder").field("query", &self.query).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug, Clone, Copy)]
pub enum __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_e_45646974 {
    Allowed,
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_766572696679(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465>) -> ::std::result::Result<::nagi_runtime::auth::VerifiedIdentity, ::nagi_runtime::auth::Failure> {
    native::verify(request, state).await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_617574686f72697a65(mut scope: ::nagi_runtime::auth::AuthScope, mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465>) -> ::std::result::Result<::nagi_runtime::auth::Grant<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_e_45646974>, ::nagi_runtime::auth::Failure> {
    native::authorize(scope, request, state).await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_70726f7465637465645f757064617465<'a>(mut pool: &'a ::nagi_runtime::sqlite::Pool, mut grant: ::nagi_runtime::auth::Grant<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_e_45646974>, mut value: ::std::primitive::i64) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    native::protected_update(pool, grant, value).await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_68616e646c6572(mut request: ::nagi_runtime::http_server::Request, mut state: ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465>, mut grant: ::nagi_runtime::auth::Grant<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_e_45646974>) -> ::std::result::Result<::nagi_runtime::http_server::Response, ::nagi_runtime::Error> {
    let mut changed: ::std::primitive::i64 = ((crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_70726f7465637465645f757064617465(&((state).pool), grant, 101i64)).await)?;
    return ::nagi_runtime::http_server::json::<::std::primitive::i64>(::nagi_runtime::http_server::Status::OK, &(changed));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_7365747570(mut pool: ::nagi_runtime::sqlite::Pool, mut subject: ::std::primitive::i64, mut target: ::std::primitive::i64) -> ::std::result::Result<::nagi_runtime::http_server::App<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465, ::nagi_runtime::Error>, ::nagi_runtime::Error> {
    let mut app: ::nagi_runtime::http_server::App<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465, ::nagi_runtime::Error> = ::nagi_runtime::http_server::app_default::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465>(__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465 { pool: pool, subject: subject, target: target });
    let mut policy: ::nagi_runtime::http_server::Policy<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465, ::nagi_runtime::auth::Grant<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_e_45646974>> = ::nagi_runtime::http_server::authorized_policy(crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_766572696679, crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_617574686f72697a65);
    return ::nagi_runtime::http_server::route(app, ::nagi_runtime::http_server::Method::POST, "/update", policy, crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_68616e646c6572);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_73656c65637465645f7175657279(mut id: ::std::primitive::i64) -> ::nagi_runtime::sqlite::Query {
    if (id == 9i64) {
        return ::nagi_runtime::sqlite::literal("SELECT value FROM protected WHERE id=?");
    }
    return ::nagi_runtime::sqlite::literal("SELECT value FROM protected WHERE id=? ORDER BY id");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_6d616b655f686f6c646572(mut query: ::nagi_runtime::sqlite::Query) -> ::std::sync::Arc<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5175657279486f6c646572> {
    return ::std::sync::Arc::new(__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5175657279486f6c646572 { query: query });
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_726561645f76616c7565<'a>(mut tx: &'a ::nagi_runtime::sqlite::Tx, mut id: ::std::primitive::i64) -> ::std::result::Result<::std::option::Option<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_56616c7565526f77>, ::nagi_runtime::sqlite::Failure> {
    let mut tx: &::nagi_runtime::sqlite::Tx = tx;
    let mut query: ::nagi_runtime::sqlite::Query = crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_73656c65637465645f7175657279(id);
    return (::nagi_runtime::sqlite::query::<__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_56616c7565526f77>(tx, query, ::nagi_runtime::sqlite::bind_i64(::nagi_runtime::sqlite::parameters(), id))).await;
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5374617465 as State;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_56616c7565526f77 as ValueRow;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_c_5175657279486f6c646572 as QueryHolder;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_e_45646974 as Edit;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_766572696679 as verify;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_617574686f72697a65 as authorize;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_70726f7465637465645f757064617465 as protected_update;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_68616e646c6572 as handler;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_7365747570 as setup;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_73656c65637465645f7175657279 as selected_query;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_6d616b655f686f6c646572 as make_holder;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3930353837382d302f68616e647772697474656e2e6c6f77_f_726561645f76616c7565 as read_value;
#[allow(non_snake_case)]
pub mod auth {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::AuthScope as AuthScope;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::VerifiedIdentity as VerifiedIdentity;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::Failure as Failure;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::FailureKind as FailureKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::Grant as Grant;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::subject as subject;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::kind as kind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::message as message;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::invalid_credential as invalid_credential;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::denied as denied;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::expired as expired;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::invalid_request as invalid_request;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::unavailable as unavailable;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::auth::internal as internal;
}
#[allow(non_snake_case)]
pub mod http {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Request as Request;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Response as Response;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Method as Method;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Status as Status;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Options as Options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::App as App;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::Policy as Policy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::status as status;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::method as method;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::method_name as method_name;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::empty as empty;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::text as text;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::bytes as bytes;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::json as json;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::append_header as append_header;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::append_header_text as append_header_text;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::header as header;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::header_text as header_text;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::headers as headers;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::is_json_content_type as is_json_content_type;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::default_options as default_options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::options as options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::capacity as capacity;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::header_timeout as header_timeout;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::header_limits as header_limits;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::send_timeout as send_timeout;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::public_policy as public_policy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::authenticated_policy as authenticated_policy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::authorized_policy as authorized_policy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::security_timeout as security_timeout;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::app as app;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::app_default as app_default;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::route as route;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::route_mapped as route_mapped;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::http_server::serve as serve;
}
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
    use nagi_runtime::{auth::{AuthScope,VerifiedIdentity,Grant,Failure},http_server as http,sqlite,Error};
    use std::{sync::Arc,time::{Duration,Instant}};
    pub async fn verify(request:http::Request,state:Arc<super::State>)->Result<VerifiedIdentity,Failure>{
        assert!(request.body().is_empty());
        if http::header_text(&request,"authorization").map_err(|_|Failure::invalid_request())? != Some("Bearer fixture") { return Err(Failure::invalid_credential()); }
        VerifiedIdentity::from_verified(state.subject,Instant::now()+Duration::from_secs(5))
    }
    pub async fn authorize(scope:AuthScope,request:http::Request,state:Arc<super::State>)->Result<Grant<super::Edit>,Failure>{
        assert!(request.body().is_empty()); Grant::from_authorized(scope,state.target)
    }
    // Trusted, reviewed adapter: fixed statement and both supplied real proof
    // values go into the predicate; generic SQL does not prove tenant isolation.
    pub async fn protected_update(pool:&sqlite::Pool,grant:Grant<super::Edit>,value:i64)->Result<i64,Error>{
        let tx=sqlite::begin(pool,sqlite::BeginMode::Immediate).await.map_err(|e|Error::internal(e.to_string()))?;
        let reservation=tx.reserve_exec().await.map_err(|e|Error::internal(e.to_string()))?;
        let reply=grant.submit(reservation,|subject,target,res|res.enqueue(
            sqlite::literal("UPDATE protected SET value=? WHERE owner=? AND id=?"),
            sqlite::bind_i64(sqlite::bind_i64(sqlite::bind_i64(sqlite::parameters(),value),subject),target)))
            .map_err(|e|Error::internal(e.to_string()))?;
        let result=reply.await;
        match result {
            Ok(count)=>{sqlite::commit(tx).await.map_err(|e|Error::internal(e.to_string()))?;Ok(count)}
            Err(e)=>{sqlite::rollback(tx).await.map_err(|e|Error::internal(e.to_string()))?;Err(Error::internal(e.to_string()))}
        }
    }
}


#[test]
fn real_sqlite_and_request_bound_predicate() {
    use nagi_runtime::{sqlite,http_server as http};
    use tokio::{net::{TcpListener,TcpStream},io::{AsyncReadExt,AsyncWriteExt},sync::oneshot};
    use std::time::Duration;
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let query = selected_query(9);
        let holder = make_holder(query);
        assert_eq!(format!("{:?}", holder.query), "Query { .. }");
        let debug = format!("{:?}", holder);
        assert!(debug.contains("Query { .. }"));
        assert!(!debug.contains("SELECT") && !debug.contains("protected"));
        // Query is copied, stored, shared, returned and selected by Nagi;
        // the direct literal restriction does not constrain these value uses.
        let second_holder = make_holder(query);
        assert_eq!(format!("{:?}", second_holder.query), "Query { .. }");
        let pool=sqlite::open(":memory:",sqlite::options(1,2,1000,0).unwrap()).await.unwrap();
        // Administrative fixture bootstrap with reviewed fixed SQL, outside a
        // request; no dynamic SQL factory or request-facing management export.
        let tx=sqlite::begin(&pool,sqlite::BeginMode::Immediate).await.unwrap();
        sqlite::exec(&tx,sqlite::literal("CREATE TABLE protected(id INTEGER PRIMARY KEY,owner INTEGER,value INTEGER)"),sqlite::parameters()).await.unwrap();
        sqlite::exec(&tx,sqlite::literal("INSERT INTO protected VALUES(9,7,100),(10,8,200)"),sqlite::parameters()).await.unwrap();
        sqlite::commit(tx).await.unwrap();
        for (subject,target,body) in [(7,9,"1"),(8,9,"0"),(7,10,"0")] {
            let app=setup(sqlite::clone_pool(&pool),subject,target).unwrap();
            let listener=TcpListener::bind("127.0.0.1:0").await.unwrap();let address=listener.local_addr().unwrap();
            let (stop,done)=oneshot::channel();
            let task=tokio::spawn(http::serve_listener(listener,app,http::default_options(),async{let _=done.await;}));
            for (credentials,status,expected) in [("Authorization: Bearer fixture\r\n",200,body),("",401,"invalid credential")] {
                let mut stream=TcpStream::connect(address).await.unwrap();
                stream.write_all(format!("POST /update HTTP/1.1\r\nHost: localhost\r\n{credentials}Content-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
                let mut output=String::new();tokio::time::timeout(Duration::from_secs(3),stream.read_to_string(&mut output)).await.unwrap().unwrap();
                assert!(output.starts_with(&format!("HTTP/1.1 {status}")),"{output}");assert!(output.ends_with(expected),"{output}");
            }
            stop.send(()).unwrap();task.await.unwrap().unwrap();
            let tx=sqlite::begin(&pool,sqlite::BeginMode::Deferred).await.unwrap();
            assert_eq!(read_value(&tx,9).await.unwrap().unwrap().value,101);
            assert_eq!(read_value(&tx,10).await.unwrap().unwrap().value,200);
            sqlite::rollback(tx).await.unwrap();
        }
        sqlite::close(&pool,2000).await.unwrap();
    });
}
