#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(mut code: ::std::primitive::i64) -> () {
    native::mark(code)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_70726f6265(mut value: ::std::primitive::bool) -> ::std::primitive::bool {
    native::probe(value)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_66616c6c6261636b() -> ::std::string::String {
    native::fallback()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_74616b65(mut result: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::TaskFailure>, mut value: ::std::primitive::bool) -> ::std::primitive::bool {
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(1i64);
    match result {
        ::std::result::Result::Ok(mut number) => {
            assert!((number == 7i64));
            return value;
        },
        ::std::result::Result::Err(mut failure) => {
            assert!(false);
            return false;
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_74657874(mut result: ::std::result::Result<::std::string::String, ::nagi_runtime::TaskFailure>) -> ::std::string::String {
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(4i64);
    match result {
        ::std::result::Result::Ok(mut value) => {
            return value;
        },
        ::std::result::Result::Err(mut failure) => {
            assert!(false);
            return ::std::string::String::from("failed");
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_7472757468(mut value: ()) -> ::std::primitive::bool {
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(6i64);
    return true;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6b6579(mut value: ()) -> ::std::string::String {
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(7i64);
    return ::std::string::String::from("NAGI_CONDITIONAL_PRESENT");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6e756d626572() -> ::std::primitive::i64 {
    return 7i64;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_737472696e67(mut value: ::std::string::String) -> ::std::string::String {
    return value;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6c656674(mut flag: ::std::primitive::bool) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    {
        let mut __nagi_task_scope_0 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut task: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6e756d626572(); __nagi_task_scope_0.spawn_value(async move { __nagi_spawn_future.await }) };
            let mut value: ::std::primitive::bool = (crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_74616b65(__nagi_task_scope_0.receive(task).await, flag) && crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_70726f6265(true));
            assert!((value == flag));
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_0.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_0.join().await)?;
    }
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(3i64);
    {
        let mut __nagi_task_scope_1 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut task: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6e756d626572(); __nagi_task_scope_1.spawn_value(async move { __nagi_spawn_future.await }) };
            let mut value: ::std::primitive::bool = (crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_74616b65(__nagi_task_scope_1.receive(task).await, flag) || crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_70726f6265(false));
            assert!((value == flag));
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_1.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_1.join().await)?;
    }
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(3i64);
    return ::std::result::Result::Ok(println!("{}", "scope-ok"));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_656e765f6b6579(mut name: ::std::string::String) -> ::std::result::Result<::std::string::String, ::nagi_runtime::Error> {
    let mut value: ::std::string::String = ::std::string::String::from("");
    {
        let mut __nagi_task_scope_2 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut task: ::nagi_runtime::Task<::std::string::String> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_737472696e67(name); __nagi_task_scope_2.spawn_value(async move { __nagi_spawn_future.await }) };
            value = match ::std::env::var(&(crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_74657874(__nagi_task_scope_2.receive(task).await))) { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => (&(crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_66616c6c6261636b())).to_owned() };
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_2.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_2.join().await)?;
    }
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(3i64);
    return ::std::result::Result::Ok(value);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6561676572(mut name: ::std::string::String) -> ::std::result::Result<::std::string::String, ::nagi_runtime::Error> {
    let mut value: ::std::string::String = ::std::string::String::from("");
    {
        let mut __nagi_task_scope_3 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut task: ::nagi_runtime::Task<::std::string::String> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_737472696e67(::std::string::String::from("payload")); __nagi_task_scope_3.spawn_value(async move { __nagi_spawn_future.await }) };
            let mut received: ::std::result::Result<::std::string::String, ::nagi_runtime::TaskFailure> = __nagi_task_scope_3.receive(task).await;
            crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(8i64);
            value = match ::std::env::var(&(name)) { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => (&(crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_74657874(::std::convert::identity(received)))).to_owned() };
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_3.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_3.join().await)?;
    }
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(3i64);
    return ::std::result::Result::Ok(value);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6c6566745f64697363617264() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    {
        let mut __nagi_task_scope_4 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut task: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6e756d626572(); __nagi_task_scope_4.spawn_value(async move { __nagi_spawn_future.await }) };
            let mut value: ::std::primitive::bool = (crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_7472757468(__nagi_task_scope_4.discard(task)) && crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_70726f6265(false));
            assert!(!(value));
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_4.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_4.join().await)?;
    }
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(3i64);
    {
        let mut __nagi_task_scope_5 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut task: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6e756d626572(); __nagi_task_scope_5.spawn_value(async move { __nagi_spawn_future.await }) };
            let mut value: ::std::string::String = match ::std::env::var(&(crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6b6579(__nagi_task_scope_5.discard(task)))) { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => (&(crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_66616c6c6261636b())).to_owned() };
            assert!(((value).as_str() == "present"));
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_5.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_5.join().await)?;
    }
    crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b(3i64);
    return ::std::result::Result::Ok(println!("{}", "scope-ok"));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6d61726b as mark;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_70726f6265 as probe;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_66616c6c6261636b as fallback;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_74616b65 as take;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_74657874 as text;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_7472757468 as truth;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6b6579 as key;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6e756d626572 as number;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_737472696e67 as string;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6c656674 as left;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_656e765f6b6579 as env_key;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6561676572 as eager;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d6578706c696369742d6d6f7665732d3133363130342d302f6d61696e2e6e616769_f_6c6566745f64697363617264 as left_discard;
#[allow(unused_imports)]
pub use ::nagi_runtime::TaskFailure as TaskFailure;
#[allow(unused_imports)]
pub use ::std::convert::identity as transfer;
fn main() {}


mod native {
    static EVENTS: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());
    pub fn mark(code: i64) { EVENTS.lock().unwrap().push(code); }
    pub fn probe(value: bool) -> bool { mark(2); value }
    pub fn fallback() -> String { mark(5); "fallback".into() }
    pub fn expect(expected: &[i64]) {
        assert_eq!(&*EVENTS.lock().unwrap(), expected);
        EVENTS.lock().unwrap().clear();
    }
}


#[test]
fn conditional_native_contract() {
    std::env::set_var("NAGI_CONDITIONAL_PRESENT", "present");
    std::env::remove_var("NAGI_CONDITIONAL_MISSING");
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        left(false).await.unwrap(); native::expect(&[1,3,1,2,3]);
        left(true).await.unwrap(); native::expect(&[1,2,3,1,3]);
        assert_eq!(env_key("NAGI_CONDITIONAL_PRESENT".into()).await.unwrap(), "present");
        native::expect(&[4,3]);
        assert_eq!(env_key("NAGI_CONDITIONAL_MISSING".into()).await.unwrap(), "fallback");
        native::expect(&[4,5,3]);
        assert_eq!(eager("NAGI_CONDITIONAL_PRESENT".into()).await.unwrap(), "present");
        native::expect(&[8,3]);
        assert_eq!(eager("NAGI_CONDITIONAL_MISSING".into()).await.unwrap(), "payload");
        native::expect(&[8,4,3]);
        left_discard().await.unwrap(); native::expect(&[6,2,3,7,3]);
    });
}
