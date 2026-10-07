#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_74616b65(mut result: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::TaskFailure>) -> ::std::primitive::bool {
    println!("{}", "loop-receipt");
    match result {
        ::std::result::Result::Ok(mut number) => {
            assert!((number == 7i64));
            return true;
        },
        ::std::result::Result::Err(mut failure) => {
            return false;
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_74657874(mut result: ::std::result::Result<::std::string::String, ::nagi_runtime::TaskFailure>) -> ::std::string::String {
    println!("{}", "env-receipt");
    match result {
        ::std::result::Result::Ok(mut value) => {
            return value;
        },
        ::std::result::Result::Err(mut failure) => {
            return ::std::string::String::from("failed");
        },
    }
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_656e76(mut key: ::std::string::String, mut value: ::std::string::String) -> ::std::string::String {
    println!("{}", "user-env");
    return value;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_6e756d626572() -> ::std::primitive::i64 {
    return 7i64;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_776f7264() -> ::std::string::String {
    return ::std::string::String::from("payload");
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    {
        let mut __nagi_task_scope_0 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            for mut index in 0i64..2i64 {
                let mut task: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_6e756d626572(); __nagi_task_scope_0.spawn_value(async move { __nagi_spawn_future.await }) };
                let mut value: ::std::primitive::bool = (crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_74616b65(__nagi_task_scope_0.receive(task).await) && ((index == 0i64) || (index == 1i64)));
                assert!(value);
            }
            let mut text_task: ::nagi_runtime::Task<::std::string::String> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_776f7264(); __nagi_task_scope_0.spawn_value(async move { __nagi_spawn_future.await }) };
            let mut value: ::std::string::String = crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_656e76(::std::string::String::from("ignored"), crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_74657874(__nagi_task_scope_0.receive(text_task).await));
            assert!(((value).as_str() == "payload"));
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_0.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_0.join().await)?;
    }
    return ::std::result::Result::Ok(println!("{}", "loop-and-user-env-ok"));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_74616b65 as take;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_74657874 as text;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_656e76 as env;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_6e756d626572 as number;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d637269746963616c2d72656c656173652d7265766965772f706f73742d6669782f626f756e646172792d636f6e74726f6c732f68616e647772697474656e2e6c6f77_f_776f7264 as word;
#[allow(unused_imports)]
pub use ::nagi_runtime::TaskFailure as TaskFailure;
fn main() { ::nagi_runtime::block_on(async {
if let ::std::result::Result::Err(e) = __nagi_main().await { eprintln!("{}",e); ::std::process::exit(1); }
}); }
