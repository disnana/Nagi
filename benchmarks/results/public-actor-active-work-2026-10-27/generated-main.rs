#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636865636b706f696e74(mut stage: ::std::primitive::i64) -> () {
    native::checkpoint(stage)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_626567696e5f7068617365(mut clients: ::std::primitive::i64) -> () {
    native::begin_phase(clients)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_7265636f72645f636c69656e74(mut client: ::std::primitive::i64, mut starts: ::std::vec::Vec<::std::primitive::i64>, mut ends: ::std::vec::Vec<::std::primitive::i64>, mut successes: ::std::vec::Vec<::std::primitive::bool>) -> () {
    native::record_client(client, starts, ends, successes)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_656e645f7068617365() -> () {
    native::end_phase()
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6661756c745f6d61726b(mut complete: ::std::primitive::bool, mut time: ::std::primitive::i64) -> () {
    native::fault_mark(complete, time)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_7265636f76657279(mut failures: ::std::primitive::i64, mut restarts: ::std::primitive::i64, mut generation: ::std::primitive::i64, mut state: ::std::primitive::i64) -> () {
    native::recovery(failures, restarts, generation, state)
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_666163746f7279(mut context: ::std::sync::Arc<()>) -> ::std::result::Result<::std::primitive::i64, ::nagi_runtime::Error> {
    return ::std::result::Result::Ok(0i64);
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_68616e646c6572(mut state: ::std::primitive::i64, mut message: ::std::primitive::i64) -> ::std::result::Result<::nagi_runtime::actor::Turn<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error>, ::nagi_runtime::Error> {
    if (message < 0i64) {
        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("finite public actor worker fault")));
    }
    let mut next: ::std::primitive::i64 = (state + message);
    return ::std::result::Result::Ok(::nagi_runtime::actor::turn::<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error>(next, ::std::result::Result::Ok(next)));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6d6f6e69746f72(mut group: ::nagi_runtime::actor::Supervisor<()>) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    return (::nagi_runtime::actor::run(group)).await;
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636c69656e74(mut worker: ::nagi_runtime::actor::Actor<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error>, mut index: ::std::primitive::i64, mut iterations: ::std::primitive::i64) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut starts: ::std::vec::Vec<::std::primitive::i64> = vec![];
    let mut ends: ::std::vec::Vec<::std::primitive::i64> = vec![];
    let mut successes: ::std::vec::Vec<::std::primitive::bool> = vec![];
    for mut i in 0i64..iterations {
        let mut start: ::std::primitive::i64 = ::nagi_runtime::clock_ns();
        let mut success: ::std::primitive::bool = false;
        match (::nagi_runtime::actor::call(&(worker), 1i64, 2000i64, 2000i64)).await {
            ::std::result::Result::Ok(mut reply) => {
                match reply {
                    ::std::result::Result::Ok(mut value) => {
                        assert!((value == (i + 1i64)));
                        success = true;
                    },
                    ::std::result::Result::Err(_) => {
                        assert!(true);
                    },
                }
            },
            ::std::result::Result::Err(_) => {
                assert!(true);
            },
        }
        let mut end: ::std::primitive::i64 = ::nagi_runtime::clock_ns();
        starts.push(start);
        ends.push(end);
        successes.push(success);
    }
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_7265636f72645f636c69656e74(index, starts, ends, successes);
    return ::std::result::Result::Ok(assert!(true));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6661696c5f6f6e6365(mut worker: ::nagi_runtime::actor::Actor<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error>, mut control: ::nagi_runtime::actor::Control) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6661756c745f6d61726b(false, ::nagi_runtime::clock_ns());
    match (::nagi_runtime::actor::call(&(worker), -(1i64), 2000i64, 2000i64)).await {
        ::std::result::Result::Ok(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("faulted worker returned a reply")));
        },
        ::std::result::Result::Err(mut problem) => {
            assert!(((problem).kind() == ::nagi_runtime::actor::CallKind::REPLY_LOST));
        },
    }
    let mut restarted: ::std::primitive::bool = false;
    let mut count: ::std::primitive::i64 = 0i64;
    let mut failures: ::std::primitive::i64 = 0i64;
    let mut restarts: ::std::primitive::i64 = 0i64;
    while (!(restarted) && (count < 16i64)) {
        count = (count + 1i64);
        match (::nagi_runtime::actor::next_event_timeout(&(control), 2000i64)).await {
            ::std::result::Result::Ok(mut next) => {
                match next {
                    ::std::option::Option::Some(mut event) => {
                        assert!(((event).kind() != ::nagi_runtime::actor::EventKind::LAGGED));
                        assert!(((event).child_name() == "fault"));
                        if ((event).kind() == ::nagi_runtime::actor::EventKind::FAILED) {
                            assert!(((event).generation() == 1i64));
                            failures = (failures + 1i64);
                        }
                        if ((event).kind() == ::nagi_runtime::actor::EventKind::RESTART_SCHEDULED) {
                            restarts = (restarts + 1i64);
                        }
                        if ((((event).kind() == ::nagi_runtime::actor::EventKind::STARTED) && ((event).child_name() == "fault")) && ((event).generation() == 2i64)) {
                            restarted = true;
                        }
                    },
                    ::std::option::Option::None => {
                        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("supervisor stopped before recovery")));
                    },
                }
            },
            ::std::result::Result::Err(_) => {
                return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("recovery event deadline")));
            },
        }
    }
    assert!(restarted);
    assert!(((failures == 1i64) && (restarts == 1i64)));
    match (::nagi_runtime::actor::call(&(worker), 0i64, 2000i64, 2000i64)).await {
        ::std::result::Result::Ok(mut reply) => {
            match reply {
                ::std::result::Result::Ok(mut value) => {
                    assert!((value == 0i64));
                    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_7265636f76657279(failures, restarts, 2i64, value);
                },
                ::std::result::Result::Err(_) => {
                    return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("recovered actor business failure")));
                },
            }
        },
        ::std::result::Result::Err(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("recovered actor unavailable")));
        },
    }
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6661756c745f6d61726b(true, ::nagi_runtime::clock_ns());
    return ::std::result::Result::Ok(assert!(true));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6472697665(mut workers: ::std::vec::Vec<::nagi_runtime::actor::Actor<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error>>, mut fault: ::nagi_runtime::actor::Actor<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error>, mut control: ::nagi_runtime::actor::Control, mut clients: ::std::primitive::i64, mut iterations: ::std::primitive::i64) -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut ready: ::std::vec::Vec<::nagi_runtime::actor::Actor<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error>> = vec![];
    for mut worker in (workers).iter() {
        match (::nagi_runtime::actor::ready(&((*worker)), 5000i64)).await {
            ::std::result::Result::Ok(_) => {
                assert!(true);
            },
            ::std::result::Result::Err(_) => {
                return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("active actor did not become ready")));
            },
        }
        ready.push(::nagi_runtime::actor::clone_actor(&((*worker))));
    }
    match (::nagi_runtime::actor::ready(&(fault), 5000i64)).await {
        ::std::result::Result::Ok(_) => {
            assert!(true);
        },
        ::std::result::Result::Err(_) => {
            return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("fault actor did not become ready")));
        },
    }
    let mut starts: ::std::primitive::i64 = 0i64;
    let mut started: ::std::primitive::i64 = 0i64;
    for mut i in 0i64..((clients + 1i64) * 2i64) {
        match (::nagi_runtime::actor::next_event_timeout(&(control), 2000i64)).await {
            ::std::result::Result::Ok(mut next) => {
                match next {
                    ::std::option::Option::Some(mut event) => {
                        assert!(((event).generation() == 1i64));
                        if ((event).kind() == ::nagi_runtime::actor::EventKind::STARTING) {
                            starts = (starts + 1i64);
                        } else {
                            if ((event).kind() == ::nagi_runtime::actor::EventKind::STARTED) {
                                started = (started + 1i64);
                            } else {
                                return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("unexpected initial event")));
                            }
                        }
                    },
                    ::std::option::Option::None => {
                        return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("startup stream closed")));
                    },
                }
            },
            ::std::result::Result::Err(_) => {
                return ::std::result::Result::Err(::nagi_runtime::Error::invalid(::std::string::String::from("startup event deadline")));
            },
        }
    }
    assert!(((starts == (clients + 1i64)) && (started == (clients + 1i64))));
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636865636b706f696e74(1i64);
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_626567696e5f7068617365(clients);
    {
        let mut __scope = ::nagi_runtime::Scope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut index: ::std::primitive::i64 = 0i64;
            for mut worker in (ready).iter() {
                { let __nagi_spawn_future = crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636c69656e74(::nagi_runtime::actor::clone_actor(&((*worker))), index, iterations); __scope.spawn(async move { __nagi_spawn_future.await }); }
                index = (index + 1i64);
            }
            { let __nagi_spawn_future = crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6661696c5f6f6e6365(fault, ::nagi_runtime::actor::clone_control(&(control))); __scope.spawn(async move { __nagi_spawn_future.await }); }
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { __scope.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__scope.join().await)?;
    }
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_656e645f7068617365();
    ((::nagi_runtime::actor::shutdown(&(control))).await)?;
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636865636b706f696e74(2i64);
    return ::std::result::Result::Ok(assert!(true));
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_main() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    let mut clients: ::std::primitive::i64 = (::nagi_runtime::parse_i64(&(match ::std::env::var("NAGI_PROBE_CLIENTS") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("16").to_owned() })))?;
    let mut iterations: ::std::primitive::i64 = (::nagi_runtime::parse_i64(&(match ::std::env::var("NAGI_PROBE_ITERATIONS") { ::std::result::Result::Ok(__nagi_env_value) => __nagi_env_value, ::std::result::Result::Err(_) => ("5000").to_owned() })))?;
    assert!(((((clients >= 1i64) && (clients <= 64i64)) && (iterations >= 1i64)) && (iterations <= 5000i64)));
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636865636b706f696e74(0i64);
    let mut group: ::nagi_runtime::actor::Supervisor<()> = ::nagi_runtime::actor::supervisor::<()>(assert!(true), (::nagi_runtime::actor::options((clients + 1i64), 1024i64, 2i64, 60000i64, 2000i64))?);
    let mut fault: ::nagi_runtime::actor::Actor<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error> = (::nagi_runtime::actor::register(&(group), "fault", crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_666163746f7279, crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_68616e646c6572, (::nagi_runtime::actor::actor_options(32i64, 4096i64, 4096i64, 5000i64, ::nagi_runtime::actor::RestartPolicy::TRANSIENT))?))?;
    let mut workers: ::std::vec::Vec<::nagi_runtime::actor::Actor<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error>> = vec![];
    for mut i in 0i64..clients {
        let mut name: ::std::string::String = (::nagi_runtime::encode(&i))?;
        let mut worker: ::nagi_runtime::actor::Actor<::std::primitive::i64, ::std::primitive::i64, ::nagi_runtime::Error> = (::nagi_runtime::actor::register(&(group), (name).as_str(), crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_666163746f7279, crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_68616e646c6572, (::nagi_runtime::actor::actor_options(32i64, 4096i64, 4096i64, 5000i64, ::nagi_runtime::actor::RestartPolicy::TRANSIENT))?))?;
        workers.push(worker);
    }
    let mut control: ::nagi_runtime::actor::Control = ::nagi_runtime::actor::control(&(group));
    {
        let mut __scope = ::nagi_runtime::Scope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            { let __nagi_spawn_future = crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6d6f6e69746f72(group); __scope.spawn(async move { __nagi_spawn_future.await }); }
            { let __nagi_spawn_future = crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6472697665(workers, fault, control, clients, iterations); __scope.spawn(async move { __nagi_spawn_future.await }); }
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { __scope.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__scope.join().await)?;
    }
    (::nagi_runtime::sleep(50i64)).await;
    crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636865636b706f696e74(3i64);
    return ::std::result::Result::Ok(assert!(true));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636865636b706f696e74 as checkpoint;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_626567696e5f7068617365 as begin_phase;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_7265636f72645f636c69656e74 as record_client;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_656e645f7068617365 as end_phase;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6661756c745f6d61726b as fault_mark;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_7265636f76657279 as recovery;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_666163746f7279 as factory;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_68616e646c6572 as handler;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6d6f6e69746f72 as monitor;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_636c69656e74 as client;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6661696c5f6f6e6365 as fail_once;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f776f726b73706163652f4e6167692d7075626c69632d6163746f722d6163746976652d776f726b2f62656e63686d61726b732f7075626c69632d6163746f722d6163746976652d776f726b2f6d61696e2e6e616769_f_6472697665 as drive;
#[allow(non_snake_case)]
pub mod actor {
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Supervisor as Supervisor;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Control as Control;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Actor as Actor;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Turn as Turn;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Options as Options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::ActorOptions as ActorOptions;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::RestartPolicy as RestartPolicy;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::CallKind as CallKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::EventKind as EventKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::CallError as CallError;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::Event as Event;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::TaskReady as TaskReady;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::WaitKind as WaitKind;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::WaitError as WaitError;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::default_options as default_options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::options as options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::default_actor_options as default_actor_options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::actor_options as actor_options;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::restart_delay as restart_delay;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::supervisor as supervisor;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::control as control;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::clone_control as clone_control;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::register as register;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::task as task;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::turn as turn;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::clone_actor as clone_actor;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::ready as ready;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::call as call;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::run as run;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::shutdown as shutdown;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::next_event as next_event;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::next_event_timeout as next_event_timeout;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::task_with_ready as task_with_ready;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::mark_ready as mark_ready;
    #[allow(unused_imports)]
    pub use ::nagi_runtime::actor::yield_now as yield_now;
}
fn main() { ::nagi_runtime::block_on(async {
if let ::std::result::Result::Err(e) = __nagi_main().await { eprintln!("{}",e); ::std::process::exit(1); }
}); }

#[path = "/workspace/Nagi-public-actor-active-work/benchmarks/public-actor-active-work/observe.rs"]
mod native;
