#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f5f5f6e6167695f6d656d6f72795f5f2f70726f6772616d2e6e616769_f_776f726b() -> ::std::primitive::i64 {
    return 7i64;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    {
        let mut __nagi_task_scope_0 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut task: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f5f5f6e6167695f6d656d6f72795f5f2f70726f6772616d2e6e616769_f_776f726b(); __nagi_task_scope_0.spawn_value(async move { __nagi_spawn_future.await }) };
            let mut received: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::TaskFailure> = __nagi_task_scope_0.receive(task).await;
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_0.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_0.join().await)?;
    }
    return ::std::result::Result::Ok(println!("{}", 0i64));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f5f5f6e6167695f6d656d6f72795f5f2f70726f6772616d2e6e616769_f_776f726b as work;
fn main() { ::nagi_runtime::block_on(async {
if let ::std::result::Result::Err(e) = __nagi_main().await { eprintln!("{}",e); ::std::process::exit(1); }
}); }
