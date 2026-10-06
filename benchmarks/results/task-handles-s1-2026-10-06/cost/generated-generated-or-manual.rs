#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692f62656e63686d61726b732f7461736b2d68616e646c65732d73312f62617463682e6e616769_f_776f726b(mut value: ::std::primitive::i64) -> ::std::primitive::i64 {
    return value;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692f62656e63686d61726b732f7461736b2d68616e646c65732d73312f62617463682e6e616769_f_726563656976655f6261746368(mut n: ::std::primitive::i64) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    let mut total: ::std::primitive::i64 = 0i64;
    {
        let mut __nagi_task_scope_0 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            for mut number in 0i64..n {
                let mut child: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f776f726b73706163652f4e6167692f62656e63686d61726b732f7461736b2d68616e646c65732d73312f62617463682e6e616769_f_776f726b(number); __nagi_task_scope_0.spawn_value(async move { __nagi_spawn_future.await }) };
                let mut received: ::std::result::Result<::std::primitive::i64, ::nagi_runtime::TaskFailure> = __nagi_task_scope_0.receive(child).await;
                match received {
                    ::std::result::Result::Ok(mut value) => {
                        total = (total + value);
                    },
                    ::std::result::Result::Err(mut failure) => {
                        assert!(false);
                    },
                }
            }
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_0.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_0.join().await)?;
    }
    return ::std::result::Result::Ok(total);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692f62656e63686d61726b732f7461736b2d68616e646c65732d73312f62617463682e6e616769_f_646973636172645f6261746368(mut n: ::std::primitive::i64) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    {
        let mut __nagi_task_scope_1 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            for mut number in 0i64..n {
                let mut child: ::nagi_runtime::Task<::std::primitive::i64> = { let __nagi_spawn_future = crate::__nagi_def_2f776f726b73706163652f4e6167692f62656e63686d61726b732f7461736b2d68616e646c65732d73312f62617463682e6e616769_f_776f726b(number); __nagi_task_scope_1.spawn_value(async move { __nagi_spawn_future.await }) };
                __nagi_task_scope_1.discard(child);
            }
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_1.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_1.join().await)?;
    }
    return ::std::result::Result::Ok(0i64);
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692f62656e63686d61726b732f7461736b2d68616e646c65732d73312f62617463682e6e616769_f_776f726b as work;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692f62656e63686d61726b732f7461736b2d68616e646c65732d73312f62617463682e6e616769_f_726563656976655f6261746368 as receive_batch;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692f62656e63686d61726b732f7461736b2d68616e646c65732d73312f62617463682e6e616769_f_646973636172645f6261746368 as discard_batch;
fn main() {}
