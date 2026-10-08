#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_c_486f6c646572 {
    pub policy: ::nagi_runtime::http_server::Policy<::std::primitive::i64, ()>,
}
impl ::std::fmt::Debug for __nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_c_486f6c646572 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Holder").field("policy", &self.policy).finish()
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_f_696e7370656374(mut value: ::std::sync::Arc<::nagi_runtime::http_server::Policy<::std::primitive::i64, ()>>) -> () {
    println!("{}", 0i64);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_main() -> () {
    let mut wrapped: ::std::sync::Arc<::std::option::Option<::nagi_runtime::http_server::Policy<::std::primitive::i64, ()>>> = ::std::sync::Arc::new(::std::option::Option::Some(::nagi_runtime::http_server::public_policy::<::std::primitive::i64>()));
    let mut again: ::std::sync::Arc<::std::option::Option<::nagi_runtime::http_server::Policy<::std::primitive::i64, ()>>> = ::std::sync::Arc::clone(&wrapped);
    let mut holder: __nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_c_486f6c646572 = __nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_c_486f6c646572 { policy: ::nagi_runtime::http_server::public_policy::<::std::primitive::i64>() };
    let mut shared_holder: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_c_486f6c646572> = ::std::sync::Arc::new(holder);
    let mut again_holder: ::std::sync::Arc<__nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_c_486f6c646572> = ::std::sync::Arc::clone(&shared_holder);
    println!("{}", 7i64);
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_c_486f6c646572 as Holder;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f6e6167692d736630312d6172746966616374732f696e646570656e64656e742d7265766965772d643132376232382d61353233673633622f70322d706f6c6963792e6e616769_f_696e7370656374 as inspect;
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
fn main() {
__nagi_main();
}
