#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f76652d70726f6f662f636f73742d636f6d70617269736f6e2f636f6d6d6f6e2f6d61696e2e6e616769_f_6f627365727665() -> () {
    native::observe()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f76652d70726f6f662f636f73742d636f6d70617269736f6e2f636f6d6d6f6e2f6d61696e2e6e616769_f_7061757365() -> () {
    native::pause().await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f76652d70726f6f662f636f73742d636f6d70617269736f6e2f636f6d6d6f6e2f6d61696e2e6e616769_f_617272616e6765<'a>(mut text: &'a ::std::primitive::str) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut text: &::std::primitive::str = text;
    let mut __nagi_view_flow_8_53_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![text]);
    let mut __nagi_view_flow_8_53_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut __nagi_view_flow_8_53_v2: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut saved: ::std::vec::Vec<&::std::primitive::str> = ::std::convert::identity(__nagi_view_flow_8_53_v0.expect("checked view binding"));
    let mut local: ::std::string::String = (text).to_owned();
    let __nagi_view_flow_rhs_0 = vec![(local).as_str()];
    __nagi_view_flow_8_53_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_8_53_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_8_53_v0);
    let __nagi_view_flow_rhs_1 = ::std::convert::identity(saved);
    __nagi_view_flow_8_53_v2 = ::std::option::Option::Some(__nagi_view_flow_rhs_1);
    __nagi_view_flow_8_53_v1 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_8_53_v1);
    (crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f76652d70726f6f662f636f73742d636f6d70617269736f6e2f636f6d6d6f6e2f6d61696e2e6e616769_f_7061757365()).await;
    return __nagi_view_flow_8_53_v2.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_main() -> () {
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f76652d70726f6f662f636f73742d636f6d70617269736f6e2f636f6d6d6f6e2f6d61696e2e6e616769_f_6f627365727665();
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f76652d70726f6f662f636f73742d636f6d70617269736f6e2f636f6d6d6f6e2f6d61696e2e6e616769_f_6f627365727665 as observe;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f76652d70726f6f662f636f73742d636f6d70617269736f6e2f636f6d6d6f6e2f6d61696e2e6e616769_f_7061757365 as pause;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f76652d70726f6f662f636f73742d636f6d70617269736f6e2f636f6d6d6f6e2f6d61696e2e6e616769_f_617272616e6765 as arrange;
#[allow(unused_imports)]
pub use ::std::convert::identity as r#move;
fn main() {
__nagi_main();
}

#[path = "/tmp/nagi-explicit-move-proof/cost-comparison/common/native.rs"]
mod native;
