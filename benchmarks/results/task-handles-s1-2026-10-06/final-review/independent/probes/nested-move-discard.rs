#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f6e65737465642d6d6f76652d646973636172642e6e616769_f_776f726b() -> ::std::primitive::i64 {
    return 7i64;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f6e65737465642d6d6f76652d646973636172642e6e616769_f_6578657263697365(mut flag: ::std::primitive::bool) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    {
        let mut __nagi_task_scope_0 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut task: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f6e65737465642d6d6f76652d646973636172642e6e616769_f_776f726b(); __nagi_task_scope_0.spawn_value(async move { __nagi_spawn_future.await }) };
            __nagi_task_scope_0.discard(::std::convert::identity(::std::convert::identity(task)));
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_0.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_0.join().await)?;
    }
    return ::std::result::Result::Ok(println!("{}", 0i64));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f6e65737465642d6d6f76652d646973636172642e6e616769_f_776f726b as work;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f6e65737465642d6d6f76652d646973636172642e6e616769_f_6578657263697365 as exercise;
#[allow(unused_imports)]
pub use ::std::convert::identity as r#move;
fn main() {}

#[test] fn independent_native_run(){let rt=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();rt.block_on(async{exercise(false).await.unwrap();exercise(true).await.unwrap();});}
