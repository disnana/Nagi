#![allow(unused_mut, unused_parens, unused_variables, dead_code)]
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_6d6f6e69746f72() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    native::monitor().await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_68747470() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    native::http().await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_6578657263697365() -> () {
    native::exercise().await
}
#[allow(non_snake_case, arithmetic_overflow)]
pub async fn __nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_74797065645f6d6f6e69746f725f73657276696365() -> ::std::result::Result<(), ::nagi_runtime::Error> {
    {
        let mut __nagi_task_scope_0 = ::nagi_runtime::TaskScope::new();
        let __scope_result: ::std::result::Result<(), ::nagi_runtime::Error> = '__nagi_scope_body_1: {
            let mut monitor_task: ::nagi_runtime::Task<::std::result::Result<(), ::nagi_runtime::Error>> = { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_6d6f6e69746f72(); __nagi_task_scope_0.spawn_value(async move { __nagi_spawn_future.await }) };
            { let __nagi_spawn_future = crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_68747470(); __nagi_task_scope_0.spawn(async move { __nagi_spawn_future.await }); }
            (crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_6578657263697365()).await;
            let mut received: ::std::result::Result<::std::result::Result<(), ::nagi_runtime::Error>, ::nagi_runtime::TaskFailure> = __nagi_task_scope_0.receive(monitor_task).await;
            match received {
                ::std::result::Result::Ok(mut inner) => {
                    match (inner) { ::std::result::Result::Ok(__nagi_try_value) => __nagi_try_value, ::std::result::Result::Err(__nagi_try_error) => break '__nagi_scope_body_1 ::std::result::Result::Err(::std::convert::From::from(__nagi_try_error)) };
                },
                ::std::result::Result::Err(mut failure) => {
                    println!("{}", "monitor task fault");
                },
            }
            ::std::result::Result::Ok(())
        };
        if let ::std::result::Result::Err(e) = __scope_result { let _ = __nagi_task_scope_0.cancel().await; return ::std::result::Result::Err(::std::convert::From::from(e)); }
        (__nagi_task_scope_0.join().await)?;
    }
    return ::std::result::Result::Ok(println!("{}", 0i64));
}
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_6d6f6e69746f72 as monitor;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_68747470 as http;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_6578657263697365 as exercise;
#[allow(unused_imports)]
pub use crate::__nagi_def_2f746d702f6e6167692d73312d66696e616c2d7265766965772f70726f6265732f73322d74797065642d6d6f6e69746f722e6e616769_f_74797065645f6d6f6e69746f725f73657276696365 as typed_monitor_service;
fn main() {}

mod native {
    use nagi_runtime::{actor, http_server as web, Error};
    use std::{net::SocketAddr, sync::{Mutex, atomic::{AtomicBool, Ordering}}};
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::oneshot};

    struct Service {
        group: Option<actor::Supervisor<()>>,
        listener: Option<TcpListener>,
        counter: actor::Actor<i64, i64, String>,
        control: actor::Control,
        shutdown: Option<oneshot::Sender<()>>,
        stopped: Option<oneshot::Receiver<()>>,
        address: SocketAddr,
        terminal: bool,
    }
    static SERVICE: Mutex<Option<Service>> = Mutex::new(None);
    static HTTP_DROPPED: AtomicBool = AtomicBool::new(false);
    struct HttpGuard;
    impl Drop for HttpGuard {
        fn drop(&mut self) { HTTP_DROPPED.store(true, Ordering::SeqCst); }
    }
    pub async fn setup(terminal: bool) {
        let group = actor::supervisor((), actor::options(1, 16, 0, 10000, 1000).unwrap());
        let counter = actor::register(&group, "counter", |_| async { Ok(9_i64) },
            |state: i64, change: i64| async move {
                if change == -2 { return Err(Error::invalid("terminal sentinel")); }
                let reply = if change < 0 { Err("business sentinel".to_owned()) } else { Ok(state) };
                Ok(actor::turn(state, reply))
            }, actor::default_actor_options()).unwrap();
        let control = actor::control(&group);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, stopped) = oneshot::channel();
        HTTP_DROPPED.store(false, Ordering::SeqCst);
        let mut slot = SERVICE.lock().unwrap();
        assert!(slot.is_none());
        *slot = Some(Service { group: Some(group), listener: Some(listener), counter,
            control, shutdown: Some(shutdown), stopped: Some(stopped), address, terminal });
    }
    pub fn monitor() -> impl std::future::Future<Output = Result<(), Error>> {
        let group = SERVICE.lock().unwrap().as_mut().unwrap().group.take().unwrap();
        actor::run(group)
    }
    pub async fn http() -> Result<(), Error> {
        let _guard = HttpGuard;
        let (listener, counter, stopped) = {
            let mut slot = SERVICE.lock().unwrap();
            let service = slot.as_mut().unwrap();
            (service.listener.take().unwrap(), actor::clone_actor(&service.counter), service.stopped.take().unwrap())
        };
        let app = web::route(web::app_default(counter), web::Method::GET, "/counter",
            |_, counter| async move {
                Ok(match actor::call(&counter, 0, 0, 1000).await {
                    Ok(Ok(value)) => web::text(web::Status::OK, &value.to_string()),
                    _ => web::text(web::Status::SERVICE_UNAVAILABLE, "stopped"),
                })
            }).unwrap();
        let app = web::route(app, web::Method::GET, "/business", |_, counter| async move {
            let reply = actor::call(&counter, -1, 0, 1000).await.unwrap();
            assert_eq!(reply, Err("business sentinel".to_owned()));
            Ok(web::text(web::Status::CONFLICT, "business sentinel"))
        }).unwrap();
        web::serve_listener(listener, app, web::default_options(), async { let _ = stopped.await; }).await
    }
    async fn request(path: &str) -> Vec<u8> {
        let address = SERVICE.lock().unwrap().as_ref().unwrap().address;
        let mut socket = TcpStream::connect(address).await.unwrap();
        socket.write_all(format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        let mut response = Vec::new();
        socket.read_to_end(&mut response).await.unwrap();
        response
    }
    pub async fn exercise() {
        let (counter, control, terminal) = {
            let slot = SERVICE.lock().unwrap();
            let service = slot.as_ref().unwrap();
            (actor::clone_actor(&service.counter), actor::clone_control(&service.control), service.terminal)
        };
        actor::ready(&counter, 1000).await.unwrap();
        let response = request("/business").await;
        assert!(response.starts_with(b"HTTP/1.1 409 "), "{response:?}");
        assert!(response.ends_with(b"business sentinel"));
        let response = request("/counter").await;
        assert!(response.starts_with(b"HTTP/1.1 200 "), "{response:?}");
        assert!(response.ends_with(b"9"));
        assert!(!HTTP_DROPPED.load(Ordering::SeqCst));
        if terminal {
            assert_eq!(actor::call(&counter, -2, 0, 1000).await.unwrap_err().kind(), actor::CallKind::REPLY_LOST);
        } else {
            actor::shutdown(&control).await.unwrap();
            let response = request("/counter").await;
            assert!(response.starts_with(b"HTTP/1.1 503 "), "{response:?}");
            assert!(!HTTP_DROPPED.load(Ordering::SeqCst), "normal Supervisor shutdown cancelled HTTP");
            SERVICE.lock().unwrap().as_mut().unwrap().shutdown.take().unwrap().send(()).unwrap();
        }
    }
    pub async fn verify_closed() {
        assert!(HTTP_DROPPED.load(Ordering::SeqCst), "scope returned before HTTP future destruction");
        let address = SERVICE.lock().unwrap().as_ref().unwrap().address;
        assert!(TcpStream::connect(address).await.is_err(), "HTTP listener survived actual scope joining");
        SERVICE.lock().unwrap().take().unwrap();
    }
}


#[test]
fn service_contract() {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    rt.block_on(async {
        for _mixed in [true] {
            for terminal in [false, true] {
                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    native::setup(terminal).await;
                    let result = typed_monitor_service().await;
                    if terminal {
                        let error = result.unwrap_err();
                        assert!(matches!(error.kind, nagi_runtime::ErrorKind::Internal));
                        assert!(error.message.contains("restart intensity"), "{error}");
                    } else { result.unwrap(); }
                    native::verify_closed().await;
                }).await.unwrap();
            }
        }
    });
}
