#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_camel_case_types, non_snake_case)]
pub struct __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 {
    pub id: ::std::primitive::i64,
    pub tag: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_e_546167,
}
impl ::std::fmt::Debug for __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.debug_struct("Marker").field("id", &self.id).field("tag", &self.tag).finish()
    }
}
#[allow(non_camel_case_types, non_snake_case)]
#[derive(Debug)]
pub enum __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_e_546167 {
    Text {
        value: ::std::string::String,
    },
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(mut id: ::std::primitive::i64) -> __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 {
    native::marker(id)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(mut id: ::std::primitive::i64) -> () {
    native::capture(id)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6261645f696e646578() -> ::std::primitive::i64 {
    native::bad_index()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6372617368() -> () {
    native::crash()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_636f6e73756d65<'a>(mut parts: ::std::vec::Vec<&'a ::std::primitive::str>) -> ::std::primitive::bool {
    native::consume(parts)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_636865636b706f696e74() -> () {
    native::checkpoint()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_61626f72745f6d75746174696f6e() -> () {
    native::abort_mutation()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6661696c5f73746570() -> ::std::result::Result<(), ::std::primitive::i64> {
    native::fail_step()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_7061757365() -> () {
    native::pause().await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d75746174696f6e5f6372617368<'a>(mut value: &'a ::std::primitive::str) -> &'a ::std::primitive::str {
    let mut value: &::std::primitive::str = value;
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_61626f72745f6d75746174696f6e();
    return value;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_7265706c6163655f616e645f72657475726e<'a>(mut parts: ::std::vec::Vec<&'a ::std::primitive::str>) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut parts: ::std::vec::Vec<&::std::primitive::str> = parts;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(10i64);
    let mut local: ::std::string::String = ((parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(1i64);
    let mut __nagi_view_flow_36_243_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(local).as_str()]);
    let mut __nagi_view_flow_36_243_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(2i64);
    let __nagi_view_flow_rhs_0 = vec![(parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]];
    __nagi_view_flow_36_243_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_36_243_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_36_243_v0);
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(20i64);
    return __nagi_view_flow_36_243_v1.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_70616e69635f647572696e675f7265706c6163656d656e74<'a>(mut parts: ::std::vec::Vec<&'a ::std::primitive::str>) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut parts: ::std::vec::Vec<&::std::primitive::str> = parts;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(30i64);
    let mut local: ::std::string::String = ((parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(3i64);
    let mut __nagi_view_flow_46_323_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(local).as_str()]);
    let mut __nagi_view_flow_46_323_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(40i64);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(4i64);
    let __nagi_view_flow_rhs_0 = vec![(parts)[::std::primitive::usize::try_from(crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6261645f696e646578()).expect("negative index")]];
    __nagi_view_flow_46_323_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_46_323_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_46_323_v0);
    return __nagi_view_flow_46_323_v1.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_70616e69635f61667465725f7265706c6163656d656e74<'a>(mut parts: ::std::vec::Vec<&'a ::std::primitive::str>) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut parts: ::std::vec::Vec<&::std::primitive::str> = parts;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(50i64);
    let mut local: ::std::string::String = ((parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(5i64);
    let mut __nagi_view_flow_56_405_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(local).as_str()]);
    let mut __nagi_view_flow_56_405_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(60i64);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(6i64);
    let __nagi_view_flow_rhs_0 = vec![(parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]];
    __nagi_view_flow_56_405_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_56_405_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_56_405_v0);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6372617368();
    return __nagi_view_flow_56_405_v1.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_73686f72745f636972637569745f7265706c6163656d656e74<'a>(mut flag: ::std::primitive::bool, mut parts: ::std::vec::Vec<&'a ::std::primitive::str>) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut parts: ::std::vec::Vec<&::std::primitive::str> = parts;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(70i64);
    let mut local: ::std::string::String = ((parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(7i64);
    let mut __nagi_view_flow_67_493_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(local).as_str()]);
    let mut __nagi_view_flow_67_493_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(80i64);
    let mut selected: ::std::primitive::bool = (flag && crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_636f6e73756d65(__nagi_view_flow_67_493_v0.expect("checked view binding")));
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(8i64);
    let __nagi_view_flow_rhs_0 = vec![(parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]];
    __nagi_view_flow_67_493_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_67_493_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_67_493_v0);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_636865636b706f696e74();
    return __nagi_view_flow_67_493_v1.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6272616e63685f6d75746174696f6e<'a>(mut flag: ::std::primitive::bool, mut parts: ::std::vec::Vec<&'a ::std::primitive::str>) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut parts: ::std::vec::Vec<&::std::primitive::str> = parts;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(90i64);
    let mut local: ::std::string::String = ((parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(9i64);
    let mut __nagi_view_flow_79_590_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]]);
    let mut __nagi_view_flow_79_590_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut __nagi_view_flow_79_590_v2: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut __nagi_view_flow_79_590_v3: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(100i64);
    if flag {
        __nagi_view_flow_79_590_v1 = __nagi_view_flow_79_590_v0;
        { let __nagi_view_flow_item = (local).as_str(); (*__nagi_view_flow_79_590_v1.as_mut().expect("checked view binding")).push(__nagi_view_flow_item) };
        crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(10i64);
        let __nagi_view_flow_rhs_0 = vec![(parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]];
        __nagi_view_flow_79_590_v2 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
        __nagi_view_flow_79_590_v1 = ::std::option::Option::None;
        ::std::mem::drop(__nagi_view_flow_79_590_v1);
        __nagi_view_flow_79_590_v3 = __nagi_view_flow_79_590_v2;
    } else {
        __nagi_view_flow_79_590_v3 = __nagi_view_flow_79_590_v0;
    }
    return __nagi_view_flow_79_590_v3.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_70616e69635f647572696e675f6d75746174696f6e<'a>(mut parts: ::std::vec::Vec<&'a ::std::primitive::str>) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut parts: ::std::vec::Vec<&::std::primitive::str> = parts;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(110i64);
    let mut local: ::std::string::String = ((parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(11i64);
    let mut __nagi_view_flow_91_686_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]]);
    let mut __nagi_view_flow_91_686_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut __nagi_view_flow_91_686_v2: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(120i64);
    __nagi_view_flow_91_686_v1 = __nagi_view_flow_91_686_v0;
    { let __nagi_view_flow_item = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d75746174696f6e5f6372617368((local).as_str()); (*__nagi_view_flow_91_686_v1.as_mut().expect("checked view binding")).push(__nagi_view_flow_item) };
    let __nagi_view_flow_rhs_0 = vec![(parts)[::std::primitive::usize::try_from(0i64).expect("negative index")]];
    __nagi_view_flow_91_686_v2 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_91_686_v1 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_91_686_v1);
    return __nagi_view_flow_91_686_v2.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_616c6961735f6d6f76655f7468656e5f70616e6963<'a>(mut part: &'a ::std::primitive::str) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut part: &::std::primitive::str = part;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(130i64);
    let mut local: ::std::string::String = (part).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(13i64);
    let mut __nagi_view_flow_101_768_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(local).as_str()]);
    let mut __nagi_view_flow_101_768_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(140i64);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(14i64);
    let __nagi_view_flow_rhs_0 = vec![part];
    __nagi_view_flow_101_768_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_101_768_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_101_768_v0);
    let mut selected: ::std::vec::Vec<&::std::primitive::str> = ::std::convert::identity(__nagi_view_flow_101_768_v1.expect("checked view binding"));
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6372617368();
    return selected;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_726573746f7265645f6c6973745f7468656e5f6572726f72<'a>(mut part: &'a ::std::primitive::str) -> ::std::result::Result<::std::vec::Vec<&'a ::std::primitive::str>, ::std::primitive::i64> {
    let mut part: &::std::primitive::str = part;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(150i64);
    let mut local: ::std::string::String = (part).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(15i64);
    let mut __nagi_view_flow_113_855_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(local).as_str()]);
    let mut __nagi_view_flow_113_855_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(160i64);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(16i64);
    let __nagi_view_flow_rhs_0 = vec![part];
    __nagi_view_flow_113_855_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_113_855_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_113_855_v0);
    (crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6661696c5f73746570())?;
    return ::std::result::Result::Ok(__nagi_view_flow_113_855_v1.expect("checked view binding"));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_7061747465726e5f6c6973745f7468656e5f70616e6963<'a>(mut part: &'a ::std::primitive::str, mut input: ::std::result::Result<::std::vec::Vec<&'a ::std::primitive::str>, ::std::primitive::i64>) -> ::std::result::Result<::std::vec::Vec<&'a ::std::primitive::str>, ::std::primitive::i64> {
    let mut part: &::std::primitive::str = part;
    let mut input: ::std::result::Result<::std::vec::Vec<&::std::primitive::str>, ::std::primitive::i64> = input;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(170i64);
    match input {
        ::std::result::Result::Ok(mut parts) => {
            let mut __nagi_view_flow_123_950_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
            __nagi_view_flow_123_950_v0 = ::std::option::Option::Some(parts);
            let mut __nagi_view_flow_123_950_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
            let mut __nagi_view_flow_123_950_v2: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
            let mut local: ::std::string::String = (part).to_owned();
            crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(17i64);
            let __nagi_view_flow_rhs_0 = vec![(local).as_str()];
            __nagi_view_flow_123_950_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
            __nagi_view_flow_123_950_v0 = ::std::option::Option::None;
            ::std::mem::drop(__nagi_view_flow_123_950_v0);
            let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(180i64);
            crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(18i64);
            let __nagi_view_flow_rhs_1 = vec![part];
            __nagi_view_flow_123_950_v2 = ::std::option::Option::Some(__nagi_view_flow_rhs_1);
            __nagi_view_flow_123_950_v1 = ::std::option::Option::None;
            ::std::mem::drop(__nagi_view_flow_123_950_v1);
            crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6372617368();
            return ::std::result::Result::Ok(__nagi_view_flow_123_950_v2.expect("checked view binding"));
        },
        ::std::result::Result::Err(mut code) => {
            return ::std::result::Result::Err(code);
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6c6973745f776974685f6f776e65645f6572726f725f656c656d656e7473<'a>(mut part: &'a ::std::primitive::str) -> ::std::vec::Vec<::std::result::Result<&'a ::std::primitive::str, __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572>> {
    let mut part: &::std::primitive::str = part;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(190i64);
    let mut local: ::std::string::String = (part).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(19i64);
    let mut __nagi_view_flow_139_1067_v0: ::std::option::Option<::std::vec::Vec<::std::result::Result<&::std::primitive::str, __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572>>> = ::std::option::Option::Some(vec![::std::result::Result::Ok((local).as_str()), ::std::result::Result::Err(crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(200i64))]);
    let mut __nagi_view_flow_139_1067_v1: ::std::option::Option<::std::vec::Vec<::std::result::Result<&::std::primitive::str, __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572>>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(220i64);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(20i64);
    let __nagi_view_flow_rhs_0 = vec![::std::result::Result::Ok(part), ::std::result::Result::Err(crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(210i64))];
    __nagi_view_flow_139_1067_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_139_1067_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_139_1067_v0);
    return __nagi_view_flow_139_1067_v1.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_726573756c745f6572726f725f7265706c6163656d656e74<'a>(mut part: &'a ::std::primitive::str) -> ::std::result::Result<::std::vec::Vec<&'a ::std::primitive::str>, __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572> {
    let mut part: &::std::primitive::str = part;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(260i64);
    let mut local: ::std::string::String = (part).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(41i64);
    let mut value: ::std::result::Result<::std::vec::Vec<&::std::primitive::str>, __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572> = ::std::result::Result::Ok(vec![(local).as_str()]);
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(270i64);
    value = ::std::result::Result::Err(crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(280i64));
    value = ::std::result::Result::Err(crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(290i64));
    match value {
        ::std::result::Result::Ok(_) => {
            return ::std::result::Result::Ok(vec![part]);
        },
        ::std::result::Result::Err(mut problem) => {
            return ::std::result::Result::Err(problem);
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6f7074696f6e5f7265706c6163656d656e74<'a>(mut part: &'a ::std::primitive::str) -> ::std::option::Option<::std::vec::Vec<&'a ::std::primitive::str>> {
    let mut part: &::std::primitive::str = part;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(300i64);
    let mut local: ::std::string::String = (part).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(42i64);
    let mut __nagi_view_flow_163_1312_v0: ::std::option::Option<::std::option::Option<::std::vec::Vec<&::std::primitive::str>>> = ::std::option::Option::Some(::std::option::Option::Some(vec![(local).as_str()]));
    let mut __nagi_view_flow_163_1312_v1: ::std::option::Option<::std::option::Option<::std::vec::Vec<&::std::primitive::str>>>;
    let mut __nagi_view_flow_163_1312_v2: ::std::option::Option<::std::option::Option<::std::vec::Vec<&::std::primitive::str>>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(310i64);
    let __nagi_view_flow_rhs_0 = ::std::option::Option::None;
    __nagi_view_flow_163_1312_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_163_1312_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_163_1312_v0);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(43i64);
    let __nagi_view_flow_rhs_1 = ::std::option::Option::Some(vec![part]);
    __nagi_view_flow_163_1312_v2 = ::std::option::Option::Some(__nagi_view_flow_rhs_1);
    __nagi_view_flow_163_1312_v1 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_163_1312_v1);
    return __nagi_view_flow_163_1312_v2.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6173796e635f7265706c6163656d656e74<'a>(mut part: &'a ::std::primitive::str, mut panic_now: ::std::primitive::bool, mut error_now: ::std::primitive::bool) -> ::std::result::Result<::std::vec::Vec<&'a ::std::primitive::str>, ::std::primitive::i64> {
    let mut part: &::std::primitive::str = part;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(320i64);
    let mut local: ::std::string::String = (part).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(44i64);
    let mut __nagi_view_flow_174_1418_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(local).as_str()]);
    let mut __nagi_view_flow_174_1418_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(330i64);
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(45i64);
    let __nagi_view_flow_rhs_0 = vec![part];
    __nagi_view_flow_174_1418_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_174_1418_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_174_1418_v0);
    (crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_7061757365()).await;
    if panic_now {
        crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6372617368();
    }
    if error_now {
        (crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6661696c5f73746570())?;
    }
    return ::std::result::Result::Ok(__nagi_view_flow_174_1418_v1.expect("checked view binding"));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6173796e635f6265666f72655f7265706c6163656d656e74<'a>(mut part: &'a ::std::primitive::str) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut part: &::std::primitive::str = part;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(340i64);
    let mut local: ::std::string::String = (part).to_owned();
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(46i64);
    let mut __nagi_view_flow_189_1519_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![(local).as_str()]);
    let mut __nagi_view_flow_189_1519_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(350i64);
    (crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_7061757365()).await;
    crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(47i64);
    let __nagi_view_flow_rhs_0 = vec![part];
    __nagi_view_flow_189_1519_v1 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
    __nagi_view_flow_189_1519_v0 = ::std::option::Option::None;
    ::std::mem::drop(__nagi_view_flow_189_1519_v0);
    return __nagi_view_flow_189_1519_v1.expect("checked view binding");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6c6f6f705f7265706c6163656d656e74<'a>(mut part: &'a ::std::primitive::str, mut count: ::std::primitive::i64) -> ::std::vec::Vec<&'a ::std::primitive::str> {
    let mut part: &::std::primitive::str = part;
    let mut before: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(360i64);
    let mut __nagi_view_flow_198_1587_v0: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>> = ::std::option::Option::Some(vec![part]);
    let mut __nagi_view_flow_198_1587_v1: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut __nagi_view_flow_198_1587_v2: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    let mut __nagi_view_flow_198_1587_v3: ::std::option::Option<::std::vec::Vec<&::std::primitive::str>>;
    __nagi_view_flow_198_1587_v1 = __nagi_view_flow_198_1587_v0;
    for mut number in 0i64..count {
        let mut local: ::std::string::String = (part).to_owned();
        crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(48i64);
        let __nagi_view_flow_rhs_0 = vec![(local).as_str()];
        __nagi_view_flow_198_1587_v2 = ::std::option::Option::Some(__nagi_view_flow_rhs_0);
        __nagi_view_flow_198_1587_v1 = ::std::option::Option::None;
        ::std::mem::drop(__nagi_view_flow_198_1587_v1);
        __nagi_view_flow_198_1587_v3 = ::std::option::Option::None;
        ::std::mem::drop(__nagi_view_flow_198_1587_v3);
        let mut within: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(370i64);
        crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265(49i64);
        let __nagi_view_flow_rhs_1 = vec![part];
        __nagi_view_flow_198_1587_v3 = ::std::option::Option::Some(__nagi_view_flow_rhs_1);
        __nagi_view_flow_198_1587_v1 = ::std::option::Option::None;
        ::std::mem::drop(__nagi_view_flow_198_1587_v1);
        __nagi_view_flow_198_1587_v2 = ::std::option::Option::None;
        ::std::mem::drop(__nagi_view_flow_198_1587_v2);
        __nagi_view_flow_198_1587_v1 = __nagi_view_flow_198_1587_v3;
    }
    let mut after: __nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 = crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572(380i64);
    return __nagi_view_flow_198_1587_v1.expect("checked view binding");
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_c_4d61726b6572 as Marker;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_e_546167 as Tag;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d61726b6572 as marker;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_63617074757265 as capture;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6261645f696e646578 as bad_index;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6372617368 as crash;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_636f6e73756d65 as consume;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_636865636b706f696e74 as checkpoint;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_61626f72745f6d75746174696f6e as abort_mutation;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6661696c5f73746570 as fail_step;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_7061757365 as pause;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6d75746174696f6e5f6372617368 as mutation_crash;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_7265706c6163655f616e645f72657475726e as replace_and_return;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_70616e69635f647572696e675f7265706c6163656d656e74 as panic_during_replacement;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_70616e69635f61667465725f7265706c6163656d656e74 as panic_after_replacement;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_73686f72745f636972637569745f7265706c6163656d656e74 as short_circuit_replacement;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6272616e63685f6d75746174696f6e as branch_mutation;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_70616e69635f647572696e675f6d75746174696f6e as panic_during_mutation;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_616c6961735f6d6f76655f7468656e5f70616e6963 as alias_move_then_panic;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_726573746f7265645f6c6973745f7468656e5f6572726f72 as restored_list_then_error;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_7061747465726e5f6c6973745f7468656e5f70616e6963 as pattern_list_then_panic;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6c6973745f776974685f6f776e65645f6572726f725f656c656d656e7473 as list_with_owned_error_elements;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_726573756c745f6572726f725f7265706c6163656d656e74 as result_error_replacement;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6f7074696f6e5f7265706c6163656d656e74 as option_replacement;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6173796e635f7265706c6163656d656e74 as async_replacement;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6173796e635f6265666f72655f7265706c6163656d656e74 as async_before_replacement;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d766965772d636f6e7461696e65722d64726f702d313430383036302d302f696e7075742e6e616769_f_6c6f6f705f7265706c6163656d656e74 as loop_replacement;
#[allow(unused_imports)]
pub use ::std::convert::identity as r#move;
fn main() {}


use ::std::alloc::{GlobalAlloc, Layout, System};
use ::std::sync::atomic::{AtomicUsize, Ordering};

const EVENT_CAPACITY: usize = 128;
const TRACK_CAPACITY: usize = 16;
const ALLOC_BASE: usize = 1_000;
const FREE_BASE: usize = 2_000;
const DROP_BASE: usize = 3_000;
const BAD_INDEX: usize = 4_000;
const CAUGHT: usize = 5_000;
const CRASH: usize = 6_000;
const CHECKPOINT: usize = 7_000;
const REALLOC_BASE: usize = 8_000;
const MUTATION_PANIC: usize = 9_000;

static EVENTS: [AtomicUsize; EVENT_CAPACITY] =
    [const { AtomicUsize::new(0) }; EVENT_CAPACITY];
static EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);
static PANIC_MARKER_ID: AtomicUsize = AtomicUsize::new(0);
static PAUSE_READY: ::std::sync::atomic::AtomicBool =
    ::std::sync::atomic::AtomicBool::new(false);
static CAPTURE_VEC_ID: AtomicUsize = AtomicUsize::new(0);
static TRACKED_POINTERS: [AtomicUsize; TRACK_CAPACITY] =
    [const { AtomicUsize::new(0) }; TRACK_CAPACITY];
static TRACKED_IDS: [AtomicUsize; TRACK_CAPACITY] =
    [const { AtomicUsize::new(0) }; TRACK_CAPACITY];

fn record(event: usize) {
    let index = EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
    if index < EVENT_CAPACITY {
        EVENTS[index].store(event, Ordering::Relaxed);
    }
}

fn tracked_count() -> usize {
    TRACKED_POINTERS
        .iter()
        .filter(|pointer| pointer.load(Ordering::Relaxed) != 0)
        .count()
}

fn event_log() -> Vec<usize> {
    let count = EVENT_COUNT.load(Ordering::Relaxed).min(EVENT_CAPACITY);
    (0..count)
        .map(|index| EVENTS[index].load(Ordering::Relaxed))
        .collect()
}

fn reset_events() {
    assert_eq!(tracked_count(), 0, "a tracked view-container buffer escaped");
    EVENT_COUNT.store(0, Ordering::Relaxed);
    CAPTURE_VEC_ID.store(0, Ordering::Relaxed);
    PANIC_MARKER_ID.store(0, Ordering::Relaxed);
}

fn assert_events(expected: &[usize], tracked: usize) {
    assert_eq!(event_log(), expected);
    assert_eq!(tracked_count(), tracked);
}

struct AllocationLog;

#[global_allocator]
static ALLOCATOR: AllocationLog = AllocationLog;

unsafe impl GlobalAlloc for AllocationLog {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Safety: delegate the same allocation layout to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        let id = CAPTURE_VEC_ID.swap(0, Ordering::Relaxed);
        if !pointer.is_null() && id != 0 {
            for index in 0..TRACK_CAPACITY {
                if TRACKED_POINTERS[index]
                    .compare_exchange(
                        0,
                        pointer as usize,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    TRACKED_IDS[index].store(id, Ordering::Relaxed);
                    record(ALLOC_BASE + id);
                    break;
                }
            }
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let tracked = TRACKED_POINTERS.iter().position(|entry| {
            entry.load(Ordering::Relaxed) == pointer as usize
        });
        // Safety: forward the original pointer/layout and requested size to
        // the same allocator. A failed realloc leaves the old buffer valid.
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            if let Some(index) = tracked {
                TRACKED_POINTERS[index].store(next as usize, Ordering::Relaxed);
                record(REALLOC_BASE + TRACKED_IDS[index].load(Ordering::Relaxed));
            }
        }
        next
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let mut freed_id = 0;
        for index in 0..TRACK_CAPACITY {
            if TRACKED_POINTERS[index]
                .compare_exchange(
                    pointer as usize,
                    0,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                freed_id = TRACKED_IDS[index].swap(0, Ordering::Relaxed);
                break;
            }
        }
        // Safety: this pointer and matching layout came from `System.alloc`.
        unsafe { System.dealloc(pointer, layout) };
        if freed_id != 0 {
            record(FREE_BASE + freed_id);
        }
    }
}

#[path = "high-native.rs"]
mod native;

// Same public arguments, resources, allocation count, await and output as
// async_replacement, with caller-only borrows that need no lifetime splitting.
async fn caller_only_reference(part: &str, panic_now: bool, error_now: bool) -> Result<Vec<&str>, i64> {
    let before = native::marker(320);
    let local = part.to_owned();
    let mut parts = vec![part];
    let after = native::marker(330);
    parts = vec![part];
    native::pause().await;
    if panic_now { native::crash(); }
    if error_now { native::fail_step()?; }
    Ok(parts)
}

// Match the generated function's local-to-caller borrow history and original
// cleanup anchors. Use the same async extern wrapper, not native::pause directly.
// These are measurement oracles; exact Future sizes are toolchain-dependent.
async fn same_history_reference(part: &str, panic_now: bool, error_now: bool) -> Result<Vec<&str>, i64> {
    let before = native::marker(320);
    let local = part.to_owned();
    let mut previous = Some(vec![local.as_str()]);
    let next;
    let after = native::marker(330);
    next = Some(vec![part]);
    previous = None;
    drop(previous);
    pause().await;
    if panic_now { native::crash(); }
    if error_now { native::fail_step()?; }
    Ok(next.expect("initialized"))
}

// Change only the await bridge, keeping the same slots and cleanup anchors.
async fn same_history_native_pause(part: &str, panic_now: bool, error_now: bool) -> Result<Vec<&str>, i64> {
    let before = native::marker(320);
    let local = part.to_owned();
    let mut previous = Some(vec![local.as_str()]);
    let next;
    let after = native::marker(330);
    next = Some(vec![part]);
    previous = None;
    drop(previous);
    native::pause().await;
    if panic_now { native::crash(); }
    if error_now { native::fail_step()?; }
    Ok(next.expect("initialized"))
}

fn poll_once<F: ::std::future::Future>(future: ::std::pin::Pin<&mut F>) -> ::std::task::Poll<F::Output> {
    let mut context = ::std::task::Context::from_waker(::std::task::Waker::noop());
    future.poll(&mut context)
}

#[test]
fn replacement_observes_rhs_then_old_free_and_scope_drop_order() {
    let source = String::from("caller-owned");
    let generated_size = ::std::mem::size_of_val(&async_replacement(source.as_str(), false, false));
    let reference_size = ::std::mem::size_of_val(&caller_only_reference(source.as_str(), false, false));
    println!("future-bytes: generated={generated_size} caller-only-reference={reference_size}");
    let same_history_size = ::std::mem::size_of_val(&same_history_reference(source.as_str(), false, false));
    println!("future-layout: same-history-same-bridge={same_history_size} same-history-native-pause={} native-pause={} extern-wrapper={}",
        ::std::mem::size_of_val(&same_history_native_pause(source.as_str(), false, false)),
        ::std::mem::size_of_val(&native::pause()), ::std::mem::size_of_val(&pause()));

    reset_events();
    let returned = replace_and_return(vec![source.as_str()]);
    assert_eq!(returned, vec!["caller-owned"]);
    assert_events(
        &[
            ALLOC_BASE + 1,
            ALLOC_BASE + 2,
            FREE_BASE + 1,
            DROP_BASE + 20,
            DROP_BASE + 10,
        ],
        1,
    );
    drop(returned);
    assert_events(
        &[
            ALLOC_BASE + 1,
            ALLOC_BASE + 2,
            FREE_BASE + 1,
            DROP_BASE + 20,
            DROP_BASE + 10,
            FREE_BASE + 2,
        ],
        0,
    );

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        panic_during_replacement(vec![source.as_str()])
    }));
    assert!(result.is_err());
    record(CAUGHT);
    assert_events(
        &[
            ALLOC_BASE + 3,
            ALLOC_BASE + 4,
            BAD_INDEX,
            FREE_BASE + 4,
            DROP_BASE + 40,
            FREE_BASE + 3,
            DROP_BASE + 30,
            CAUGHT,
        ],
        0,
    );

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        panic_after_replacement(vec![source.as_str()])
    }));
    assert!(result.is_err());
    record(CAUGHT);
    assert_events(
        &[
            ALLOC_BASE + 5,
            ALLOC_BASE + 6,
            FREE_BASE + 5,
            CRASH,
            DROP_BASE + 60,
            FREE_BASE + 6,
            DROP_BASE + 50,
            CAUGHT,
        ],
        0,
    );

    for flag in [false, true] {
        reset_events();
        let returned = short_circuit_replacement(flag, vec![source.as_str()]);
        assert_eq!(returned, vec!["caller-owned"]);
        let expected = if flag {
            vec![ALLOC_BASE + 7, FREE_BASE + 7, ALLOC_BASE + 8, CHECKPOINT,
                 DROP_BASE + 80, DROP_BASE + 70]
        } else {
            vec![ALLOC_BASE + 7, ALLOC_BASE + 8, FREE_BASE + 7, CHECKPOINT,
                 DROP_BASE + 80, DROP_BASE + 70]
        };
        assert_events(&expected, 1);
        drop(returned);
        let mut finished = expected;
        finished.push(FREE_BASE + 8);
        assert_events(&finished, 0);
    }

    for flag in [false, true] {
        reset_events();
        let returned = branch_mutation(flag, vec![source.as_str()]);
        assert_eq!(returned, vec!["caller-owned"]);
        let (expected, returned_id) = if flag {
            (vec![ALLOC_BASE + 9, REALLOC_BASE + 9, ALLOC_BASE + 10,
                  FREE_BASE + 9, DROP_BASE + 100, DROP_BASE + 90], 10)
        } else {
            (vec![ALLOC_BASE + 9, DROP_BASE + 100, DROP_BASE + 90], 9)
        };
        assert_events(&expected, 1);
        drop(returned);
        let mut finished = expected;
        finished.push(FREE_BASE + returned_id);
        assert_events(&finished, 0);
    }

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        panic_during_mutation(vec![source.as_str()])
    }));
    assert!(result.is_err());
    record(CAUGHT);
    assert_events(
        &[ALLOC_BASE + 11, MUTATION_PANIC, DROP_BASE + 120,
          FREE_BASE + 11, DROP_BASE + 110, CAUGHT],
        0,
    );

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        alias_move_then_panic(source.as_str())
    }));
    assert!(result.is_err());
    record(CAUGHT);
    // Moving into a new source binding transfers its cleanup position too.
    // `selected` is declared after the later Marker and must drop first.
    assert_events(
        &[ALLOC_BASE + 13, ALLOC_BASE + 14, FREE_BASE + 13, CRASH,
          FREE_BASE + 14, DROP_BASE + 140, DROP_BASE + 130, CAUGHT],
        0,
    );

    reset_events();
    assert_eq!(restored_list_then_error(source.as_str()), Err(7));
    assert_events(
        &[ALLOC_BASE + 15, ALLOC_BASE + 16, FREE_BASE + 15, CHECKPOINT,
          DROP_BASE + 160, FREE_BASE + 16, DROP_BASE + 150],
        0,
    );

    reset_events();
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        pattern_list_then_panic(source.as_str(), Ok(vec![source.as_str()]))
    }));
    assert!(result.is_err());
    record(CAUGHT);
    // A pattern-bound container has the pattern's cleanup anchor, before
    // the Marker in the arm. Synthetic storage must not move that anchor.
    assert_events(
        &[ALLOC_BASE + 17, ALLOC_BASE + 18, FREE_BASE + 17, CRASH,
          DROP_BASE + 180, FREE_BASE + 18, DROP_BASE + 170, CAUGHT],
        0,
    );

    reset_events();
    let returned = list_with_owned_error_elements(source.as_str());
    assert!(matches!(&returned[0], Ok(value) if *value == "caller-owned"));
    assert!(matches!(&returned[1], Err(value) if value.id == 210));
    let expected = [ALLOC_BASE + 19, ALLOC_BASE + 20, DROP_BASE + 200,
                    FREE_BASE + 19, DROP_BASE + 220, DROP_BASE + 190];
    assert_events(&expected, 1);
    drop(returned);
    let mut expected = expected.to_vec();
    expected.extend([DROP_BASE + 210, FREE_BASE + 20]);
    assert_events(&expected, 0);

    reset_events();
    PANIC_MARKER_ID.store(200, Ordering::Relaxed);
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        list_with_owned_error_elements(source.as_str())
    }));
    assert!(result.is_err());
    record(CAUGHT);
    // Rust assignment keeps the already evaluated replacement at the source
    // binding's cleanup position even if destroying an old element panics.
    assert_events(
        &[ALLOC_BASE + 19, ALLOC_BASE + 20, DROP_BASE + 200, FREE_BASE + 19,
          DROP_BASE + 220, DROP_BASE + 210, FREE_BASE + 20, DROP_BASE + 190, CAUGHT],
        0,
    );

    reset_events();
    let returned = result_error_replacement(source.as_str());
    assert!(matches!(&returned, Err(value) if value.id == 290));
    assert_events(&[ALLOC_BASE + 41, FREE_BASE + 41, DROP_BASE + 280,
                    DROP_BASE + 270, DROP_BASE + 260], 0);
    drop(returned);
    assert_events(&[ALLOC_BASE + 41, FREE_BASE + 41, DROP_BASE + 280,
                    DROP_BASE + 270, DROP_BASE + 260, DROP_BASE + 290], 0);

    reset_events();
    PANIC_MARKER_ID.store(280, Ordering::Relaxed);
    let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        result_error_replacement(source.as_str())
    }));
    assert!(result.is_err());
    record(CAUGHT);
    assert_events(&[ALLOC_BASE + 41, FREE_BASE + 41, DROP_BASE + 280,
                    DROP_BASE + 270, DROP_BASE + 290, DROP_BASE + 260, CAUGHT], 0);

    reset_events();
    let returned = option_replacement(source.as_str());
    assert_eq!(returned.as_deref(), Some(&[source.as_str()][..]));
    assert_events(&[ALLOC_BASE + 42, FREE_BASE + 42, ALLOC_BASE + 43,
                    DROP_BASE + 310, DROP_BASE + 300], 1);
    drop(returned);
    assert_events(&[ALLOC_BASE + 42, FREE_BASE + 42, ALLOC_BASE + 43,
                    DROP_BASE + 310, DROP_BASE + 300, FREE_BASE + 43], 0);

    for count in [0, 1, 3, 10] {
        reset_events();
        let returned = loop_replacement(source.as_str(), count);
        assert_eq!(returned, vec![source.as_str()]);
        let mut expected = Vec::new();
        for index in 0..count {
            expected.push(ALLOC_BASE + 48);
            if index != 0 { expected.push(FREE_BASE + 49); }
            expected.extend([ALLOC_BASE + 49, FREE_BASE + 48, DROP_BASE + 370]);
        }
        expected.extend([DROP_BASE + 380, DROP_BASE + 360]);
        assert_events(&expected, usize::from(count != 0));
        drop(returned);
        if count != 0 { expected.push(FREE_BASE + 49); }
        assert_events(&expected, 0);
    }

    // A never-polled Future has not initialized any source-local resource.
    reset_events();
    drop(async_replacement(source.as_str(), false, false));
    assert_events(&[], 0);

    // Cancel before restoration: the local borrow and its owner must stay
    // together in the suspended Future, then be destroyed in source order.
    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_before_replacement(source.as_str()));
    assert!(poll_once(future.as_mut()).is_pending());
    assert_events(&[ALLOC_BASE + 46], 1);
    drop(future);
    assert_events(&[ALLOC_BASE + 46, DROP_BASE + 350, FREE_BASE + 46, DROP_BASE + 340], 0);

    // Cancel after restoration. No executor sleeps or timing races are used.
    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_replacement(source.as_str(), false, false));
    assert!(poll_once(future.as_mut()).is_pending());
    let initialized = [ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44];
    assert_events(&initialized, 1);
    drop(future);
    assert_events(&[ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    DROP_BASE + 330, FREE_BASE + 45, DROP_BASE + 320], 0);

    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_replacement(source.as_str(), false, false));
    assert!(poll_once(future.as_mut()).is_pending());
    PAUSE_READY.store(true, Ordering::Relaxed);
    let returned = match poll_once(future.as_mut()) {
        ::std::task::Poll::Ready(Ok(value)) => value,
        _ => panic!("resumed Future did not return its restored container"),
    };
    assert_eq!(returned, vec![source.as_str()]);
    let expected = [ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    DROP_BASE + 330, DROP_BASE + 320];
    assert_events(&expected, 1);
    drop(future);
    assert_events(&expected, 1);
    drop(returned);
    assert_events(&[ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    DROP_BASE + 330, DROP_BASE + 320, FREE_BASE + 45], 0);

    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_replacement(source.as_str(), false, true));
    assert!(poll_once(future.as_mut()).is_pending());
    PAUSE_READY.store(true, Ordering::Relaxed);
    assert!(matches!(poll_once(future.as_mut()), ::std::task::Poll::Ready(Err(7))));
    let expected = [ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    CHECKPOINT, DROP_BASE + 330, FREE_BASE + 45, DROP_BASE + 320];
    assert_events(&expected, 0);
    drop(future);
    assert_events(&expected, 0);

    reset_events();
    PAUSE_READY.store(false, Ordering::Relaxed);
    let mut future = Box::pin(async_replacement(source.as_str(), true, false));
    assert!(poll_once(future.as_mut()).is_pending());
    PAUSE_READY.store(true, Ordering::Relaxed);
    let caught = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
        poll_once(future.as_mut())
    }));
    assert!(caught.is_err());
    record(CAUGHT);
    let expected = [ALLOC_BASE + 44, ALLOC_BASE + 45, FREE_BASE + 44,
                    CRASH, DROP_BASE + 330, FREE_BASE + 45, DROP_BASE + 320, CAUGHT];
    assert_events(&expected, 0);
    drop(future);
    assert_events(&expected, 0);
}
